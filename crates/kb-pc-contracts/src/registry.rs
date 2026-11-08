//! domain 注册表（PC-01B §13.1 逐域通道表的操作化 + §7.3 规则 2）。
//!
//! 「裸引用（脱离信封的审计存档）按 domain 注册表解析归属，解析失败→审计
//! 告警（不臆测）」。本模块即该注册表：wire 字符串 ↔ 枚举、归属服务、通道
//! 开放状态。归属映射由注册表承载（Phase 7 后映射可变，引用不随之改写）。

use thiserror::Error;

/// 领域归属服务（§5.4.4 领域操作矩阵 / §7.1 所有权矩阵）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwningService {
    /// PM-AI Platform（业务接收、流程路由、任务调度、状态聚合）。
    Platform,
    /// KB-Service（知识资料与记忆）。
    Kb,
    /// PC-Service（正式产品认知）。
    Pc,
}

/// domain 通道开放状态（PC-01B §13.1「设计状态」列）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainChannelState {
    /// V1 启用：可被四元组引用（服务层校验照做）。
    Open,
    /// V1 只读：仅只读引用、不钉 Revision。
    ReadOnly,
    /// 通道未开放：服务层按通道状态表拒绝该 domain 值。
    NotYetOpen,
    /// 永久排除出 PC 权威模型（〔裁决〕Q-PC-13）。
    PermanentlyExcluded,
}

/// 已登记的 target_domain 全集（PC-01B §13.1 逐域通道表 + 架构稿 §7.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetDomain {
    /// `requirement`：交付需求（delivery.requirement*）。
    Requirement,
    /// `project_goal`：项目 Goal（ai.project_document doc_key='project-goal'，
    /// 过渡期共享容器；§7.1 G-1=a' 终态共址归 Platform）。
    ProjectGoal,
    /// `knowledge_base`：KB 文档/记忆（目标态 kb.document*；过渡期 ai.document*）。
    KnowledgeBase,
    /// `project_cognition`：S01-04 技术项目认知（ai.cognition_*，V1 只读）。
    ProjectCognition,
    /// `pc`：PC-Service 自身领域对象（proposal/conflict/version/…）。
    ProductCognition,
    /// `design`：Design/Architecture 域（通道未开放；不以 document_revision 冒充）。
    Design,
    /// `adr`：ADR 域（通道未开放）。
    Adr,
    /// `product_reality`：通道未开放（01C 后评估）。
    ProductReality,
    /// `product_plan`：〔裁决〕Q-PC-13 永久排除出 PC 权威模型。
    ProductPlan,
}

impl TargetDomain {
    /// wire 字符串（事件 payload / 引用 JSON 中的规范形态）。
    pub fn as_str(self) -> &'static str {
        match self {
            TargetDomain::Requirement => "requirement",
            TargetDomain::ProjectGoal => "project_goal",
            TargetDomain::KnowledgeBase => "knowledge_base",
            TargetDomain::ProjectCognition => "project_cognition",
            TargetDomain::ProductCognition => "pc",
            TargetDomain::Design => "design",
            TargetDomain::Adr => "adr",
            TargetDomain::ProductReality => "product_reality",
            TargetDomain::ProductPlan => "product_plan",
        }
    }

    /// 归属服务（§7.1 所有权矩阵；Phase 7 后映射可变，引用不随之改写）。
    pub fn owning_service(self) -> OwningService {
        match self {
            TargetDomain::Requirement
            | TargetDomain::ProjectGoal
            | TargetDomain::ProjectCognition => OwningService::Platform,
            TargetDomain::KnowledgeBase => OwningService::Kb,
            TargetDomain::ProductCognition => OwningService::Pc,
            // 通道未开放域暂挂 Platform（无实际写入方；开放时随评审改判）。
            TargetDomain::Design | TargetDomain::Adr | TargetDomain::ProductReality => {
                OwningService::Platform
            }
            TargetDomain::ProductPlan => OwningService::Platform,
        }
    }

    /// 通道开放状态（PC-01B §13.1「设计状态」列）。
    pub fn channel_state(self) -> DomainChannelState {
        match self {
            TargetDomain::Requirement | TargetDomain::ProjectGoal | TargetDomain::KnowledgeBase => {
                DomainChannelState::Open
            }
            TargetDomain::ProjectCognition => DomainChannelState::ReadOnly,
            TargetDomain::ProductCognition => DomainChannelState::Open,
            TargetDomain::Design | TargetDomain::Adr | TargetDomain::ProductReality => {
                DomainChannelState::NotYetOpen
            }
            TargetDomain::ProductPlan => DomainChannelState::PermanentlyExcluded,
        }
    }

    /// 已登记 domain 的 wire 字符串全集（供守卫测试与诊断）。
    pub fn all() -> &'static [TargetDomain] {
        &[
            TargetDomain::Requirement,
            TargetDomain::ProjectGoal,
            TargetDomain::KnowledgeBase,
            TargetDomain::ProjectCognition,
            TargetDomain::ProductCognition,
            TargetDomain::Design,
            TargetDomain::Adr,
            TargetDomain::ProductReality,
            TargetDomain::ProductPlan,
        ]
    }
}

