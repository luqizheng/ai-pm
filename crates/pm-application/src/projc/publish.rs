//! ProjC 版本发布事务（pc/publish.rs 01B §18.1 同款镜像，真源 = pc 同名文件；
//! 域内改名：projc_* 表 / projc_ 前缀审计 / projc.cognition.version_published 事件）。
//! 提案/审批命令归 proposal.rs，此处只消费 approved 提案。

use std::collections::BTreeSet;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use kb_pc_contracts::registry::TargetDomain;
use kb_pc_contracts::DomainRef;

use crate::actor::AuditActor;
use crate::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};

use super::{
    canonical_members_hash, canonical_proposal_hash, ensure_formal_authorized, MemberKind,
    ProjcError, VersionMemberPin,
};

/// 发布入参（成员集从提案行构建；发布幂等键=服务端确定性派生
/// `projc-decision:{proposal_id}`——GAP-31-A 键来源裁决：与 close 链同款，
/// 调用方不得给定任意键；内化后非规范键无输入面=fail-closed 结构性满足）。
pub struct PublishVersionInput {
    pub proposal_id: Uuid,
}

#[derive(Debug)]
pub struct PublishReceipt {
    pub version_id: Uuid,
    pub version_no: i64,
    pub snapshot_hash: String,
    pub member_count: usize,
    pub replayed: bool,
}

/// 提案成员行（发布与提案命令共用形状）。
#[derive(Debug, Clone)]
pub struct ProposalMemberPin {
    pub change_kind: String,
    pub object_id: Option<Uuid>,
    pub object_revision_id: Option<Uuid>,
    pub relationship_id: Option<Uuid>,
    pub relationship_revision_id: Option<Uuid>,
}

impl ProposalMemberPin {
    pub(crate) fn pin(&self) -> Option<VersionMemberPin> {
        match (self.object_id, self.object_revision_id) {
            (Some(o), Some(r)) => Some(VersionMemberPin {
                kind: MemberKind::Object,
                identity_id: o,
                revision_id: r,
            }),
            _ => match (self.relationship_id, self.relationship_revision_id) {
                (Some(r), Some(rr)) => Some(VersionMemberPin {
                    kind: MemberKind::Relationship,
                    identity_id: r,
                    revision_id: rr,
                }),
                _ => None,
            },
        }
    }
}

/// 前版本成员携带行（object/revision × 双维度的四元组投影）。
type CarriedRow = (Option<Uuid>, Option<Uuid>, Option<Uuid>, Option<Uuid>);

/// 发布事务（pc 01B §18.1 十四步语义同构；PCGIT 后置——write_operation 仅发布
/// 幂等，无 Git 侧动作；⑫b 同事务 outbox）。
///
/// 门禁链（顺序即锁序）：advisory(project) → write_operation(幂等) → proposal
/// FOR UPDATE → approval FOR UPDATE + 锚定重算 → 真人+A3 → L3 冻结 → 成员构建
/// （携带 + 目标 + 端点并入）→ snapshot_hash 先算后插 → version → members →
/// 闭合断言 → 审计 → outbox → proposal applied + 生命周期推进 → 账本 completed。
pub async fn publish_version(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: PublishVersionInput,
) -> Result<PublishReceipt, ProjcError> {
    publish_inner(pool, project_id, actor, input).await
}

