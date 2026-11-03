//! Git 操作能力模型（S01 · I3-B，`docs/ui/smark-pm-s01-i3-…v1.md` §3.1~§3.3）。
//!
//! 六能力**相互独立**：允许 modify 不隐含 commit，允许 commit 不隐含 push。
//! 依赖链是**服务端校验规则**（§3.3），不是 UI 约束：
//!
//! ```text
//! read ← modify ← commit ← push
//! read ← create_pr        （远程仓库与分支访问能力）
//! merge：独立授权
//! ```
//!
//! 默认最小权限（§3.2）：无显式授权 = 全部拒绝；`read` 也必须显式授予。
//! 任何未配置、未识别或无法解析的权限一律视为拒绝。

/// Git 操作能力（I3 §3.1 六分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GitCapability {
    Read,
    Modify,
    Commit,
    Push,
    CreatePr,
    Merge,
}

impl GitCapability {
    pub const ALL: [GitCapability; 6] = [
        Self::Read,
        Self::Modify,
        Self::Commit,
        Self::Push,
        Self::CreatePr,
        Self::Merge,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Modify => "modify",
            Self::Commit => "commit",
            Self::Push => "push",
            Self::CreatePr => "create_pr",
            Self::Merge => "merge",
        }
    }

    /// 未识别的值一律拒绝（§3.2）——解析失败返回 None，调用方按拒绝处理。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read" => Some(Self::Read),
            "modify" => Some(Self::Modify),
            "commit" => Some(Self::Commit),
            "push" => Some(Self::Push),
            "create_pr" => Some(Self::CreatePr),
            "merge" => Some(Self::Merge),
            _ => None,
        }
    }

    /// 依赖链前提（§3.3）：授予本能力时**同一授权集内**必须已具备的能力。
    /// `merge` 独立授权；`read` 无前提。
    pub fn prerequisites(self) -> &'static [GitCapability] {
        match self {
            Self::Read => &[],
            Self::Modify => &[Self::Read],
            Self::Commit => &[Self::Read, Self::Modify],
            Self::Push => &[Self::Read, Self::Modify, Self::Commit],
            Self::CreatePr => &[Self::Read],
            Self::Merge => &[],
        }
    }
}

/// 校验一组原始能力值：未识别的值 → Err（§3.2 未识别即拒绝）。
pub fn parse_capabilities(values: &[String]) -> Result<Vec<GitCapability>, String> {
    values
        .iter()
        .map(|v| GitCapability::parse(v).ok_or_else(|| v.clone()))
        .collect()
}

/// 服务端依赖校验（§3.3）：请求授予的集合内，每个能力的前提必须**同集合内**已具备。
/// 返回缺失前提的（能力, 缺失前提）列表；为空即合法。
pub fn prerequisite_violations(
    requested: &[GitCapability],
) -> Vec<(GitCapability, GitCapability)> {
    let mut violations = Vec::new();
    for cap in requested {
        for pre in cap.prerequisites() {
            if !requested.contains(pre) {
                violations.push((*cap, *pre));
            }
        }
    }
    violations
}

/// 判定层：由原始授权集计算**有效能力**（依赖闭包）。
///
/// 规则：某能力有效 ⟺ 它在授权集中 **且** 其全部前提也在授权集中。
/// 两层授权（调用人授权 ∩ 项目/仓库策略）的交集由调用方先做集合交集，
/// 再交给本函数做闭包——四层模型的第三、四层（Task 范围 / Run 快照）在
/// I3-C / I3-D 接入后同样以本函数收口。
pub fn effective_capabilities(granted: &[GitCapability]) -> std::collections::BTreeSet<GitCapability> {
    granted
        .iter()
        .copied()
        .filter(|cap| cap.prerequisites().iter().all(|pre| granted.contains(pre)))
        .collect()
}

/// 有效能力是否允许某操作（判定出口）。
pub fn allows(granted: &[GitCapability], required: GitCapability) -> bool {
    effective_capabilities(granted).contains(&required)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(values: &[&str]) -> Vec<GitCapability> {
        values.iter().map(|v| GitCapability::parse(v).unwrap()).collect()
    }

    /// §3.3 依赖链：modify 需要 read。
    #[test]
    fn modify_requires_read() {
        let granted = caps(&["modify"]);
        assert!(!allows(&granted, GitCapability::Modify));
        let granted = caps(&["read", "modify"]);
        assert!(allows(&granted, GitCapability::Modify));
    }

    /// §3.3：commit 需要 modify（且经闭包隐含 read）。
    #[test]
    fn commit_requires_modify_and_read() {
        assert!(!allows(&caps(&["read", "commit"]), GitCapability::Commit));
        assert!(allows(&caps(&["read", "modify", "commit"]), GitCapability::Commit));
    }

    /// §3.3：push 需要 commit；无 commit 的 push 无效。
    #[test]
    fn push_requires_full_chain() {
        assert!(!allows(&caps(&["read", "modify", "push"]), GitCapability::Push));
        assert!(allows(&caps(&["read", "modify", "commit", "push"]), GitCapability::Push));
    }

    /// §3.3：merge 独立授权，不依赖其他能力。
    #[test]
    fn merge_is_independent() {
        assert!(allows(&caps(&["merge"]), GitCapability::Merge));
        assert!(!allows(&caps(&["read", "modify", "commit", "push"]), GitCapability::Merge));
    }

    /// §3.2 默认最小权限：空授权 = 全部拒绝（含 read）。
    #[test]
    fn empty_grant_denies_everything() {
        let effective = effective_capabilities(&[]);
        assert!(effective.is_empty());
        assert!(!allows(&[], GitCapability::Read));
    }

    /// §3.2 未识别值拒绝：parse 失败由调用方按拒绝处理。
    #[test]
    fn unknown_value_is_rejected() {
        assert_eq!(GitCapability::parse("write"), None);
        assert_eq!(GitCapability::parse("release"), None);
        assert_eq!(GitCapability::parse(""), None);
    }

    /// 服务端依赖校验：push 缺 modify 与 commit → 按前提序逐条报。
    #[test]
    fn prerequisite_violations_point_at_missing_prereq() {
        let requested = caps(&["read", "push"]);
        let violations = prerequisite_violations(&requested);
        assert_eq!(
            violations,
            vec![(GitCapability::Push, GitCapability::Modify), (GitCapability::Push, GitCapability::Commit)]
        );
        assert!(prerequisite_violations(&caps(&["read", "modify", "commit", "push", "create_pr", "merge"])).is_empty());
    }
}
