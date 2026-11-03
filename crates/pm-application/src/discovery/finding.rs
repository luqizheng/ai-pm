use super::*;

const FINDING_COLS: &str = "id, session_id, kind, content, confidence, verification_status, \
                           project_id, actor_role, created_at";
type FindingRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<f64>,
    String,
    Uuid,
    Option<String>,
    DateTime<Utc>,
);

/// 证据类型（R-6-2 判别器的取值域）。
pub const EVIDENCE_TYPES: [&str; 5] = [
    "code",
    "test_result",
    "commit",
    "user_statement",
    "system_observation",
];

/// 代码类证据：**必须**绑定 Attempt（import/repository/commit/attempt 四列全非空）。
const CODE_LIKE_EVIDENCE: [&str; 3] = ["code", "test_result", "commit"];

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryFindingView {
    pub id: Uuid,
    pub session_id: Uuid,
    pub kind: String,
    pub content: String,
    pub confidence: Option<f64>,
    pub verification_status: String,
    /// 证据列表（由 `discovery.discovery_finding_evidence` 聚合而来）。
    pub evidence: serde_json::Value,
    pub actor_role: Option<String>,
    pub repository_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewFinding {
    pub kind: String,
    pub content: String,
    pub confidence: Option<f64>,
    /// 证据数组。兼容旧形状 `{repository, file, description}`；
    /// 新形状带 `evidence_type` 与 Commit 链路（见 [`parse_evidence`]）。
    #[serde(default)]
    pub evidence: serde_json::Value,
    #[serde(default)]
    pub repository_ids: Vec<Uuid>,
    pub verification_status: Option<String>,
    pub actor_role: Option<String>,
}

/// 证据输入项。
#[derive(Debug, Clone, Deserialize)]
pub struct EvidenceInput {
    /// 显式类型；缺省按 `system_observation` 处理（旧形状）。
    #[serde(default)]
    pub evidence_type: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// 旧形状的 `file`（与 `file_path` 同义）。
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub code_range: Option<String>,
    #[serde(default)]
    pub repository_id: Option<Uuid>,
    #[serde(default)]
    pub import_id: Option<Uuid>,
    #[serde(default)]
    pub commit_sha: Option<String>,
    #[serde(default)]
    pub attempt_no: Option<i32>,
}

/// 证据落库行。
struct EvidenceRow {
    evidence_type: String,
    description: Option<String>,
    file_path: Option<String>,
    symbol: Option<String>,
    code_range: Option<String>,
    import_id: Option<Uuid>,
    repository_id: Option<Uuid>,
    commit_sha: Option<String>,
    attempt_no: Option<i32>,
}

fn to_finding(
    row: FindingRow,
    repository_ids: Vec<Uuid>,
    evidence: serde_json::Value,
) -> DiscoveryFindingView {
    DiscoveryFindingView {
        id: row.0,
        session_id: row.1,
        kind: row.2,
        content: row.3,
        confidence: row.4,
        verification_status: row.5,
        evidence,
        actor_role: row.7,
        repository_ids,
        created_at: row.8,
    }
}

/// 解析证据数组 → 落库行。
///
/// 规则（R-6-2）：
/// - 显式声明 `evidence_type` 的代码类证据（`code` / `test_result` / `commit`）
///   **必须**携带 `import_id` / `repository_id` / `commit_sha` / `attempt_no`；
/// - 非代码类（`user_statement` / `system_observation`）**必须**不带这四列；
/// - 旧形状 `{repository, file, description}` 没有 Commit 链路，按
///   `system_observation` 落库并保留 `file`（不伪造 Commit）。
fn parse_evidence(value: &serde_json::Value) -> Result<Vec<EvidenceRow>, DiscoveryError> {
    let items: Vec<EvidenceInput> = if value.is_null() {
        Vec::new()
    } else {
        serde_json::from_value(value.clone()).map_err(|e| {
            DiscoveryError::Validation(format!("evidence must be an array of objects: {e}"))
        })?
    };

    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        let file_path = item.file_path.clone().or_else(|| item.file.clone());
        let (evidence_type, linked) = match item.evidence_type.as_deref() {
            Some(declared) => {
                if !EVIDENCE_TYPES.contains(&declared) {
                    return Err(DiscoveryError::Validation(format!(
                        "evidence_type must be one of {EVIDENCE_TYPES:?}"
                    )));
                }
                (declared.to_string(), CODE_LIKE_EVIDENCE.contains(&declared))
            }
            None => ("system_observation".to_string(), false),
        };

        if linked {
            if item.import_id.is_none()
                || item.repository_id.is_none()
                || item.commit_sha.is_none()
                || item.attempt_no.is_none()
            {
                return Err(DiscoveryError::Validation(
                    "code evidence requires import_id, repository_id, commit_sha and attempt_no"
                        .to_string(),
                ));
            }
        } else if item.import_id.is_some()
            || item.repository_id.is_some()
            || item.commit_sha.is_some()
            || item.attempt_no.is_some()
        {
            return Err(DiscoveryError::Validation(
                "non-code evidence must not carry import/repository/commit/attempt linkage"
                    .to_string(),
            ));
        }

        rows.push(EvidenceRow {
            evidence_type,
            description: item.description,
            file_path,
            symbol: item.symbol,
            code_range: item.code_range,
            import_id: item.import_id,
            repository_id: item.repository_id,
            commit_sha: item.commit_sha,
            attempt_no: item.attempt_no,
        });
    }
    Ok(rows)
}

