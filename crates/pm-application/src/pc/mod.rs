//! PC-01B 正式认知模型服务层（KP-1 数据面 + 发布事务；KP-2 确认闭环；V-6 写入纪律：
//! PC 正式表唯一业务写入面）。
//!
//! 范围：
//! * **数据面（PC-I1/I2，KP-1）**：对象草稿 upsert（内容寻址 + CAS 推号）、对象 identity
//!   占位、关系草稿（端点解析/类型合法性/跨域四元组通道校验）、来源与证据登记；
//!   Profile/Vision 单例骨架随项目创建（KP-2，PC-D-06）。
//! * **版本面（PC-I3）**：发布事务 §18.1（十四步语义；⑫ 审计 + ⑫b outbox 同事务回写）、
//!   成员集构建（前版本携带 + 提案目标替换同身份 + F-05 端点自动并入）与规范哈希；
//!   提案 create/submit/approve/reject/withdraw 命令 + P5 决策编排 + P4 读面 + 分析→
//!   提案起草（KP-2，`proposal.rs`）。
//!
//! 门禁不破（fail-closed）：
//! * 正式写入（审批 approve/reject + 发布）= 真人主体（`kind='human'`）+ A3 权限闸
//!   （Lead OR `core.approver_grant` 生效行）——AI/服务主体恒 403（Q2-C / 01A §9.3）；
//! * 提案面（create/submit）与草稿面（对象/关系/来源/证据）不判主体身份——AI 只能
//!   提案、产物恒 draft，`lifecycle_status` 永不入 active（仅发布事务推进）；
//! * L3 冻结：提案目标命中未解决冲突 → submit / approve / 发布 403（三处同口径）。
//!
//! 哈希纪律（确定性，单元测试锁定）：`content_hash` = sha256(markdown)（0039
//! 口径）；成员集/提案内容哈希 = 规范序列化（排序：kind → identity → revision）
//! 后 sha256。锁序（§17.3）：advisory → proposal → approval → members 读写 →
//! version → version_member → audit → outbox（尾项）。
//!
//! 偏差登记（详见迁移头注与验收记录）：PCGIT 后置（正文暂存
//! pc.document_revision.content，`git_commit_sha` 恒 NULL，write_operation 仅
//! 发布幂等用）；analysis_mode 仅分析产物携带（人工草稿 NULL）。
//!
//! 文件分工：`object.rs`（对象/修订草稿 + 单例骨架）、`relationship.rs`（关系与端点）、
//! `evidence.rs`（来源与证据）、`proposal.rs`（提案/审批命令）、`publish.rs`（发布事务）。

mod evidence;
mod object;
mod proposal;
mod publish;
mod relationship;

pub use evidence::{
    register_evidence, register_source, EvidenceInput, EvidenceSubject, SourceInput,
};
pub use object::{
    create_object_identity, ensure_product_singletons, upsert_object_draft, ObjectUpsertReceipt,
    UpsertObjectDraft,
};
pub use proposal::{
    analysis_outcome_key, approve_proposal, create_proposal, decide_proposal, load_proposal,
    propose_from_analysis, reject_proposal, submit_proposal, withdraw_proposal,
    AnalysisProposalInput, AnalysisProposalOutcome, ApprovalReceipt, ApprovalView,
    CreateProposalInput, DecisionAction, DecisionInput, DecisionOutcome, ProposalDecisionInput,
    ProposalHeadView, ProposalMemberView, ProposalReceipt, ProposalView, PublishedVersionView,
    ORIGIN_KINDS,
};
pub use publish::{publish_version, ProposalMemberPin, PublishReceipt, PublishVersionInput};
pub use relationship::{create_relationship_draft, PcEndpoint, RelationshipDraftInput};

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::approver::ensure_governance_authorized;

// ── 错误 ────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum PcError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
}

// ── 哈希（规范序列化；确定性由单元测试锁定）────────────────────────────────

/// 版本成员钉扎（规范哈希与成员集的统一形状）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionMemberPin {
    /// 排序键首位：object < relationship（字典序 "object" < "relationship"）。
    pub kind: MemberKind,
    pub identity_id: Uuid,
    pub revision_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemberKind {
    Object,
    Relationship,
}

