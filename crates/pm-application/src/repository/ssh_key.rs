use super::*;

/// 项目下可并存多把 key；仓库可经 `project_repository.ssh_key_id` 绑定一把，
/// 未绑定的仓库回退「项目默认 key」（`is_default`）→ 最新 active。
/// 迁移 0022：PK=(id)、UNIQUE (project_id, label)（同名导入 = 替换该把）。

/// 列出项目所有密钥（含已吊销），默认把排最前，其余按创建时间倒序。
pub async fn list_ssh_keys(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<SshKeyView>, RepositoryError> {
    let rows: Vec<(Uuid, String, String, String, String, bool, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, label, public_key, fingerprint, status, is_default, created_at \
         FROM integration.project_ssh_key WHERE project_id = $1 \
         ORDER BY is_default DESC, created_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, label, public_key, fingerprint, status, is_default, created_at)| SshKeyView {
                id,
                project_id,
                label,
                public_key,
                fingerprint,
                status,
                is_default,
                created_at,
            },
        )
        .collect())
}

/// 项目默认 key（is_default 且 active）；wizard 幂等锚点。
async fn default_key_view(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<SshKeyView>, RepositoryError> {
    let row: Option<(Uuid, String, String, String, String, bool, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, label, public_key, fingerprint, status, is_default, created_at \
         FROM integration.project_ssh_key \
         WHERE project_id = $1 AND is_default AND status = 'active'",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(id, label, public_key, fingerprint, status, is_default, created_at)| SshKeyView {
            id,
            project_id,
            label,
            public_key,
            fingerprint,
            status,
            is_default,
            created_at,
        },
    ))
}

/// 生成或复用「项目默认 key」。`rotate=false` 时已有 active 默认把直接返回
/// （wizard 幂等）；`rotate=true` 时把 label='default' 那把覆盖为新密钥材料
/// （同 row 换钥——绑定它的仓库自动跟随，无需重绑）。其它把的 `is_default`
/// 先清位，保证部分唯一索引不冲突。
pub async fn ensure_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    rotate: bool,
) -> Result<SshKeyView, RepositoryError> {
    if !rotate {
        if let Some(existing) = default_key_view(pool, project_id).await? {
            return Ok(existing);
        }
    }

    let code = project_code(pool, project_id).await?;
    let generated = secret::generate_ed25519(&format!("ai-pm:{code}"))?;
    let ciphertext = secret::seal_text(project_id, generated.private_key.as_bytes())?;

    let mut tx = pool.begin().await?;
    // 默认标记易主：把其它 is_default 行先清位（含已吊销行），避免唯一索引冲突。
    sqlx::query(
        "UPDATE integration.project_ssh_key SET is_default = false, updated_at = now() \
         WHERE project_id = $1 AND is_default AND label <> 'default'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;
    let row: (Uuid, String, String, String, bool, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO integration.project_ssh_key \
         (project_id, label, key_type, public_key, private_key_ciphertext, fingerprint, \
          is_default, status, created_by, tenant_id) \
         VALUES ($1, 'default', 'ed25519', $2, $3, $4, true, 'active', $5, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) \
         ON CONFLICT (project_id, label) DO UPDATE SET \
             key_type = EXCLUDED.key_type, \
             public_key = EXCLUDED.public_key, \
             private_key_ciphertext = EXCLUDED.private_key_ciphertext, \
             fingerprint = EXCLUDED.fingerprint, \
             status = 'active', \
             revoked_at = NULL, \
             is_default = true, \
             created_by = EXCLUDED.created_by, \
             updated_at = now() \
         RETURNING id, label, public_key, fingerprint, is_default, created_at",
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
        id: row.0,
        project_id,
        label: row.1,
        public_key: row.2,
        fingerprint: row.3,
        status: "active".to_string(),
        is_default: row.4,
        created_at: row.5,
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
/// 推导失败（格式错误 / 带口令）→ 400 Validation。按 label 落位：
/// 同名 = 替换该把密钥材料（is_default 保持原状），新名 = 新增一把；
/// 项目第一把 active key 自动成为默认把。
pub async fn import_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    private_key: &str,
    label: &str,
) -> Result<SshKeyView, RepositoryError> {
    let label = require_text(
        Some(label.to_string()),
        "ssh key label is required",
        MAX_KEY_LABEL_LEN,
    )?;
    let normalized = secret::normalize_private_key(private_key);
    let derived = secret::derive_public_key(&normalized)?;
    let ciphertext = secret::seal_text(project_id, normalized.as_bytes())?;

    let mut tx = pool.begin().await?;
    let row: (Uuid, String, String, String, bool, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO integration.project_ssh_key \
         (project_id, label, key_type, public_key, private_key_ciphertext, fingerprint, \
          is_default, status, created_by, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, \
                 NOT EXISTS (SELECT 1 FROM integration.project_ssh_key \
                             WHERE project_id = $1 AND status = 'active'), \
                 'active', $7, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         ON CONFLICT (project_id, label) DO UPDATE SET \
             key_type = EXCLUDED.key_type, \
             public_key = EXCLUDED.public_key, \
             private_key_ciphertext = EXCLUDED.private_key_ciphertext, \
             fingerprint = EXCLUDED.fingerprint, \
             status = 'active', \
             revoked_at = NULL, \
             created_by = EXCLUDED.created_by, \
             updated_at = now() \
         RETURNING id, label, public_key, fingerprint, is_default, created_at",
    )
    .bind(project_id)
    .bind(&label)
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
        id: row.0,
        project_id,
        label: row.1,
        public_key: row.2,
        fingerprint: row.3,
        status: "active".to_string(),
        is_default: row.4,
        created_at: row.5,
    })
}

/// 吊销一把 key；仍被 verified 仓库绑定引用时拒绝（409 key_in_use）。
pub async fn revoke_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    key_id: Uuid,
) -> Result<(), RepositoryError> {
    let in_use: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.project_repository \
         WHERE ssh_key_id = $1 AND status = 'verified'",
    )
    .bind(key_id)
    .fetch_one(pool)
    .await?;
    if in_use > 0 {
        return Err(RepositoryError::KeyInUse);
    }
    let mut tx = pool.begin().await?;
    let affected = sqlx::query(
        "UPDATE integration.project_ssh_key \
         SET status = 'revoked', revoked_at = now(), updated_at = now() \
         WHERE id = $1 AND project_id = $2 AND status = 'active'",
    )
    .bind(key_id)
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
        Some(key_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 重命名一把 key（label 即显示名；UNIQUE (project_id, label) 冲突 → 409 LabelTaken）。
/// 允许重命名默认把：is_default 跟行走，`ensure_ssh_key(rotate=false)` 按
/// is_default 定位不受影响；rotate=true 会重建 label='default' 行并接管默认位。
pub async fn rename_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    key_id: Uuid,
    label: &str,
) -> Result<SshKeyView, RepositoryError> {
    let label = require_text(
        Some(label.to_string()),
        "ssh key label is required",
        MAX_KEY_LABEL_LEN,
    )?;
    let mut tx = pool.begin().await?;
    let row: Option<(Uuid, String, String, String, bool, String, DateTime<Utc>)> = sqlx::query_as(
        "UPDATE integration.project_ssh_key SET label = $3, updated_at = now() \
             WHERE id = $1 AND project_id = $2 \
             RETURNING id, label, public_key, fingerprint, is_default, status, created_at",
    )
    .bind(key_id)
    .bind(project_id)
    .bind(&label)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        if e.as_database_error()
            .map(|db| {
                db.constraint()
                    .is_some_and(|c| c == "uq_project_ssh_key_project_label")
            })
            .unwrap_or(false)
        {
            RepositoryError::LabelTaken
        } else {
            RepositoryError::Db(e)
        }
    })?;
    let Some(row) = row else {
        return Err(RepositoryError::SshKeyNotFound);
    };
    append_audit(
        &mut tx,
        project_id,
        actor,
        "ssh_key.renamed",
        "project_ssh_key",
        Some(key_id),
    )
    .await?;
    tx.commit().await?;

    Ok(SshKeyView {
        id: row.0,
        project_id,
        label: row.1,
        public_key: row.2,
        fingerprint: row.3,
        status: row.5,
        is_default: row.4,
        created_at: row.6,
    })
}

