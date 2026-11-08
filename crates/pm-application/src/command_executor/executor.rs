use super::*;

impl PgCommandExecutor {
    pub fn new(pool: sqlx::PgPool, policy: Arc<PolicyEngine>) -> Self {
        Self {
            pool,
            policy,
            outbox: Arc::new(PgOutboxAppend),
        }
    }

    /// 构造一个注入自定义 outbox 端口的执行器。端口可替换，正是为了证明网页
    /// 「事件写入与业务状态变更同事务」——测试注入一个必失败的端口，验证写入
    /// 失败会让整个业务变更回滚。
    pub fn with_outbox(
        pool: sqlx::PgPool,
        policy: Arc<PolicyEngine>,
        outbox: Arc<dyn OutboxAppend>,
    ) -> Self {
        Self {
            pool,
            policy,
            outbox,
        }
    }

    /// Handler registry check — the orchestrator validates executability with
    /// this before dispatching. `rollback_command` is registered because its
    /// handler exists — [`PgCommandExecutor::compensate`] — but note its entry
    /// point is `compensate`, not `execute` (see `execute`).
    pub fn has_handler(&self, kind: &str) -> bool {
        matches!(
            kind,
            "create_task"
                | "update_task_status"
                | "approve_requirement_revision"
                | "adjust_milestone_plan"
                | "sign_acceptance"
                | "rollback_command"
                | "create_milestones"
                | "create_requirements"
                | "create_tasks"
                | "write_documents"
        )
    }

    /// Executes one command in a single transaction and writes its audit row.
    ///
    /// Red lines are enforced before any transaction opens: non-human actors
    /// are rejected (`RedLineBlocked`). A human executing a red line must have
    /// come through the red-line approval flow — in M1 the executor trusts the
    /// [`CommandSource::RedlineApproval`] channel that produced the command.
    pub async fn execute(
        &self,
        cmd: ProjectCommand,
        ctx: ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        if self.policy.is_redline(&cmd) && ctx.actor_kind != ActorKind::Human {
            return Err(CommandError::RedLineBlocked {
                command: cmd.kind(),
            });
        }
        match cmd {
            ProjectCommand::CreateTask {
                project_id,
                title,
                description,
                assignee_id,
            } => {
                self.create_task(project_id, title, description, assignee_id, &ctx)
                    .await
            }
            ProjectCommand::UpdateTaskStatus {
                task_id,
                expected_revision,
                new_status,
            } => {
                self.update_task_status(task_id, expected_revision, new_status, &ctx)
                    .await
            }
            ProjectCommand::ApproveRequirementRevision { .. } => {
                self.refuse_requirement_confirmation(&ctx).await
            }
            ProjectCommand::AdjustMilestonePlan {
                project_id,
                milestone_id,
                new_planned_at,
                expected_revision,
            } => {
                self.adjust_milestone_plan(
                    project_id,
                    milestone_id,
                    new_planned_at,
                    expected_revision,
                    &ctx,
                )
                .await
            }
            ProjectCommand::SignAcceptance {
                project_id,
                milestone_id,
                expected_revision,
            } => {
                self.sign_acceptance(project_id, milestone_id, expected_revision, &ctx)
                    .await
            }
            ProjectCommand::CreateMilestones { milestones } => {
                self.create_milestones(milestones, &ctx).await
            }
            ProjectCommand::CreateRequirements { requirements } => {
                self.create_requirements(requirements, &ctx).await
            }
            ProjectCommand::CreateTasks { tasks } => self.create_tasks(tasks, &ctx).await,
            ProjectCommand::WriteDocuments { documents } => {
                self.write_documents(documents, &ctx).await
            }
            // `rollback_command` executes through `compensate()`, not here.
            // `change_policy` is a red line with no M1 handler; it fails the
            // same way as any other handler-less command.
            other => Err(CommandError::NotHandled {
                command: other.kind().to_string(),
            }),
        }
    }

