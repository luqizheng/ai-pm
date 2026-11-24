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
    ("crates/pm-application/src/pm_write/audit.rs", 2), // A1, A2
    ("crates/pm-application/src/repository/support.rs", 1), // A3
    // A4（tenant_audit_log；批 B 域5 迁仓储后落点 pm-infrastructure）
    ("crates/pm-infrastructure/src/repo_team_members.rs", 1),
    ("crates/pm-application/src/command_executor/audit.rs", 1), // A5
    ("crates/pm-application/src/delivery/mod.rs", 1),           // A6
    ("crates/pm-application/src/knowledge_proposal/support.rs", 1), // A7
    ("crates/pm-application/src/project_import/advance.rs", 1), // A8
    ("crates/pm-application/src/review/queue.rs", 1),           // A9
    ("crates/pm-application/src/run_dispatch/lease.rs", 1),     // A10
    ("crates/pm-infrastructure/src/outbox_handlers.rs", 3), // A11 + D-2=A：TaskReady 自动写 assignee / no_executor_available skip（系统面 dispatch principal，四列显式 NULL）
    // T-088 新增：设备绑定审计（devenv 域）
    ("crates/pm-application/src/devenv/binding.rs", 1),
    // Team LLM Provider / 模型目录 / Onboarding / 设备凭据四域的 tenant_audit_log
    // 写入已归并到 pm_write::insert_tenant_audit 单点（批 A2，原四条清单项撤销；
    // 转发壳只带 target_type，不再含 INSERT 文本）。
    ("crates/pm-application/src/pm_write/support.rs", 1),
    // KB-PC 服务化（0048 Intake）：信号路由留痕（系统面 dispatch principal，四列显式 NULL；
    // 批 C-3 拆目录后落点 intake/signal.rs）
    ("crates/pm-application/src/intake/signal.rs", 1),
    // KB-PC 服务化：intake_task 人工重试（REST 面，写全四身份列；批 B 收尾迁仓储）
    // 与 outbox 死信重放（A12）、设备声明忽略（T-088）一并落 repo_platform_ops.rs
    ("crates/pm-infrastructure/src/repo_platform_ops.rs", 3),
    // Checkpoint-1：KB 仓库快照分析 + PC 认知派生（worker 系统面 dispatch principal，四列显式 NULL）
    ("crates/pm-infrastructure/src/kb_pc_analysis.rs", 2),
    // KP-1 / PC-01B 发布事务（pc/publish.rs）：⑫ pc_version.publish（发布者四列）
    // + CAS 失配补偿 pc_version.publish_failed（系统面 dispatch principal，四列显式 NULL）
    ("crates/pm-application/src/pc/publish.rs", 2),
    // Client Registry（0049，跨批补登记）：设备凭据 issue/revoke/permissions 审计
    // （原在 device_credential.rs，批 A2 归并入 pm_write/support.rs 单点）
    // KP-3 KB 三层最小面（0065）：来源登记/关联、记忆提案、K4 审核（批准/驳回/退回）共用本
    // helper（REST 面真人或 Key 调用方，四身份列全写；source_kind='kb_service'）
    ("crates/pm-application/src/kb_service/mod.rs", 1),
    // KP-2 / PC-01C 确认闭环（pc/proposal.rs）：pc_proposal.create/submit/withdraw/approve/reject
    // 共用本 helper（审批=真人四列；分析起草=系统面 dispatch principal，三身份列 NULL 显式写）
    ("crates/pm-application/src/pc/proposal.rs", 1),
    // P3-11 ProjC 发布事务（projc/publish.rs）：projc_version.publish（发布者四列）
    // + CAS 失配补偿 projc_version.publish_failed（系统面 dispatch principal，四列显式 NULL）
    // ——pc/publish.rs 同款镜像，T-1/T-2/T-3 阶段一
    ("crates/pm-application/src/projc/publish.rs", 2),
    // P3-11 ProjC 确认闭环（projc/proposal.rs）：projc_proposal.create/submit/withdraw/
    // approve/reject 共用本 helper（pc/proposal.rs 同款镜像）
    ("crates/pm-application/src/projc/proposal.rs", 1),
    // P4-ORG Product 域（product.rs）：product / product_member 变更留痕
    // （governance.tenant_audit_log，发布者四列）
    ("crates/pm-application/src/product.rs", 1),
    // P1 Project Settings General：项目基本信息更新审计（四身份列显式写入）
    ("crates/pm-application/src/project_info.rs", 1),
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
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
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
        let Ok(src) = fs::read_to_string(&file) else {
            continue;
        };
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
