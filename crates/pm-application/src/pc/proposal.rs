//! 提案与审批命令（KP-2 / PC-01C 确认闭环；01B §10 + §17.2 + §18.1 ⑥ + §19.1）。
//!
//! 状态机（服务层唯一推进面；库层触发器锚定内容列与审批事实）：
//!
//! ```text
//! draft ──submit──▶ submitted ──approve──▶ approved ──publish──▶ applied
//!                   submitted ──reject───▶ rejected
//! draft / submitted ──withdraw──▶ withdrawn
//! submitted / approved ──发布 CAS 失配──▶ failed（publish.rs）
//! ```
//!
//! 门禁（fail-closed）：
//! * create / submit = 提案面，AI 可达（Q2-C「AI 只能提案」）；V1 无 REST 入口——
//!   分析链自动起草（[`propose_from_analysis`]）+ 服务层调用；
//! * approve / reject = 真人 + A3（Lead OR `core.approver_grant` 生效行；AI/服务主体
//!   恒 403）；REST 侧另挂可信交互上下文（A*：API Key 403）；MCP 不暴露确认工具；
//! * L3 冻结：submit（§10.3）与 approve（§18.1 ⑥ 双落点之确认入口，F-06）前置校验，
//!   发布事务⑥兜底；reject 不受冻结约束（不改变正式认知）；
//! * 审批锚定（八原则 1/4/6/7）：submit 写 pending 审批行（content_hash +
//!   expected_version_no；`uq_pc_approval_pending` 兜底）；裁决时 FOR UPDATE 审批行 +
//!   库内重算哈希三方比对（提案行 / 审批行 / 成员集）+ 请求侧 expected_version_no CAS。
//!
//! 成员构建（§10.1 成员时序契约）：成员 identity + revision 建提案即齐备（PCGIT 后置，
//! 修订 draft 期已落 PG，偏差 a）；每身份恰一个 Revision；F-05 端点自动并入（关系成员
//! 的 PC 内端点缺失 → 补 `update_object` 行，与既有成员修订冲突 → 拒绝）；R7/F-07 治理
//! 等级下限 = 全部目标类型固定等级之最大值；闭合预检（提案对现行版本的投影不得断开
//! 携带关系的端点——端点改钉须在同一提案内更新/废弃该关系）。
//!
//! 审计（§19.1）：`pc_proposal.create/submit/withdraw/approve/reject`，原事务内落
//! `governance.audit_log`（四身份列全写，C174/C180 已登记）。锁序（§17.3）：
//! advisory（分析起草）→ proposal → approval → audit → outbox。

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use kb_pc_contracts::registry::TargetDomain;
use kb_pc_contracts::DomainRef;

use crate::actor::AuditActor;
use crate::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};

use super::publish::canonical_proposal_hash_from_rows;
use super::{
    canonical_proposal_hash, ensure_formal_authorized, frozen_by_conflict, PcError,
    ProposalMemberPin, PublishReceipt, VersionMemberPin,
};

/// 提案来源（01B §10.1 `origin_kind` CHECK 同集）。
pub const ORIGIN_KINDS: [&str; 5] = [
    "ai_agent",
    "kb_memory",
    "manual_input",
    "requirement_change",
    "conflict_resolution",
];

const OBJECT_CHANGE_KINDS: [&str; 3] = ["create_object", "update_object", "deprecate_object"];
const RELATIONSHIP_CHANGE_KINDS: [&str; 3] = [
    "create_relationship",
    "update_relationship",
    "deprecate_relationship",
];
const GOVERNANCE_LEVELS: [&str; 2] = ["L1", "L2"];
const RISK_LEVELS: [&str; 4] = ["low", "medium", "high", "critical"];
const CONFIDENCES: [&str; 3] = ["high", "medium", "low"];
const AUDIT_SOURCE_KIND: &str = "pc_proposal";

// ── 入参 / 回执 ─────────────────────────────────────────────────────────────

pub struct CreateProposalInput {
    pub title: String,
    pub rationale: String,
    /// [`ORIGIN_KINDS`] 之一。
    pub origin_kind: String,
    /// 追溯链入口（可空：多来源时经成员修订的 evidence 追溯）。
    pub source_id: Option<Uuid>,
    /// None = 取目标类型等级下限（R7/F-07）；显式给出不得低于下限。
    pub governance_level: Option<String>,
    /// None = `medium`。
    pub risk_level: Option<String>,
    /// None = `medium`。
    pub confidence: Option<String>,
    /// 拟变更成员（端点成员由 F-05 自动并入，无需手工凑齐）。
    pub members: Vec<ProposalMemberPin>,
}

#[derive(Debug, Clone)]
pub struct ProposalReceipt {
    pub proposal_id: Uuid,
    pub status: String,
    pub governance_level: String,
    pub expected_version_no: i64,
    pub proposal_content_hash: String,
    pub member_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionAction {
    Approve,
    Reject,
}

impl DecisionAction {
    /// P5 wire 形态（`approve` / `reject`）。
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "approve" => Some(Self::Approve),
            "reject" => Some(Self::Reject),
            _ => None,
        }
    }

    fn status(self) -> &'static str {
        match self {
            Self::Approve => "approved",
            Self::Reject => "rejected",
        }
    }

    fn audit_action(self) -> &'static str {
        match self {
            Self::Approve => "pc_proposal.approve",
            Self::Reject => "pc_proposal.reject",
        }
    }
}

/// 审批裁决入参（approve/reject 共用）。
pub struct DecisionInput {
    pub reason: Option<String>,
    /// 请求侧 CAS：调用方确认的提案基线版本号，须等于提案锚定值。
    pub expected_version_no: i64,
}

#[derive(Debug, Clone)]
pub struct ApprovalReceipt {
    pub proposal_id: Uuid,
    pub approval_id: Uuid,
    pub status: String,
}

/// P5 决策入参（approve/reject 均为终局决定，Q-02b/Q-11；正式版本在报告 close 批量发布）。
pub struct ProposalDecisionInput {
    pub action: DecisionAction,
    pub reason: Option<String>,
    pub expected_version_no: i64,
}