    /// Rolls back the command behind `target_audit_id` by replaying its stored
    /// [`CompensationSpec`], inside one transaction:
    ///
    /// 1. read the audit row `FOR UPDATE`; `compensated_command_id IS NOT NULL`
    ///    → [`CommandError::AlreadyCompensated`] (idempotency guard),
    /// 2. no stored compensation command (irreversible command) → `Validation`,
    /// 3. restore rows under a revision guard — a target row that moved on
    ///    since the command fails with [`CommandError::RevisionConflict`],
    /// 4. write the `rollback_command` audit row and point the original row's
    ///    `compensated_command_id` at it.
    pub async fn compensate(
        &self,
        target_audit_id: AuditId,
        reason: &str,
        ctx: ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let row = sqlx::query(
            "SELECT project_id, action, target_type, target_id, compensation_command, compensated_command_id \
             FROM governance.audit_log WHERE id = $1 FOR UPDATE",
        )
        .bind(target_audit_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        let row = row.ok_or_else(|| {
            CommandError::Validation(format!("audit row {target_audit_id} not found"))
        })?;

        let audit_project_id: ProjectId = row.get("project_id");
        if audit_project_id != ctx.project_id {
            return Err(CommandError::Validation(format!(
                "audit row {target_audit_id} belongs to project {audit_project_id}, not {}",
                ctx.project_id
            )));
        }
        if row
            .get::<Option<Uuid>, _>("compensated_command_id")
            .is_some()
        {
            return Err(CommandError::AlreadyCompensated {
                audit_id: target_audit_id,
            });
        }
        let action: String = row.get("action");
        let target_type: String = row.get("target_type");
        let target_id: Option<Uuid> = row.get("target_id");
        let stored: Option<Value> = row.get("compensation_command");
        let spec: CompensationSpec = serde_json::from_value(stored.ok_or_else(|| {
            CommandError::Validation(format!(
                "audit row {target_audit_id} carries no compensation command (irreversible command)"
            ))
        })?)
        .map_err(|e| {
            CommandError::Validation(format!("stored compensation command is unreadable: {e}"))
        })?;

        let (before_rows, after_rows, resulting_revision, inverse) =
            apply_compensation(&mut tx, &spec).await?;

        let before = json!({
            "reason": reason,
            "target_audit_id": target_audit_id,
            "compensated_action": action,
            "rows": before_rows,
        });
        let after = json!({ "rows": after_rows });
        let rollback_audit_id = insert_audit(
            &mut tx,
            &ctx,
            "rollback_command",
            &target_type,
            target_id,
            &before,
            &after,
            inverse.as_ref(),
            // The rollback's source identity is the audit row it undoes.
            ("rollback", Some(target_audit_id)),
        )
        .await?;
        sqlx::query("UPDATE governance.audit_log SET compensated_command_id = $1 WHERE id = $2")
            .bind(rollback_audit_id)
            .bind(target_audit_id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;

        Ok(ExecutionReceipt {
            audit_id: rollback_audit_id,
            command_kind: "rollback_command",
            target_id: target_id.unwrap_or(target_audit_id),
            resulting_revision,
        })
    }

    /// UpdateTaskStatus: 经**唯一受守卫的迁移服务** [`apply_task_transition`] 执行
    /// （`validate_transition` + 行锁 + revision guard + 审计），再在同一事务内
    /// 登记异步事件。R1：任何 Task 状态变更都必须走这条统一通道。
    async fn update_task_status(
        &self,
        task_id: TaskId,
        expected_revision: i64,
        new_status: TaskStatus,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let facts = TransitionFacts::for_command_channel();
        let (audit_id, resulting_revision) = apply_task_transition(
            &mut tx,
            task_id,
            Some(expected_revision),
            new_status,
            &facts,
            ctx,
        )
        .await?;
        // F-8（实现对照审计 2026-11-07）：Task → Cancelled 时同一事务联动取消
        // 未决介入链（V1.1 §5.1 CANCELLED 触发行 / §14：取消 → 取消尚未完成的
        // Interaction，guard 归 none，审计保留）。事务核内 Task 已终态 → 自动
        // 跳过 awaiting_decision 回转（§5.3"或随 Task 终态"）。
        if new_status == TaskStatus::Cancelled {
            crate::interaction::cancel_open_interaction_tx(
                &mut tx,
                ctx.project_id,
                task_id,
                "task cancelled",
                &ctx.actor,
            )
            .await
            .map_err(|e| match e {
                crate::interaction::InteractionError::Db(e) => db_err(e),
                other => CommandError::Validation(other.to_string()),
            })?;
        }
        // §20.6.1 / §20.7 L3：状态迁移成功的同时，在同一事务内登记需异步分发的
        // 事件；幂等键以迁移后的 revision 钉死，重复执行同一迁移不会重复写事件。
        if let Some(event_type) = outbox_event_for_task_target(new_status.as_str())
            .filter(|event_type| crate::outbox::event_is_enabled(event_type))
        {
            let aggregate_id = task_id.to_string();
            let idempotency_key = format!("task:{task_id}:{event_type}:rev{resulting_revision}");
            self.outbox
                .append(
                    &mut tx,
                    OutboxEvent {
                        project_id: ctx.project_id,
                        event_type,
                        aggregate_type: "Task",
                        aggregate_id: &aggregate_id,
                        payload: json!({
                            "task_id": task_id,
                            "to": new_status.as_str(),
                            "revision": resulting_revision,
                        }),
                        idempotency_key: &idempotency_key,
                    },
                )
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "update_task_status",
            target_id: task_id,
            resulting_revision,
        })
    }

    /// SignAcceptance (red line): `delivery.milestone` row lock → revision
    /// guard → `status = 'accepted'` with `revision + 1` → audit row. The red
    /// line is irreversible ([`ProjectCommand::reversibility()`]), so the audit
    /// row carries no compensation command.
    async fn sign_acceptance(
        &self,
        project_id: ProjectId,
        milestone_id: MilestoneId,
        expected_revision: i64,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        if project_id != ctx.project_id {
            return Err(CommandError::Validation(format!(
                "command targets project {project_id} but the execution context is project {}",
                ctx.project_id
            )));
        }
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let row = sqlx::query(
            "SELECT status, revision FROM delivery.milestone WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(milestone_id)
        .bind(ctx.project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        let (status_before, revision) = match row {
            Some(row) => (
                row.get::<String, _>("status"),
                row.get::<i64, _>("revision"),
            ),
            None => {
                return Err(CommandError::Validation(format!(
                    "milestone {milestone_id} not found in project {}",
                    ctx.project_id
                )))
            }
        };
        if revision != expected_revision {
            return Err(CommandError::RevisionConflict {
                expected: expected_revision,
                actual: revision,
            });
        }
        sqlx::query("UPDATE delivery.milestone SET status = 'accepted', revision = revision + 1 WHERE id = $1")
            .bind(milestone_id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        let resulting_revision = revision + 1;
        let before = json!({ "status": status_before, "revision": revision });
        let after = json!({ "status": "accepted", "revision": resulting_revision });
        let audit_id = insert_audit(
            &mut tx,
            ctx,
            "sign_acceptance",
            "milestone",
            Some(milestone_id),
            &before,
            &after,
            None,
            audit_source(ctx),
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "sign_acceptance",
            target_id: milestone_id,
            resulting_revision,
        })
    }

    /// 冷启动批量建里程碑：同一项目内按名称幂等复用。
    async fn create_milestones(
        &self,
        milestones: Vec<MilestoneDraft>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        validate_batch(milestones.len(), "milestone")?;
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let mut items = Vec::new();
        for draft in milestones {
            let name = draft.name.trim();
            if name.is_empty() {
                return Err(CommandError::Validation(
                    "milestone name is required".to_string(),
                ));
            }
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM delivery.milestone WHERE project_id = $1 AND name = $2",
            )
            .bind(ctx.project_id)
            .bind(name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
            let (id, created) = match existing {
                Some(id) => (id, false),
                None => {
                    let id: Uuid = sqlx::query_scalar(
                        "INSERT INTO delivery.milestone (project_id, name, planned_at, tenant_id) \
                         VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1)) \
                         RETURNING id",
                    )
                    .bind(ctx.project_id)
                    .bind(name)
                    .bind(draft.planned_at)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db_err)?;
                    (id, true)
                }
            };
            items.push(json!({"key": draft.key, "name": name, "id": id, "created": created}));
        }
        let after = json!({"items": items});
        let audit_id = insert_audit(
            &mut tx,
            ctx,
            "create_milestones",
            "project",
            Some(ctx.project_id),
            &Value::Null,
            &after,
            None,
            audit_source(ctx),
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "create_milestones",
            target_id: ctx.project_id,
            resulting_revision: 0,
        })
    }

    /// 冷启动批量建需求：需求落 `draft`，revision 也落 `draft`；`(project, code)` 幂等。
    async fn create_requirements(
        &self,
        requirements: Vec<RequirementDraft>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        validate_batch(requirements.len(), "requirement")?;
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let mut items = Vec::new();
        for draft in requirements {
            let code = draft.code.trim();
            let title = draft.title.trim();
            if code.is_empty() || title.is_empty() {
                return Err(CommandError::Validation(
                    "requirement code and title are required".to_string(),
                ));
            }
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM delivery.requirement WHERE project_id = $1 AND code = $2",
            )
            .bind(ctx.project_id)
            .bind(code)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
            if let Some(id) = existing {
                items.push(json!({"key": draft.key, "code": code, "id": id, "created": false}));
                continue;
            }

            let content = json!({
                "title": title,
                "markdown": draft.markdown,
                "priority": draft.priority,
            });
            let content_text = serde_json::to_string(&content).unwrap_or_default();
            let hash = knowledge::content_hash(&content_text);
            let requirement_id: Uuid = sqlx::query_scalar(
                "INSERT INTO delivery.requirement (project_id, code, title, lifecycle_status, tenant_id) \
                 VALUES ($1, $2, $3, 'active', (SELECT tenant_id FROM core.project WHERE id = $1)) \
                 RETURNING id",
            )
            .bind(ctx.project_id)
            .bind(code)
            .bind(title)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)?;
            let revision_id: Uuid = sqlx::query_scalar(
                "INSERT INTO delivery.requirement_revision \
                 (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
                 VALUES ($1, $2, 1, $3, 'draft', $4, $5, \
                         (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
            )
            .bind(requirement_id)
            .bind(ctx.project_id)
            .bind(&content)
            .bind(&hash)
            .bind(ctx.actor_id())
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)?;
            // S02 §6.6 债务清偿（S02-I2，2026-11-06）：不再自动设 current 指针——
            // 「current 非空 ⇒ approved + Confirmation」不变量成立；v1 保持 draft，
            // 投影绑定随人类 submit 发生（requirement_projection::bind_projection）。
            // 后续 create_tasks 对未确认需求按 I1-D07 降级：无正式版本不建立执行
            // 依据（现有查找对指针 NULL 产出 requirement_unresolved finding）。
            let _ = revision_id;
            items.push(json!({
                "key": draft.key,
                "code": code,
                "id": requirement_id,
                "revision_id": revision_id,
                "created": true,
            }));
        }
        let after = json!({"items": items});
        let audit_id = insert_audit(
            &mut tx,
            ctx,
            "create_requirements",
            "project",
            Some(ctx.project_id),
            &Value::Null,
            &after,
            None,
            audit_source(ctx),
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "create_requirements",
            target_id: ctx.project_id,
            resulting_revision: 0,
        })
    }

    /// 冷启动批量建任务：`milestone_key` / `requirement_key` 在 apply 阶段已
    /// 解析成里程碑名称与需求 code；`assignee_role` 解析成对应 Agent principal。
    /// 解析不到的引用不阻断整批，留空并计入 findings。
    async fn create_tasks(
        &self,
        tasks: Vec<TaskDraft>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        validate_batch(tasks.len(), "task")?;
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let mut items = Vec::new();
        let mut findings: Vec<serde_json::Value> = Vec::new();

        for draft in tasks {
            let title = draft.title.trim();
            if title.is_empty() {
                return Err(CommandError::Validation(
                    "task title is required".to_string(),
                ));
            }

            let milestone_id: Option<Uuid> = match draft.milestone_key.as_deref() {
                Some(name) if !name.trim().is_empty() => sqlx::query_scalar(
                    "SELECT id FROM delivery.milestone WHERE project_id = $1 AND name = $2",
                )
                .bind(ctx.project_id)
                .bind(name.trim())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_err)?,
                _ => None,
            };
            if draft.milestone_key.is_some() && milestone_id.is_none() {
                findings.push(json!({
                    "kind": "milestone_unresolved",
                    "key": draft.key,
                    "ref": draft.milestone_key,
                }));
            }

            let requirement: Option<(Uuid, Option<Uuid>)> = match draft.requirement_key.as_deref() {
                Some(code) if !code.trim().is_empty() => sqlx::query_as(
                    "SELECT id, current_revision_id FROM delivery.requirement \
                     WHERE project_id = $1 AND code = $2",
                )
                .bind(ctx.project_id)
                .bind(code.trim())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_err)?,
                _ => None,
            };
            if draft.requirement_key.is_some() && requirement.is_none() {
                findings.push(json!({
                    "kind": "requirement_unresolved",
                    "key": draft.key,
                    "ref": draft.requirement_key,
                }));
            } else if let Some((_, None)) = &requirement {
                // S02-I2/I1-D07：需求存在但无正式版本（未确认）——不建立执行
                // 依据，可观测信号区分于「完全找不到」（确认 + I4 改绑后可链）。
                findings.push(json!({
                    "kind": "requirement_unconfirmed",
                    "key": draft.key,
                    "ref": draft.requirement_key,
                }));
            }

            let assignee_id: Option<PrincipalId> = match draft.assignee_role {
                Some(role) => match resolve_assignee(&self.pool, ctx.project_id, role.as_str())
                    .await
                    .map_err(|e| CommandError::Validation(e.to_string()))?
                {
                    Some(id) => Some(id),
                    None => {
                        findings.push(json!({
                            "kind": "assignee_unresolved",
                            "key": draft.key,
                            "role": role.as_str(),
                        }));
                        None
                    }
                },
                None => None,
            };

            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM delivery.task WHERE project_id = $1 AND title = $2",
            )
            .bind(ctx.project_id)
            .bind(title)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
            let (task_id, created) = match existing {
                Some(id) => (id, false),
                None => {
                    let id: Uuid = sqlx::query_scalar(
                        "INSERT INTO delivery.task \
                         (project_id, milestone_id, title, description, status, priority, assignee_id, tenant_id) \
                         VALUES ($1, $2, $3, $4, 'draft', $5, $6, \
                                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
                    )
                    .bind(ctx.project_id)
                    .bind(milestone_id)
                    .bind(title)
                    .bind(&draft.description)
                    .bind(&draft.priority)
                    .bind(assignee_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db_err)?;
                    (id, true)
                }
            };

            if let Some((requirement_id, Some(revision_id))) = requirement {
                sqlx::query(
                    "INSERT INTO delivery.task_requirement \
                     (project_id, task_id, requirement_id, revision_id, tenant_id) \
                     VALUES ($1, $2, $3, $4, \
                             (SELECT tenant_id FROM core.project WHERE id = $1)) ON CONFLICT DO NOTHING",
                )
                .bind(ctx.project_id)
                .bind(task_id)
                .bind(requirement_id)
                .bind(revision_id)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            }

            items.push(json!({
                "key": draft.key,
                "id": task_id,
                "created": created,
                "assignee_id": assignee_id,
            }));
        }

        let after = json!({ "items": items, "findings": findings });
        let audit_id = insert_audit(
            &mut tx,
            ctx,
            "create_tasks",
            "project",
            Some(ctx.project_id),
            &Value::Null,
            &after,
            None,
            audit_source(ctx),
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "create_tasks",
            target_id: ctx.project_id,
            resulting_revision: 0,
        })
    }

    /// 批量写知识文档：文件 + git commit + 两表。DB 事务不覆盖文件写入，
    /// 失败时补偿删除本次已写文档（保留此前已存在、被复用的文档）。
    async fn write_documents(
        &self,
        documents: Vec<DocumentDraft>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        validate_batch(documents.len(), "document")?;
        let root = KnowledgeRoot::from_env();
        root.ensure_ready()
            .map_err(|e| CommandError::Validation(format!("knowledge root unavailable: {e}")))?;

        let mut items = Vec::new();
        let mut created: Vec<Uuid> = Vec::new();
        for draft in documents {
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = $2",
            )
            .bind(ctx.project_id)
            .bind(&draft.doc_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
            if let Some(id) = existing {
                items.push(json!({"doc_key": draft.doc_key, "document_id": id, "created": false}));
                continue;
            }
            match knowledge::create_document(
                &self.pool,
                &root,
                ctx.project_id,
                ctx.actor_id(),
                CreateDocumentRequest {
                    doc_key: draft.doc_key.clone(),
                    doc_type: draft.doc_type.clone(),
                    markdown: draft.markdown.clone(),
                    change_summary: draft.change_summary.clone(),
                    // 一次命令内的批量创建天然幂等：doc_key 已是天然唯一键，
                    // operation_id 由 draft_key 派生。
                    operation_id: format!("command_create_document:{}", draft.doc_key),
                    base_revision_id: None,
                },
            )
            .await
            {
                Ok(doc) => {
                    created.push(doc.document_id);
                    items.push(json!({
                        "doc_key": draft.doc_key,
                        "document_id": doc.document_id,
                        "git_commit_sha": doc.git_commit_sha,
                        "created": true,
                    }));
                }
                Err(err) => {
                    for id in created {
                        let _ =
                            knowledge::delete_document(&self.pool, &root, ctx.project_id, id).await;
                    }
                    return Err(CommandError::Validation(format!(
                        "document write failed: {err}"
                    )));
                }
            }
        }

        let after = json!({"items": items});
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let audit_id = insert_audit(
            &mut tx,
            ctx,
            "write_documents",
            "project",
            Some(ctx.project_id),
            &Value::Null,
            &after,
            None,
            audit_source(ctx),
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "write_documents",
            target_id: ctx.project_id,
            resulting_revision: 0,
        })
    }

    /// CreateTask: insert the `delivery.task` row at `status = 'draft'`,
    /// `revision = 1` and audit with a [`CompensationSpec::Delete`] — the
    /// rollback removes the created row. There is no expected_revision: a new
    /// object has no prior state to guard.
    async fn create_task(
        &self,
        project_id: ProjectId,
        title: String,
        description: String,
        assignee_id: Option<PrincipalId>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        if project_id != ctx.project_id {
            return Err(CommandError::Validation(format!(
                "command targets project {project_id} but the execution context is project {}",
                ctx.project_id
            )));
        }
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let task_id: TaskId = sqlx::query_scalar(
            "INSERT INTO delivery.task \
             (project_id, title, description, assignee_id, status, revision, tenant_id) \
             VALUES ($1, $2, $3, $4, 'draft', 1, \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
        )
        .bind(project_id)
        .bind(&title)
        .bind(&description)
        .bind(assignee_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;
        let after = json!({
            "id": task_id,
            "title": title,
            "status": "draft",
            "revision": 1,
        });
        let compensation = CompensationSpec::Delete {
            table: CompensableTable::Task,
            target_id: task_id,
        };
        let audit_id = insert_audit(
            &mut tx,
            ctx,
            "create_task",
            "task",
            Some(task_id),
            // A new row has no prior state; the compensation is a delete.
            &Value::Null,
            &after,
            Some(&compensation),
            audit_source(ctx),
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "create_task",
            target_id: task_id,
            resulting_revision: 1,
        })
    }

    /// ApproveRequirementRevision：**无条件拒绝**（S02-I6-B；S02D-02 #2/S02D-21）。
    ///
    /// 正式确认的唯一合法路径 = REST/UI 统一服务（`requirement::confirm_revision`，
    /// 六要素 + CAS + 真人主体 + Confirmation 落账）；approval_request 不得复用为
    /// 需求确认（§7.1）。AI 提案由红线拦入 pending 人工审批（S02D-02 #3），
    /// 人工裁决也不得经执行器定稿——本拒绝是双入口收敛后的最后防线（DEF-02/DEF-04）。
    async fn refuse_requirement_confirmation(
        &self,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        let _ = ctx;
        Err(CommandError::Validation(
            "requirement confirmation is refused via the command executor — the only path is \
             POST /v1/pm/requirements/{id}/revisions/{rev}/confirm (unified service, S02D-21)"
                .to_string(),
        ))
    }

    /// AdjustMilestonePlan: lock the milestone row → revision guard → swap
    /// `planned_at` (a `None` stores SQL NULL) and bump `revision`. The
    /// compensation restores the previous `planned_at` — possibly SQL NULL —
    /// guarded at the resulting revision.
    async fn adjust_milestone_plan(
        &self,
        project_id: ProjectId,
        milestone_id: MilestoneId,
        new_planned_at: Option<chrono::DateTime<chrono::Utc>>,
        expected_revision: i64,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        if project_id != ctx.project_id {
            return Err(CommandError::Validation(format!(
                "command targets project {project_id} but the execution context is project {}",
                ctx.project_id
            )));
        }
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let row = sqlx::query(
            "SELECT planned_at, revision FROM delivery.milestone WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(milestone_id)
        .bind(ctx.project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        let (planned_before, revision) = match row {
            Some(row) => (
                row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("planned_at"),
                row.get::<i64, _>("revision"),
            ),
            None => {
                return Err(CommandError::Validation(format!(
                    "milestone {milestone_id} not found in project {}",
                    ctx.project_id
                )))
            }
        };
        if revision != expected_revision {
            return Err(CommandError::RevisionConflict {
                expected: expected_revision,
                actual: revision,
            });
        }
        sqlx::query(
            "UPDATE delivery.milestone SET planned_at = $2, revision = revision + 1 WHERE id = $1",
        )
        .bind(milestone_id)
        .bind(new_planned_at)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        let resulting_revision = revision + 1;
        let before = json!({ "planned_at": planned_before, "revision": revision });
        let after = json!({ "planned_at": new_planned_at, "revision": resulting_revision });
        let compensation = CompensationSpec::Restore(vec![SnapshotEntry {
            table: CompensableTable::Milestone,
            target_id: milestone_id,
            fields: json!({ "planned_at": planned_before, "revision": resulting_revision }),
        }]);
        let audit_id = insert_audit(
            &mut tx,
            ctx,
            "adjust_milestone_plan",
            "milestone",
            Some(milestone_id),
            &before,
            &after,
            Some(&compensation),
            audit_source(ctx),
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "adjust_milestone_plan",
            target_id: milestone_id,
            resulting_revision,
        })
    }
}
