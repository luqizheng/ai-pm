use super::*;

/// 读取项目的 active 密钥（无密钥返回 None）。响应只含公钥与指纹。
pub async fn get_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<SshKeyView>, RepositoryError> {
    let row: Option<(String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT label, public_key, fingerprint, status, created_at \
         FROM integration.project_ssh_key WHERE project_id = $1 AND status = 'active'",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(label, public_key, fingerprint, status, created_at)| SshKeyView {
            project_id,
            label,
            public_key,
            fingerprint,
            status,
            created_at,
        },
    ))
}

/// 列出项目所有密钥（含已吊销），供管理页展示。
pub async fn list_ssh_keys(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<SshKeyView>, RepositoryError> {
    let rows: Vec<(String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT label, public_key, fingerprint, status, created_at \
         FROM integration.project_ssh_key WHERE project_id = $1 ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(label, public_key, fingerprint, status, created_at)| SshKeyView {
                project_id,
                label,
                public_key,
                fingerprint,
                status,
                created_at,
            },
        )
        .collect())
}

/// 生成或复用项目密钥。`rotate=false` 时已有 active 直接返回（向导幂等）；
/// `rotate=true` 时覆盖为新的密钥对（管理页「重新生成」）。
pub async fn ensure_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    rotate: bool,
) -> Result<SshKeyView, RepositoryError> {
    if !rotate {
        if let Some(existing) = get_ssh_key(pool, project_id).await? {
            return Ok(existing);
        }
    }

    let code = project_code(pool, project_id).await?;
    let generated = secret::generate_ed25519(&format!("ai-pm:{code}"))?;
    let ciphertext = secret::seal_text(project_id, generated.private_key.as_bytes())?;

    let mut tx = pool.begin().await?;
    let (created_at,): (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO integration.project_ssh_key \
         (project_id, public_key, private_key_ciphertext, fingerprint, status, created_by, tenant_id) \
         VALUES ($1, $2, $3, $4, 'active', $5, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         ON CONFLICT (project_id) DO UPDATE SET \
             public_key = EXCLUDED.public_key, \
             private_key_ciphertext = EXCLUDED.private_key_ciphertext, \
             fingerprint = EXCLUDED.fingerprint, \
             status = 'active', \
             revoked_at = NULL, \
             created_by = EXCLUDED.created_by, \
             updated_at = now() \
         RETURNING created_at",
    )
    .bind(project_id)
    .bind(&generated.public_key)
    .bind(&ciphertext)
    .bind(&generated.fingerprint)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        if rotate {
            "ssh_key.rotated"
        } else {
            "ssh_key.generated"
        },
        "project_ssh_key",
        Some(project_id),
    )
    .await?;
    tx.commit().await?;

    Ok(SshKeyView {
        project_id,
        label: "default".to_string(),
        public_key: generated.public_key,
        fingerprint: generated.fingerprint,
        status: "active".to_string(),
        created_at,
    })
}

/// 兼容既有调用方：等同于 rotate=true。
pub async fn generate_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
) -> Result<SshKeyView, RepositoryError> {
    ensure_ssh_key(pool, project_id, actor, true).await
}

/// 导入外部私钥（仓库设置页粘贴通道）：`ssh-keygen -y` 推导公钥/指纹/key_type，
/// 推导失败（格式错误 / 带口令）→ 400 Validation。覆盖式 upsert——一项目一把，
/// 与 rotate 语义一致；label = 'imported' 区分于生成密钥。
pub async fn import_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    private_key: &str,
) -> Result<SshKeyView, RepositoryError> {
    let normalized = secret::normalize_private_key(private_key);
    let derived = secret::derive_public_key(&normalized)?;
    let ciphertext = secret::seal_text(project_id, normalized.as_bytes())?;

    let mut tx = pool.begin().await?;
    let (created_at,): (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO integration.project_ssh_key \
         (project_id, label, key_type, public_key, private_key_ciphertext, fingerprint, \
          status, created_by, tenant_id) \
         VALUES ($1, 'imported', $2, $3, $4, $5, 'active', $6, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         ON CONFLICT (project_id) DO UPDATE SET \
             label = EXCLUDED.label, \
             key_type = EXCLUDED.key_type, \
             public_key = EXCLUDED.public_key, \
             private_key_ciphertext = EXCLUDED.private_key_ciphertext, \
             fingerprint = EXCLUDED.fingerprint, \
             status = 'active', \
             revoked_at = NULL, \
             created_by = EXCLUDED.created_by, \
             updated_at = now() \
         RETURNING created_at",
    )
    .bind(project_id)
    .bind(&derived.key_type)
    .bind(&derived.public_key)
    .bind(&ciphertext)
    .bind(&derived.fingerprint)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "ssh_key.imported",
        "project_ssh_key",
        Some(project_id),
    )
    .await?;
    tx.commit().await?;

    Ok(SshKeyView {
        project_id,
        label: "imported".to_string(),
        public_key: derived.public_key,
        fingerprint: derived.fingerprint,
        status: "active".to_string(),
        created_at,
    })
}

/// 吊销密钥；仍被 verified 仓库引用时拒绝。
pub async fn revoke_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
) -> Result<(), RepositoryError> {
    let in_use: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.project_repository \
         WHERE project_id = $1 AND status = 'verified'",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    if in_use > 0 {
        return Err(RepositoryError::KeyInUse);
    }
    let mut tx = pool.begin().await?;
    let affected = sqlx::query(
        "UPDATE integration.project_ssh_key \
         SET status = 'revoked', revoked_at = now(), updated_at = now() \
         WHERE project_id = $1 AND status = 'active'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(RepositoryError::SshKeyNotFound);
    }
    append_audit(
        &mut tx,
        project_id,
        actor,
        "ssh_key.revoked",
        "project_ssh_key",
        Some(project_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let affected = sqlx::query("DELETE FROM integration.project_ssh_key WHERE project_id = $1")
        .bind(project_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(RepositoryError::SshKeyNotFound);
    }
    append_audit(
        &mut tx,
        project_id,
        actor,
        "ssh_key.deleted",
        "project_ssh_key",
        Some(project_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 解密私钥（仅供后续 clone 切片使用；P0 不暴露 REST）。
pub fn open_ssh_private_key(
    project_id: Uuid,
    ciphertext: &[u8],
) -> Result<String, RepositoryError> {
    let plain = secret::open_text(project_id, ciphertext)?;
    String::from_utf8(plain).map_err(|_| RepositoryError::Crypto)
}

// ----------------------------------------------------------------- repositories
