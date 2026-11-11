//! 知识空间 V2 真实数据库集成测试：质量门 8 项、提案流、红线确认、否决回滚、
//! 索引回退、证据引用与需求正文迁移。每个用例使用独立临时 Git 知识根，不触碰
//! 真实知识仓库。

use std::process::Command;

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::knowledge::{self, KnowledgeRoot};
use pm_application::knowledge_indexer::{DbSectionIndexer, KnowledgeIndexer, SearchPage};
use pm_application::knowledge_migrate;
use pm_application::knowledge_proposal::{self, ProposalOutcome, ProposalRequest};
use pm_application::knowledge_quality::{GateFailure, QualityGateResult};
use pm_application::knowledge_reference::{self, AddReferenceRequest};
use pm_application::knowledge_search::KnowledgeSearchService;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

fn temp_root() -> KnowledgeRoot {
    KnowledgeRoot::new(std::env::temp_dir().join(format!("ai-pm-kn-v2-{}", Uuid::new_v4())))
}

fn cleanup(root: &KnowledgeRoot) {
    let _ = std::fs::remove_dir_all(root.path());
}

fn commit_count(root: &KnowledgeRoot) -> usize {
    if !root.path().join(".git").exists() {
        return 0;
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .expect("git rev-list");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn git_head(root: &KnowledgeRoot) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn markdown(title: &str, body: &str) -> String {
    format!("---\nsource_events:\n  - EVT-1\n---\n\n# {title}\n\n## 正文\n\n{body}\n")
}

struct Boot {
    project_id: Uuid,
    human_id: Uuid,
    ai_pm_id: Uuid,
}

async fn boot(pool: &PgPool, code: &str) -> Boot {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("{code} project"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let ai_pm_id: Uuid = sqlx::query_scalar(
        "SELECT p.id FROM core.principal p \
           JOIN core.project_member m ON m.principal_id = p.id \
          WHERE m.project_id = $1 AND p.kind = 'ai_pm' LIMIT 1",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Boot {
        project_id: outcome.project_id,
        human_id: outcome.human_id,
        ai_pm_id,
    }
}

fn proposal(
    boot: &Boot,
    doc_key: &str,
    markdown: String,
    change_summary: Option<&str>,
) -> ProposalRequest {
    ProposalRequest {
        project_id: boot.project_id,
        doc_key: doc_key.to_string(),
        doc_type: "requirement".to_string(),
        markdown,
        change_summary: change_summary.map(str::to_string),
        source_event_id: Some(Uuid::new_v4()),
        analysis_run_id: None,
        source_ref: None,
        redline_kind: None,
        base_revision_id: None,
        operation_id: format!("kv2-{doc_key}-{}", Uuid::new_v4()),
        created_by: boot.human_id,
    }
}

fn failures_of(outcome: &ProposalOutcome) -> &[GateFailure] {
    match outcome {
        ProposalOutcome::Rejected { failures, .. } => failures,
        other => panic!("expected rejection, got {other:?}"),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn clean_proposal_auto_commits_and_records_gate_result(pool: PgPool) {
    let boot = boot(&pool, "KV2-1").await;
    let root = temp_root();
    let outcome = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(
            &boot,
            "REQ-1",
            markdown("需求一", "自动提交正文。"),
            Some("首版"),
        ),
    )
    .await
    .unwrap();
    let ProposalOutcome::Committed {
        proposal_id,
        document_id,
        revision_id,
        revision_no,
        git_commit_sha,
        ..
    } = outcome
    else {
        panic!("expected committed");
    };
    assert_eq!(revision_no, 1);
    assert_eq!(git_commit_sha.len(), 40);

    let (status, hash, doc_type): (String, String, String) = sqlx::query_as(
        "SELECT status, content_hash, 'requirement' FROM ai.document_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "published");
    assert!(!hash.is_empty());
    assert_eq!(doc_type, "requirement");

    let gate_json: serde_json::Value =
        sqlx::query_scalar("SELECT quality_gate_result FROM ai.document_proposal WHERE id = $1")
            .bind(proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let gate: QualityGateResult = serde_json::from_value(gate_json).unwrap();
    assert!(gate.passed, "clean proposal must pass all gates");
    assert!(!gate.duplicate);

    let index_status: String = sqlx::query_scalar(
        "SELECT status FROM ai.document_index_state WHERE document_revision_id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        index_status, "indexed",
        "publish must rebuild the index synchronously (T-060)"
    );

    let proposals = knowledge_proposal::list_proposals(&pool, boot.project_id, Some(document_id))
        .await
        .unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, "committed");
    assert_eq!(proposals[0].resulting_revision_id, Some(revision_id));

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_content_is_an_idempotent_no_op(pool: PgPool) {
    let boot = boot(&pool, "KV2-2").await;
    let root = temp_root();
    let body = markdown("需求二", "同一内容。");
    let first = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(&boot, "REQ-2", body.clone(), Some("首版")),
    )
    .await
    .unwrap();
    let ProposalOutcome::Committed { document_id, .. } = first else {
        panic!("expected committed");
    };
    let commits = commit_count(&root);

    let second = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(&boot, "REQ-2", body, Some("重复")),
    )
    .await
    .unwrap();
    assert!(
        matches!(second, ProposalOutcome::Duplicate { document_id: id, .. } if id == document_id)
    );
    assert_eq!(
        commit_count(&root),
        commits,
        "duplicate must not commit again"
    );

    let revisions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.document_revision WHERE document_id = $1")
            .bind(document_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revisions, 1);

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn missing_source_is_rejected_by_the_source_gate(pool: PgPool) {
    let boot = boot(&pool, "KV2-3").await;
    let root = temp_root();
    let mut req = proposal(&boot, "REQ-3", markdown("需求三", "无来源。"), Some("首版"));
    req.source_event_id = None;
    let outcome = knowledge_proposal::submit_proposal(&pool, &root, req)
        .await
        .unwrap();
    let failures = failures_of(&outcome);
    assert!(failures
        .iter()
        .any(|f| f.gate == "source" && f.code == "source_missing"));
    assert_eq!(
        commit_count(&root),
        0,
        "rejected proposal must not write Git"
    );

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn proposal_without_heading_is_rejected_by_the_index_gate(pool: PgPool) {
    let boot = boot(&pool, "KV2-4").await;
    let root = temp_root();
    let outcome = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(
            &boot,
            "REQ-4",
            "---\nsource_events:\n  - EVT-1\n---\n\n只有纯文本，没有标题。\n".to_string(),
            Some("首版"),
        ),
    )
    .await
    .unwrap();
    let failures = failures_of(&outcome);
    assert!(failures
        .iter()
        .any(|f| f.gate == "index" && f.code == "index_no_heading"));

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn redline_proposal_holds_until_a_human_confirms(pool: PgPool) {
    let boot = boot(&pool, "KV2-5").await;
    let root = temp_root();
    let mut req = proposal(
        &boot,
        "REQ-5",
        markdown("需求五", "对外承诺。"),
        Some("首版"),
    );
    req.redline_kind = Some("external_commitment".to_string());
    let outcome = knowledge_proposal::submit_proposal(&pool, &root, req)
        .await
        .unwrap();
    let ProposalOutcome::RedlinePending { proposal_id } = outcome else {
        panic!("expected redline_pending");
    };
    assert_eq!(
        commit_count(&root),
        0,
        "red-line proposal must not write Git"
    );
    let status: String =
        sqlx::query_scalar("SELECT status FROM ai.document_proposal WHERE id = $1")
            .bind(proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "redline_pending");

    let confirmed = knowledge_proposal::confirm_redline_proposal(
        &pool,
        &root,
        proposal_id,
        boot.project_id,
        boot.human_id,
        format!("kv2-confirm-{proposal_id}"),
    )
    .await
    .unwrap();
    let ProposalOutcome::Committed { revision_id, .. } = confirmed else {
        panic!("expected committed after confirmation");
    };
    assert!(commit_count(&root) >= 1, "confirmation must write a commit");
    let confirmed_by: Option<Uuid> =
        sqlx::query_scalar("SELECT redline_confirmed_by FROM ai.document_revision WHERE id = $1")
            .bind(revision_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(confirmed_by, Some(boot.human_id));

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_human_cannot_confirm_a_redline_proposal(pool: PgPool) {
    let boot = boot(&pool, "KV2-6").await;
    let root = temp_root();
    let mut req = proposal(
        &boot,
        "REQ-6",
        markdown("需求六", "对外承诺。"),
        Some("首版"),
    );
    req.redline_kind = Some("external_commitment".to_string());
    let ProposalOutcome::RedlinePending { proposal_id } =
        knowledge_proposal::submit_proposal(&pool, &root, req)
            .await
            .unwrap()
    else {
        panic!("expected redline_pending");
    };
    let err = knowledge_proposal::confirm_redline_proposal(
        &pool,
        &root,
        proposal_id,
        boot.project_id,
        boot.ai_pm_id,
        format!("kv2-confirm-{proposal_id}"),
    )
    .await
    .unwrap_err();
    // DEF-P0-03：非真人确认 = 授权拒绝（Forbidden），不是状态冲突（Conflict）。
    assert!(matches!(err, knowledge::KnowledgeError::Forbidden(_)));

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn veto_reverts_git_and_rolls_the_pointer_back(pool: PgPool) {
    let boot = boot(&pool, "KV2-7").await;
    let root = temp_root();
    let first = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(&boot, "REQ-7", markdown("需求七", "第一版。"), Some("v1")),
    )
    .await
    .unwrap();
    let ProposalOutcome::Committed {
        revision_id: rev1,
        document_id,
        ..
    } = first
    else {
        panic!("expected v1 committed");
    };
    let second = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(&boot, "REQ-7", markdown("需求七", "第二版。"), Some("v2")),
    )
    .await
    .unwrap();
    let ProposalOutcome::Committed {
        revision_id: rev2, ..
    } = second
    else {
        panic!("expected v2 committed");
    };
    let commits_before = commit_count(&root);

    let outcome = knowledge_proposal::veto_revision(
        &pool,
        &root,
        boot.project_id,
        rev2,
        boot.human_id,
        "内容有误",
        format!("kv2-veto-{rev2}"),
    )
    .await
    .unwrap();
    assert_eq!(outcome.rolled_back_revision_id, rev2);
    assert_eq!(outcome.restored_revision_id, Some(rev1));
    assert_eq!(
        commit_count(&root),
        commits_before + 1,
        "veto must add a revert commit"
    );
    assert_eq!(
        git_head(&root),
        outcome.new_commit_sha,
        "HEAD is the revert commit"
    );

    let status: String =
        sqlx::query_scalar("SELECT status FROM ai.document_revision WHERE id = $1")
            .bind(rev2)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "rolled_back");
    let proposal_status: String = sqlx::query_scalar(
        "SELECT status FROM ai.document_proposal WHERE resulting_revision_id = $1",
    )
    .bind(rev2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(proposal_status, "rolled_back");

    // 指针回退：最新 published 回到 v1，正文也是 v1。
    let read = knowledge::read_document(&pool, &root, boot.project_id, document_id)
        .await
        .unwrap();
    assert_eq!(read.revision_no, 1);
    assert!(read.markdown.contains("第一版"));

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn index_hash_drift_falls_back_to_the_authoritative_file(pool: PgPool) {
    let boot = boot(&pool, "KV2-8").await;
    let root = temp_root();
    let ProposalOutcome::Committed {
        revision_id,
        document_id,
        ..
    } = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(&boot, "REQ-8", markdown("需求八", "索引正文。"), Some("v1")),
    )
    .await
    .unwrap()
    else {
        panic!("expected committed");
    };

    // 发布即索引：自动提交后同步重建，状态已落定。
    let status: String = sqlx::query_scalar(
        "SELECT status FROM ai.document_index_state WHERE document_revision_id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "indexed");

    // 模拟索引停留在旧内容：索引哈希与修订哈希不一致。
    sqlx::query(
        "UPDATE ai.document_index_state SET indexed_content_hash = 'stale-hash' \
          WHERE document_revision_id = $1",
    )
    .bind(revision_id)
    .execute(&pool)
    .await
    .unwrap();

    let service = KnowledgeSearchService::new(pool.clone(), root.clone());
    let result = service
        .search(
            boot.project_id,
            SYSTEM_TENANT,
            "索引正文",
            SearchPage::normalized(None, None),
        )
        .await
        .unwrap();
    assert!(
        result.used_fallback,
        "hash drift must fall back to direct read"
    );
    assert_eq!(result.stale_revision_ids, vec![revision_id]);
    assert!(
        !result.hits.is_empty(),
        "fallback still returns the authoritative section"
    );

    // 重建修复：状态与哈希回到一致，检索重新走 DB 路径。
    let indexer = DbSectionIndexer::new(pool.clone(), root.clone());
    let outcome = indexer
        .rebuild_document(boot.project_id, revision_id)
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        pm_application::IndexOutcome::Indexed { .. }
    ));

    let repaired = service
        .search(
            boot.project_id,
            SYSTEM_TENANT,
            "索引正文",
            SearchPage::normalized(None, None),
        )
        .await
        .unwrap();
    assert!(
        !repaired.used_fallback,
        "a rebuilt index must serve the DB path"
    );
    assert!(!repaired.hits.is_empty());

    let states =
        pm_application::knowledge_indexer::index_states(&pool, boot.project_id, document_id)
            .await
            .unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].status, "indexed");

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn knowledge_reference_is_idempotent_and_listable(pool: PgPool) {
    let boot = boot(&pool, "KV2-9").await;
    let root = temp_root();
    let ProposalOutcome::Committed {
        revision_id,
        document_id,
        ..
    } = knowledge_proposal::submit_proposal(
        &pool,
        &root,
        proposal(&boot, "REQ-9", markdown("需求九", "引用正文。"), Some("v1")),
    )
    .await
    .unwrap()
    else {
        panic!("expected committed");
    };

    let make = || AddReferenceRequest {
        project_id: boot.project_id,
        document_id,
        document_revision_id: revision_id,
        section_anchor: Some("正文".to_string()),
        section_title: Some("正文".to_string()),
        start_line: Some(7),
        reference_kind: "evidence".to_string(),
        source_kind: "manual".to_string(),
        source_id: None,
        created_by: boot.human_id,
    };
    let first = knowledge_reference::add_reference(&pool, make())
        .await
        .unwrap();
    let second = knowledge_reference::add_reference(&pool, make())
        .await
        .unwrap();
    assert_eq!(first, second, "same reference must be idempotent");

    let rows = knowledge_reference::list_references(&pool, boot.project_id, document_id)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].section_anchor.as_deref(), Some("正文"));

    cleanup(&root);
}

#[sqlx::test(migrations = "../../migrations")]
async fn requirement_body_migration_is_reversible_and_leaves_pg_untouched(pool: PgPool) {
    let boot = boot(&pool, "KV2-10").await;
    let root = temp_root();

    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, tenant_id) \
         VALUES ($1, 'REQ-100', '迁移需求', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 1, $3, 'approved', 'req-hash', $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
    )
    .bind(requirement_id)
    .bind(boot.project_id)
    .bind(serde_json::json!({"goal": "map", "acceptance": ["lod"]}))
    .bind(boot.human_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let outcome = knowledge_migrate::migrate_requirement_revision(
        &pool,
        &root,
        boot.project_id,
        revision_id,
        boot.human_id,
    )
    .await
    .unwrap();
    let ProposalOutcome::Committed { document_id, .. } = outcome else {
        panic!("expected migration commit");
    };

    let path: String =
        sqlx::query_scalar("SELECT canonical_path FROM ai.project_document WHERE id = $1")
            .bind(document_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let markdown = std::fs::read_to_string(root.path().join(&path)).unwrap();
    assert!(markdown.contains("REQ-100"));
    assert!(markdown.contains("迁移需求"));
    assert!(markdown.contains("source_events:"));

    // 回滚：删除迁移生成的文档与 Git 文件，PG 需求修订保持不变。
    knowledge_migrate::rollback_requirement_migration(&pool, &root, boot.project_id, document_id)
        .await
        .unwrap();
    let docs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.project_document WHERE project_id = $1")
            .bind(boot.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(docs, 0);
    let pg_revision: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.requirement_revision WHERE id = $1")
            .bind(revision_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        pg_revision, 1,
        "rollback must not touch the PG requirement revision"
    );

    cleanup(&root);
}