/// 读出证据行 → 前端兼容的 JSON 数组。
async fn evidence_by_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    finding_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, serde_json::Value>, DiscoveryError> {
    let mut map: std::collections::HashMap<Uuid, serde_json::Value> =
        std::collections::HashMap::new();
    if finding_ids.is_empty() {
        return Ok(map);
    }
    let rows: Vec<(Uuid, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<Uuid>, Option<String>, Option<i32>)> =
        sqlx::query_as(
            "SELECT finding_id, evidence_type, description, file_path, symbol, code_range, \
                    repository_id, commit_sha, attempt_no \
             FROM discovery.discovery_finding_evidence \
             WHERE tenant_id = $1 AND finding_id = ANY($2) \
             ORDER BY created_at, id",
        )
        .bind(tenant_id)
        .bind(finding_ids)
        .fetch_all(pool)
        .await?;

    for (finding_id, evidence_type, description, file_path, symbol, code_range, repository_id, commit_sha, attempt_no) in rows {
        let item = serde_json::json!({
            "evidence_type": evidence_type,
            "description": description,
            "file": file_path,
            "file_path": file_path,
            "symbol": symbol,
            "code_range": code_range,
            "repository": repository_id.map(|id| id.to_string()),
            "repository_id": repository_id,
            "commit_sha": commit_sha,
            "attempt_no": attempt_no,
        });
        map.entry(finding_id)
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .expect("evidence array")
            .push(item);
    }
    Ok(map)
}