#[derive(Debug)]
pub struct DecisionOutcome {
    pub proposal_id: Uuid,
    pub status: String,
    pub approval_id: Option<Uuid>,
    pub published_version: Option<PublishReceipt>,
}

// ── 内部助手 ────────────────────────────────────────────────────────────────

async fn project_tenant(pool: &PgPool, project_id: Uuid) -> Result<Uuid, PcError> {
    sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| PcError::NotFound("project not found".into()))
}

/// 提案成员行投影（change_kind + 双维度目标）。
type MemberRow = (
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
);

async fn load_members(
    exec: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    project_id: Uuid,
    proposal_id: Uuid,
) -> Result<Vec<ProposalMemberPin>, PcError> {
    let rows: Vec<MemberRow> = sqlx::query_as(
        "SELECT change_kind, object_id, object_revision_id, relationship_id, relationship_revision_id \
           FROM pc.proposal_member \
          WHERE tenant_id = $1 AND project_id = $2 AND proposal_id = $3 \
          ORDER BY created_at, id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_all(exec)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                change_kind,
                object_id,
                object_revision_id,
                relationship_id,
                relationship_revision_id,
            )| {
                ProposalMemberPin {
                    change_kind,
                    object_id,
                    object_revision_id,
                    relationship_id,
                    relationship_revision_id,
                }
            },
        )
        .collect())
}

