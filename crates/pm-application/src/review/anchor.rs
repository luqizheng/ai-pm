//! 审查锚定：双哈希 + 策略版本（`docs/slice-d/ai-pm-slice-d5-review-v1.md` §3.2）。
//!
//! 锚定回答一个问题：**这轮审查看的到底是哪一份交付、哪一版验收条目**。
//! 三个值全部内容寻址——同一份输入必得同一结果，输入变了结果必变：
//!
//! - `reviewed_manifest_hash`：锚定交付快照（被审 delivery run + 逐仓 commit 事实）。
//! - `input_manifest_hash`：在交付锚定之上再锚定验收条目快照。验收条目被修订
//!   （T-052 场景）后哈希漂移，verdict 落库前的防御性校验（§3.3）据此拒收。
//! - `review_policy_version`：检查项清单声明的摘要前 12 位——清单变更即版本变更，
//!   历史 run 行的版本不可变、可追溯（§2.5 触发器保证 once-written）。

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// 策略清单的 schema 标识：检查项清单的**语义**变更必须升版本
/// （同 hash 不同含义是最难查的一类缺陷）。
const REVIEW_POLICY_SCHEMA: &str = "review-checks-v1";

/// 被审交付里的一仓（交付锚定载荷的元素）。
#[derive(Debug, Clone, Serialize)]
pub struct RepositoryAnchor {
    pub repository_id: Uuid,
    /// 领取快照的基线（`task_run_repository.base_commit`）。
    pub base_commit: String,
    /// 实际落库的交付 commit（`task_repository_commit.commit_sha`）。
    pub commit_sha: String,
}

/// 验收条目快照（输入锚定载荷的元素）。
#[derive(Debug, Clone, Serialize)]
pub struct CriterionAnchor {
    pub ordinal: i32,
    pub text: String,
    pub verification_kind: String,
}

#[derive(Serialize)]
struct ReviewedManifestPayload<'a> {
    task_run_id: Uuid,
    repositories: Vec<&'a RepositoryAnchor>,
}

#[derive(Serialize)]
struct InputManifestPayload<'a> {
    reviewed_manifest_hash: String,
    criteria: Vec<&'a CriterionAnchor>,
}

/// §4 检查项声明。**必须与 [`crate::review::pipeline`] 的实现逐条对应**——
/// 这里是「以什么口径审的」的声明，声明变了版本就变。
#[derive(Serialize)]
struct CheckDeclaration {
    id: &'static str,
    pass_condition: &'static str,
    failure_output: &'static str,
}

#[derive(Serialize)]
struct ReviewPolicyPayload {
    schema: &'static str,
    checks: Vec<CheckDeclaration>,
}

fn review_checks() -> Vec<CheckDeclaration> {
    vec![
        CheckDeclaration {
            id: "L1",
            pass_condition: "delivery_state = 'delivered'",
            failure_output: "incomplete_delivery (blocker)",
        },
        CheckDeclaration {
            id: "L2",
            pass_condition: "integration_status = 'passed'",
            failure_output: "integration_failed (blocker)",
        },
        CheckDeclaration {
            id: "L3",
            pass_condition: "每条确定性验收项的证据齐备且通过",
            failure_output: "test_failed / artifact_missing (blocker)",
        },
        CheckDeclaration {
            id: "L4",
            pass_condition: "integration_status 声明与 tests[] 证据不矛盾",
            failure_output: "evidence_conflict (blocker)",
        },
    ]
}

/// 交付锚定哈希：被审 run + 逐仓 `(repository_id, base_commit, commit_sha)`。
///
/// 仓按 `repository_id` 排序后入载荷——锚定必须与入库顺序无关。
pub fn reviewed_manifest_hash(task_run_id: Uuid, repositories: &[RepositoryAnchor]) -> String {
    let mut sorted: Vec<&RepositoryAnchor> = repositories.iter().collect();
    sorted.sort_by_key(|repo| repo.repository_id);
    let payload = ReviewedManifestPayload {
        task_run_id,
        repositories: sorted,
    };
    digest_json(&payload)
}

