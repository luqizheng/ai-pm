use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyProject {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyRequest {
    pub draft_revision: i64,
    pub project: ApplyProject,
    #[serde(default)]
    pub milestone_keys: Vec<String>,
    #[serde(default)]
    pub requirement_keys: Vec<String>,
    #[serde(default)]
    pub task_keys: Vec<String>,
    #[serde(default)]
    pub write_knowledge_documents: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProposalRef {
    pub kind: String,
    pub proposal_id: Uuid,
    pub approval_id: Uuid,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProposedImport {
    pub outcome: String,
    pub proposals: Vec<ProposalRef>,
    pub analysis_run_id: Uuid,
}

pub(crate) fn lowest_confidence(draft: &ProjectDraft) -> &'static str {
    use pm_domain::proposal::Confidence;
    let mut lowest = Confidence::High;
    for confidence in draft
        .requirements
        .iter()
        .map(|item| item.confidence)
        .chain(draft.tasks.iter().map(|item| item.confidence))
    {
        lowest = match (lowest, confidence) {
            (Confidence::Low, _) | (_, Confidence::Low) => Confidence::Low,
            (Confidence::Medium, _) | (_, Confidence::Medium) => Confidence::Medium,
            _ => Confidence::High,
        };
    }
    match lowest {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

pub(crate) fn evidence_labels(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut labels: Vec<String> = items.collect();
    labels.sort();
    labels.dedup();
    labels.truncate(50);
    labels
}

pub(crate) fn profile_markdown(draft: &ProjectDraft) -> String {
    let mut out = format!(
        "# {}（{}）\n\n{}\n\n",
        draft.profile.suggested_name, draft.profile.suggested_code, draft.profile.summary
    );
    if !draft.profile.tech_stack.is_empty() {
        out.push_str(&format!(
            "## 技术栈\n\n{}\n\n",
            draft.profile.tech_stack.join(", ")
        ));
    }
    if !draft.risks.is_empty() {
        out.push_str("## 风险\n\n");
        for risk in &draft.risks {
            out.push_str(&format!("- [{}] {}\n", risk.severity, risk.description));
        }
        out.push('\n');
    }
    if !draft.unknowns.is_empty() {
        out.push_str("## 待确认问题\n\n");
        for unknown in &draft.unknowns {
            out.push_str(&format!(
                "- {}（阻断原因：{}）\n",
                unknown.question, unknown.why_blocking
            ));
        }
        out.push('\n');
    }
    out.push_str("> 本文由仓库冷启动导入生成，需人工确认。\n");
    out
}

/// 把草案按类型转成 ≤4 条提案。只写 `ai.analysis_run` / `ai.change_proposal` /
/// `governance.approval_request`，`delivery.*` 零直接写入。
pub async fn propose_import(
    pool: &PgPool,
    project_id: Uuid,
    import_id: Uuid,
    req: ApplyRequest,
) -> Result<ProposedImport, ImportError> {
    let existing: Option<(String, Option<Uuid>, Option<serde_json::Value>, i64)> = sqlx::query_as(
        "SELECT i.status, i.analysis_run_id, dv.draft, i.draft_revision \
         FROM integration.repo_import i \
         LEFT JOIN integration.repo_import_draft_version dv \
                ON dv.tenant_id = i.tenant_id AND dv.import_id = i.id \
               AND dv.revision = i.draft_revision \
         WHERE i.id = $1 AND i.project_id = $2",
    )
    .bind(import_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let (status, analysis_run_id, draft_value, draft_revision) =
        existing.ok_or(ImportError::NotFound)?;

    if status == "applied" {
        return Err(ImportError::AlreadyApplied);
    }
    if status == "proposed" {
        let run_id = analysis_run_id.ok_or(ImportError::InvalidState("proposed without run"))?;
        return existing_proposals(pool, project_id, run_id).await;
    }
    if status != "draft_ready" {
        return Err(ImportError::InvalidState("import is not ready to apply"));
    }
    if draft_revision != req.draft_revision {
        return Err(ImportError::StaleDraft);
    }
    let draft: ProjectDraft = draft_value
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or(ImportError::InvalidState("draft is missing"))?;

    let selected_milestones: Vec<_> = draft
        .milestones
        .iter()
        .filter(|item| req.milestone_keys.contains(&item.key))
        .cloned()
        .collect();
    let selected_requirements: Vec<_> = draft
        .requirements
        .iter()
        .filter(|item| req.requirement_keys.contains(&item.key))
        .cloned()
        .collect();
    let selected_tasks: Vec<_> = draft
        .tasks
        .iter()
        .filter(|item| req.task_keys.contains(&item.key))
        .cloned()
        .collect();
    if selected_milestones.is_empty()
        && selected_requirements.is_empty()
        && selected_tasks.is_empty()
        && !req.write_knowledge_documents
    {
        return Err(ImportError::NoSelection);
    }

    let code_owner: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE code = $1 AND id <> $2")
            .bind(&req.project.code)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if code_owner.is_some() {
        return Err(ImportError::ProjectCodeTaken);
    }

    let event_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM integration.inbound_event \
         WHERE project_id = $1 AND idempotency_key = $2",
    )
    .bind(project_id)
    .bind(import_id.to_string())
    .fetch_one(pool)
    .await?;

    let milestone_names: std::collections::HashMap<String, String> = draft
        .milestones
        .iter()
        .map(|item| (item.key.clone(), item.name.clone()))
        .collect();
    let requirement_codes: std::collections::HashMap<String, String> = draft
        .requirements
        .iter()
        .map(|item| (item.key.clone(), item.code.clone()))
        .collect();

    let mut commands: Vec<(String, pm_domain::ProjectCommand, Vec<String>)> = Vec::new();
    if !selected_milestones.is_empty() {
        let evidence = evidence_labels(selected_milestones.iter().map(|item| item.key.clone()));
        commands.push((
            "create_milestones".to_string(),
            pm_domain::ProjectCommand::CreateMilestones {
                milestones: selected_milestones.clone(),
            },
            evidence,
        ));
    }
    if !selected_requirements.is_empty() {
        let evidence = evidence_labels(selected_requirements.iter().flat_map(|item| {
            item.evidence
                .iter()
                .map(|ev| format!("{}:{}", ev.repo_id, ev.r#ref))
        }));
        // 大草案拆批（spec §16「超出拆批并提示」）：执行器对单条批量命令有
        // `MAX_BATCH_ITEMS` 上限（200），真实仓库的 docs/commits 轻易超过它
        // （ai-pm 自身 300 条 commit + 402 篇 doc）。不拆批时整批命令被守卫
        // 拒绝 → 审批 409「command execution failed」→ 导入永远停在 proposed。
        // 按批拆分后每条命令各自成提案，审批链逐条放行即可全部落地。
        for chunk in selected_requirements.chunks(MAX_BATCH_ITEMS) {
            commands.push((
                "create_requirements".to_string(),
                pm_domain::ProjectCommand::CreateRequirements {
                    requirements: chunk.to_vec(),
                },
                evidence.clone(),
            ));
        }
    }
    if !selected_tasks.is_empty() {
        let tasks: Vec<pm_domain::import::TaskDraft> = selected_tasks
            .iter()
            .map(|task| {
                let mut copy = task.clone();
                copy.milestone_key = task
                    .milestone_key
                    .as_ref()
                    .and_then(|key| milestone_names.get(key).cloned());
                copy.requirement_key = task
                    .requirement_key
                    .as_ref()
                    .and_then(|key| requirement_codes.get(key).cloned());
                copy
            })
            .collect();
        let evidence = evidence_labels(selected_tasks.iter().flat_map(|item| {
            item.evidence
                .iter()
                .map(|ev| format!("{}:{}", ev.repo_id, ev.r#ref))
        }));
        for chunk in tasks.chunks(MAX_BATCH_ITEMS) {
            commands.push((
                "create_tasks".to_string(),
                pm_domain::ProjectCommand::CreateTasks {
                    tasks: chunk.to_vec(),
                },
                evidence.clone(),
            ));
        }
    }
    if req.write_knowledge_documents {
        let mut documents: Vec<pm_domain::import::DocumentDraft> = selected_requirements
            .iter()
            .map(|item| pm_domain::import::DocumentDraft {
                doc_key: item.code.clone(),
                doc_type: "requirement".to_string(),
                markdown: if item.markdown.is_empty() {
                    format!("# {}\n\n", item.title)
                } else {
                    item.markdown.clone()
                },
                change_summary: Some("冷启动导入：来自仓库事实".to_string()),
            })
            .collect();
        documents.push(pm_domain::import::DocumentDraft {
            doc_key: "project-profile".to_string(),
            doc_type: "knowledge".to_string(),
            markdown: profile_markdown(&draft),
            change_summary: Some("冷启动导入：项目画像".to_string()),
        });
        for chunk in documents.chunks(MAX_BATCH_ITEMS) {
            commands.push((
                "write_documents".to_string(),
                pm_domain::ProjectCommand::WriteDocuments {
                    documents: chunk.to_vec(),
                },
                Vec::new(),
            ));
        }
    }

    let confidence = lowest_confidence(&draft);
    let model_name: String =
        sqlx::query_scalar("SELECT model_name FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_one(pool)
            .await?;
    let rationale = "AI 项目经理读取代码仓库 commit 与 docs 后生成的冷启动提案；执行前须人工确认。";

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE core.project SET code = $2, name = $3, description = $4, updated_at = now() \
         WHERE id = $1",
    )
    .bind(project_id)
    .bind(&req.project.code)
    .bind(&req.project.name)
    .bind(&req.project.description)
    .execute(&mut *tx)
    .await?;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.analysis_run \
         (project_id, event_id, status, model_name, policy_version, config_snapshot, result, finished_at, tenant_id) \
         VALUES ($1, $2, 'succeeded', $3, 1, '{}'::jsonb, $4, now(), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(event_id)
    .bind(&model_name)
    .bind(serde_json::json!({ "import_id": import_id }))
    .fetch_one(&mut *tx)
    .await?;

    let mut proposals = Vec::new();
    for (seq, (kind, command, evidence)) in commands.into_iter().enumerate() {
        let item_count = match &command {
            pm_domain::ProjectCommand::CreateMilestones { milestones } => milestones.len(),
            pm_domain::ProjectCommand::CreateRequirements { requirements } => requirements.len(),
            pm_domain::ProjectCommand::CreateTasks { tasks } => tasks.len(),
            pm_domain::ProjectCommand::WriteDocuments { documents } => documents.len(),
            _ => 0,
        };
        let proposed_data = serde_json::to_value(&command)
            .map_err(|e| ImportError::Model(format!("command serialization failed: {e}")))?;
        let evidence_value = serde_json::to_value(&evidence).unwrap_or(serde_json::Value::Null);
        let proposal_id: Uuid = sqlx::query_scalar(
            "INSERT INTO ai.change_proposal \
             (project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, \
              proposed_data, rationale, evidence, risk_level, confidence, status, tenant_id) \
             VALUES ($1, $2, $3, 'project', $4, $5, $6, $7, $8, 'high', $9, 'redline_pending', \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) \
             RETURNING id",
        )
        .bind(project_id)
        .bind(run_id)
        .bind((seq + 1) as i32)
        .bind(project_id)
        .bind(&kind)
        .bind(&proposed_data)
        .bind(rationale)
        .bind(&evidence_value)
        .bind(confidence)
        .fetch_one(&mut *tx)
        .await?;
        let approval_id: Uuid = sqlx::query_scalar(
            "INSERT INTO governance.approval_request \
             (project_id, proposal_id, required_role, tenant_id) \
             VALUES ($1, $2, 'project_owner', \
                     (SELECT tenant_id FROM ai.change_proposal WHERE id = $2)) RETURNING id",
        )
        .bind(project_id)
        .bind(proposal_id)
        .fetch_one(&mut *tx)
        .await?;
        proposals.push(ProposalRef {
            kind,
            proposal_id,
            approval_id,
            item_count,
        });
    }

    sqlx::query(
        "UPDATE integration.repo_import SET status = 'proposed', analysis_run_id = $2, \
         proposed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(import_id)
    .bind(run_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(ProposedImport {
        outcome: "proposed".to_string(),
        proposals,
        analysis_run_id: run_id,
    })
}
