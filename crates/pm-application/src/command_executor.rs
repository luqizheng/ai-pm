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

use std::sync::Arc;

use pm_domain::{AuditId, CommandError, MilestoneId, ProjectCommand, ProjectId, TaskId, TaskStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

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
}

type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

impl PgCommandExecutor {
    pub fn new(pool: sqlx::PgPool, policy: Arc<PolicyEngine>) -> Self {
        Self { pool, policy }
    }

    /// Handler registry check — the orchestrator validates executability with
    /// this before dispatching.
    pub fn has_handler(&self, kind: &str) -> bool {
        matches!(kind, "update_task_status" | "sign_acceptance")
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
            ProjectCommand::UpdateTaskStatus {
                task_id,
                expected_revision,
                new_status,
            } => self.update_task_status(task_id, expected_revision, new_status, &ctx).await,
            ProjectCommand::SignAcceptance {
                project_id,
                milestone_id,
                expected_revision,
            } => self.sign_acceptance(project_id, milestone_id, expected_revision, &ctx).await,
            // `change_policy` is a red line with no M1 handler; every other
            // variant without a handler fails the same way (Task 7 adds them).
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

    /// UpdateTaskStatus: `delivery.task` row lock → revision guard → status
    /// swap with `revision + 1` → audit row with a Restore snapshot of the old
    /// status.
    async fn update_task_status(
        &self,
        task_id: TaskId,
        expected_revision: i64,
        new_status: TaskStatus,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, CommandError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let row = sqlx::query(
            "SELECT status, revision FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(task_id)
        .bind(ctx.project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        let (status_before, revision) = match row {
            Some(row) => (row.get::<String, _>("status"), row.get::<i64, _>("revision")),
            None => {
                return Err(CommandError::Validation(format!(
                    "task {task_id} not found in project {}",
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
            "UPDATE delivery.task SET status = $1, revision = revision + 1, updated_at = now() WHERE id = $2",
        )
        .bind(new_status.as_str())
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        let resulting_revision = revision + 1;
        let before = json!({ "status": status_before, "revision": revision });
        let after = json!({ "status": new_status.as_str(), "revision": resulting_revision });
        let compensation = CompensationSpec::Restore(vec![SnapshotEntry {
            table: CompensableTable::Task,
            target_id: task_id,
            fields: json!({ "status": status_before.clone(), "revision": resulting_revision }),
        }]);
        let audit_id = insert_audit(
            &mut tx,
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
    let set_cols: Vec<&String> = fields.keys().filter(|key| *key != revcol).collect();
    if set_cols.is_empty() {
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
    // Params: $1 = target id, $2 = expected revision, $3.. = column values.
    let mut sql = format!("UPDATE {table} SET ");
    for (idx, col) in set_cols.iter().enumerate() {
        if idx > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("{} = ${}", col, idx + 3));
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
         (project_id, actor_id, action, target_type, target_id, before_data, after_data, compensation_command, analysis_run_id, source_kind, source_ref) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
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

fn db_err(e: sqlx::Error) -> CommandError {
    CommandError::Db(e)
}