async fn repository_ids_by_session(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<std::collections::HashMap<Uuid, Vec<Uuid>>, DiscoveryError> {
    let links: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT fr.finding_id, fr.repository_id \
         FROM discovery.discovery_finding_repository fr \
         JOIN discovery.discovery_finding f \
           ON f.tenant_id = fr.tenant_id AND f.id = fr.finding_id \
         WHERE fr.tenant_id = $1 AND f.session_id = $2",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let mut map: std::collections::HashMap<Uuid, Vec<Uuid>> = std::collections::HashMap::new();
    for (finding_id, repository_id) in links {
        map.entry(finding_id).or_default().push(repository_id);
    }
    Ok(map)
}

pub async fn add_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    new_finding: NewFinding,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    validate_one_of(&new_finding.kind, &FINDING_KINDS, "kind")?;
    validate_confidence(new_finding.confidence)?;
    let verification_status = new_finding.verification_status.as_deref().unwrap_or("PENDING");
    validate_one_of(verification_status, &VERIFICATION_STATUSES, "verification_status")?;
    if new_finding.content.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "content must not be blank".to_string(),
        ));
    }
    let session = get_session(pool, tenant_id, session_id).await?;
    let project_id = session.project_id;
    let evidence_rows = parse_evidence(&new_finding.evidence)?;
    let repository_ids = new_finding.repository_ids;
    if !repository_ids.is_empty() {
        // 仓库必须属于**本项目**——同租户跨项目仓库不得挂到本项目的发现上
        // （I6-B V1.1 §5.1；DB 层由 finding_repository 的换轨 FK 再兜一层）。
        let found: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM integration.project_repository \
             WHERE tenant_id = $1 AND project_id = $2 AND id = ANY($3)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(&repository_ids)
        .fetch_one(pool)
        .await?;
        if found != repository_ids.len() as i64 {
            return Err(DiscoveryError::Validation(
                "repository_ids must all belong to the project".to_string(),
            ));
        }
    }
    let mut tx = pool.begin().await?;
    let row: FindingRow = sqlx::query_as(&format!(
        "INSERT INTO discovery.discovery_finding \
         (tenant_id, project_id, session_id, kind, content, confidence, verification_status, actor_role) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {FINDING_COLS}"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .bind(session_id)
    .bind(&new_finding.kind)
    .bind(&new_finding.content)
    .bind(new_finding.confidence)
    .bind(verification_status)
    .bind(&new_finding.actor_role)
    .fetch_one(&mut *tx)
    .await?;
    for evidence in &evidence_rows {
        sqlx::query(
            "INSERT INTO discovery.discovery_finding_evidence \
             (tenant_id, project_id, finding_id, evidence_type, description, file_path, symbol, \
              code_range, import_id, repository_id, commit_sha, attempt_no) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(row.0)
        .bind(&evidence.evidence_type)
        .bind(&evidence.description)
        .bind(&evidence.file_path)
        .bind(&evidence.symbol)
        .bind(&evidence.code_range)
        .bind(evidence.import_id)
        .bind(evidence.repository_id)
        .bind(&evidence.commit_sha)
        .bind(evidence.attempt_no)
        .execute(&mut *tx)
        .await
        .map_err(map_evidence_error)?;
    }
    for repository_id in &repository_ids {
        sqlx::query(
            "INSERT INTO discovery.discovery_finding_repository \
             (tenant_id, project_id, finding_id, repository_id) VALUES ($1, $2, $3, $4)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(row.0)
        .bind(repository_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let mut evidence_map = evidence_by_finding(pool, tenant_id, &[row.0]).await?;
    let evidence = evidence_map.remove(&row.0).unwrap_or_else(|| serde_json::json!([]));
    Ok(to_finding(row, repository_ids, evidence))
}

pub async fn list_findings(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<DiscoveryFindingView>, DiscoveryError> {
    let rows: Vec<FindingRow> = sqlx::query_as(&format!(
        "SELECT {FINDING_COLS} FROM discovery.discovery_finding \
         WHERE tenant_id = $1 AND session_id = $2 ORDER BY created_at, id"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let links = repository_ids_by_session(pool, tenant_id, session_id).await?;
    let ids: Vec<Uuid> = rows.iter().map(|row| row.0).collect();
    let evidence_map = evidence_by_finding(pool, tenant_id, &ids).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let ids = links.get(&row.0).cloned().unwrap_or_default();
            let evidence = evidence_map.get(&row.0).cloned().unwrap_or_else(|| serde_json::json!([]));
            to_finding(row, ids, evidence)
        })
        .collect())
}

pub async fn get_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    finding_id: Uuid,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    let row: Option<FindingRow> = sqlx::query_as(&format!(
        "SELECT {FINDING_COLS} FROM discovery.discovery_finding WHERE tenant_id = $1 AND id = $2"
    ))
    .bind(tenant_id)
    .bind(finding_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Err(DiscoveryError::FindingNotFound);
    };
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT repository_id FROM discovery.discovery_finding_repository \
         WHERE tenant_id = $1 AND finding_id = $2 ORDER BY repository_id",
    )
    .bind(tenant_id)
    .bind(row.0)
    .fetch_all(pool)
    .await?;
    let mut evidence_map = evidence_by_finding(pool, tenant_id, &[row.0]).await?;
    let evidence = evidence_map.remove(&row.0).unwrap_or_else(|| serde_json::json!([]));
    Ok(to_finding(row, ids, evidence))
}

pub async fn set_finding_verification(
    pool: &PgPool,
    tenant_id: Uuid,
    finding_id: Uuid,
    status: &str,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    validate_one_of(status, &VERIFICATION_STATUSES, "verification_status")?;
    let updated = sqlx::query(
        "UPDATE discovery.discovery_finding SET verification_status = $3 \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .bind(status)
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(DiscoveryError::FindingNotFound);
    }
    get_finding(pool, tenant_id, finding_id).await
}

/// 证据落库的约束违例（外键 / CHECK）映射成**业务校验错误**，不冒 500。
fn map_evidence_error(err: sqlx::Error) -> DiscoveryError {
    if let Some(db) = err.as_database_error() {
        if db.is_foreign_key_violation() {
            return DiscoveryError::Validation(
                "evidence references an import attempt that does not belong to this project"
                    .to_string(),
            );
        }
        if db.is_check_violation() {
            return DiscoveryError::Validation(
                "evidence violates the code/non-code linkage rules".to_string(),
            );
        }
    }
    DiscoveryError::Db(err)
}
