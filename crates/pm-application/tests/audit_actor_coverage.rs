//! T-088 审计写入点覆盖率（C174 / C180）——**清单驱动**，源码级断言。
//!
//! 设计稿 §6.1 盘点出的全部生产 `INSERT INTO governance.audit_log` /
//! `governance.tenant_audit_log` 写入点，每处都必须写齐统一身份字段：
//! `actor_identity_id` / `actor_client` / `actor_auth_mode` / `claimed_device_id`。
//!
//! 两道闸：
//! 1. **清单内每处**：源文件中该 INSERT 语句文本含四个列名；
//! 2. **清单外零遗漏**：扫描全部生产源文件，任何不在清单里的审计 INSERT 都让测试失败
//!    ——新增写入点必须同时进清单并写齐字段，不能悄悄回到旧口径。
//!
//! 「数量与清单一致」是断言本身，**不把 8 / 9 / 12 钉死成常量**。测试夹具
//! （`tests/` 目录）与非审计的 governance 业务表（`approval_request` /
//! `review_approval` / `human_intervention`）不在范围内。

use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_COLUMNS: [&str; 4] = [
    "actor_identity_id",
    "actor_client",
    "actor_auth_mode",
    "claimed_device_id",
];

/// 生产审计写入点清单（相对工作区根；`(路径, 期望该文件内审计 INSERT 条数)`）。
/// 与设计稿 §6.1 A1~A12 一一对应（A2 与 A1 同文件，故 pm_write/audit.rs 计 2）。
const PRODUCTION_SITES: &[(&str, usize)] = &[
    ("crates/pm-application/src/pm_write/audit.rs", 2),            // A1, A2
    ("crates/pm-application/src/repository/support.rs", 1),        // A3
    ("apps/pm-api/src/tenant_members.rs", 1),                      // A4 (tenant_audit_log)
    ("crates/pm-application/src/command_executor/audit.rs", 1),    // A5
    ("crates/pm-application/src/delivery/mod.rs", 1),              // A6
    ("crates/pm-application/src/knowledge_proposal/support.rs", 1),// A7
    ("crates/pm-application/src/project_import/advance.rs", 1),    // A8
    ("crates/pm-application/src/review/queue.rs", 1),              // A9
    ("crates/pm-application/src/run_dispatch/lease.rs", 1),        // A10
    ("crates/pm-infrastructure/src/outbox_handlers.rs", 1),        // A11
    ("apps/pm-api/src/outbox_ops.rs", 1),                          // A12
    ("apps/pm-api/src/device_claim.rs", 1),                        // T-088：设备声明被忽略（tenant_audit_log）
    // T-088 新增：设备绑定审计（devenv 域）
    ("crates/pm-application/src/devenv/binding.rs", 1),
    // Team LLM Provider 管理面（0041）：created/updated/rotated/tested/enabled/disabled/archived/deleted 共用本 helper
    ("crates/pm-application/src/team_llm.rs", 1),
    // Team Onboarding（0044）：team.created/updated + 邀请/链接/激活全链共用本 helper
    ("crates/pm-application/src/team_onboarding.rs", 1),
    // KB-PC 服务化（0048 Intake）：信号路由留痕（系统面 dispatch principal，四列显式 NULL）
    ("crates/pm-application/src/intake.rs", 1),
    // KB-PC 服务化：intake_task 人工重试（REST 面，写全四身份列）
    ("apps/pm-api/src/intakes.rs", 1),
    // Checkpoint-1：KB 仓库快照分析 + PC 认知派生（worker 系统面 dispatch principal，四列显式 NULL）
    ("crates/pm-infrastructure/src/kb_pc_analysis.rs", 2),
];

fn workspace_root() -> PathBuf {
    // crates/pm-application → 上两级是工作区根
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

/// 把 Rust 字面量里的续行 `\` + 换行 + 缩进折叠掉，得到可按子串匹配的 SQL 文本。
fn normalize(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && matches!(chars.peek(), Some('\n') | Some('\r')) {
            // 吞掉换行与随后的空白
            while let Some(&n) = chars.peek() {
                if n == '\n' || n == '\r' || n == ' ' || n == '\t' {
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(' ');
            continue;
        }
        out.push(c);
    }
    out
}

/// 统计一段源码里审计 INSERT 的出现次数，并返回每段 INSERT 语句文本（到下一个 `"` 为止）。
fn audit_inserts(normalized: &str) -> Vec<String> {
    let mut found = Vec::new();
    for needle in [
        "INSERT INTO governance.audit_log",
        "INSERT INTO governance.tenant_audit_log",
    ] {
        let mut start = 0;
        while let Some(pos) = normalized[start..].find(needle) {
            let abs = start + pos;
            // 取到字面量结束的引号
            let end = normalized[abs..]
                .find('"')
                .map(|e| abs + e)
                .unwrap_or(normalized.len());
            found.push(normalized[abs..end].to_string());
            start = end.max(abs + needle.len());
        }
    }
    found
}

fn is_production_source(path: &Path) -> bool {
    let s = path.to_string_lossy().replace('\\', "/");
    s.ends_with(".rs")
        && s.contains("/src/")
        && !s.contains("/tests/")
        && !s.contains("/target/")
        && !s.contains("/examples/")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if name == "target" || name == "node_modules" || name == ".git" {
                continue;
            }
            walk(&p, out);
        } else if is_production_source(&p) {
            out.push(p);
        }
    }
}

#[test]
fn c174_every_listed_site_writes_all_four_identity_columns() {
    let root = workspace_root();
    let mut problems = Vec::new();
    for (rel, expected) in PRODUCTION_SITES {
        let path = root.join(rel);
        let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        let norm = normalize(&src);
        let inserts = audit_inserts(&norm);
        if inserts.len() != *expected {
            problems.push(format!(
                "{rel}: expected {expected} audit INSERT(s), found {}",
                inserts.len()
            ));
        }
        for (i, stmt) in inserts.iter().enumerate() {
            for col in REQUIRED_COLUMNS {
                if !stmt.contains(col) {
                    problems.push(format!("{rel} INSERT #{}: missing column `{col}`", i + 1));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "audit identity coverage failures:\n  {}",
        problems.join("\n  ")
    );
}

#[test]
fn c180_no_production_audit_insert_outside_the_inventory() {
    let root = workspace_root();
    let mut files = Vec::new();
    for sub in ["crates", "apps"] {
        walk(&root.join(sub), &mut files);
    }
    let listed: std::collections::HashSet<PathBuf> = PRODUCTION_SITES
        .iter()
        .map(|(rel, _)| root.join(rel).canonicalize().unwrap())
        .collect();

    let mut unlisted = Vec::new();
    let mut total_inserts = 0usize;
    for file in files {
        let Ok(src) = fs::read_to_string(&file) else { continue };
        let norm = normalize(&src);
        let n = audit_inserts(&norm).len();
        if n == 0 {
            continue;
        }
        total_inserts += n;
        let canon = file.canonicalize().unwrap();
        if !listed.contains(&canon) {
            unlisted.push(format!(
                "{} ({n} INSERT)",
                canon.strip_prefix(&root).unwrap_or(&canon).display()
            ));
        }
    }
    let expected_total: usize = PRODUCTION_SITES.iter().map(|(_, n)| n).sum();
    assert!(
        unlisted.is_empty(),
        "audit INSERT sites not in the inventory (add to PRODUCTION_SITES and write all four columns):\n  {}",
        unlisted.join("\n  ")
    );
    assert_eq!(
        total_inserts, expected_total,
        "production audit INSERT count drifted from the inventory"
    );
}