/// 物理删除一把 key；被任何仓库绑定引用（FK RESTRICT）→ 409 KeyInUse。
pub async fn delete_ssh_key(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    key_id: Uuid,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let affected =
        sqlx::query("DELETE FROM integration.project_ssh_key WHERE id = $1 AND project_id = $2")
            .bind(key_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                if e.as_database_error()
                    .map(|db| db.is_foreign_key_violation())
                    .unwrap_or(false)
                {
                    RepositoryError::KeyInUse
                } else {
                    RepositoryError::Db(e)
                }
            })?
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
        Some(key_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 仓库绑定 key 的回退解析（迁移 0022 语义，verify/import/delivery/executor 共用）：
/// 项目默认 key → 最新 active key；都没有 → None（调用方落 ssh_key_missing）。
pub async fn fallback_key_ciphertext(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<Vec<u8>>, RepositoryError> {
    let ciphertext: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key \
         WHERE project_id = $1 AND status = 'active' \
         ORDER BY is_default DESC, created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(ciphertext)
}

/// 按 id 取绑定 key 的密文（校验同项目且 active；失效/越权 → None）。
pub async fn key_ciphertext_by_id(
    pool: &PgPool,
    project_id: Uuid,
    key_id: Uuid,
) -> Result<Option<Vec<u8>>, RepositoryError> {
    let ciphertext: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key \
         WHERE id = $1 AND project_id = $2 AND status = 'active'",
    )
    .bind(key_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(ciphertext)
}

/// key 所属项目（路由层租户边界判定用；不存在 → None）。
pub async fn ssh_key_project_id(
    pool: &PgPool,
    key_id: Uuid,
) -> Result<Option<Uuid>, RepositoryError> {
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM integration.project_ssh_key WHERE id = $1")
            .bind(key_id)
            .fetch_optional(pool)
            .await?;
    Ok(project_id)
}

/// 解密私钥（仅供后续 clone 切片使用；REST 不暴露）。
pub fn open_ssh_private_key(
    project_id: Uuid,
    ciphertext: &[u8],
) -> Result<String, RepositoryError> {
    let plain = secret::open_text(project_id, ciphertext)?;
    String::from_utf8(plain).map_err(|_| RepositoryError::Crypto)
}

// ----------------------------------------------------------------- repositories