/// 输入锚定哈希：交付锚定 + 验收条目快照 `(ordinal, text, verification_kind)`。
///
/// 条目按 `ordinal` 排序后入载荷——条目顺序调整不改变锚定，内容改写必变。
pub fn input_manifest_hash(reviewed_manifest_hash: &str, criteria: &[CriterionAnchor]) -> String {
    let mut sorted: Vec<&CriterionAnchor> = criteria.iter().collect();
    sorted.sort_by_key(|criterion| criterion.ordinal);
    let payload = InputManifestPayload {
        reviewed_manifest_hash: reviewed_manifest_hash.to_string(),
        criteria: sorted,
    };
    digest_json(&payload)
}

/// 审查策略版本：检查项清单声明摘要的前 12 位（对齐 0016 `policy_version` 惯例）。
pub fn review_policy_version() -> String {
    let payload = ReviewPolicyPayload {
        schema: REVIEW_POLICY_SCHEMA,
        checks: review_checks(),
    };
    digest_json(&payload)[..12].to_string()
}

fn digest_json<T: Serialize>(value: &T) -> String {
    let bytes =
        serde_json::to_vec(value).expect("anchor payload only contains serializable primitives");
    hex(&Sha256::digest(&bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(id: u128, base: &str, commit: &str) -> RepositoryAnchor {
        RepositoryAnchor {
            repository_id: Uuid::from_u128(id),
            base_commit: base.to_string(),
            commit_sha: commit.to_string(),
        }
    }

    fn criterion(ordinal: i32, text: &str, kind: &str) -> CriterionAnchor {
        CriterionAnchor {
            ordinal,
            text: text.to_string(),
            verification_kind: kind.to_string(),
        }
    }

    #[test]
    fn reviewed_hash_ignores_input_order() {
        let run_id = Uuid::from_u128(9);
        let a = repo(1, "base-a", "commit-a");
        let b = repo(2, "base-b", "commit-b");
        let forward = reviewed_manifest_hash(run_id, &[a.clone(), b.clone()]);
        let backward = reviewed_manifest_hash(run_id, &[b, a]);
        assert_eq!(forward, backward);
    }

    #[test]
    fn reviewed_hash_moves_with_any_anchored_field() {
        let run_id = Uuid::from_u128(9);
        let base = reviewed_manifest_hash(run_id, &[repo(1, "base-a", "commit-a")]);
        let other_run =
            reviewed_manifest_hash(Uuid::from_u128(10), &[repo(1, "base-a", "commit-a")]);
        let other_commit = reviewed_manifest_hash(run_id, &[repo(1, "base-a", "commit-a2")]);
        let other_base = reviewed_manifest_hash(run_id, &[repo(1, "base-a2", "commit-a")]);
        assert_ne!(base, other_run);
        assert_ne!(base, other_commit);
        assert_ne!(base, other_base);
    }

    #[test]
    fn input_hash_ignores_criterion_order_and_tracks_content() {
        let manifest = reviewed_manifest_hash(Uuid::from_u128(9), &[repo(1, "base-a", "commit-a")]);
        let first = criterion(10, "编译通过", "automated_test");
        let second = criterion(20, "文档更新", "manual");
        let forward = input_manifest_hash(&manifest, &[first.clone(), second.clone()]);
        let backward = input_manifest_hash(&manifest, &[second, first.clone()]);
        assert_eq!(forward, backward);

        let edited = criterion(10, "编译通过且无回归", "automated_test");
        assert_ne!(
            forward,
            input_manifest_hash(&manifest, &[edited, criterion(20, "文档更新", "manual")])
        );
    }

    #[test]
    fn policy_version_is_twelve_hex_chars_and_stable() {
        let version = review_policy_version();
        assert_eq!(version.len(), 12);
        assert!(version.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(version, review_policy_version());
    }
}