/// 注册表解析错误（告警错误类型：调用方必须留痕/告警，不得臆测归属）。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    #[error("unknown target_domain `{0}` (not in registry; alert, do not guess)")]
    UnknownDomain(String),
    #[error("invalid DomainRef: {0}")]
    InvalidRef(String),
}

/// wire 字符串 → 枚举（解析失败 → [`RegistryError::UnknownDomain`]）。
pub fn parse_domain(wire: &str) -> Result<TargetDomain, RegistryError> {
    TargetDomain::all()
        .iter()
        .copied()
        .find(|d| d.as_str() == wire)
        .ok_or_else(|| RegistryError::UnknownDomain(wire.to_string()))
}

/// 四元组引用合法性裁决（§7.3 规则 2 + PC-01B §13.1 通道状态）：
/// 已登记且通道开放/只读 → `Ok(归属服务)`；未开放/排除 → `Err`（服务层
/// 按通道状态表拒绝该 domain 值）。
pub fn resolve_reference(domain: TargetDomain) -> Result<OwningService, RegistryError> {
    match domain.channel_state() {
        DomainChannelState::Open | DomainChannelState::ReadOnly => Ok(domain.owning_service()),
        DomainChannelState::NotYetOpen => Err(RegistryError::UnknownDomain(format!(
            "domain `{}` channel not open",
            domain.as_str()
        ))),
        DomainChannelState::PermanentlyExcluded => Err(RegistryError::UnknownDomain(format!(
            "domain `{}` permanently excluded (Q-PC-13)",
            domain.as_str()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        for d in TargetDomain::all() {
            assert_eq!(parse_domain(d.as_str()).unwrap(), *d);
        }
    }

    #[test]
    fn unknown_wire_is_alert_error() {
        let e = parse_domain("kb_typo").unwrap_err();
        assert!(matches!(e, RegistryError::UnknownDomain(_)));
    }

    #[test]
    fn channel_states_match_pc01b() {
        assert_eq!(
            resolve_reference(TargetDomain::KnowledgeBase),
            Ok(OwningService::Kb)
        );
        assert_eq!(
            resolve_reference(TargetDomain::Requirement),
            Ok(OwningService::Platform)
        );
        assert_eq!(
            resolve_reference(TargetDomain::ProductCognition),
            Ok(OwningService::Pc)
        );
        // design/adr 通道未开放；product_plan 永久排除。
        assert!(resolve_reference(TargetDomain::Design).is_err());
        assert!(resolve_reference(TargetDomain::Adr).is_err());
        assert!(resolve_reference(TargetDomain::ProductPlan).is_err());
    }
}
