//! Command channel core (Task 6): the single write path into the database for
//! project commands — execute + audit + snapshot compensation.
//!
//! Every command runs inside one sqlx transaction: `SELECT ... FOR UPDATE` on
//! the target row → revision guard → apply the change and bump `revision` →
//! `INSERT governance.audit_log` (before/after JSONB, compensation spec) →
//! commit and return an [`ExecutionReceipt`].
//!
//! Compensation is status-recovery style: the audit row carries a
//! [`CompensationSpec`] snapshotted at execution time. `compensate` replays it
//! under a fresh revision guard — the snapshot's revision column holds the
//! executed command's `resulting_revision`, so if the target row has moved on
//! since, the rollback fails with [`CommandError::RevisionConflict`] instead of
//! silently overwriting newer state. Idempotency of compensation itself is
//! guarded by `compensated_command_id` ([`CommandError::AlreadyCompensated`]).
//!
//! Red lines: the red-line set comes from the shared [`PolicyEngine`] (the
//! single source of truth; never a local list). A red-line command executes
//! only for an [`ActorKind::Human`] actor through the
//! [`CommandSource::RedlineApproval`] flow; in M1 the executor rejects every
//! non-human actor outright (`RedLineBlocked`) before any transaction opens.

use std::collections::HashMap;
use std::sync::Arc;

use pm_domain::{
    validate_transition, AuditId, CommandError, MilestoneId, PrincipalId, ProjectCommand, ProjectId,
    RequirementId, RevisionId, TaskId, TaskStatus, TransitionInput,
};
use pm_domain::import::{DocumentDraft, MilestoneDraft, RequirementDraft, TaskDraft};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::bootstrap::resolve_assignee;
use crate::knowledge::{self, CreateDocumentRequest, KnowledgeRoot};
use crate::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};
use crate::policy_engine::PolicyEngine;

/// Who issues a command. The executor only branches on [`ActorKind::Human`]
/// (red-line gate); the other variants mirror `core.principal.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Human,
    AiPm,
    DevAgent,
    Service,
}

/// Where a command came from. `AiProposal` additionally lands the run id in
/// `governance.audit_log.analysis_run_id`.
#[derive(Debug, Clone)]
pub enum CommandSource {
    AiProposal {
        run_id: pm_domain::RunId,
        proposal_id: pm_domain::ProposalId,
    },
    HumanIntervention {
        intervention_id: Uuid,
    },
    RedlineApproval {
        approval_id: Uuid,
    },
    Bootstrap,
    /// 平台内部动作（如 Run 调度领取、租约回收）——无人类/提案身份。
    System,
}

/// Execution identity: which project, which principal, on whose behalf, from
/// which channel.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub project_id: ProjectId,
    pub actor_id: pm_domain::PrincipalId,
    pub actor_kind: ActorKind,
    pub source: CommandSource,
}

/// A compensable table, mapped to `(schema.table, primary-key column,
/// revision column)` by [`CompensableTable::mapping`] (shared with the Task 7
/// handlers — do not inline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensableTable {
    Task,
    Milestone,
    Requirement,
    RequirementRevision,
}

impl CompensableTable {
    /// Fully-qualified table name, primary-key column, optimistic-lock column.
    pub(crate) fn mapping(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Task => ("delivery.task", "id", "revision"),
            Self::Milestone => ("delivery.milestone", "id", "revision"),
            Self::Requirement => ("delivery.requirement", "id", "row_revision"),
            // Requirement revisions lock on their version counter.
            Self::RequirementRevision => ("delivery.requirement_revision", "id", "version"),
        }
    }
}

/// One row snapshot inside a [`CompensationSpec::Restore`].
///
/// `fields` maps column names to the values to write back; the entry's
/// revision column holds the revision the row must currently be at when this
/// entry is restored (the executing command's `resulting_revision`). Restore
/// bumps the revision by one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub table: CompensableTable,
    pub target_id: Uuid,
    pub fields: Value,
}