async fn current_version(
    exec: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Option<(Uuid, i64)>, PcError> {
    Ok(sqlx::query_as(
        "SELECT id, version_no FROM pc.cognition_version \
          WHERE tenant_id = $1 AND project_id = $2 ORDER BY version_no DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(exec)
    .await?)
}

fn same_identity(a: &ProposalMemberPin, b: &ProposalMemberPin) -> bool {
    (a.object_id.is_some() && a.object_id == b.object_id)
        || (a.relationship_id.is_some() && a.relationship_id == b.relationship_id)
}

fn targets(members: &[ProposalMemberPin]) -> (Vec<Uuid>, Vec<Uuid>) {
    (
        members.iter().filter_map(|m| m.object_id).collect(),
        members.iter().filter_map(|m| m.relationship_id).collect(),
    )
}

/// 生命周期 × change_kind 合法性（终态不可提案；create 仅限未正式化；deprecate 仅限
/// 已正式化）。
fn check_lifecycle(
    kind: &str,
    change_kind: &str,
    id: Uuid,
    lifecycle: &str,
) -> Result<(), PcError> {
    if lifecycle == "deprecated" || lifecycle == "superseded" {
        return Err(PcError::Invalid(format!(
            "{kind} {id} is {lifecycle}; terminal targets cannot be proposed"
        )));
    }
    if change_kind.starts_with("create_") && lifecycle == "active" {
        return Err(PcError::Invalid(format!(
            "{kind} {id} is already formalized; propose update_{kind} instead"
        )));
    }
    if change_kind.starts_with("deprecate_") && lifecycle != "active" {
        return Err(PcError::Invalid(format!(
            "{kind} {id} is `{lifecycle}`; only formalized (active) targets can be deprecated"
        )));
    }
    Ok(())
}

/// 对象成员解析：修订归属对象且同项目；返回 (lifecycle, 类型治理等级)。
async fn resolve_object_member(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    object_id: Uuid,
    revision_id: Uuid,
) -> Result<(String, String), PcError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT o.lifecycle_status, t.governance_level \
           FROM pc.cognition_object o \
           JOIN pc.object_type t ON t.type_key = o.object_type \
           JOIN pc.cognition_revision r ON r.tenant_id = o.tenant_id AND r.project_id = o.project_id \
                                       AND r.object_id = o.id AND r.id = $4 \
          WHERE o.tenant_id = $1 AND o.project_id = $2 AND o.id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(object_id)
    .bind(revision_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.ok_or_else(|| {
        PcError::NotFound(format!(
            "revision {revision_id} of object {object_id} not found in project"
        ))
    })
}

/// 关系成员行投影（lifecycle, 类型治理等级, RR 端点 source/target 对象 + 修订）。
type RelationshipMemberRow = (
    String,
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
);

async fn resolve_relationship_member(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    relationship_id: Uuid,
    revision_id: Uuid,
) -> Result<RelationshipMemberRow, PcError> {
    let row: Option<RelationshipMemberRow> = sqlx::query_as(
        "SELECT rel.lifecycle_status, t.governance_level, \
                rr.source_object_id, rr.source_object_revision_id, \
                rr.target_object_id, rr.target_object_revision_id \
           FROM pc.relationship rel \
           JOIN pc.relationship_type t ON t.type_key = rel.relationship_type_key \
           JOIN pc.relationship_revision rr ON rr.tenant_id = rel.tenant_id \
                                          AND rr.project_id = rel.project_id \
                                          AND rr.relationship_id = rel.id AND rr.id = $4 \
          WHERE rel.tenant_id = $1 AND rel.project_id = $2 AND rel.id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(relationship_id)
    .bind(revision_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.ok_or_else(|| {
        PcError::NotFound(format!(
            "revision {revision_id} of relationship {relationship_id} not found in project"
        ))
    })
}

/// 携带关系行投影（relationship_id, source 对象 + 修订, target 对象 + 修订）。
type CarriedRelationshipRow = (Uuid, Option<Uuid>, Option<Uuid>, Option<Uuid>, Option<Uuid>);

/// 闭合预检：现行版本携带、且未被本提案触及的关系，其 PC 内端点不得被本提案废弃或
/// 改钉到其他 Revision——否则发布闭合断言（§8.3）必败，提案只会卡在审批之后。
async fn check_projected_closure(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    version_id: Uuid,
    members: &[ProposalMemberPin],
) -> Result<(), PcError> {
    let carried: Vec<CarriedRelationshipRow> = sqlx::query_as(
        "SELECT rr.relationship_id, rr.source_object_id, rr.source_object_revision_id, \
                rr.target_object_id, rr.target_object_revision_id \
           FROM pc.version_member vm \
           JOIN pc.relationship_revision rr \
             ON rr.tenant_id = vm.tenant_id AND rr.project_id = vm.project_id \
            AND rr.relationship_id = vm.relationship_id AND rr.id = vm.relationship_revision_id \
          WHERE vm.tenant_id = $1 AND vm.project_id = $2 AND vm.version_id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(version_id)
    .fetch_all(&mut **tx)
    .await?;
    for (relationship_id, so, sr, to, tr) in carried {
        if members
            .iter()
            .any(|m| m.relationship_id == Some(relationship_id))
        {
            continue;
        }
        for (object_id, revision_id) in [(so, sr), (to, tr)] {
            let (Some(object_id), Some(revision_id)) = (object_id, revision_id) else {
                continue;
            };
            let Some(member) = members.iter().find(|m| m.object_id == Some(object_id)) else {
                continue;
            };
            if member.change_kind == "deprecate_object"
                || member.object_revision_id != Some(revision_id)
            {
                return Err(PcError::Invalid(format!(
                    "object {object_id} is an endpoint of carried relationship {relationship_id} \
                     (pinned at revision {revision_id}); update or deprecate that relationship \
                     in the same proposal"
                )));
            }
        }
    }
    Ok(())
}

/// 提案审计（§19.1；四身份列全写——C174/C180 登记本文件 1 处；P5 原子确认经
/// publish.rs 复用本 helper，不新增审计 INSERT 点）。
#[allow(clippy::too_many_arguments)]
pub(super) async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    proposal_id: Uuid,
    before: Option<serde_json::Value>,
    after: serde_json::Value,
) -> Result<(), PcError> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, before_data, after_data, source_kind, source_ref, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'pc_proposal', $8, $9, $10, $11, $8, $12)",
    )
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(proposal_id)
    .bind(before)
    .bind(after)
    .bind(AUDIT_SOURCE_KIND)
    .bind(tenant_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ── create ──────────────────────────────────────────────────────────────────

/// 起草提案（draft）：成员规范化 + F-05 端点并入 + R7 等级下限 + 闭合预检 + 内容锚定
/// 哈希；`expected_version_no` = 当前正式版本号（0 = 尚无版本）。
pub async fn create_proposal(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: CreateProposalInput,
) -> Result<ProposalReceipt, PcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    let mut tx = pool.begin().await?;
    let receipt = create_proposal_in(&mut tx, tenant_id, project_id, actor, input).await?;
    tx.commit().await?;
    Ok(receipt)
}

async fn create_proposal_in(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    actor: &AuditActor,
    input: CreateProposalInput,
) -> Result<ProposalReceipt, PcError> {
    let title = input.title.trim().to_string();
    let rationale = input.rationale.trim().to_string();
    if title.is_empty() || rationale.is_empty() {
        return Err(PcError::Invalid("title and rationale are required".into()));
    }
    if !ORIGIN_KINDS.contains(&input.origin_kind.as_str()) {
        return Err(PcError::Invalid(format!(
            "unknown origin_kind `{}`",
            input.origin_kind
        )));
    }
    let risk_level = input.risk_level.unwrap_or_else(|| "medium".into());
    if !RISK_LEVELS.contains(&risk_level.as_str()) {
        return Err(PcError::Invalid(format!(
            "unknown risk_level `{risk_level}`"
        )));
    }
    let confidence = input.confidence.unwrap_or_else(|| "medium".into());
    if !CONFIDENCES.contains(&confidence.as_str()) {
        return Err(PcError::Invalid(format!(
            "unknown confidence `{confidence}`"
        )));
    }
    if let Some(level) = input.governance_level.as_deref() {
        if !GOVERNANCE_LEVELS.contains(&level) {
            return Err(PcError::Invalid(format!(
                "unknown governance_level `{level}`"
            )));
        }
    }
    if input.members.is_empty() {
        return Err(PcError::Invalid(
            "a proposal needs at least one member".into(),
        ));
    }
    if let Some(source_id) = input.source_id {
        let found: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM pc.source WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(source_id)
        .fetch_optional(&mut **tx)
        .await?;
        if found.is_none() {
            return Err(PcError::NotFound("source not found in project".into()));
        }
    }

    // ① 成员规范化：维度一致 + 每身份恰一个 Revision
    let mut members: Vec<ProposalMemberPin> = Vec::with_capacity(input.members.len());
    for m in input.members {
        let object_kind = OBJECT_CHANGE_KINDS.contains(&m.change_kind.as_str());
        if !object_kind && !RELATIONSHIP_CHANGE_KINDS.contains(&m.change_kind.as_str()) {
            return Err(PcError::Invalid(format!(
                "unknown change_kind `{}`",
                m.change_kind
            )));
        }
        let pinned = if object_kind {
            m.object_id.is_some()
                && m.object_revision_id.is_some()
                && m.relationship_id.is_none()
                && m.relationship_revision_id.is_none()
        } else {
            m.relationship_id.is_some()
                && m.relationship_revision_id.is_some()
                && m.object_id.is_none()
                && m.object_revision_id.is_none()
        };
        if !pinned {
            return Err(PcError::Invalid(format!(
                "member `{}` must pin exactly one identity + revision of its dimension",
                m.change_kind
            )));
        }
        if members.iter().any(|x| same_identity(x, &m)) {
            return Err(PcError::Invalid(format!(
                "proposal pins {} more than once (one revision per identity)",
                m.object_id.or(m.relationship_id).unwrap_or_default()
            )));
        }
        members.push(m);
    }

    // ② 目标解析：存在性/归属 + 生命周期 + 类型治理等级；留存关系成员收集 PC 内端点
    let mut levels: Vec<String> = Vec::new();
    let mut endpoints: Vec<(Uuid, Uuid, Uuid)> = Vec::new();
    for m in &members {
        if let (Some(object_id), Some(revision_id)) = (m.object_id, m.object_revision_id) {
            let (lifecycle, level) =
                resolve_object_member(tx, tenant_id, project_id, object_id, revision_id).await?;
            check_lifecycle("object", &m.change_kind, object_id, &lifecycle)?;
            levels.push(level);
        } else if let (Some(relationship_id), Some(revision_id)) =
            (m.relationship_id, m.relationship_revision_id)
        {
            let (lifecycle, level, so, sr, to, tr) = resolve_relationship_member(
                tx,
                tenant_id,
                project_id,
                relationship_id,
                revision_id,
            )
            .await?;
            check_lifecycle("relationship", &m.change_kind, relationship_id, &lifecycle)?;
            levels.push(level);
            if !m.change_kind.starts_with("deprecate_") {
                for (o, r) in [(so, sr), (to, tr)] {
                    if let (Some(o), Some(r)) = (o, r) {
                        endpoints.push((relationship_id, o, r));
                    }
                }
            }
        }
    }

    // ③ F-05 端点自动并入（§10.1-3）：缺失端点补 update_object 行；与既有成员冲突拒绝
    let mut merged = 0usize;
    for (relationship_id, object_id, revision_id) in endpoints {
        let existing = members.iter().position(|m| m.object_id == Some(object_id));
        match existing {
            Some(i) if members[i].change_kind == "deprecate_object" => {
                return Err(PcError::Invalid(format!(
                    "relationship {relationship_id} keeps object {object_id} as an endpoint \
                     while the proposal deprecates it"
                )));
            }
            Some(i) if members[i].object_revision_id == Some(revision_id) => {}
            Some(i) => {
                return Err(PcError::Invalid(format!(
                    "relationship {relationship_id} pins endpoint object {object_id} at revision \
                     {revision_id}, but the proposal pins revision {}",
                    members[i].object_revision_id.unwrap_or_default()
                )));
            }
            None => {
                let (lifecycle, level) =
                    resolve_object_member(tx, tenant_id, project_id, object_id, revision_id)
                        .await?;
                check_lifecycle("object", "update_object", object_id, &lifecycle)?;
                levels.push(level);
                members.push(ProposalMemberPin {
                    change_kind: "update_object".into(),
                    object_id: Some(object_id),
                    object_revision_id: Some(revision_id),
                    relationship_id: None,
                    relationship_revision_id: None,
                });
                merged += 1;
            }
        }
    }

    // ④ R7/F-07 治理等级下限（类型固定等级不可下调）
    let floor = if levels.iter().any(|l| l == "L2") {
        "L2"
    } else {
        "L1"
    };
    let governance_level = match input.governance_level.as_deref() {
        None => floor.to_string(),
        Some("L1") if floor == "L2" => {
            return Err(PcError::Invalid(
                "governance level L1 is below the L2 floor fixed by the target types (R7/F-07)"
                    .into(),
            ));
        }
        Some(level) => level.to_string(),
    };

    // ⑤ 基线 + 闭合预检
    let current = current_version(&mut **tx, tenant_id, project_id).await?;
    let expected_version_no = current.map(|(_, no)| no).unwrap_or(0);
    if let Some((version_id, _)) = current {
        check_projected_closure(tx, tenant_id, project_id, version_id, &members).await?;
    }

    // ⑥ 内容锚定哈希（发布③ 以库内成员集重算比对）+ 落库
    let pins: Vec<VersionMemberPin> = members.iter().filter_map(|m| m.pin()).collect();
    let content_hash = canonical_proposal_hash(
        &title,
        &rationale,
        &input.origin_kind,
        &governance_level,
        expected_version_no,
        &pins,
    );
    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.proposal \
         (tenant_id, project_id, title, rationale, origin_kind, source_id, governance_level, \
          risk_level, confidence, expected_version_no, proposal_content_hash, status, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'draft', $12) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&title)
    .bind(&rationale)
    .bind(&input.origin_kind)
    .bind(input.source_id)
    .bind(&governance_level)
    .bind(&risk_level)
    .bind(&confidence)
    .bind(expected_version_no)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .fetch_one(&mut **tx)
    .await?;
    for m in &members {
        sqlx::query(
            "INSERT INTO pc.proposal_member \
             (tenant_id, project_id, proposal_id, change_kind, object_id, object_revision_id, \
              relationship_id, relationship_revision_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(proposal_id)
        .bind(&m.change_kind)
        .bind(m.object_id)
        .bind(m.object_revision_id)
        .bind(m.relationship_id)
        .bind(m.relationship_revision_id)
        .execute(&mut **tx)
        .await?;
    }
    audit(
        tx,
        tenant_id,
        project_id,
        actor,
        "pc_proposal.create",
        proposal_id,
        None,
        json!({
            "status": "draft",
            "origin_kind": input.origin_kind,
            "governance_level": governance_level,
            "expected_version_no": expected_version_no,
            "proposal_content_hash": content_hash,
            "member_count": members.len(),
            "auto_merged_endpoints": merged,
            "source_id": input.source_id,
        }),
    )
    .await?;
    Ok(ProposalReceipt {
        proposal_id,
        status: "draft".into(),
        governance_level,
        expected_version_no,
        proposal_content_hash: content_hash,
        member_count: members.len(),
    })
}

// ── submit ──────────────────────────────────────────────────────────────────

/// 提交（draft → submitted）：成员完整性断言（②b）+ 内容哈希一致 + L3 冻结 + 基线
/// 核对；同事务写 pending 审批行（锚点 = 提案内容哈希 + expected_version_no）。
/// `expected_version_no` = 调用方 CAS（None = 不核对调用方视图，仍核对现行版本）。
pub async fn submit_proposal(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    proposal_id: Uuid,
    expected_version_no: Option<i64>,
) -> Result<ProposalReceipt, PcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    let mut tx = pool.begin().await?;
    let receipt = submit_proposal_in(
        &mut tx,
        tenant_id,
        project_id,
        actor,
        proposal_id,
        expected_version_no,
    )
    .await?;
    tx.commit().await?;
    Ok(receipt)
}

async fn submit_proposal_in(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    actor: &AuditActor,
    proposal_id: Uuid,
    expected_version_no: Option<i64>,
) -> Result<ProposalReceipt, PcError> {
    let row: Option<(String, String, i64, String)> = sqlx::query_as(
        "SELECT status, governance_level, expected_version_no, proposal_content_hash \
           FROM pc.proposal WHERE tenant_id = $1 AND project_id = $2 AND id = $3 \
           FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (status, governance_level, baseline, content_hash) =
        row.ok_or_else(|| PcError::NotFound("proposal not found".into()))?;
    if status != "draft" {
        return Err(PcError::Conflict(format!(
            "proposal is `{status}`; only draft proposals can be submitted"
        )));
    }
    if let Some(expected) = expected_version_no {
        if expected != baseline {
            return Err(PcError::Conflict(format!(
                "expected_version_no {expected} does not match the proposal baseline {baseline}"
            )));
        }
    }

    // ②b 成员完整性断言（§10.1 成员时序契约：submitted 时刻必须齐备）
    let members = load_members(&mut **tx, tenant_id, project_id, proposal_id).await?;
    if members.is_empty() {
        return Err(PcError::Invalid("proposal has no members".into()));
    }
    if members.iter().any(|m| m.pin().is_none()) {
        return Err(PcError::Invalid(
            "proposal members incomplete (every member must pin identity + revision)".into(),
        ));
    }
    let recomputed =
        canonical_proposal_hash_from_rows(&mut **tx, tenant_id, project_id, proposal_id, &members)
            .await?;
    if recomputed != content_hash {
        return Err(PcError::Conflict(
            "proposal content hash mismatch (members changed after drafting)".into(),
        ));
    }

    // L3 冻结（§10.3：提交服务校验目标不在冻结集内）
    let (objects, relationships) = targets(&members);
    if let Some(conflict_id) =
        frozen_by_conflict(&mut **tx, tenant_id, project_id, &objects, &relationships).await?
    {
        return Err(PcError::Forbidden(format!(
            "proposal targets are frozen by unresolved conflict {conflict_id} (L3)"
        )));
    }
    let current_no = current_version(&mut **tx, tenant_id, project_id)
        .await?
        .map(|(_, no)| no)
        .unwrap_or(0);
    if current_no != baseline {
        return Err(PcError::Conflict(format!(
            "version baseline moved ({baseline} -> {current_no}); re-propose against the current version"
        )));
    }

    sqlx::query(
        "UPDATE pc.proposal SET status = 'submitted', submitted_at = now(), updated_at = now() \
          WHERE tenant_id = $1 AND project_id = $2 AND id = $3 AND status = 'draft'",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .execute(&mut **tx)
    .await?;
    let approval_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO pc.approval \
         (tenant_id, project_id, proposal_id, proposal_content_hash, expected_version_no) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .bind(&content_hash)
    .bind(baseline)
    .fetch_one(&mut **tx)
    .await
    {
        Ok(id) => id,
        Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
            return Err(PcError::Conflict(
                "a pending approval already exists for this proposal".into(),
            ));
        }
        Err(e) => return Err(PcError::Db(e)),
    };
    audit(
        tx,
        tenant_id,
        project_id,
        actor,
        "pc_proposal.submit",
        proposal_id,
        Some(json!({ "status": "draft" })),
        json!({
            "status": "submitted",
            "approval_id": approval_id,
            "proposal_content_hash": content_hash,
            "expected_version_no": baseline,
        }),
    )
    .await?;
    Ok(ProposalReceipt {
        proposal_id,
        status: "submitted".into(),
        governance_level,
        expected_version_no: baseline,
        proposal_content_hash: content_hash,
        member_count: members.len(),
    })
}

// ── approve / reject ────────────────────────────────────────────────────────

/// 审批通过（submitted → approved）：真人 + A3 + L3 冻结前置 + 锚点三方比对。
/// 只落审批事实；正式版本由 [`publish_version`]（或 [`decide_proposal`] 链式）产出。
pub async fn approve_proposal(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    proposal_id: Uuid,
    input: DecisionInput,
) -> Result<ApprovalReceipt, PcError> {
    record_decision(
        pool,
        project_id,
        actor,
        proposal_id,
        DecisionAction::Approve,
        input,
    )
    .await
}

/// 驳回（submitted → rejected）：真人 + A3 + 锚点比对（冻结不阻断驳回）。
pub async fn reject_proposal(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    proposal_id: Uuid,
    input: DecisionInput,
) -> Result<ApprovalReceipt, PcError> {
    record_decision(
        pool,
        project_id,
        actor,
        proposal_id,
        DecisionAction::Reject,
        input,
    )
    .await
}

async fn record_decision(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    proposal_id: Uuid,
    action: DecisionAction,
    input: DecisionInput,
) -> Result<ApprovalReceipt, PcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    // 真人 + A3（事务外只读判定，与发布④同口径；H-11 已知窗口，§23 R-3）
    ensure_formal_authorized(pool, project_id, actor).await?;

    // 审阅流 v2：决定在关联报告关闭前可反复修改（approved/rejected 互转）；报告
    // closed 即冻结。无报告关联的提案（如身份综合）不受报告门约束。
    let report_status: Option<String> = sqlx::query_scalar(
        "SELECT r.status FROM integration.repo_analysis_report r \
          JOIN pc.proposal p ON p.source_id = r.pc_source_id \
          WHERE p.tenant_id = $1 AND p.project_id = $2 AND p.id = $3 \
          ORDER BY r.created_at DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_optional(pool)
    .await?;
    if report_status.as_deref() == Some("closed") {
        return Err(PcError::Conflict(
            "analysis report is closed; proposal decisions are frozen".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let row: Option<(String, i64, String)> = sqlx::query_as(
        "SELECT status, expected_version_no, proposal_content_hash FROM pc.proposal \
          WHERE tenant_id = $1 AND project_id = $2 AND id = $3 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (status, baseline, content_hash) =
        row.ok_or_else(|| PcError::NotFound("proposal not found".into()))?;
    let new_status = action.status();
    if status == new_status {
        // 幂等：同态重放（approved→approve / rejected→reject）→ 返回既有审批事实。
        let approval_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM pc.approval \
              WHERE tenant_id = $1 AND project_id = $2 AND proposal_id = $3 AND status = $4 \
              ORDER BY created_at DESC LIMIT 1",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(proposal_id)
        .bind(new_status)
        .fetch_optional(&mut *tx)
        .await?;
        tx.rollback().await?;
        return Ok(ApprovalReceipt {
            proposal_id,
            approval_id: approval_id.ok_or_else(|| {
                PcError::Conflict("proposal has no matching approval fact".into())
            })?,
            status: new_status.to_string(),
        });
    }
    if status != "submitted" {
        // 决定终局（Q-02b/Q-11，GAP-35）：approved/rejected 为终态——同态重放已在
        // 上方幂等分支返回，跨态重决一律 409。
        return Err(PcError::Conflict(format!(
            "proposal is `{status}`; only submitted proposals can be decided (decisions are terminal)"
        )));
    }
    if input.expected_version_no != baseline {
        return Err(PcError::Conflict(format!(
            "expected_version_no {} does not match the proposal baseline {baseline}",
            input.expected_version_no
        )));
    }
    // 待决审批行 FOR UPDATE（§17.2 场景 4：并发裁决者串行，后到者读到非 pending → 409）。
    // 决定终局（Q-02b/Q-11，GAP-35）：submitted 提案的 pending 行由 submit 创建，
    // 裁决后转终态且提案终态不可再决——重决补插 pending 行路径随跨态翻转废止。
    let approval: Option<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT id, proposal_content_hash, expected_version_no FROM pc.approval \
          WHERE tenant_id = $1 AND project_id = $2 AND proposal_id = $3 AND status = 'pending' \
          FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (approval_id, approval_hash, approval_baseline) = approval
        .ok_or_else(|| PcError::Conflict("proposal has no pending approval fact".into()))?;
    // 锚点三方比对：库内成员集重算 vs 提案行 vs 审批行（不信任任何单一可变列）
    let members = load_members(&mut *tx, tenant_id, project_id, proposal_id).await?;
    let recomputed =
        canonical_proposal_hash_from_rows(&mut *tx, tenant_id, project_id, proposal_id, &members)
            .await?;
    if recomputed != content_hash || recomputed != approval_hash {
        return Err(PcError::Conflict(
            "proposal content hash mismatch (approval anchor vs recomputed members)".into(),
        ));
    }
    if approval_baseline != baseline {
        return Err(PcError::Conflict(
            "approval expected_version_no != proposal expected_version_no".into(),
        ));
    }
    // L3 冻结前置（§18.1 ⑥ 双落点之确认入口，F-06）
    if action == DecisionAction::Approve {
        let (objects, relationships) = targets(&members);
        if let Some(conflict_id) =
            frozen_by_conflict(&mut *tx, tenant_id, project_id, &objects, &relationships).await?
        {
            return Err(PcError::Forbidden(format!(
                "proposal targets are frozen by unresolved conflict {conflict_id} (L3)"
            )));
        }
    }

    let reason = input
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_string);
    let decided = sqlx::query(
        "UPDATE pc.approval \
            SET status = $2, decided_by = $3, decided_at = now(), decision_reason = $4 \
          WHERE id = $1 AND status = 'pending'",
    )
    .bind(approval_id)
    .bind(new_status)
    .bind(actor.principal_id)
    .bind(&reason)
    .execute(&mut *tx)
    .await?;
    if decided.rows_affected() != 1 {
        return Err(PcError::Conflict(
            "approval was decided concurrently".into(),
        ));
    }
    let moved = sqlx::query(
        "UPDATE pc.proposal SET status = $4, updated_at = now() \
          WHERE tenant_id = $1 AND project_id = $2 AND id = $3 \
            AND status = 'submitted'",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .bind(new_status)
    .execute(&mut *tx)
    .await?;
    if moved.rows_affected() != 1 {
        return Err(PcError::Conflict(
            "proposal status moved concurrently".into(),
        ));
    }
    audit(
        &mut tx,
        tenant_id,
        project_id,
        actor,
        action.audit_action(),
        proposal_id,
        Some(json!({ "status": status })),
        json!({
            "status": new_status,
            "approval_id": approval_id,
            "proposal_content_hash": content_hash,
            "expected_version_no": baseline,
            "reason": reason,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(ApprovalReceipt {
        proposal_id,
        approval_id,
        status: new_status.to_string(),
    })
}

// ── decide（P5 编排：approve/reject 终局决定（Q-02b/Q-11）；正式版本经 close 批量发布）───

/// P5 决策（审阅流 v2）：决定为**终局**（Q-02b/Q-11，GAP-35）——approve → approved、
/// reject → rejected 即终态，不可跨态重决（409）；同态重放幂等返回既有审批事实；
/// 正式版本由报告 close 批量发布（`analysis_report::close_report`，发布键 `pc-decision:{id}`）。
/// applied/failed/expired/withdrawn/draft 一律 409。
pub async fn decide_proposal(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    proposal_id: Uuid,
    input: ProposalDecisionInput,
) -> Result<DecisionOutcome, PcError> {
    let decision = DecisionInput {
        reason: input.reason,
        expected_version_no: input.expected_version_no,
    };
    let receipt = match input.action {
        DecisionAction::Reject => {
            reject_proposal(pool, project_id, actor, proposal_id, decision).await?
        }
        DecisionAction::Approve => {
            approve_proposal(pool, project_id, actor, proposal_id, decision).await?
        }
    };
    Ok(DecisionOutcome {
        proposal_id,
        status: receipt.status,
        approval_id: Some(receipt.approval_id),
        published_version: None,
    })
}

// ── withdraw ────────────────────────────────────────────────────────────────

/// 撤回（draft/submitted → withdrawn；V1 从简）：提案人本人，或真人 + A3。
/// pending 审批行保留原状（审批表只承载裁决事实，pending→终态必须有裁决人）——
/// 撤回后由 `proposal.status = withdrawn` 判死，任何裁决入口均 409。
pub async fn withdraw_proposal(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    proposal_id: Uuid,
    reason: Option<String>,
) -> Result<ProposalReceipt, PcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    let created_by: Uuid = sqlx::query_scalar(
        "SELECT created_by FROM pc.proposal WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| PcError::NotFound("proposal not found".into()))?;
    if created_by != actor.principal_id {
        ensure_formal_authorized(pool, project_id, actor).await?;
    }

    let mut tx = pool.begin().await?;
    let (status, governance_level, baseline, content_hash): (String, String, i64, String) =
        sqlx::query_as(
            "SELECT status, governance_level, expected_version_no, proposal_content_hash \
               FROM pc.proposal WHERE tenant_id = $1 AND project_id = $2 AND id = $3 \
               FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(proposal_id)
        .fetch_one(&mut *tx)
        .await?;
    if status != "draft" && status != "submitted" {
        return Err(PcError::Conflict(format!(
            "proposal is `{status}`; only draft or submitted proposals can be withdrawn"
        )));
    }
    sqlx::query(
        "UPDATE pc.proposal SET status = 'withdrawn', updated_at = now() \
          WHERE tenant_id = $1 AND project_id = $2 AND id = $3 \
            AND status IN ('draft','submitted')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .execute(&mut *tx)
    .await?;
    let member_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.proposal_member WHERE proposal_id = $1")
            .bind(proposal_id)
            .fetch_one(&mut *tx)
            .await?;
    let reason = reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_string);
    audit(
        &mut tx,
        tenant_id,
        project_id,
        actor,
        "pc_proposal.withdraw",
        proposal_id,
        Some(json!({ "status": status })),
        json!({ "status": "withdrawn", "reason": reason }),
    )
    .await?;
    tx.commit().await?;
    Ok(ProposalReceipt {
        proposal_id,
        status: "withdrawn".into(),
        governance_level,
        expected_version_no: baseline,
        proposal_content_hash: content_hash,
        member_count: usize::try_from(member_count).unwrap_or_default(),
    })
}

// ── P4 视图 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProposalMemberView {
    pub change_kind: String,
    pub object_id: Option<Uuid>,
    pub object_revision_id: Option<Uuid>,
    pub object_type: Option<String>,
    pub object_key: Option<String>,
    pub display_name: Option<String>,
    pub object_revision_no: Option<i32>,
    pub change_summary: Option<String>,
    pub relationship_id: Option<Uuid>,
    pub relationship_revision_id: Option<Uuid>,
    pub relationship_type: Option<String>,
    pub relationship_revision_no: Option<i32>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ApprovalView {
    pub approval_id: Uuid,
    pub status: String,
    pub proposal_content_hash: String,
    pub expected_version_no: i64,
    pub decided_by: Option<Uuid>,
    pub decided_by_name: Option<String>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PublishedVersionView {
    pub version_id: Uuid,
    pub version_no: i64,
    pub snapshot_hash: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProposalHeadView {
    pub proposal_id: Uuid,
    pub title: String,
    pub rationale: String,
    pub origin_kind: String,
    pub source_id: Option<Uuid>,
    pub governance_level: String,
    pub risk_level: String,
    pub confidence: String,
    pub expected_version_no: i64,
    pub proposal_content_hash: String,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// P4 详情：提案头 + 成员（含对象/关系展示投影）+ 审批历史 + 已发布版本。
#[derive(Debug, Clone, Serialize)]
pub struct ProposalView {
    #[serde(flatten)]
    pub head: ProposalHeadView,
    pub members: Vec<ProposalMemberView>,
    pub approvals: Vec<ApprovalView>,
    pub published_version: Option<PublishedVersionView>,
}

/// P4 读面（项目作用域；跨项目/不存在一律 NotFound）。
pub async fn load_proposal(
    pool: &PgPool,
    project_id: Uuid,
    proposal_id: Uuid,
) -> Result<ProposalView, PcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    let head: ProposalHeadView = sqlx::query_as(
        "SELECT id AS proposal_id, title, rationale, origin_kind, source_id, governance_level, \
                risk_level, confidence, expected_version_no, proposal_content_hash, status, \
                submitted_at, created_by, created_at, updated_at \
           FROM pc.proposal WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| PcError::NotFound("proposal not found".into()))?;
    let members: Vec<ProposalMemberView> = sqlx::query_as(
        "SELECT pm.change_kind, pm.object_id, pm.object_revision_id, \
                o.object_type, o.object_key, o.display_name, \
                cr.revision_no AS object_revision_no, cr.change_summary, \
                pm.relationship_id, pm.relationship_revision_id, \
                rel.relationship_type_key AS relationship_type, \
                rr.revision_no AS relationship_revision_no \
           FROM pc.proposal_member pm \
           LEFT JOIN pc.cognition_object o \
                  ON o.tenant_id = pm.tenant_id AND o.project_id = pm.project_id AND o.id = pm.object_id \
           LEFT JOIN pc.cognition_revision cr \
                  ON cr.tenant_id = pm.tenant_id AND cr.project_id = pm.project_id \
                 AND cr.id = pm.object_revision_id \
           LEFT JOIN pc.relationship rel \
                  ON rel.tenant_id = pm.tenant_id AND rel.project_id = pm.project_id \
                 AND rel.id = pm.relationship_id \
           LEFT JOIN pc.relationship_revision rr \
                  ON rr.tenant_id = pm.tenant_id AND rr.project_id = pm.project_id \
                 AND rr.id = pm.relationship_revision_id \
          WHERE pm.tenant_id = $1 AND pm.project_id = $2 AND pm.proposal_id = $3 \
          ORDER BY pm.created_at, pm.id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_all(pool)
    .await?;
    let approvals: Vec<ApprovalView> = sqlx::query_as(
        "SELECT a.id AS approval_id, a.status, a.proposal_content_hash, a.expected_version_no, \
                a.decided_by, p.display_name AS decided_by_name, a.decision_reason, \
                a.decided_at, a.created_at \
           FROM pc.approval a \
           LEFT JOIN core.principal p ON p.id = a.decided_by \
          WHERE a.tenant_id = $1 AND a.project_id = $2 AND a.proposal_id = $3 \
          ORDER BY a.created_at, a.id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_all(pool)
    .await?;
    let published_version: Option<PublishedVersionView> = sqlx::query_as(
        "SELECT id AS version_id, version_no, snapshot_hash FROM pc.cognition_version \
          WHERE tenant_id = $1 AND project_id = $2 AND source_proposal_id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(proposal_id)
    .fetch_optional(pool)
    .await?;
    Ok(ProposalView {
        head,
        members,
        approvals,
        published_version,
    })
}

// ── 分析 → 提案（AI 起草、人工确认）────────────────────────────────────────

/// 分析任务的结论事件幂等键：`pc.change_proposed`（有提案）与 `pc.analysis.completed`
/// （NO_CHANGE）共用——一个分析任务恰好一个结论事件，重投递不会追加第二种结论
/// （任务与分析提案 1:1，等价于事件目录 hint `proposal_id`）。
pub fn analysis_outcome_key(task_id: Uuid) -> String {
    format!("pc_analysis:{task_id}")
}

pub struct AnalysisProposalInput {
    /// 本次分析登记的 pc.source（证据钉扎的修订集 = 候选集）。
    pub source_id: Uuid,
    /// pc_analyze 任务（事件 task_ref；Platform 据此推进任务终态）。
    pub task_id: Uuid,
    pub title: String,
    pub rationale: String,
}

#[derive(Debug)]
pub enum AnalysisProposalOutcome {
    /// 已起草并提交（pending 人工审批），同事务发出 `pc.change_proposed`。
    Proposed(ProposalReceipt),
    /// 无未覆盖修订：调用方走 NO_CHANGE 结论。
    NoChange,
}

/// 分析产出 → 提案（AI 起草 + 提交待人工确认；AI 无确认权——approve 对服务主体恒 403）。
///
/// 候选 = 本次分析来源证据钉扎、且仍为对象当前修订的对象修订；剔除①现行版本已含
/// 的修订 ②被有效提案覆盖的修订（failed/expired 以外的任何状态——draft/submitted 待
/// 审、approved/applied 已收、rejected/withdrawn 已人工裁决同一内容）。新修订
/// （upsert `revision_created=true`）恒在候选内；此口径额外覆盖重投递/起草前崩溃的
/// 自愈与存量草稿首轮入链，且同一修订集不会重复提案。
///
/// 项目级 advisory lock 串行化「覆盖判定 → 起草」，并发分析不会重复提同一修订。
pub async fn propose_from_analysis(
    pool: &PgPool,
    project_id: Uuid,
    input: AnalysisProposalInput,
) -> Result<AnalysisProposalOutcome, PcError> {
    let tenant_id = project_tenant(pool, project_id).await?;
    let actor = AuditActor::dispatch();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtext('pc_analysis_proposal:' || $1::text)::bigint)",
    )
    .bind(project_id.to_string())
    .execute(&mut *tx)
    .await?;

    let candidates: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT o.id, e.object_revision_id, o.lifecycle_status \
           FROM pc.evidence e \
           JOIN pc.cognition_object o \
             ON o.tenant_id = e.tenant_id AND o.project_id = e.project_id \
            AND o.current_revision_id = e.object_revision_id \
          WHERE e.tenant_id = $1 AND e.project_id = $2 AND e.source_id = $3 \
            AND o.lifecycle_status IN ('identity','draft','active') \
            AND NOT EXISTS ( \
                SELECT 1 FROM pc.version_member vm \
                 WHERE vm.tenant_id = e.tenant_id AND vm.project_id = e.project_id \
                   AND vm.object_revision_id = e.object_revision_id \
                   AND vm.version_id = ( \
                       SELECT v.id FROM pc.cognition_version v \
                        WHERE v.tenant_id = e.tenant_id AND v.project_id = e.project_id \
                        ORDER BY v.version_no DESC LIMIT 1)) \
            AND NOT EXISTS ( \
                SELECT 1 FROM pc.proposal_member pm \
                  JOIN pc.proposal p ON p.tenant_id = pm.tenant_id AND p.project_id = pm.project_id \
                                    AND p.id = pm.proposal_id \
                 WHERE pm.tenant_id = e.tenant_id AND pm.project_id = e.project_id \
                   AND pm.object_revision_id = e.object_revision_id \
                   AND p.status NOT IN ('failed','expired')) \
          ORDER BY o.id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(input.source_id)
    .fetch_all(&mut *tx)
    .await?;
    if candidates.is_empty() {
        tx.commit().await?;
        return Ok(AnalysisProposalOutcome::NoChange);
    }

    // 已正式化（active）→ update_object；尚未入版本 → create_object
    let members: Vec<ProposalMemberPin> = candidates
        .into_iter()
        .map(|(object_id, revision_id, lifecycle)| ProposalMemberPin {
            change_kind: if lifecycle == "active" {
                "update_object".into()
            } else {
                "create_object".into()
            },
            object_id: Some(object_id),
            object_revision_id: Some(revision_id),
            relationship_id: None,
            relationship_revision_id: None,
        })
        .collect();
    let drafted = create_proposal_in(
        &mut tx,
        tenant_id,
        project_id,
        &actor,
        CreateProposalInput {
            title: input.title,
            rationale: input.rationale,
            origin_kind: "ai_agent".into(),
            source_id: Some(input.source_id),
            governance_level: None,
            risk_level: None,
            confidence: None,
            members,
        },
    )
    .await?;
    let receipt = submit_proposal_in(
        &mut tx,
        tenant_id,
        project_id,
        &actor,
        drafted.proposal_id,
        Some(drafted.expected_version_no),
    )
    .await?;

    let members = load_members(&mut *tx, tenant_id, project_id, receipt.proposal_id).await?;
    let proposal_ref = DomainRef::for_domain(
        TargetDomain::ProductCognition,
        "proposal",
        receipt.proposal_id.to_string(),
        None,
    );
    let source_ref = DomainRef::for_domain(
        TargetDomain::ProductCognition,
        "source",
        input.source_id.to_string(),
        None,
    );
    let task_ref = DomainRef::for_domain(
        TargetDomain::Requirement,
        "intake_task",
        input.task_id.to_string(),
        None,
    );
    PgOutboxAppend
        .append(
            &mut tx,
            OutboxEvent {
                project_id,
                event_type: kb_pc_contracts::events::PC_CHANGE_PROPOSED,
                aggregate_type: "pc_proposal",
                aggregate_id: &receipt.proposal_id.to_string(),
                payload: json!({
                    "proposal_ref": proposal_ref,
                    "governance_level": receipt.governance_level,
                    "members": members
                        .iter()
                        .map(|m| json!({
                            "change_kind": m.change_kind,
                            "object_id": m.object_id,
                            "object_revision_id": m.object_revision_id,
                        }))
                        .collect::<Vec<_>>(),
                    "content_hash": receipt.proposal_content_hash,
                    "source_ref": source_ref,
                    "task_ref": task_ref,
                }),
                idempotency_key: &analysis_outcome_key(input.task_id),
            },
        )
        .await?;
    tx.commit().await?;
    Ok(AnalysisProposalOutcome::Proposed(receipt))
}
