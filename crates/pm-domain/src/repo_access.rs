//! 仓库访问档位（S01 · I3 V1.1 简化裁决，2026-11-04）。
//!
//! V1.0 的六能力模型（read/modify/commit/push/create_pr/merge）与四层授权交集
//! 已被**用户简化裁决废止**（`docs/ui/smark-pm-s01-i3-…-v1.md` §0.0）：
//!
//! > 「不区分，都可以读写。因为这部分太复杂。」
//!
//! 因此本模块只保留**一个档位**：
//!
//! - [`RepoAccess::ReadWrite`]：仓库可读即可写——写操作**不再按类型分级**
//!   （不再有「允许 push 必须先有 commit」这类依赖闭包）；
//! - 「不在范围内」= **不可访问**（拒绝），而不是"降级为只读"。
//!
//! 判定「哪些仓库在范围内」不在本模块，而在
//! `pm_application::repo_access`（Task 仓库范围 ∩ 仓库状态 verified）。

/// 仓库访问档位（当前唯一档位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoAccess {
    /// 可读可写。
    ReadWrite,
}

impl RepoAccess {
    /// 全部档位（当前只有一档；保留集合形态便于将来扩展）。
    pub const ALL: [RepoAccess; 1] = [Self::ReadWrite];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadWrite => "read_write",
        }
    }

    /// 解析档位字符串——未识别值返回 `None`（调用方按拒绝处理）。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read_write" => Some(Self::ReadWrite),
            _ => None,
        }
    }

    /// 该档位是否满足所需档位（当前只有一档，故为相等判断）。
    pub fn allows(self, required: RepoAccess) -> bool {
        self == required
    }
}

/// 解析一组档位字符串——任一未识别即整体失败（避免"部分识别"式放行）。
pub fn parse_access_levels(values: &[String]) -> Result<Vec<RepoAccess>, String> {
    values
        .iter()
        .map(|v| RepoAccess::parse(v).ok_or_else(|| v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 当前唯一档位：read_write。
    #[test]
    fn single_level_round_trip() {
        assert_eq!(RepoAccess::parse("read_write"), Some(RepoAccess::ReadWrite));
        assert_eq!(RepoAccess::ReadWrite.as_str(), "read_write");
        assert_eq!(RepoAccess::ALL.len(), 1);
    }

    /// V1.0 的六能力字符串已废止——不得再被当作合法档位（防止旧数据复活）。
    #[test]
    fn v1_capability_strings_are_rejected() {
        for legacy in [
            "read",
            "modify",
            "commit",
            "push",
            "create_pr",
            "merge",
            "write",
        ] {
            assert_eq!(
                RepoAccess::parse(legacy),
                None,
                "`{legacy}` must not be accepted"
            );
        }
    }

    /// 未识别档位整体失败（不部分放行）。
    #[test]
    fn parse_levels_fails_on_any_unknown() {
        assert!(parse_access_levels(&["read_write".to_string()]).is_ok());
        assert!(parse_access_levels(&["read_write".to_string(), "push".to_string()]).is_err());
        assert!(parse_access_levels(&[]).is_ok());
    }

    /// 档位判定：唯一档位互相满足。
    #[test]
    fn allows_only_same_level() {
        assert!(RepoAccess::ReadWrite.allows(RepoAccess::ReadWrite));
    }
}