/// How to undo an executed command, stored in
/// `governance.audit_log.compensation_command`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompensationSpec {
    /// Restore previously snapshotted field values (status-recovery style).
    Restore(Vec<SnapshotEntry>),
    /// Delete a row created by the command (create-style handlers, Task 7).
    Delete {
        table: CompensableTable,
        target_id: Uuid,
    },
}

/// What one successful execution (or compensation) produced.
#[derive(Debug, Clone)]
pub struct ExecutionReceipt {
    pub audit_id: AuditId,
    pub command_kind: &'static str,
    pub target_id: Uuid,
    pub resulting_revision: i64,
}

/// Postgres-backed command executor: the write-safety base for V2. All project
/// commands go through [`PgCommandExecutor::execute`]; rollbacks go through
/// [`PgCommandExecutor::compensate`].
pub struct PgCommandExecutor {
    pool: sqlx::PgPool,
    policy: Arc<PolicyEngine>,
    /// §20.6.1：outbox 事件与业务状态变更写在同一事务里（`append` 收 `&mut tx`）。
    outbox: Arc<dyn OutboxAppend>,
}

type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

/// §20.7 L3：任务状态迁移的目标状态 → 需异步分发的 outbox 事件。只登记语义无
/// 歧义的迁移边：进入 `ready` 即「任务满足执行条件」（TaskReady），进入
/// `submitted` 即「交付已提交」（DeliverySubmitted）。`RunRequested` /
/// `RunCompleted` / `ReviewCompleted` / `KnowledgeBaseWriteRequested` 的触发动作
/// 是创建 Run、评审结论落库与知识回写请求——这些业务动作目前没有落点，故不
/// 由纯状态列变更产生（§20.6「不假装消费」）。
fn outbox_event_for_task_target(to: &str) -> Option<&'static str> {
    match to {
        "ready" => Some("TaskReady"),
        "submitted" => Some("DeliverySubmitted"),
        _ => None,
    }
}

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
            return Err(CommandError::RedLineBlocked { command: cmd.kind() });
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
            } => self.update_task_status(task_id, expected_revision, new_status, &ctx).await,
            ProjectCommand::ApproveRequirementRevision {
                project_id,
                requirement_id,
                revision_id,
            } => {
                self.approve_requirement_revision(project_id, requirement_id, revision_id, &ctx)
                    .await
            }
            ProjectCommand::AdjustMilestonePlan {
                project_id,
                milestone_id,
                new_planned_at,
                expected_revision,
            } => {
                self.adjust_milestone_plan(project_id, milestone_id, new_planned_at, expected_revision, &ctx)
                    .await
            }
            ProjectCommand::SignAcceptance {
                project_id,
                milestone_id,
                expected_revision,
            } => self.sign_acceptance(project_id, milestone_id, expected_revision, &ctx).await,
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
        let row = row.ok_or_else(|| CommandError::Validation(format!("audit row {target_audit_id} not found")))?;

        let audit_project_id: ProjectId = row.get("project_id");
        if audit_project_id != ctx.project_id {
            return Err(CommandError::Validation(format!(
                "audit row {target_audit_id} belongs to project {audit_project_id}, not {}",
                ctx.project_id
            )));
        }
        if row.get::<Option<Uuid>, _>("compensated_command_id").is_some() {
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
        .map_err(|e| CommandError::Validation(format!("stored compensation command is unreadable: {e}")))?;

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
            Some(row) => (row.get::<String, _>("status"), row.get::<i64, _>("revision")),
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
                "INSERT INTO delivery.requirement (project_id, code, title, status, tenant_id) \
                 VALUES ($1, $2, $3, 'draft', (SELECT tenant_id FROM core.project WHERE id = $1)) \
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
            .bind(ctx.actor_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)?;
            sqlx::query("UPDATE delivery.requirement SET current_revision_id = $2 WHERE id = $1")
                .bind(requirement_id)
                .bind(revision_id)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
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
                return Err(CommandError::Validation("task title is required".to_string()));
            }

            let milestone_id: Option<Uuid> = match draft.milestone_key.as_deref() {
                Some(name) if !name.trim().is_empty() => {
                    sqlx::query_scalar(
                        "SELECT id FROM delivery.milestone WHERE project_id = $1 AND name = $2",
                    )
                    .bind(ctx.project_id)
                    .bind(name.trim())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(db_err)?
                }
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
                ctx.actor_id,
                CreateDocumentRequest {
                    doc_key: draft.doc_key.clone(),
                    doc_type: draft.doc_type.clone(),
                    markdown: draft.markdown.clone(),
                    change_summary: draft.change_summary.clone(),
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

    /// ApproveRequirementRevision: lock the requirement row → lock and
    /// ownership-check the target revision → supersede the currently approved
    /// revision (when there is one and it is not the target) → approve the
    /// target → move `current_revision_id` and bump `row_revision`. The
    /// compensation restores the pointer plus `row_revision` and both
    /// revisions' states; the pointer may be SQL NULL (first approval), which
    /// the snapshot carries as a JSON null field.
    async fn approve_requirement_revision(
        &self,
        project_id: ProjectId,
        requirement_id: RequirementId,
        revision_id: RevisionId,
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
            "SELECT current_revision_id, row_revision FROM delivery.requirement \
             WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(requirement_id)
        .bind(ctx.project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        let (current_revision_id, row_revision) = match row {
            Some(row) => (
                row.get::<Option<Uuid>, _>("current_revision_id"),
                row.get::<i64, _>("row_revision"),
            ),
            None => {
                return Err(CommandError::Validation(format!(
                    "requirement {requirement_id} not found in project {}",
                    ctx.project_id
                )))
            }
        };

        // The target revision must exist and belong to this requirement (its
        // FK (project_id, requirement_id) then also pins the project).
        let rev = sqlx::query(
            "SELECT state, version FROM delivery.requirement_revision \
             WHERE id = $1 AND requirement_id = $2 FOR UPDATE",
        )
        .bind(revision_id)
        .bind(requirement_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        let (target_state, target_version) = match rev {
            Some(row) => (
                row.get::<String, _>("state"),
                i64::from(row.get::<i32, _>("version")),
            ),
            None => {
                return Err(CommandError::Validation(format!(
                    "revision {revision_id} does not belong to requirement {requirement_id}"
                )))
            }
        };

        // Supersede the currently approved revision — unless the target already
        // is the current one (re-approval), where there is nothing to supersede.
        let mut superseded = None;
        if let Some(old_id) = current_revision_id.filter(|old| *old != revision_id) {
            let old = sqlx::query(
                "SELECT state, version FROM delivery.requirement_revision \
                 WHERE id = $1 AND requirement_id = $2 FOR UPDATE",
            )
            .bind(old_id)
            .bind(requirement_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
            let (old_state, old_version) = match old {
                Some(row) => (
                    row.get::<String, _>("state"),
                    i64::from(row.get::<i32, _>("version")),
                ),
                None => {
                    return Err(CommandError::Validation(format!(
                        "current revision {old_id} of requirement {requirement_id} not found"
                    )))
                }
            };
            sqlx::query("UPDATE delivery.requirement_revision SET state = 'superseded' WHERE id = $1")
                .bind(old_id)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            superseded = Some((old_id, old_state, old_version));
        }

        sqlx::query(
            "UPDATE delivery.requirement_revision SET state = 'approved', approved_by = $2 WHERE id = $1",
        )
        .bind(revision_id)
        .bind(ctx.actor_id)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query(
            "UPDATE delivery.requirement SET current_revision_id = $2, row_revision = row_revision + 1 WHERE id = $1",
        )
        .bind(requirement_id)
        .bind(revision_id)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        let resulting_revision = row_revision + 1;

        let mut entries = vec![
            SnapshotEntry {
                table: CompensableTable::Requirement,
                target_id: requirement_id,
                fields: json!({
                    "current_revision_id": current_revision_id,
                    "row_revision": resulting_revision,
                }),
            },
            SnapshotEntry {
                table: CompensableTable::RequirementRevision,
                target_id: revision_id,
                fields: json!({ "state": target_state, "version": target_version }),
            },
        ];
        if let Some((old_id, old_state, old_version)) = superseded {
            entries.push(SnapshotEntry {
                table: CompensableTable::RequirementRevision,
                target_id: old_id,
                fields: json!({ "state": old_state, "version": old_version }),
            });
        }
        let compensation = CompensationSpec::Restore(entries);

        let before = json!({
            "current_revision_id": current_revision_id,
            "row_revision": row_revision,
        });
        let after = json!({
            "current_revision_id": revision_id,
            "row_revision": resulting_revision,
        });
        let audit_id = insert_audit(
            &mut tx,
            ctx,
            "approve_requirement_revision",
            "requirement",
            Some(requirement_id),
            &before,
            &after,
            Some(&compensation),
            audit_source(ctx),
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(ExecutionReceipt {
            audit_id,
            command_kind: "approve_requirement_revision",
            target_id: requirement_id,
            resulting_revision,
        })
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

/// Applies a compensation spec inside the caller's transaction. Returns the
/// per-row before/after JSON for the rollback audit row, the last resulting
/// revision, and the inverse spec (undo-the-undo) when one exists.
async fn apply_compensation(
    tx: &mut Tx,
    spec: &CompensationSpec,
) -> Result<(Value, Value, i64, Option<CompensationSpec>), CommandError> {
    match spec {
        CompensationSpec::Restore(entries) => {
            let mut before_rows = Vec::with_capacity(entries.len());
            let mut after_rows = Vec::with_capacity(entries.len());
            let mut inverse_entries = Vec::with_capacity(entries.len());
            let mut resulting_revision = 0i64;
            for entry in entries {
                let (table, _, revcol) = entry.table.mapping();
                let (before, after, revision) = restore_entry(tx, entry).await?;
                before_rows.push(json!({ "table": table, "target_id": entry.target_id, "row": before }));
                after_rows.push(json!({ "table": table, "target_id": entry.target_id, "row": after }));
                // Undo-the-undo: restore the observed pre-rollback state,
                // guarded at the revision the rollback leaves behind.
                let mut inverse_fields = before
                    .as_object()
                    .expect("to_jsonb(t) always yields a JSON object")
                    .clone();
                inverse_fields.insert(revcol.to_string(), json!(revision));
                inverse_entries.push(SnapshotEntry {
                    table: entry.table,
                    target_id: entry.target_id,
                    fields: Value::Object(inverse_fields),
                });
                resulting_revision = revision;
            }
            Ok((
                Value::Array(before_rows),
                Value::Array(after_rows),
                resulting_revision,
                Some(CompensationSpec::Restore(inverse_entries)),
            ))
        }
        CompensationSpec::Delete { table, target_id } => {
            let (table_name, pk, _) = table.mapping();
            let result = sqlx::query(&format!("DELETE FROM {table_name} WHERE {pk} = $1"))
                .bind(target_id)
                .execute(&mut **tx)
                .await
                .map_err(db_err)?;
            if result.rows_affected() == 0 {
                return Err(CommandError::Validation(format!(
                    "compensation target row missing in {table_name}: {target_id}"
                )));
            }
            let after = json!({ "deleted": true, "table": table_name, "target_id": target_id });
            // A deleted row cannot be re-created from a snapshot spec; the
            // rollback audit row carries no inverse command (M1: no producer).
            Ok((Value::Null, after, 0, None))
        }
    }
}

/// Restores one snapshot entry under a fresh revision guard. Returns the
/// pre-restore row JSON, the post-restore row JSON and the new revision.
async fn restore_entry(
    tx: &mut Tx,
    entry: &SnapshotEntry,
) -> Result<(Value, Value, i64), CommandError> {
    let (table, pk, revcol) = entry.table.mapping();
    let fields = entry.fields.as_object().ok_or_else(|| {
        CommandError::Validation("snapshot entry `fields` must be a JSON object".to_string())
    })?;
    for key in fields.keys() {
        validate_identifier(key)?;
    }
    let expected = fields.get(revcol).and_then(Value::as_i64).ok_or_else(|| {
        CommandError::Validation(format!(
            "snapshot entry is missing its revision column `{revcol}`"
        ))
    })?;
    // JSON null fields mean SQL NULL for any column type: they are written as
    // NULL literals, not bound parameters — a TEXT-typed NULL parameter fails
    // to prepare against non-text columns (uuid, timestamptz, …).
    let (null_pairs, set_pairs): (Vec<_>, Vec<_>) = fields
        .iter()
        .filter(|(key, _)| key.as_str() != revcol)
        .partition(|(_, value)| value.is_null());
    let null_cols: Vec<&String> = null_pairs.into_iter().map(|(key, _)| key).collect();
    let set_cols: Vec<&String> = set_pairs.into_iter().map(|(key, _)| key).collect();
    if set_cols.is_empty() && null_cols.is_empty() {
        return Err(CommandError::Validation(
            "snapshot entry has no columns to restore".to_string(),
        ));
    }
    // Whole-row snapshot of the current state (before the restore), as JSON.
    let before: Value = sqlx::query_scalar(&format!(
        "SELECT to_jsonb(t) FROM {table} t WHERE t.{pk} = $1 FOR UPDATE"
    ))
    .bind(entry.target_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?
    .ok_or_else(|| {
        CommandError::Validation(format!(
            "compensation target row missing in {table}: {}",
            entry.target_id
        ))
    })?;
    let actual = before.get(revcol).and_then(Value::as_i64).ok_or_else(|| {
        CommandError::Validation(format!("row in {table} is missing revision column `{revcol}`"))
    })?;
    if actual != expected {
        return Err(CommandError::RevisionConflict {
            expected,
            actual,
        });
    }
    // String snapshots may target non-text columns (uuid ids, timestamptz
    // plans): a TEXT-typed parameter does not assignment-cast into them, so
    // those assignments get an explicit `::type` cast.
    let types = column_types(tx, table).await?;
    // Params: $1 = target id, $2 = expected revision, $3.. = column values.
    // Null columns come first (no parameters), then the value columns in order.
    let mut sql = format!("UPDATE {table} SET ");
    for (idx, col) in null_cols.iter().enumerate() {
        if idx > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("{col} = NULL"));
    }
    if !null_cols.is_empty() && !set_cols.is_empty() {
        sql.push_str(", ");
    }
    for (idx, col) in set_cols.iter().enumerate() {
        if idx > 0 {
            sql.push_str(", ");
        }
        match types.get(*col) {
            Some(data_type) if data_type != "text" && fields[*col].is_string() => {
                sql.push_str(&format!("{col} = ${}::{data_type}", idx + 3));
            }
            _ => sql.push_str(&format!("{col} = ${}", idx + 3)),
        }
    }
    sql.push_str(&format!(", {revcol} = $2 + 1 WHERE {pk} = $1"));
    let mut query = sqlx::query(&sql).bind(entry.target_id).bind(expected);
    for col in &set_cols {
        query = bind_field(query, &fields[*col]);
    }
    query.execute(&mut **tx).await.map_err(db_err)?;
    let resulting_revision = expected + 1;
    let mut after = entry.fields.clone();
    after[revcol] = json!(resulting_revision);
    Ok((before, after, resulting_revision))
}

/// Column data types for a schema-qualified table, read from the catalog.
/// Used by [`restore_entry`] to decide which SET assignments need an explicit
/// cast when replaying a stored snapshot.
async fn column_types(
    tx: &mut Tx,
    table: &str,
) -> Result<HashMap<String, String>, CommandError> {
    let (schema, name) = table.split_once('.').ok_or_else(|| {
        CommandError::Validation(format!("table `{table}` must be schema-qualified"))
    })?;
    let rows = sqlx::query(
        "SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_schema = $1 AND table_name = $2",
    )
    .bind(schema)
    .bind(name)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_err)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("column_name"),
                row.get::<String, _>("data_type"),
            )
        })
        .collect())
}

/// Writes one `governance.audit_log` row and returns its id. `source` is the
/// `(source_kind, source_ref)` identity of the write (see [`audit_source`] for
/// command executions; compensations pass `("rollback", original audit id)`).
#[allow(clippy::too_many_arguments)]
async fn insert_audit(
    tx: &mut Tx,
    ctx: &ExecutionContext,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    before_data: &Value,
    after_data: &Value,
    compensation: Option<&CompensationSpec>,
    source: (&'static str, Option<Uuid>),
) -> Result<AuditId, CommandError> {
    let analysis_run_id = match &ctx.source {
        CommandSource::AiProposal { run_id, .. } => Some(*run_id),
        _ => None,
    };
    let compensation = compensation.map(|spec| serde_json::to_value(spec).expect("CompensationSpec serializes"));
    sqlx::query_scalar(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, action, target_type, target_id, before_data, after_data, compensation_command, analysis_run_id, source_kind, source_ref, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(ctx.project_id)
    .bind(ctx.actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(before_data)
    .bind(after_data)
    .bind(compensation)
    .bind(analysis_run_id)
    .bind(source.0)
    .bind(source.1)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_err)
}

/// The audit identity of a command execution: `(source_kind, source_ref)` —
/// the join anchor Task 15's approval flow reads (`analysis_run_id` keeps
/// carrying the AiProposal run id as before).
fn audit_source(ctx: &ExecutionContext) -> (&'static str, Option<Uuid>) {
    match &ctx.source {
        CommandSource::AiProposal { proposal_id, .. } => ("ai_proposal", Some(*proposal_id)),
        CommandSource::HumanIntervention { intervention_id } => {
            ("human_intervention", Some(*intervention_id))
        }
        CommandSource::RedlineApproval { approval_id } => ("redline_approval", Some(*approval_id)),
        CommandSource::Bootstrap => ("bootstrap", None),
        CommandSource::System => ("system", None),
    }
}

/// Binds a snapshot field value with the right Postgres representation. SQLx
/// sends `serde_json::Value` as JSONB, so a plain JSON string bound into a
/// TEXT column would store its quoted JSON text (`"backlog"`); JSON scalars
/// therefore bind as their SQL scalar types, only objects/arrays as JSONB.
fn bind_field<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: &'q Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        Value::String(text) => query.bind(text.clone()),
        Value::Bool(flag) => query.bind(*flag),
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                query.bind(int)
            } else {
                query.bind(number.as_f64().unwrap_or(0.0))
            }
        }
        Value::Null => query.bind(Option::<String>::None),
        object_or_array => query.bind(object_or_array.clone()),
    }
}

