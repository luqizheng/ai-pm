//! S03 设计与技术决策域的纯状态机（总体设计 V1.0 §4 / §3.3 落地）。
//!
//! 三个生命周期、各自的合法转移矩阵，与数据库 CHECK 约束一一对应：
//! - [`DesignRevisionState`]：设计版本状态（§4.1/§4.2——版本不可覆盖，
//!   批准新版本时旧批准版本置 superseded）；
//! - [`AdrStatus`]：ADR 六态生命周期（§3.3）；
//! - [`AdrRevisionState`]：ADR 内容版本两态（draft 可改，submitted 冻结）。
//!
//! 设计修改与设计审批是两件事（§2/§5）：draft 可以自由改写，进入
//! `PendingConfirmation` 后正文即冻结（库级触发器 + 本矩阵共同保障）；
//! AI 可以产生 `source=ai` 的草案，但任何向 approved/accepted 的转移
//! 都必须由人类审批行驱动——本模块不做身份判断，授权在应用层
//! （`require_human`）与 REST 适配层（`ensure_trusted_context`）。

/// 设计版本状态（§4.1 生命周期 + §4.2 不可变原则）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignRevisionState {
    /// 草稿：内容可自由改写（触发器放行 content 更新仅限本状态）。
    Draft,
    /// 待人类正式批准：正文已冻结，等待审批行裁决。
    PendingConfirmation,
    /// 已批准：曾是或正是当前实施依据。
    Approved,
    /// 评审否决（终态之一；修改后须走新修订）。
    Rejected,
    /// 被同设计的新批准版本取代（§4.2：旧版本保留、可查询、引用不变）。
    Superseded,
}

impl DesignRevisionState {
    pub const ALL: [Self; 5] = [
        Self::Draft,
        Self::PendingConfirmation,
        Self::Approved,
        Self::Rejected,
        Self::Superseded,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::PendingConfirmation => "pending_confirmation",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }

    /// §4.1/§4.2 转移矩阵。
    ///
    /// - `Draft → PendingConfirmation`：提交评审；
    /// - `Draft → Rejected`：草稿阶段直接否决（不经冻结正文）；
    /// - `PendingConfirmation → Approved`：人类批准（审批行同事务）；
    /// - `PendingConfirmation → Rejected`：人类否决；
    /// - `Approved → Superseded`：同设计新版本批准时旧版本退位（§4.2）；
    /// - 终态无出边；`Draft → Approved` 非法（必须经待批，S02D-04 同款纪律）。
    pub fn can_transition_to(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Draft, Self::PendingConfirmation)
                | (Self::Draft, Self::Rejected)
                | (Self::PendingConfirmation, Self::Approved)
                | (Self::PendingConfirmation, Self::Rejected)
                | (Self::Approved, Self::Superseded)
        )
    }

    /// 正文是否仍可编辑（仅 draft；库级触发器同口径）。
    pub fn content_editable(self) -> bool {
        self == Self::Draft
    }
}

/// ADR 六态生命周期（§3.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdrStatus {
    Proposed,
    UnderReview,
    Accepted,
    Rejected,
    Superseded,
    Deprecated,
}

impl AdrStatus {
    pub const ALL: [Self; 6] = [
        Self::Proposed,
        Self::UnderReview,
        Self::Accepted,
        Self::Rejected,
        Self::Superseded,
        Self::Deprecated,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::UnderReview => "under_review",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
            Self::Deprecated => "deprecated",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }

    /// §3.3 转移矩阵：已接受决策不直接改历史内容——决策改变 = 新 ADR +
    /// supersede 替代关系（不可变决策记录原则）。Rejected/Superseded/
    /// Deprecated 为终态，无出边。
    pub fn can_transition_to(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Proposed, Self::UnderReview)
                | (Self::Proposed, Self::Rejected)
                | (Self::Proposed, Self::Deprecated)
                | (Self::UnderReview, Self::Accepted)
                | (Self::UnderReview, Self::Rejected)
                | (Self::UnderReview, Self::Deprecated)
                | (Self::Accepted, Self::Superseded)
                | (Self::Accepted, Self::Deprecated)
        )
    }

    /// Readiness Gate 条件 3 的「已解决」判定（§6.1/§6.2）：实施阻塞项 ADR
    /// 必须 `accepted`。superseded/deprecated 意味着决策已变或失效——设计
    /// 引用的是死决策，须改链到后继 ADR 后重新过门（§6.2 拦截行为）。
    pub fn resolved_for_implementation(self) -> bool {
        self == Self::Accepted
    }
}

/// ADR 内容版本两态：draft 可改，submitted 后内容冻结（触发器同口径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdrRevisionState {
    Draft,
    Submitted,
}

impl AdrRevisionState {
    pub const ALL: [Self; 2] = [Self::Draft, Self::Submitted];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }

    pub fn can_transition_to(self, to: Self) -> bool {
        matches!((self, to), (Self::Draft, Self::Submitted))
    }

    pub fn content_editable(self) -> bool {
        self == Self::Draft
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_revision_matrix_matches_doc() {
        let d = DesignRevisionState::Draft;
        assert!(d.can_transition_to(DesignRevisionState::PendingConfirmation));
        assert!(d.can_transition_to(DesignRevisionState::Rejected));
        // Draft → Approved 非法：必须经待批（设计修改与审批是两件事）。
        assert!(!d.can_transition_to(DesignRevisionState::Approved));
        assert!(DesignRevisionState::PendingConfirmation
            .can_transition_to(DesignRevisionState::Approved));
        assert!(DesignRevisionState::PendingConfirmation
            .can_transition_to(DesignRevisionState::Rejected));
        assert!(DesignRevisionState::Approved.can_transition_to(DesignRevisionState::Superseded));
        for terminal in [
            DesignRevisionState::Approved,
            DesignRevisionState::Rejected,
            DesignRevisionState::Superseded,
        ] {
            assert!(!terminal.can_transition_to(DesignRevisionState::Draft));
            assert!(!terminal.can_transition_to(DesignRevisionState::Approved));
        }
        assert!(DesignRevisionState::Draft.content_editable());
        assert!(!DesignRevisionState::PendingConfirmation.content_editable());
    }

    #[test]
    fn adr_matrix_and_resolution() {
        assert!(AdrStatus::Proposed.can_transition_to(AdrStatus::UnderReview));
        assert!(AdrStatus::UnderReview.can_transition_to(AdrStatus::Accepted));
        assert!(AdrStatus::Accepted.can_transition_to(AdrStatus::Superseded));
        // 已接受的 ADR 不得回到 proposed（不可变决策记录）。
        assert!(!AdrStatus::Accepted.can_transition_to(AdrStatus::Proposed));
        assert!(!AdrStatus::Rejected.can_transition_to(AdrStatus::Accepted));
        assert!(AdrStatus::Accepted.resolved_for_implementation());
        assert!(!AdrStatus::UnderReview.resolved_for_implementation());
        // 阻塞项 ADR 已被取代/否决 = 未解决（须改链后继决策，§6.2）。
        assert!(!AdrStatus::Superseded.resolved_for_implementation());
        assert!(!AdrStatus::Rejected.resolved_for_implementation());
    }

    #[test]
    fn wire_roundtrip() {
        for s in DesignRevisionState::ALL {
            assert_eq!(DesignRevisionState::parse(s.as_str()), Some(s));
        }
        for s in AdrStatus::ALL {
            assert_eq!(AdrStatus::parse(s.as_str()), Some(s));
        }
        for s in AdrRevisionState::ALL {
            assert_eq!(AdrRevisionState::parse(s.as_str()), Some(s));
        }
    }
}
