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
    /// I2-033 业务展示态（§8.1 四态）：由**最近一次有效人工决定审计**推导，
    /// 不从 open question 推导。取值 `PENDING`（待确认）/ `REVISION_REQUESTED`
    /// （待补充）/ `CONFIRMED`（已确认）/ `REJECTED`（已拒绝）/ `CONFLICT`。
    #[serde(default)]
    pub display_state: String,
}

/// 人工决定审计动作（I2-033 推导依据；`discovery_finding_revision_requested`
/// 为 I2 V1.1 §8.2 指定名，其余为既有实现名）。
pub const DECISION_AUDIT_ACTIONS: [&str; 2] =
    ["finding.decided", "discovery_finding_revision_requested"];

/// 「要求补充」决定（I2-C §7：不改变 `verification_status`，只落审计）。
pub const DECISION_REVISION_REQUESTED: &str = "REVISION_REQUESTED";

/// 批量取每个 Finding 最近一次有效人工决定审计的 action（I2-033 推导第一步）。
///
/// 按 `created_at DESC, id DESC` 取最近一条（§8.3「时间 + 稳定顺序」）；
/// 无审计 → 不出现在结果里（历史数据保守展示，§8.4）。
pub async fn latest_decision_actions(
    pool: &PgPool,
    project_id: Uuid,
    finding_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, String>, sqlx::Error> {
    let mut map = std::collections::HashMap::new();
    if finding_ids.is_empty() {
        return Ok(map);
    }
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT ON (target_id) target_id, action \
         FROM governance.audit_log \
         WHERE project_id = $1 AND target_type = 'discovery_finding' \
           AND target_id = ANY($2) AND action = ANY($3) \
         ORDER BY target_id, created_at DESC, id DESC",
    )
    .bind(project_id)
    .bind(finding_ids)
    .bind(DECISION_AUDIT_ACTIONS)
    .fetch_all(pool)
    .await?;
    for (finding_id, action) in rows {
        map.insert(finding_id, action);
    }
    Ok(map)
}

/// §8.1 展示态映射（结合当前 `verification_status` 与最近一次有效决定）。
pub fn derive_display_state(
    verification_status: &str,
    latest_decision_action: Option<&str>,
) -> String {
    match verification_status {
        "CONFIRMED" => "CONFIRMED".to_string(),
        "REJECTED" => "REJECTED".to_string(),
        // PENDING + 要求补充 → 待补充；挂有 AI open question **不**参与推导。
        "PENDING" if latest_decision_action == Some("discovery_finding_revision_requested") => {
            "REVISION_REQUESTED".to_string()
        }
        // 其余（无有效决定 / 历史不完整 / CONFLICT）保守展示原状态。
        other => other.to_string(),
    }
}

/// Finding 的创建通道（I2-B / I2-D：**入口决定定态**，外部不可指定）。
///
/// - [`FindingCreator::HumanDirect`]：真人在平台 UI 上的直接陈述（REST 入口，
///   调用主体经鉴权层解析为 `core.principal.kind='human'`）。
/// - [`FindingCreator::AgentRelayed`]：Agent 的采集回传（MCP 入口）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingCreator {
    HumanDirect,
    AgentRelayed,
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
    pub actor_role: Option<String>,
}