/// 发布主体：只消费 approved 提案 + approved 审批事实（§18.1）。决定（暂记）
/// 与发布解耦后，发布入口 = 报告 close 批量（报告 close 双域分流，W5）。
pub(super) async fn publish_inner(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: PublishVersionInput,
) -> Result<PublishReceipt, ProjcError> {
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|_| ProjcError::NotFound("project not found".into()))?;

    // 发布幂等键=服务端确定性派生（GAP-31-A 键来源裁决：全发布入口统一，
    // 与 close 链同款；调用方不得给定任意键）。
    let operation_id = format!("projc-decision:{}", input.proposal_id);

    // ④ 前置门禁（事务外只读判定；事务内不再重判——与 H-11 已知窗口一致）
    ensure_formal_authorized(pool, project_id, actor).await?;

    let mut tx = pool.begin().await?;

    // ① 项目级 advisory lock（发布入口串行化）
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('projc_publish:' || $1::text)::bigint)")
        .bind(project_id.to_string())
        .execute(&mut *tx)
        .await?;

    // 写幂等：同 operation_id 已 completed → 幂等返回既有版本
    let existing: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "INSERT INTO projc.write_operation \
         (tenant_id, project_id, operation_id, command, status, expected_content_hash, created_by) \
         VALUES ($1, $2, $3, 'projc_publish_version', 'pending', 'pending', $4) \
         ON CONFLICT (tenant_id, project_id, operation_id) DO NOTHING \
         RETURNING status, version_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&operation_id)
    .bind(actor.principal_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (op_status, op_id) = match existing {
        Some(row) => {
            let id: Uuid = sqlx::query_scalar(
                "SELECT id FROM projc.write_operation \
                  WHERE tenant_id = $1 AND project_id = $2 AND operation_id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(&operation_id)
            .fetch_one(&mut *tx)
            .await?;
            (row.0, id)
        }
        None => {
            let row: (String, Option<Uuid>) = sqlx::query_as(
                "SELECT status, version_id FROM projc.write_operation \
                  WHERE tenant_id = $1 AND project_id = $2 AND operation_id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(&operation_id)
            .fetch_one(&mut *tx)
            .await?;
            if row.0 == "completed" {
                if let Some(version_id) = row.1 {
                    let (version_no, snapshot_hash): (i64, String) = sqlx::query_as(
                        "SELECT version_no, snapshot_hash FROM projc.cognition_version WHERE id = $1",
                    )
                    .bind(version_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    return Ok(PublishReceipt {
                        version_id,
                        version_no,
                        snapshot_hash,
                        member_count: 0,
                        replayed: true,
                    });
                }
            }
            let id: Uuid = sqlx::query_scalar(
                "SELECT id FROM projc.write_operation \
                  WHERE tenant_id = $1 AND project_id = $2 AND operation_id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(&operation_id)
            .fetch_one(&mut *tx)
            .await?;
            (row.0, id)
        }
    };
    // 既有行非终态（pending/failed 重放）：继续走发布（提案状态与版本号唯一
    // 约束兜底）；completed 已在上方短路返回。
    let _ = op_status;

    // ②③ 提案 + 审批锚点（FOR UPDATE；重算 hash 主校验——不信任 proposal 可变列）
    let proposal: (String, i64, String, String) = sqlx::query_as(
        "SELECT status, expected_version_no, governance_level, proposal_content_hash \
           FROM projc.proposal WHERE tenant_id = $1 AND project_id = $2 AND id = $3 \
           FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(input.proposal_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ProjcError::NotFound("proposal not found".into()))?;
    let (proposal_status, expected_version_no, _gov_level, proposal_hash) = proposal;
    let required_status = "approved";
    if proposal_status != required_status {
        return Err(ProjcError::Conflict(format!(
            "proposal is `{proposal_status}`; this step requires `{required_status}`"
        )));
    }
    let approval: Option<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT id, proposal_content_hash, expected_version_no FROM projc.approval \
          WHERE tenant_id = $1 AND project_id = $2 AND proposal_id = $3 AND status = $4 \
          FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(input.proposal_id)
    .bind(required_status)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((_approval_id, approval_hash, approval_expected)) = approval else {
        return Err(ProjcError::Conflict(format!(
            "no {required_status} approval for proposal"
        )));
    };

    // 成员集装载（proposal_member 全量；②b 完整性断言）
    let member_rows: Vec<ProposalMemberPin> = sqlx::query_as(
        "SELECT change_kind, object_id, object_revision_id, relationship_id, relationship_revision_id \
           FROM projc.proposal_member \
          WHERE tenant_id = $1 AND project_id = $2 AND proposal_id = $3 \
          ORDER BY created_at, id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(input.proposal_id)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|(change_kind, object_id, object_revision_id, relationship_id, relationship_revision_id): (String, Option<Uuid>, Option<Uuid>, Option<Uuid>, Option<Uuid>)| {
        ProposalMemberPin { change_kind, object_id, object_revision_id, relationship_id, relationship_revision_id }
    })
    .collect();
    if member_rows.is_empty() {
        return Err(ProjcError::Invalid("proposal has no members".into()));
    }
    let incomplete = member_rows.iter().any(|m| m.pin().is_none());
    if incomplete {
        return Err(ProjcError::Invalid(
            "proposal members incomplete (every member must pin identity + revision)".into(),
        ));
    }

    // 提案内容重算（③ 主校验）：以库内成员集为准
    let recomputed = canonical_proposal_hash_from_rows(
        &mut *tx,
        tenant_id,
        project_id,
        input.proposal_id,
        &member_rows,
    )
    .await?;
    if recomputed != approval_hash || recomputed != proposal_hash {
        return Err(ProjcError::Conflict(
            "proposal content hash mismatch (approval anchor vs recomputed members)".into(),
        ));
    }
    if approval_expected != expected_version_no {
        return Err(ProjcError::Conflict(
            "approval expected_version_no != proposal expected_version_no".into(),
        ));
    }

    // ② CAS：当前版本号必须与提案基线一致
    let current_version: Option<(i64, Uuid)> = sqlx::query_as(
        "SELECT version_no, id FROM projc.cognition_version \
          WHERE tenant_id = $1 AND project_id = $2 ORDER BY version_no DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    let current_no = current_version.as_ref().map(|(n, _)| *n).unwrap_or(0);
    if current_no != expected_version_no {
        // CAS 失配（§18.1 ②）：同事务内置 failed（主事务持有 proposal 行锁，
        // 独立事务补偿会自死锁）→ COMMIT → 返回 Conflict。
        sqlx::query(
            "UPDATE projc.proposal SET status = 'failed', updated_at = now() \
              WHERE tenant_id = $1 AND project_id = $2 AND id = $3 \
                AND status IN ('submitted','approved')",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(input.proposal_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE projc.write_operation SET status = 'failed', last_error = $2, updated_at = now() \
              WHERE id = $1",
        )
        .bind(op_id)
        .bind("version baseline moved; re-propose against the new version")
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO governance.audit_log \
             (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
              action, target_type, target_id, after_data, source_kind, source_ref, tenant_id) \
             VALUES ($1, $2, NULL, NULL, NULL, NULL, \
                     'projc_version.publish_failed', 'projc_proposal', $3, $4, 'projc_publish', $3, $5)",
        )
        .bind(project_id)
        .bind(crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
        .bind(input.proposal_id)
        .bind(json!({
            "reason": "version baseline moved; re-propose against the new version",
            "phase": "cas_mismatch",
            "operation_id": operation_id.clone(),
        }))
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Err(ProjcError::Conflict(
            "version baseline moved; re-propose against the new version".into(),
        ));
    }

    // ⑥ L3 冻结校验（未解决冲突圈定提案目标 → 拒绝）
    let target_objects: Vec<Uuid> = member_rows.iter().filter_map(|m| m.object_id).collect();
    let target_relationships: Vec<Uuid> = member_rows
        .iter()
        .filter_map(|m| m.relationship_id)
        .collect();
    let frozen: Option<Uuid> = sqlx::query_scalar(
        "SELECT c.id FROM projc.conflict c \
           JOIN projc.conflict_object co ON (co.tenant_id, co.project_id, co.conflict_id) \
             = (c.tenant_id, c.project_id, c.id) \
          WHERE c.tenant_id = $1 AND c.project_id = $2 \
            AND c.status IN ('detected','under_review','awaiting_human','reopened') \
            AND ((co.object_id = ANY($3) AND cardinality($3) > 0) \
              OR (co.relationship_id = ANY($4) AND cardinality($4) > 0)) \
          LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&target_objects)
    .bind(&target_relationships)
    .fetch_optional(&mut *tx)
    .await?;
    if frozen.is_some() {
        return Err(ProjcError::Forbidden(
            "proposal targets are frozen by an unresolved conflict (L3)".into(),
        ));
    }

    // ⑦ 成员集构建：前版本携带 − 废弃目标 − 被替换身份 + 提案目标 + 端点自动并入
    // （F-05）。完整快照 = 每个身份在版本内恰一个 Revision——提案目标与端点钉扎
    // 替换前版本携带的同身份旧钉扎。
    let mut incoming: BTreeSet<VersionMemberPin> = BTreeSet::new();
    for m in &member_rows {
        if m.change_kind.starts_with("deprecate_") {
            continue;
        }
        if let Some(p) = m.pin() {
            incoming.insert(p);
        }
    }
    // 端点自动并入（§10.1 成员时序契约 3）：留存关系成员的 ProjC 内端点必须可还原
    // （废弃关系离开版本，其端点无需并入）
    for m in &member_rows {
        if m.change_kind.starts_with("deprecate_") {
            continue;
        }
        let Some(rr) = m.relationship_revision_id else {
            continue;
        };
        let endpoints: Vec<CarriedRow> = sqlx::query_as(
            "SELECT source_object_id, source_object_revision_id, \
                    target_object_id, target_object_revision_id \
               FROM projc.relationship_revision \
              WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(rr)
        .fetch_all(&mut *tx)
        .await?;
        for (so, sr, to, tr) in endpoints {
            for (o, r) in [(so, sr), (to, tr)] {
                if let (Some(o), Some(r)) = (o, r) {
                    incoming.insert(VersionMemberPin {
                        kind: MemberKind::Object,
                        identity_id: o,
                        revision_id: r,
                    });
                }
            }
        }
    }
    let mut set: BTreeSet<VersionMemberPin> = BTreeSet::new();
    if let Some((_, pred_id)) = current_version {
        let carried: Vec<CarriedRow> = sqlx::query_as(
            "SELECT object_id, object_revision_id, relationship_id, relationship_revision_id \
               FROM projc.version_member WHERE version_id = $1",
        )
        .bind(pred_id)
        .fetch_all(&mut *tx)
        .await?;
        for (o, or, r, rr) in carried {
            let pin = ProposalMemberPin {
                change_kind: String::new(),
                object_id: o,
                object_revision_id: or,
                relationship_id: r,
                relationship_revision_id: rr,
            };
            if let Some(p) = pin.pin() {
                set.insert(p);
            }
        }
    }
    let mut deprecating: BTreeSet<VersionMemberPin> = BTreeSet::new();
    for m in &member_rows {
        if m.change_kind == "deprecate_object" || m.change_kind == "deprecate_relationship" {
            if let Some(p) = m.pin() {
                // 废弃 = 从新版本集合剔除该身份的所有历史钉扎
                deprecating.insert(VersionMemberPin {
                    kind: p.kind,
                    identity_id: p.identity_id,
                    revision_id: p.revision_id,
                });
            }
        }
    }
    let same_identity = |a: &VersionMemberPin, b: &VersionMemberPin| {
        a.kind == b.kind && a.identity_id == b.identity_id
    };
    set.retain(|p| {
        !deprecating.iter().any(|d| same_identity(d, p))
            && !incoming.iter().any(|i| same_identity(i, p))
    });
    set.extend(incoming);
    let members: Vec<VersionMemberPin> = set.into_iter().collect();
    if members.is_empty() {
        return Err(ProjcError::Invalid("version member set is empty".into()));
    }
    // 每身份恰一个 Revision（BTreeSet 有序：同身份相邻）
    if let Some(pair) = members.windows(2).find(|w| same_identity(&w[0], &w[1])) {
        return Err(ProjcError::Conflict(format!(
            "version member set pins {} {} at multiple revisions ({} / {})",
            pair[0].kind.as_str(),
            pair[0].identity_id,
            pair[0].revision_id,
            pair[1].revision_id
        )));
    }

    // ⑦ snapshot_hash 先算后插（V1.1 R2：消除占位回填矛盾）
    let snapshot_hash = canonical_members_hash(&members);

    // ⑧⑨ 版本 + 成员（发布即原子）
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.cognition_version \
         (tenant_id, project_id, version_no, confirmed_by, source_proposal_id, snapshot_hash, \
          predecessor_version_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(current_no + 1)
    .bind(actor.principal_id)
    .bind(input.proposal_id)
    .bind(&snapshot_hash)
    .bind(current_version.map(|(_, id)| id))
    .fetch_one(&mut *tx)
    .await?;
    for m in &members {
        match m.kind {
            MemberKind::Object => {
                sqlx::query(
                    "INSERT INTO projc.version_member \
                     (tenant_id, project_id, version_id, object_id, object_revision_id) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(version_id)
                .bind(m.identity_id)
                .bind(m.revision_id)
                .execute(&mut *tx)
                .await?;
            }
            MemberKind::Relationship => {
                sqlx::query(
                    "INSERT INTO projc.version_member \
                     (tenant_id, project_id, version_id, relationship_id, relationship_revision_id) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(version_id)
                .bind(m.identity_id)
                .bind(m.revision_id)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    // ⑩ 闭合断言（§8.3；DEFERRED 触发器在 COMMIT 前兜底）
    let broken: Vec<(Uuid, i32, String)> = sqlx::query_as(
        "SELECT rr.relationship_id, rr.revision_no, \
                CASE WHEN rr.source_object_id IS NOT NULL THEN 'source' ELSE 'target' END \
           FROM projc.version_member vm \
           JOIN projc.relationship_revision rr \
             ON (rr.tenant_id, rr.project_id, rr.relationship_id, rr.id) \
              = (vm.tenant_id, vm.project_id, vm.relationship_id, vm.relationship_revision_id) \
          WHERE vm.tenant_id = $1 AND vm.project_id = $2 AND vm.version_id = $3 \
            AND ((rr.source_object_id IS NOT NULL AND NOT EXISTS ( \
                    SELECT 1 FROM projc.version_member m2 \
                     WHERE m2.tenant_id = vm.tenant_id AND m2.project_id = vm.project_id \
                       AND m2.version_id = vm.version_id \
                       AND m2.object_id = rr.source_object_id \
                       AND m2.object_revision_id = rr.source_object_revision_id)) \
              OR (rr.target_object_id IS NOT NULL AND NOT EXISTS ( \
                    SELECT 1 FROM projc.version_member m2 \
                     WHERE m2.tenant_id = vm.tenant_id AND m2.project_id = vm.project_id \
                       AND m2.version_id = vm.version_id \
                       AND m2.object_id = rr.target_object_id \
                       AND m2.object_revision_id = rr.target_object_revision_id)))",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(version_id)
    .fetch_all(&mut *tx)
    .await?;
    if !broken.is_empty() {
        let first = &broken[0];
        return Err(ProjcError::Conflict(format!(
            "endpoint closure violated: relationship {} rev {} missing {} endpoint member(s); {} total",
            first.0, first.1, first.2, broken.len()
        )));
    }

    // ⑫ 审计（同事务；四身份列全写；source_ref=提案锚，operation_id 入 after_data）
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, before_data, after_data, source_kind, source_ref, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, \
                 'projc_version.publish', 'projc_cognition_version', $7, NULL, $8, 'projc_publish', $9, $10)",
    )
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(version_id)
    .bind(json!({
        "version_no": current_no + 1,
        "snapshot_hash": snapshot_hash,
        "member_count": members.len(),
        "proposal_id": input.proposal_id,
        "operation_id": operation_id.clone(),
    }))
    .bind(input.proposal_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?;

    // ⑫b 同事务事件回写（Transactional Outbox）
    let version_ref = DomainRef::for_domain(
        TargetDomain::ProjectCognition,
        "cognition_version",
        version_id.to_string(),
        None,
    );
    let proposal_ref = DomainRef::for_domain(
        TargetDomain::ProjectCognition,
        "proposal",
        input.proposal_id.to_string(),
        None,
    );
    PgOutboxAppend
        .append(
            &mut tx,
            OutboxEvent {
                project_id,
                event_type: kb_pc_contracts::events::PROJ_COGNITION_VERSION_PUBLISHED,
                aggregate_type: "projc_cognition_version",
                aggregate_id: &version_id.to_string(),
                payload: json!({
                    "version_ref": version_ref,
                    "version_no": current_no + 1,
                    "snapshot_hash": snapshot_hash,
                    "proposal_ref": proposal_ref,
                }),
                idempotency_key: &format!("projc_publish:{version_id}"),
            },
        )
        .await?;

    // ⑬ 提案 applied + 账本 completed + 生命周期推进
    let applied = sqlx::query(
        "UPDATE projc.proposal SET status = 'applied', updated_at = now() \
          WHERE tenant_id = $1 AND project_id = $2 AND id = $3 AND status = 'approved'",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(input.proposal_id)
    .execute(&mut *tx)
    .await?;
    if applied.rows_affected() != 1 {
        return Err(ProjcError::Conflict(
            "proposal status moved during publish".into(),
        ));
    }
    sqlx::query(
        "UPDATE projc.write_operation SET status = 'completed', version_id = $2, updated_at = now() \
          WHERE id = $1",
    )
    .bind(op_id)
    .bind(version_id)
    .execute(&mut *tx)
    .await?;

    // 生命周期推进（pc §4.2 同款：首次入正式版本 = active；废弃 = deprecated 终态）
    let create_ids: Vec<Uuid> = member_rows
        .iter()
        .filter(|m| m.change_kind == "create_object" || m.change_kind == "update_object")
        .filter_map(|m| m.object_id)
        .collect();
    if !create_ids.is_empty() {
        sqlx::query(
            "UPDATE projc.cognition_object \
                SET lifecycle_status = 'active', updated_by = $2, updated_at = now() \
              WHERE id = ANY($1) AND lifecycle_status IN ('identity','draft','active')",
        )
        .bind(&create_ids)
        .bind(actor.principal_id)
        .execute(&mut *tx)
        .await?;
    }
    let deprecate_ids: Vec<Uuid> = member_rows
        .iter()
        .filter(|m| m.change_kind == "deprecate_object")
        .filter_map(|m| m.object_id)
        .collect();
    if !deprecate_ids.is_empty() {
        sqlx::query(
            "UPDATE projc.cognition_object \
                SET lifecycle_status = 'deprecated', deprecated_at = now(), \
                    updated_by = $2, updated_at = now() \
              WHERE id = ANY($1) AND lifecycle_status IN ('identity','draft','active')",
        )
        .bind(&deprecate_ids)
        .bind(actor.principal_id)
        .execute(&mut *tx)
        .await?;
    }
    let rel_active_ids: Vec<Uuid> = member_rows
        .iter()
        .filter(|m| {
            m.change_kind == "create_relationship" || m.change_kind == "update_relationship"
        })
        .filter_map(|m| m.relationship_id)
        .collect();
    if !rel_active_ids.is_empty() {
        sqlx::query(
            "UPDATE projc.relationship \
                SET lifecycle_status = 'active', updated_at = now() \
              WHERE id = ANY($1) AND lifecycle_status IN ('identity','draft','active')",
        )
        .bind(&rel_active_ids)
        .execute(&mut *tx)
        .await?;
    }
    let rel_deprecate_ids: Vec<Uuid> = member_rows
        .iter()
        .filter(|m| m.change_kind == "deprecate_relationship")
        .filter_map(|m| m.relationship_id)
        .collect();
    if !rel_deprecate_ids.is_empty() {
        sqlx::query(
            "UPDATE projc.relationship \
                SET lifecycle_status = 'deprecated', updated_at = now() \
              WHERE id = ANY($1) AND lifecycle_status IN ('identity','draft','active')",
        )
        .bind(&rel_deprecate_ids)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(PublishReceipt {
        version_id,
        version_no: current_no + 1,
        snapshot_hash,
        member_count: members.len(),
        replayed: false,
    })
}

/// 提案内容哈希的库侧重算输入（元数据来自行、成员来自 proposal_member 排序）；
/// 提交/裁决/发布三处锚点比对共用。
pub(super) async fn canonical_proposal_hash_from_rows(
    exec: impl sqlx::PgExecutor<'_>,
    _tenant_id: Uuid,
    _project_id: Uuid,
    proposal_id: Uuid,
    members: &[ProposalMemberPin],
) -> Result<String, ProjcError> {
    let row: Option<(String, String, String, String, i64)> = sqlx::query_as(
        "SELECT title, rationale, origin_kind, governance_level, expected_version_no \
           FROM projc.proposal WHERE id = $1",
    )
    .bind(proposal_id)
    .fetch_optional(exec)
    .await?;
    let Some((title, rationale, origin_kind, governance_level, expected)) = row else {
        return Err(ProjcError::NotFound("proposal not found".into()));
    };
    let pins: Vec<VersionMemberPin> = members.iter().filter_map(|m| m.pin()).collect();
    Ok(canonical_proposal_hash(
        &title,
        &rationale,
        &origin_kind,
        &governance_level,
        expected,
        &pins,
    ))
}