impl MemberKind {
    fn as_str(self) -> &'static str {
        match self {
            MemberKind::Object => "object",
            MemberKind::Relationship => "relationship",
        }
    }
}

pub(crate) fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 成员集规范哈希（§18.1 ⑦）：成员按 (kind, identity, revision) 字典序，
/// 每行 `kind:identity:revision`，换行连接后 sha256。
pub fn canonical_members_hash(members: &[VersionMemberPin]) -> String {
    let mut sorted: Vec<&VersionMemberPin> = members.iter().collect();
    sorted.sort();
    let body = sorted
        .iter()
        .map(|m| format!("{}:{}:{}", m.kind.as_str(), m.identity_id, m.revision_id))
        .collect::<Vec<_>>()
        .join("\n");
    sha256_hex(&body)
}

/// 提案内容规范哈希（§10.1）：元数据头 + 有序成员集（审批锚定比对基础；
/// 提交后锚定列 once-written，发布事务重算比对）。
pub fn canonical_proposal_hash(
    title: &str,
    rationale: &str,
    origin_kind: &str,
    governance_level: &str,
    expected_version_no: i64,
    members: &[VersionMemberPin],
) -> String {
    let mut body = format!(
        "proposal:{title}|{rationale}|{origin_kind}|{governance_level}|{expected_version_no}\n"
    );
    let mut sorted: Vec<&VersionMemberPin> = members.iter().collect();
    sorted.sort();
    body.push_str(
        &sorted
            .iter()
            .map(|m| format!("{}:{}:{}", m.kind.as_str(), m.identity_id, m.revision_id))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    sha256_hex(&body)
}

// ── 主体门禁（正式写入）──────────────────────────────────────────────────

/// 真人主体 + A3 权限（正式写入唯一门：审批裁决 + 发布；AI/服务主体恒 403，Q2-C）。
pub(crate) async fn ensure_formal_authorized(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
) -> Result<(), PcError> {
    let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
        .bind(actor.principal_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| PcError::NotFound("principal not found".into()))?;
    if kind.as_deref() != Some("human") {
        return Err(PcError::Forbidden(
            "formal pc writes require a human principal (AI is fail-closed)".into(),
        ));
    }
    ensure_governance_authorized(pool, project_id, actor.principal_id)
        .await
        .map_err(|e| match e {
            crate::pm_write::PmWriteError::Forbidden(m) => PcError::Forbidden(m.to_string()),
            crate::pm_write::PmWriteError::NotFound(m) => PcError::NotFound(m.to_string()),
            crate::pm_write::PmWriteError::Validation(m) => PcError::Invalid(m.to_string()),
            crate::pm_write::PmWriteError::Conflict(m) => PcError::Conflict(m.to_string()),
            other => PcError::Db(sqlx::Error::Configuration(format!("{other:?}").into())),
        })
}

/// L3 冻结判定（§10.3：未解决冲突经 conflict_object 圈定的对象/关系；与发布⑥同口径）。
/// 命中返回冲突 ID；`resolved/dismissed` 解冻，`reopened` 重新冻结。
pub(crate) async fn frozen_by_conflict(
    exec: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    project_id: Uuid,
    objects: &[Uuid],
    relationships: &[Uuid],
) -> Result<Option<Uuid>, PcError> {
    Ok(sqlx::query_scalar(
        "SELECT c.id FROM pc.conflict c \
           JOIN pc.conflict_object co ON (co.tenant_id, co.project_id, co.conflict_id) \
             = (c.tenant_id, c.project_id, c.id) \
          WHERE c.tenant_id = $1 AND c.project_id = $2 \
            AND c.status IN ('detected','under_review','awaiting_human','reopened') \
            AND ((co.object_id = ANY($3) AND cardinality($3) > 0) \
              OR (co.relationship_id = ANY($4) AND cardinality($4) > 0)) \
          LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(objects)
    .bind(relationships)
    .fetch_optional(exec)
    .await?)
}

// ── 类型注册校验（PC-D-04：字典表 + 代码注册表共同约束）────────────────────

pub struct ObjectTypeSpec {
    pub type_key: String,
    pub governance_level: String,
    pub singleton: bool,
}

pub(crate) async fn load_object_type(
    exec: impl sqlx::PgExecutor<'_>,
    type_key: &str,
) -> Result<ObjectTypeSpec, PcError> {
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT governance_level, singleton FROM pc.object_type \
          WHERE type_key = $1 AND status = 'active'",
    )
    .bind(type_key)
    .fetch_optional(exec)
    .await?;
    row.map(|(governance_level, singleton)| ObjectTypeSpec {
        type_key: type_key.to_string(),
        governance_level,
        singleton,
    })
    .ok_or_else(|| PcError::Invalid(format!("unknown object type `{type_key}`")))
}

pub struct RelationshipTypeSpec {
    pub type_key: String,
    pub directionality: String,
    pub source_object_types: Vec<String>,
    pub target_object_types: Vec<String>,
    pub allow_self_reference: bool,
    pub governance_level: String,
}

/// 关系类型行投影（directionality, source 集合, target 集合, self_ref, gov level）。
type RelationshipTypeRow = (String, Vec<String>, Vec<String>, bool, String);

pub(crate) async fn load_relationship_type(
    exec: impl sqlx::PgExecutor<'_>,
    type_key: &str,
) -> Result<RelationshipTypeSpec, PcError> {
    let row: Option<RelationshipTypeRow> = sqlx::query_as(
        "SELECT directionality, source_object_types, target_object_types, \
                allow_self_reference, governance_level \
           FROM pc.relationship_type WHERE type_key = $1 AND status = 'active'",
    )
    .bind(type_key)
    .fetch_optional(exec)
    .await?;
    row.map(
        |(
            directionality,
            source_object_types,
            target_object_types,
            allow_self_reference,
            governance_level,
        )| {
            RelationshipTypeSpec {
                type_key: type_key.to_string(),
                directionality,
                source_object_types,
                target_object_types,
                allow_self_reference,
                governance_level,
            }
        },
    )
    .ok_or_else(|| PcError::Invalid(format!("unknown relationship type `{type_key}`")))
}

/// 类型集合命中：PC 内端点按 object_type 命中；跨域端点按 `domain.type` 命中
/// （§15.5 种子形态）。空集合 = 未开放（该类型不声明此侧端点）。
pub(crate) fn type_set_allows(set: &[String], candidate: &str) -> bool {
    set.iter().any(|t| t == candidate)
}

// ── 单元测试（规范哈希确定性）────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(kind: MemberKind, hi: u128, ri: u128) -> VersionMemberPin {
        VersionMemberPin {
            kind,
            identity_id: Uuid::from_u128(hi),
            revision_id: Uuid::from_u128(ri),
        }
    }

    #[test]
    fn members_hash_is_order_independent_and_content_sensitive() {
        let a = vec![
            pin(MemberKind::Object, 1, 11),
            pin(MemberKind::Relationship, 2, 22),
        ];
        let b = vec![
            pin(MemberKind::Relationship, 2, 22),
            pin(MemberKind::Object, 1, 11),
        ];
        assert_eq!(canonical_members_hash(&a), canonical_members_hash(&b));
        let c = vec![
            pin(MemberKind::Object, 1, 12),
            pin(MemberKind::Object, 1, 11),
        ];
        assert_ne!(canonical_members_hash(&a), canonical_members_hash(&c));
        assert_eq!(canonical_members_hash(&[]), canonical_members_hash(&[]));
    }

    #[test]
    fn proposal_hash_binds_metadata_and_members() {
        let members = vec![pin(MemberKind::Object, 1, 11)];
        let h1 = canonical_proposal_hash("t", "r", "manual_input", "L1", 0, &members);
        let same = canonical_proposal_hash("t", "r", "manual_input", "L1", 0, &members);
        assert_eq!(h1, same);
        let retitled = canonical_proposal_hash("t2", "r", "manual_input", "L1", 0, &members);
        assert_ne!(h1, retitled, "title is part of the anchor");
        let relevel = canonical_proposal_hash("t", "r", "manual_input", "L2", 0, &members);
        assert_ne!(h1, relevel, "governance level is part of the anchor");
        let reversioned = canonical_proposal_hash("t", "r", "manual_input", "L1", 1, &members);
        assert_ne!(h1, reversioned, "expected_version_no is part of the anchor");
        let empty: Vec<VersionMemberPin> = vec![];
        assert_ne!(
            h1,
            canonical_proposal_hash("t", "r", "manual_input", "L1", 0, &empty),
            "member set is part of the anchor"
        );
    }
}
