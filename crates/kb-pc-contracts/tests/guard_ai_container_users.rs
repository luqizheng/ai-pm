//! P1-3 守卫测试：`ai.project_document` / `ai.document_revision` 使用方集合
//! 不增长（ADR-06a 过渡期规约的机械化，V1.1 §7.1 / §8.1）。
//!
//! 两张表是**跨领域共享容器**（V1.1 §4.7：KB、Goal、Discovery、Task Contract、
//! Requirement Projection 五方使用），目标态分家消解（KB→kb.document*、
//! PC→pc.document*、Platform 保留自用）。过渡期红线：**禁止新增使用方**（PC
//! 禁止接入）。本测试把该规约变成 CI 断言：静态扫描生产代码（`apps/*/src`、
//! `crates/*/src`）与迁移面，凡出现两表名（SQL 字符串/注释中的引用即算使用
//! 证据）的文件集合必须与白名单**全等**——多了拒绝（新增接入），少了也拒绝
//! （基线过期，须人工更新白名单并回填 V1.1 §4.7）。迁移面（2026-11-09 基线
//! 合并后）：唯一合法落点 = `0001_baseline.sql`，其余迁移文件零命中。
//!
//! 白名单基线 = 2026-11-07 全仓 grep 实测（V1.1 §4.7.1 全集 + pm-api 适配层
//! `documents.rs` 直查）。测试文件（`tests/`）与契约注释不计入。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const TABLE_A: &str = "ai.project_document";
const TABLE_B: &str = "ai.document_revision";

/// 生产代码白名单（相对仓库根；按 V1.1 §4.7.1 使用方清单组织）。
const CODE_WHITELIST: &[&str] = &[
    // KB 文档域（读写主体）
    "crates/pm-application/src/knowledge.rs",
    "crates/pm-application/src/knowledge_write_coordinator.rs",
    "crates/pm-application/src/knowledge_proposal/submit.rs",
    "crates/pm-application/src/knowledge_proposal/veto.rs",
    "crates/pm-application/src/knowledge_quality.rs",
    "crates/pm-application/src/knowledge_index.rs",
    "crates/pm-application/src/knowledge_indexer.rs",
    "crates/pm-application/src/knowledge_search.rs",
    "crates/pm-application/src/knowledge_migrate.rs",
    // Project Goal（Platform）
    "crates/pm-application/src/project_goal.rs",
    // SEC-P2 引用链目标校验层（bb33ea6，2026-11-16）：ProjectGoal 域目标的
    // 存在性/作用域探测读容器（历史漏登记非新增接入，2026-11-19 补登；
    // 回填见 V1.1 §4.7.1 尾注）
    "crates/pm-application/src/reference_targets.rs",
    // Discovery 提案红线（Platform/S01）
    "crates/pm-application/src/discovery/proposal.rs",
    "crates/pm-mcp/src/tools.rs",
    // Requirement Projection / Command Center / Run Context / pm_write（Platform）
    "crates/pm-application/src/requirement_projection.rs",
    // Command Center（批 C-2 拆为目录；两表引用仅落 project.rs，
    // 原单文件条目随之收敛——V1.1 §4.7 使用方语义不变）
    "crates/pm-application/src/command_center/project.rs",
    "crates/pm-application/src/run_context.rs",
    "crates/pm-application/src/pm_write/task.rs",
    "crates/pm-application/src/command_executor/executor.rs",
    // 文档适配层（直查 ai.document*；批 B 域11 守卫查询迁仓储后落点
    // pm-infrastructure，V1.1 §4.7 使用方语义不变）
    "crates/pm-infrastructure/src/repo_guard_query.rs",
];

/// 迁移面基线（2026-11-09 基线合并）：`ai.document*` 的唯一合法迁移落点 =
/// 0001_baseline.sql（pg_dump 终态，schema 所有权随基线）；此后任何新迁移
/// （0002+）命中两表名即违规（ADR-06a 冻结不变，历史白名单随旧链退役）。
const BASELINE_MIGRATION: &str = "0001_baseline.sql";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root must exist")
}

fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// 扫生产代码（`apps/*/src` + `crates/*/src`），返回命中表名的文件集合
/// （相对仓库根、统一正斜杠）。守卫 crate 自身（kb-pc-contracts）豁免——
/// 它是本规约的机械载体，文档注释会合法提及表名。
fn scan_code_hits(root: &Path) -> BTreeSet<String> {
    let mut files = Vec::new();
    for base in ["apps", "crates"] {
        let base_dir = root.join(base);
        let Ok(entries) = std::fs::read_dir(&base_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_name() == "kb-pc-contracts" {
                continue;
            }
            let src = entry.path().join("src");
            if src.is_dir() {
                walk_rs_files(&src, &mut files);
            }
        }
    }
    files
        .into_iter()
        .filter(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            text.contains(TABLE_A) || text.contains(TABLE_B)
        })
        .map(|path| {
            path.strip_prefix(root)
                .unwrap_or(path.as_path())
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

/// 扫 migrations/ 目录，返回命中表名的迁移文件名集合。
fn scan_migration_hits(root: &Path) -> BTreeSet<String> {
    let mut hits = BTreeSet::new();
    let dir = root.join("migrations");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".sql") {
                let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
                if text.contains(TABLE_A) || text.contains(TABLE_B) {
                    hits.insert(name);
                }
            }
        }
    }
    hits
}

fn diff(actual: &BTreeSet<String>, expected: &[&str]) -> (Vec<String>, Vec<String>) {
    let expected: BTreeSet<String> = expected.iter().map(|s| s.to_string()).collect();
    let extra: Vec<String> = actual.difference(&expected).cloned().collect();
    let missing: Vec<String> = expected.difference(actual).cloned().collect();
    (extra, missing)
}

#[test]
fn ai_document_container_user_set_does_not_grow() {
    let root = repo_root();
    let hits = scan_code_hits(&root);
    let (extra, missing) = diff(&hits, CODE_WHITELIST);
    assert!(
        extra.is_empty() && missing.is_empty(),
        "ADR-06a guard tripped:\n  unexpected new users of ai.document* (FORBIDDEN, §7.1/§8.1): {extra:?}\n  stale whitelist entries (update baseline + V1.1 §4.7): {missing:?}"
    );
}

#[test]
fn ai_document_container_migration_set_is_frozen() {
    let root = repo_root();
    let extra: Vec<String> = scan_migration_hits(&root)
        .into_iter()
        .filter(|name| name != BASELINE_MIGRATION)
        .collect();
    assert!(
        extra.is_empty(),
        "migration files referencing ai.document* changed:\n  唯一合法落点 = {BASELINE_MIGRATION}（2026-11-09 基线合并）；新迁移不得触碰 ai.project_document/ai.document_revision (ADR-06a; shared container is frozen): {extra:?}"
    );
}

#[test]
fn whitelist_entries_exist_on_disk() {
    let root = repo_root();
    for rel in CODE_WHITELIST {
        assert!(
            root.join(rel).is_file(),
            "whitelist entry no longer on disk (remove it and update V1.1 §4.7): {rel}"
        );
    }
}