/// 证据输入项。
#[derive(Debug, Clone, Deserialize, Serialize)]
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
    latest_decision_action: Option<&str>,
) -> DiscoveryFindingView {
    let display_state = derive_display_state(&row.5, latest_decision_action);
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
        display_state,
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

/// 读出证据行 → 前端兼容的 JSON 数组（事务版）。
async fn evidence_by_finding_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    finding_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, serde_json::Value>, DiscoveryError> {
    let mut map: std::collections::HashMap<Uuid, serde_json::Value> =
        std::collections::HashMap::new();
    if finding_ids.is_empty() {
        return Ok(map);
    }
    type Row = (
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<Uuid>,
        Option<String>,
        Option<i32>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT finding_id, evidence_type, description, file_path, symbol, code_range, \
                    repository_id, commit_sha, attempt_no \
             FROM discovery.discovery_finding_evidence \
             WHERE tenant_id = $1 AND finding_id = ANY($2) \
             ORDER BY created_at, id",
    )
    .bind(tenant_id)
    .bind(finding_ids)
    .fetch_all(&mut **tx)
    .await?;

    for (
        finding_id,
        evidence_type,
        description,
        file_path,
        symbol,
        code_range,
        repository_id,
        commit_sha,
        attempt_no,
    ) in rows
    {
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
    type Row = (
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<Uuid>,
        Option<String>,
        Option<i32>,
    );
    let rows: Vec<Row> = sqlx::query_as(
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

    for (
        finding_id,
        evidence_type,
        description,
        file_path,
        symbol,
        code_range,
        repository_id,
        commit_sha,
        attempt_no,
    ) in rows
    {
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
    creator: FindingCreator,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    let mut tx = pool.begin().await?;
    let view = add_finding_in_tx(&mut tx, tenant_id, session_id, new_finding, creator).await?;
    tx.commit().await?;
    Ok(view)
}

/// [`add_finding`] 的事务内核——供「Finding 决定 + 质询/回答同事务」的
/// 统一决定服务复用（I2-036：Finding 更新 + question/answer + 审计单事务）。
pub(crate) async fn add_finding_in_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    session_id: Uuid,
    new_finding: NewFinding,
    creator: FindingCreator,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    validate_one_of(&new_finding.kind, &FINDING_KINDS, "kind")?;
    validate_confidence(new_finding.confidence)?;
    // I2-B 定态规则（I2-023 / I2-024 / I2-026）：
    // - 外部不可指定 verification_status（字段已从契约删除，此处派生）；
    // - 可信 human **直接陈述** user_confirmed → 创建即 CONFIRMED（同事务落库）；
    // - Agent 转述的 user_confirmed → 仍 PENDING（须人工确认后才能成为已确认事实）；
    // - 其余 kind 一律 PENDING。
    let verification_status = match (&new_finding.kind[..], creator) {
        ("user_confirmed", FindingCreator::HumanDirect) => "CONFIRMED",
        _ => "PENDING",
    };
    validate_one_of(
        verification_status,
        &VERIFICATION_STATUSES,
        "verification_status",
    )?;
    if new_finding.content.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "content must not be blank".to_string(),
        ));
    }
    let project_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM discovery.discovery_session WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?;
    let project_id = project_id.ok_or(DiscoveryError::SessionNotFound)?;
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
        .fetch_one(&mut **tx)
        .await?;
        if found != repository_ids.len() as i64 {
            return Err(DiscoveryError::Validation(
                "repository_ids must all belong to the project".to_string(),
            ));
        }
    }
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
    .fetch_one(&mut **tx)
    .await?;
    for evidence in &evidence_rows {
        insert_evidence(tx, tenant_id, project_id, row.0, evidence).await?;
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
        .execute(&mut **tx)
        .await?;
    }
    let ids = vec![row.0];
    let mut evidence_map = evidence_by_finding_tx(tx, tenant_id, &ids).await?;
    let evidence = evidence_map
        .remove(&row.0)
        .unwrap_or_else(|| serde_json::json!([]));
    Ok(to_finding(row, repository_ids, evidence, None))
}