/// Compensation specs are stored as JSONB and replayed with interpolated
/// identifiers; column names must be plain lowercase SQL identifiers.
fn validate_identifier(name: &str) -> Result<(), CommandError> {
    let first_ok = name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_');
    let rest_ok = name
        .chars()
        .skip(1)
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if first_ok && rest_ok {
        Ok(())
    } else {
        Err(CommandError::Validation(format!(
            "invalid column identifier `{name}` in compensation spec"
        )))
    }
}

/// 单条批量命令的条目上限（spec §十六：建议 ≤200，超出拆批）。
const MAX_BATCH_ITEMS: usize = 200;

fn validate_batch(len: usize, what: &str) -> Result<(), CommandError> {
    if len == 0 {
        return Err(CommandError::Validation(format!(
            "{what} batch is empty"
        )));
    }
    if len > MAX_BATCH_ITEMS {
        return Err(CommandError::Validation(format!(
            "{what} batch exceeds {MAX_BATCH_ITEMS} items"
        )));
    }
    Ok(())
}

fn db_err(e: sqlx::Error) -> CommandError {
    CommandError::Db(e)
}
/// 一次状态迁移必须供给的外部事实（§7.3：域层无 I/O，事实由调用方提供）。
#[derive(Debug, Clone, Copy, Default)]
pub struct TransitionFacts {
    pub waiting_resolved: bool,
    pub replacement_task_exists: bool,
    pub active_run_stopped: bool,
    pub retry_exhausted: bool,
    pub deadline_exhausted: bool,
    pub delivery_submitted: bool,
    pub acceptance_evidence_complete: bool,
}


