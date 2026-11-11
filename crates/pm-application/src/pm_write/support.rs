use super::*;

/// 小写 hex 的 sha256（pm-domain 同款摘要约定，无 hex 依赖时手写编码）。
pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// ───────────────────────────────────────────── 修订治理通用助手（批 A1）
//
// design.rs / task_plan.rs / requirement.rs 三域逐行同构的样板在此归一：
// 行锁读取、版本状态机守卫、内容哈希、唯一约束判定、顺序发号。

/// 唯一约束冲突（23505）→ 供调用方翻译成业务 Conflict（并发撞单/重名等）。
pub(crate) fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

/// 修订内容的确定性哈希：JSON 序列化字节流的 sha256 小写 hex。
pub(crate) fn content_hash_of(content: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(content).expect("serialize content");
    hex_sha256(&bytes)
}

/// 状态机流转守卫：`legal` 不放行的流转一律 `Conflict`。
pub(crate) fn transition_or_conflict<S: Copy>(
    legal: impl Fn(S, S) -> bool,
    from: S,
    to: S,
    what: &'static str,
) -> Result<(), PmWriteError> {
    if legal(from, to) {
        Ok(())
    } else {
        Err(PmWriteError::Conflict(what))
    }
}

/// 聚合头行加锁读（`FOR UPDATE`，租户边界）：
/// 返回 (project_id, 状态列, current_revision_id, row_revision)。
pub(crate) async fn lock_head_row(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    id: Uuid,
    table: &str,
    status_col: &str,
    not_found: &'static str,
) -> Result<(Uuid, String, Option<Uuid>, i64), PmWriteError> {
    sqlx::query_as(&format!(
        "SELECT project_id, {status_col}, current_revision_id, row_revision \
         FROM {table} WHERE id = $1 AND tenant_id = $2 FOR UPDATE"
    ))
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(conn)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound(not_found))
}

/// 修订行加锁读（`FOR UPDATE`，租户边界）：`SELECT {cols} FROM {table}
/// WHERE id = $1 AND {parent_col} = $2 AND tenant_id = $3`。
///
/// 行结构由调用方的 `R`（sqlx 元组）与 `cols` 一一对应。
#[allow(clippy::too_many_arguments)] // 表名/列名/双 id 的参数化行锁
pub(crate) async fn lock_revision_row<R>(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    revision_id: Uuid,
    parent_id: Uuid,
    table: &str,
    parent_col: &str,
    cols: &str,
    not_found: &'static str,
) -> Result<R, PmWriteError>
where
    R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    sqlx::query_as::<_, R>(&format!(
        "SELECT {cols} FROM {table} \
         WHERE id = $1 AND {parent_col} = $2 AND tenant_id = $3 FOR UPDATE"
    ))
    .bind(revision_id)
    .bind(parent_id)
    .bind(tenant_id)
    .fetch_optional(conn)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound(not_found))
}

/// 租户级审计（governance.tenant_audit_log；四身份列全写——C174 守卫）。
/// `target_*` 走 0041 通用列；审计失败即整体回滚（调用方在事务内执行）。
/// 单一实现点：team_llm / team_onboarding / team_llm_catalog /
/// device_credential 四域原各持一份同构 SQL（批 A2 归并）。
#[allow(clippy::too_many_arguments)] // 审计插入行，列清单与 tenant_audit_log 对应
pub(crate) async fn insert_tenant_audit(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.tenant_audit_log \
         (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, target_principal_id, before_data, after_data) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10, $11)",
    )
    .bind(tenant_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(before)
    .bind(after)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 项目内顺序发号：`{prefix}-NNN`（NNN 三位补零），取全表同前缀最大序号 +1。
pub(crate) async fn next_code(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    table: &str,
    prefix: &str,
) -> Result<String, sqlx::Error> {
    let max: i64 = sqlx::query_scalar(&format!(
        "SELECT COALESCE(MAX(NULLIF(regexp_replace(code, '\\D', '', 'g'), '')::bigint), 0) \
         FROM {table} WHERE tenant_id = $1 AND project_id = $2 AND code LIKE '{prefix}-%'"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(conn)
    .await?;
    Ok(format!("{prefix}-{:03}", max + 1))
}

#[cfg(test)]
mod tests {
    use super::hex_sha256;

    #[test]
    fn hex_sha256_is_lowercase_hex_of_sha256() {
        // echo -n "abc" | sha256sum
        assert_eq!(
            hex_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