/// 写入一条证据（I6-B-04）。
///
/// `attempt_no` + `import_id` + `repository_id` + `commit_sha` 把证据锚定到
/// **某一次分析轮次**——五列组合 FK 保证「Attempt 属于其 Source」是键结构事实。
pub async fn submit_evidence(
    pool: &PgPool,
    tenant_id: Uuid,
    finding_id: Uuid,
    input: EvidenceInput,
) -> Result<serde_json::Value, DiscoveryError> {
    let rows = parse_evidence(&serde_json::json!([input]))?;
    let evidence = rows
        .into_iter()
        .next()
        .ok_or_else(|| DiscoveryError::Validation("evidence must not be empty".to_string()))?;
    let project_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM discovery.discovery_finding WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .fetch_optional(pool)
    .await?;
    let project_id = project_id.ok_or(DiscoveryError::FindingNotFound)?;

    let mut tx = pool.begin().await?;
    insert_evidence(&mut tx, tenant_id, project_id, finding_id, &evidence).await?;
    tx.commit().await?;

    Ok(serde_json::json!({
        "finding_id": finding_id,
        "evidence_type": evidence.evidence_type,
        "description": evidence.description,
        "file_path": evidence.file_path,
        "symbol": evidence.symbol,
        "code_range": evidence.code_range,
        "import_id": evidence.import_id,
        "repository_id": evidence.repository_id,
        "commit_sha": evidence.commit_sha,
        "attempt_no": evidence.attempt_no,
    }))
}

/// 插入一条证据行（约束违例映射成业务校验错误，不冒 500）。
async fn insert_evidence(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    finding_id: Uuid,
    evidence: &EvidenceRow,
) -> Result<(), DiscoveryError> {
    sqlx::query(
        "INSERT INTO discovery.discovery_finding_evidence \
         (tenant_id, project_id, finding_id, evidence_type, description, file_path, symbol, \
          code_range, import_id, repository_id, commit_sha, attempt_no) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(finding_id)
    .bind(&evidence.evidence_type)
    .bind(&evidence.description)
    .bind(&evidence.file_path)
    .bind(&evidence.symbol)
    .bind(&evidence.code_range)
    .bind(evidence.import_id)
    .bind(evidence.repository_id)
    .bind(&evidence.commit_sha)
    .bind(evidence.attempt_no)
    .execute(&mut **tx)
    .await
    .map_err(map_evidence_error)?;
    Ok(())
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
    // I2-033：批量取最近一次有效人工决定审计（project_id 各行相同）。
    let project_id = rows.first().map(|row| row.6);
    let decisions = match project_id {
        Some(project_id) => latest_decision_actions(pool, project_id, &ids).await?,
        None => std::collections::HashMap::new(),
    };
    Ok(rows
        .into_iter()
        .map(|row| {
            let ids = links.get(&row.0).cloned().unwrap_or_default();
            let evidence = evidence_map
                .get(&row.0)
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let latest = decisions.get(&row.0).map(String::as_str);
            to_finding(row, ids, evidence, latest)
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
    let evidence = evidence_map
        .remove(&row.0)
        .unwrap_or_else(|| serde_json::json!([]));
    let decisions = latest_decision_actions(pool, row.6, &[row.0]).await?;
    let latest = decisions.get(&row.0).map(String::as_str);
    Ok(to_finding(row, ids, evidence, latest))
}

/// 人工决定入口的**唯一**状态写点（I2-035：不再作为可绕过审计/锁的公开原语
/// 存在——`pub(crate)`，仅 [`crate::discovery::web::record_finding_verification`]
/// 的单事务决定服务可调）。
pub(crate) async fn set_finding_verification_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    finding_id: Uuid,
    status: &str,
) -> Result<String, DiscoveryError> {
    // 条件更新（I2-020）：同值重复决定是无害 no-op（rows_affected = 0），
    // 旧状态变化后旧操作不能无条件覆盖。
    let updated: Option<String> = sqlx::query_scalar(
        "UPDATE discovery.discovery_finding SET verification_status = $3 \
         WHERE tenant_id = $1 AND id = $2 AND verification_status IS DISTINCT FROM $3 \
         RETURNING verification_status",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .bind(status)
    .fetch_optional(&mut **tx)
    .await?;
    match updated {
        Some(after) => Ok(after),
        // 0 行 = 该 finding 不存在，或状态已是目标值（no-op）——二者对调用方
        // 语义一致，由调用方先 SELECT FOR UPDATE 区分。
        None => Ok(status.to_string()),
    }
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
