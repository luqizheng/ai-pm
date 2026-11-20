//! [`MemberRepo`] 的 Postgres 实现（P4-ORG Phase 2：Team 成员变更事务 SQL
//! 收口，原 repo_tenant_members 按 team 维度重写）。锁序与级联语义保留。

use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use pm_application::project_member::revoke_principal_memberships;
use pm_application::repo::team_members::{
    ChangeSnapshot, MemberAuditActor, MemberRepo, MemberRepoError, MemberWrite,
    MemberWriteOutcome, PlanFn, TeamMemberRow,
};

/// 无状态连接句柄（`PgPool` 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub struct PgMemberRepo {
    pool: sqlx::PgPool,
}

impl PgMemberRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

type RepoResult<T> = Result<T, MemberRepoError>;

fn role_of(raw: Option<&str>) -> Option<&'static str> {
    match raw {
        Some("owner") => Some("owner"),
        Some("member") => Some("member"),
        _ => None,
    }
}

#[async_trait]
impl MemberRepo for PgMemberRepo {
    async fn list_members(&self, team_id: Uuid) -> RepoResult<Vec<TeamMemberRow>> {
        let rows: Vec<TeamMemberRow> = sqlx::query_as(
            "SELECT m.principal_id, p.display_name, m.role, p.is_active, m.created_at \
               FROM core.team_member m \
               JOIN core.principal p ON p.id = m.principal_id \
              WHERE m.team_id = $1 \
              ORDER BY CASE m.role WHEN 'owner' THEN 0 ELSE 1 END, \
                       m.created_at",
        )
        .bind(team_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn find_member(
        &self,
        team_id: Uuid,
        principal_id: Uuid,
    ) -> RepoResult<Option<TeamMemberRow>> {
        let row: Option<TeamMemberRow> = sqlx::query_as(
            "SELECT m.principal_id, p.display_name, m.role, p.is_active, m.created_at \
               FROM core.team_member m \
               JOIN core.principal p ON p.id = m.principal_id \
              WHERE m.team_id = $1 AND m.principal_id = $2",
        )
        .bind(team_id)
        .bind(principal_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn apply_change(
        &self,
        team_id: Uuid,
        actor: &MemberAuditActor,
        target_id: Uuid,
        plan: PlanFn<'_>,
    ) -> RepoResult<MemberWriteOutcome> {
        let mut tx = self.pool.begin().await?;

        // 1) 锁 Team 行：同一 Team 的所有成员变更串行化。只锁 owner 行不够——
        //    两个事务可以各锁不同的 owner 行，在 READ COMMITTED 下同时读到
        //    「owner 数 = 2」而互相降级，最终剩 0 个 owner。顺带取 tenant_id
        //    供级联撤权与审计使用。
        let locked: Option<(Uuid,)> =
            sqlx::query_as("SELECT tenant_id FROM core.team WHERE id = $1 FOR UPDATE")
                .bind(team_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((tenant_id,)) = locked else {
            return Err(MemberRepoError::NotFound("team not found"));
        };

        // 2) 锁调用者行（权限校验的输入）。
        let actor_role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM core.team_member \
              WHERE team_id = $1 AND principal_id = $2 FOR UPDATE",
        )
        .bind(team_id)
        .bind(actor.principal_id)
        .fetch_optional(&mut *tx)
        .await?;

        // 3) 锁目标成员行（可能不存在）。
        let target_role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM core.team_member \
              WHERE team_id = $1 AND principal_id = $2 FOR UPDATE",
        )
        .bind(team_id)
        .bind(target_id)
        .fetch_optional(&mut *tx)
        .await?;

        let owner_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM core.team_member WHERE team_id = $1 AND role = 'owner'",
        )
        .bind(team_id)
        .fetch_one(&mut *tx)
        .await?;

        let snapshot = ChangeSnapshot {
            actor_role: role_of(actor_role.as_deref()),
            target_role: role_of(target_role.as_deref()),
            owner_count,
        };

        // 4) 规则裁决（HTTP 层纯函数，锁内同步执行；Err = 整体回滚）。
        let (writes, audits) = plan(snapshot).map_err(|(c, m)| MemberRepoError::RuleRejected(c, m))?;

        // 5) S01-W13：移除 Team 成员前，若该人是某项目的 owner，则**拒绝移除**——
        //    否则级联撤权会静默产生「无主项目」。
        for w in &writes {
            if let MemberWrite::Remove { principal_id } = w {
                let owned: Vec<Uuid> = sqlx::query_scalar(
                    "SELECT project_id FROM core.project_member \
                      WHERE tenant_id = $1 AND principal_id = $2 AND role = 'owner' FOR UPDATE",
                )
                .bind(tenant_id)
                .bind(principal_id)
                .fetch_all(&mut *tx)
                .await?;
                if !owned.is_empty() {
                    return Err(MemberRepoError::ProjectOwnerDependency);
                }
            }
        }

        // 6) DML。
        let mut outcome = MemberWriteOutcome::default();
        for w in writes {
            match w {
                MemberWrite::Add {
                    principal_id,
                    role,
                    granted_by,
                } => {
                    sqlx::query(
                        "INSERT INTO core.team_member \
                         (team_id, principal_id, tenant_id, role, granted_by) \
                         VALUES ($1, $2, $3, $4, $5)",
                    )
                    .bind(team_id)
                    .bind(principal_id)
                    .bind(tenant_id)
                    .bind(role)
                    .bind(granted_by)
                    .execute(&mut *tx)
                    .await?;
                }
                MemberWrite::SetRole { principal_id, role } => {
                    sqlx::query(
                        "UPDATE core.team_member SET role = $3, updated_at = now() \
                          WHERE team_id = $1 AND principal_id = $2",
                    )
                    .bind(team_id)
                    .bind(principal_id)
                    .bind(role)
                    .execute(&mut *tx)
                    .await?;
                }
                MemberWrite::Remove { principal_id } => {
                    sqlx::query(
                        "DELETE FROM core.team_member WHERE team_id = $1 AND principal_id = $2",
                    )
                    .bind(team_id)
                    .bind(principal_id)
                    .execute(&mut *tx)
                    .await?;

                    // S01-W13：级联撤销该真人在本租户内的**全部真人项目成员关系**。
                    // Agent 职能行不在级联范围（租户级实例，bootstrap 维护）。
                    let revoke_actor = pm_application::actor::AuditActor {
                        principal_id: actor.principal_id,
                        identity_id: actor.identity_id,
                        client: actor.client.clone(),
                        auth_mode: Some(actor.auth_mode.clone()),
                        claimed_device_id: None,
                    };
                    let revoked = revoke_principal_memberships(
                        &mut tx,
                        tenant_id,
                        principal_id,
                        &revoke_actor,
                    )
                    .await?;
                    outcome.project_memberships_revoked = revoked.len();
                    outcome.revoked_project_ids = revoked;
                }
            }
        }

        // 7) 审计（级联撤销计数并入 removed 行的 after_data）。
        for line in &audits {
            let mut after_extra = line.after_extra.clone();
            if line.action == "team_member.removed" && outcome.project_memberships_revoked > 0 {
                after_extra = Some(json!({
                    "project_memberships_revoked": outcome.project_memberships_revoked,
                    "revoked_project_ids": outcome.revoked_project_ids,
                }));
            }
            let mut map = serde_json::Map::new();
            if let Some(role) = line.after_role {
                map.insert("role".to_string(), json!(role));
            }
            if let Some(serde_json::Value::Object(extra)) = &after_extra {
                for (k, v) in extra {
                    map.insert(k.clone(), v.clone());
                }
            }
            let after_data = if map.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(map))
            };
            sqlx::query(
                "INSERT INTO governance.tenant_audit_log \
                 (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
                  action, target_principal_id, before_data, after_data) \
                 VALUES ($1, $2, $3, $4, $5, NULL, $6, $7, $8, $9)",
            )
            .bind(tenant_id)
            .bind(actor.principal_id)
            .bind(actor.identity_id)
            .bind(&actor.client)
            .bind(&actor.auth_mode)
            .bind(line.action)
            .bind(line.target)
            .bind(line.before_role.map(|r| json!({ "role": r })))
            .bind(after_data)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(outcome)
    }
}