impl TransitionFacts {
    /// 既有命令通道的事实：交付证据（Running→Submitted / Reviewing→Completed）
    /// 的完整校验属 D4；此处保持既有行为不变，其余事实保持默认 false。
    pub fn for_command_channel() -> Self {
        Self {
            delivery_submitted: true,
            acceptance_evidence_complete: true,
            ..Self::default()
        }
    }

    /// 调度/回收路径的事实：全部为默认（`ready↔claimed` 等 guard-free 边）。
    pub fn for_dispatch() -> Self {
        Self::default()
    }
}

/// **Task 状态变更的唯一受守卫入口（R1）**：行锁 → revision guard →
/// `pm-domain::validate_transition` → 状态推进 → 审计。`update_task_status`
/// 命令通道与 `run_dispatch` 领取/回收都复用本函数，因此不存在第二条绕过
/// 状态机的写入路径。调用方负责事务边界与（如适用）outbox 事件登记。
pub(crate) async fn apply_task_transition(
    tx: &mut Tx,
    task_id: Uuid,
    expected_revision: Option<i64>,
    to: TaskStatus,
    facts: &TransitionFacts,
    ctx: &ExecutionContext,
) -> Result<(AuditId, i64), CommandError> {
    let row = sqlx::query(
        "SELECT status, revision FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(ctx.project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;
    let row = row.ok_or_else(|| {
        CommandError::Validation(format!(
            "task {task_id} not found in project {}",
            ctx.project_id
        ))
    })?;
    let status_before: String = row.get("status");
    let revision: i64 = row.get("revision");
    if let Some(expected) = expected_revision {
        if revision != expected {
            return Err(CommandError::RevisionConflict {
                expected,
                actual: revision,
            });
        }
    }
    let from = TaskStatus::from_str(&status_before)
        .ok_or_else(|| CommandError::Validation(format!("unknown task status `{status_before}`")))?;
    let input = TransitionInput {
        task_id: Some(task_id),
        from,
        to,
        waiting: None,
        waiting_resolved: facts.waiting_resolved,
        replacement_task_exists: facts.replacement_task_exists,
        active_run_stopped: facts.active_run_stopped,
        retry_exhausted: facts.retry_exhausted,
        deadline_exhausted: facts.deadline_exhausted,
        failure: None,
        supersede: None,
        delivery_submitted: facts.delivery_submitted,
        acceptance_evidence_complete: facts.acceptance_evidence_complete,
    };
    validate_transition(&input).map_err(|e| {
        CommandError::Validation(format!(
            "state machine refused {} -> {}: {e}",
            status_before,
            to.as_str()
        ))
    })?;
    sqlx::query(
        "UPDATE delivery.task SET status = $1, revision = revision + 1, updated_at = now() WHERE id = $2",
    )
    .bind(to.as_str())
    .bind(task_id)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    let resulting_revision = revision + 1;
    let before = json!({ "status": status_before, "revision": revision });
    let after = json!({ "status": to.as_str(), "revision": resulting_revision });
    let compensation = CompensationSpec::Restore(vec![SnapshotEntry {
        table: CompensableTable::Task,
        target_id: task_id,
        fields: json!({ "status": status_before, "revision": resulting_revision }),
    }]);
    let audit_id = insert_audit(
        tx,
        ctx,
        "update_task_status",
        "task",
        Some(task_id),
        &before,
        &after,
        Some(&compensation),
        audit_source(ctx),
    )
    .await?;
    Ok((audit_id, resulting_revision))
}

/// 解析平台内部动作（调度/回收）的执行身份：固定 `ai-pm-dispatch` service
/// principal（`0015` 幂等种子）。平台动作没有认证身份，审计三列为 NULL。
pub(crate) async fn system_context(
    _tx: &mut Tx,
    project_id: ProjectId,
) -> Result<ExecutionContext, CommandError> {
    Ok(ExecutionContext {
        project_id,
        actor_id: crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL,
        actor_kind: ActorKind::Service,
        source: CommandSource::System,
    })
}
