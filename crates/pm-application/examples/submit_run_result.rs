//! E2E 测试助手：走平台服务层提交一次交付（`pm_submit_result` 的同业务链）。
//!
//! 为什么存在：`pm_submit_result` 的 **MCP 传输通道**未对执行端下发凭据（D4，
//! 规格 16 号 R46/045b NOT_IMPLEMENTED），但**服务层** `delivery::submit_delivery`
//! 是权威业务链（校验 / Git 验证 / commit 落库 / 任务推进 / Review 派生全在同一
//! 函数）。本例子把该函数暴露给 Playwright 侧 —— 测的是平台自己的业务链，
//! 传输层缺口仍如实登记 045b，不在测试里另写第二条交付实现。
//!
//! 用法：
//! ```text
//! DATABASE_URL=... AI_PM_REPO_CACHE_ROOT=... \
//!   cargo run -q -p pm-application --example submit_run_result -- <job.json>
//! ```
//! job.json：SubmitResultRequest 的 JSON 形态（lease_token 来自 fake 执行脚本
//! 写回的 `fake-result.json` —— 真实 claim 链下发的明文）。stdout 输出
//! `{"outcome":"accepted"|"rejected", ...}`；退出码 0=已决（含 Rejected），
//! 2=服务层错误（DeliveryError）。

use std::process::ExitCode;

#[derive(serde::Deserialize)]
struct Job {
    project_id: uuid::Uuid,
    run_id: uuid::Uuid,
    lease_token: String,
    lease_generation: i32,
    #[serde(default)]
    integration_status: Option<String>,
    repositories: Vec<SubmittedRepo>,
    summary: String,
    #[serde(default)]
    knowledge_proposals: Vec<KnowledgeProposal>,
}

#[derive(serde::Deserialize)]
struct SubmittedRepo {
    repository_id: uuid::Uuid,
    base_commit: String,
    commit_sha: String,
    changed_files: i32,
    #[serde(default)]
    tests: Vec<SubmittedTest>,
}

#[derive(serde::Deserialize)]
struct SubmittedTest {
    name: String,
    passed: bool,
    #[serde(default)]
    evidence_ref: Option<String>,
}

#[derive(serde::Deserialize)]
struct KnowledgeProposal {
    document_id: uuid::Uuid,
    change_summary: String,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: submit_run_result <job.json>");
        return ExitCode::FAILURE;
    }
    let job_text = match std::fs::read_to_string(&args[1]) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("submit_run_result: cannot read {}: {e}", args[1]);
            return ExitCode::FAILURE;
        }
    };
    let job: Job = match serde_json::from_str(&job_text) {
        Ok(job) => job,
        Err(e) => {
            eprintln!("submit_run_result: bad job json: {e}");
            return ExitCode::FAILURE;
        }
    };

    let request = pm_application::delivery::SubmitResultRequest {
        project_id: job.project_id,
        run_id: job.run_id,
        lease_token: job.lease_token,
        lease_generation: job.lease_generation,
        integration_status: job.integration_status,
        repositories: job
            .repositories
            .into_iter()
            .map(|repo| pm_application::delivery::SubmittedRepository {
                repository_id: repo.repository_id,
                base_commit: repo.base_commit,
                commit_sha: repo.commit_sha,
                changed_files: repo.changed_files,
                tests: repo
                    .tests
                    .into_iter()
                    .map(|test| pm_application::delivery::SubmittedTest {
                        name: test.name,
                        passed: test.passed,
                        evidence_ref: test.evidence_ref,
                    })
                    .collect(),
            })
            .collect(),
        summary: job.summary,
        knowledge_proposals: job
            .knowledge_proposals
            .into_iter()
            .map(
                |proposal| pm_application::delivery::KnowledgeProposalInput {
                    document_id: proposal.document_id,
                    change_summary: proposal.change_summary,
                },
            )
            .collect(),
        caller: None,
    };

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let outcome = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(async move {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(2)
                .connect(&database_url)
                .await
                .expect("db pool");
            pm_application::delivery::submit_delivery(&pool, &request).await
        });

    match outcome {
        Ok(outcome) => {
            let json = serde_json::to_string(&outcome).expect("serialize outcome");
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("submit_run_result: delivery error: {err}");
            ExitCode::from(2)
        }
    }
}
