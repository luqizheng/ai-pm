# AI-PM V2「AI 主导」M1 垂直切片实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 打通设计稿 §20 的第一版优先链路——「开发 Agent 提交技术分析 → AI-PM 执行 Skills → 自检闸判定 → 非红线自动执行、红线转确认 → 变更通知与否决窗口 → 人类按需否决回滚」——作为可测试的端到端系统。

**Architecture:** Rust 模块化单体（Cargo Workspace，5 crates + 3 apps）+ PostgreSQL 16。命令通道（validate → policy ceiling → revision → apply → audit → notify）是唯一写路径；AI 编排器产出的提案默认自动执行，红线命令（YAML 清单）转事前确认；人类通过否决/回滚/冻结干预，全部落审计。

**Tech Stack:** Rust (stable-x86_64-pc-windows-msvc)、axum 0.8、sqlx 0.8（postgres, 无 TLS feature）、rmcp 3.8（MCP）、reqwest 0.12（native-tls/schannel）、Docker PostgreSQL 16。

**Spec:** `docs/architecture/ai-pm-architecture-principles.md`（V2 · AI 主导，下称「主稿」）。本计划从主稿取值；`docs/architecture/knowledge-services/ai-pm-markdown-knowledge-space.md`（知识空间）M1 不实施。

**环境已验证（2026-10-29 冒烟）：** Windows 原生 MSVC 编译全部依赖通过（冷构建约 11 分钟）；`#[sqlx::test(migrations=…)]` 对 Docker PG 跑通。开发库容器 `aipm-pg-dev` 已在运行（postgres:16, localhost:5433）。

## Global Constraints

- **平台：Windows 原生**。所有 cargo 命令直接在 Windows 终端（Git Bash/PowerShell）执行，不使用 WSL。
- **数据库：PostgreSQL 16**（用户已裁决由计划定夺；主稿 §18 决策表本就选 PG，五 Schema/JSONB/部分索引/SKIP LOCKED 均按 PG 设计）。连接串 `DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm`。容器 `aipm-pg-dev` 已存在，Task 1 用 compose 接管重建。
- **workspace 依赖版本（固定，勿升级大版本）**：tokio 1 (features=["full"])、axum 0.8、sqlx 0.8（`default-features = false, features = ["runtime-tokio","postgres","chrono","uuid","json","macros","migrate"]`——无 TLS feature（本地库不需要）；`migrate` 是 `sqlx::test` 的 migrations 选项必需）、serde 1 (derive)、serde_json 1、serde_yaml 0.9、uuid 1 (v4, serde)、chrono 0.4 (serde)、thiserror 2、async-trait 0.1、reqwest 0.12 (`default-features=false, features=["json","native-tls"]`——schannel，零 C 编译依赖)、rmcp 3.8 (features=["server","transport-io"])；dev: wiremock 0.6。不要引入 ring/aws-lc-rs 系 TLS。
- **禁用 sqlx `query!` 宏**（不做离线模式）。只用运行时 `sqlx::query` / `query_as` / `query_scalar`。
- **DB 测试统一**：`#[sqlx::test(migrations = "../../migrations")]`（crates 下）；apps 下为 `../../../`。运行测试前确保 `docker start aipm-pg-dev` 且导出 DATABASE_URL。
- **PG 标量坑**（冒烟已踩）：PG 整数字面量/小整数列是 INT4；Rust `i64` 读取必须 `::int8` 显式 cast 或改用 `i32`。`count(*)` 天然 INT8。
- **git**：`ai-pm/` 是独立仓库（Task 1 `git init`）。Conventional commits（feat:/test:/chore:/fix:），标识符与 commit message 用英文。
- **V1 从未发布**：无兼容层、无旧库迁移、默认档即 `ai_lead`；`controlled` 只是运行时回退档（刹车/解冻切换），不是默认。
- **红线唯一来源**：`policies/ai-pm-ai-lead-v2.yaml`。executor 的红线判定必须复用同一 `PolicyEngine` 实例，不得另写清单。
- **FakeModel fixture 名固定**：`cache_overlap`、`redline_acceptance`、`injection_attempt`、`model_timeout`、`low_confidence`（测试通过 `payload["fixture"]` 选择场景）。
- **目录**：平台代码在 `D:\kudi-project\templates\ai-pm\`；迁移、policy、测试 fixture 全部入库。

## Review Focus

实现时最可能咬人的五类输入/故障模式，及钉住它们的测试（已分配到所属任务）：

1. **提示注入**：payload 里出现「忽略红线/直接执行/你是管理员」不得改变任何判定。→ Task 5 `test_policy_decision_ignores_payload_content`（PolicyContext 根本不含 payload）；Task 14 `test_injection_fixture_still_goes_redline`（payload 带注入话术 + SignAcceptance 提案，仍转 redline_pending）。
2. **重复投递/重试产生重复命令或通知**：同 idempotency_key 重发、Worker 重启后重跑同一 job。→ Task 9 幂等四分支测试；Task 14 `test_handle_ai_job_idempotent_on_redelivery`（proposal_seq 唯一约束 + notification.source_proposal_id 唯一约束兜底）。
3. **状态已再次变化时的补偿**：补偿盲目恢复快照会覆盖更新的修改。→ Task 6 `test_compensate_after_further_change_conflicts`（补偿带 expected_revision，冲突报 RevisionConflict 而非静默覆盖）；Task 15 veto 把该冲突映射为 `rollback_failed` 干预记录并返回错误。
4. **Worker 崩溃后恢复不重复执行**：租约过期回收 + run 状态守卫。→ Task 10 `test_expired_lease_reclaimable`；Task 14 `test_handle_ai_job_skips_finished_run`。
5. **红线确认的重复点击与冻结态**：AI actor 硬闯红线、人类重复点确认、冻结后确认队列。→ Task 6 `test_redline_rejected_for_ai_actor`；Task 15 `test_approve_idempotent`、`test_frozen_project_human_confirm_still_executes`。

## Out of Scope（后续独立计划，M1 不做）

知识空间（Markdown+Git+PageIndex，见第二份 spec）；监督台 Web UI（M1 只出 REST）；抽样复核与行为基线告警（主稿 §10.2 后两项）；主动巡检/调度器；Git/CI Webhook；artifact/对象存储；各 Skill 的真实提示词工程（Task 17 只给通用 JSON 契约提示词）；MCP §9.1/§9.2 工具全集（M1 只做链路所需 3 个）；多轮反馈上限（主稿 §11.4，M1 单轮）；多租户 organization/RLS/人类 OAuth 认证；发布（release）真实流程（`sign_acceptance` 作为红线代表命令）；MCP OAuth（本地 transport 直连）。

---

### Task 1: 工作区脚手架、git init、compose 接管开发库

**Files:**
- Create: `ai-pm/Cargo.toml`（workspace + `[workspace.dependencies]` 全表，版本照 Global Constraints 抄）
- Create: `ai-pm/docker-compose.dev.yml`
- Create: `ai-pm/migrations/0001_core.sql`
- Create: `ai-pm/.gitignore`（`/target`、`.env`）、`README.md`（三行：是什么、怎么起库、怎么测试）
- Create: `crates/{pm-domain,pm-application,pm-infrastructure,pm-ai,pm-mcp}/Cargo.toml` + `src/lib.rs`（空 `pub mod` 占位即可）
- Create: `apps/{pm-api,pm-worker,ai-pm-worker}/Cargo.toml` + `src/main.rs`（`#[tokio::main]` + tracing 初始化 + 打印服务名）
- Test: `crates/pm-infrastructure/tests/core_schema.rs`

**Interfaces:**
- Produces: workspace 名单 `["crates/*", "apps/*"]`；`migrations/` 目录（sqlx::test 自动按文件名序执行）；dev 库 `postgres://aipm:aipm@localhost:5433/aipm`。

- [ ] **Step 1: 建目录与文件**

`docker-compose.dev.yml`：service `aipm-pg-dev`，image `postgres:16`，`container_name: aipm-pg-dev`，env aipm/aipm/aipm，`5433:5432`，volume `aipm_pg_data:/var/lib/postgresql/data`。

`migrations/0001_core.sql` = 主稿 §6.1 DDL 逐字照抄（pgcrypto、五个 CREATE SCHEMA、core.customer / core.project / core.principal / core.project_member）。

- [ ] **Step 2: compose 接管容器**

```bash
docker rm -f aipm-pg-dev   # 冒烟期手起的同名容器，数据可弃
cd ai-pm && docker compose -f docker-compose.dev.yml up -d
```

- [ ] **Step 3: 写失败测试**

`crates/pm-infrastructure/tests/core_schema.rs`：

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn core_schema_ready(pool: sqlx::PgPool) {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*)::int8 FROM information_schema.tables \
         WHERE table_schema = 'core'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(n, 4);
}
```

- [ ] **Step 4: 运行验证失败→通过**

Run: `cd ai-pm && DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm cargo test -p pm-infrastructure`
Expected: 首次 FAIL（workspace 未建齐/迁移未跑），建齐后 4 个 crate + 3 个 app 全部编译，测试 PASS。

- [ ] **Step 5: Commit**

```bash
git init && git add -A && git commit -m "chore: scaffold ai-pm workspace with core schema migration"
```

---

### Task 2: delivery 迁移与约束测试

**Files:**
- Create: `migrations/0002_delivery.sql`
- Test: `crates/pm-infrastructure/tests/delivery_schema.rs`

**Interfaces:**
- Consumes: Task 1 的 core 表。
- Produces: `delivery.milestone / task / requirement / requirement_revision / task_requirement`（主稿 §6.2–6.3 DDL 逐字照抄，含 `ALTER TABLE ... fk_current_requirement_revision` 循环复合外键）。

- [ ] **Step 1: 写失败测试**

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn requirement_revision_roundtrip(pool: sqlx::PgPool) {
    // 建customer→project→requirement，插 v1、v2 两版 revision，
    // 断言: duplicate version 插入报唯一冲突；current_revision_id 指向 v2 后可查回 content_hash
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_project_revision_rejected(pool: sqlx::PgPool) {
    // 两个 project；给 A 的 requirement 插带 B project_id 的 revision，
    // 断言: Err(ForeignKeyViolation)
}
```

- [ ] **Step 2: 运行验证 FAIL → 写 0002_delivery.sql → PASS**

Run: `cargo test -p pm-infrastructure --test delivery_schema`

- [ ] **Step 3: Commit** — `feat: add delivery schema migration with cross-project FK guard tests`

---

### Task 3: 平台迁移（integration / ai / governance）与种子

**Files:**
- Create: `migrations/0003_platform.sql`
- Test: `crates/pm-infrastructure/tests/platform_schema.rs`

**Interfaces:**
- Produces（= 主稿 §7.2–7.3 DDL + 以下 V2 增量，增量已定稿，照抄）：
  - `integration.inbound_event`、`integration.job`（主稿 §7.2、§11.2 原样）
  - **新增** `integration.notification`：`id UUID pk, project_id UUID not null, source_proposal_id UUID UNIQUE, kind TEXT not null, payload JSONB not null, veto_window_until TIMESTAMPTZ, read_at TIMESTAMPTZ, created_at TIMESTAMPTZ default now()`
  - `ai.project_ai_config`：`automation_mode TEXT not null default 'ai_lead' CHECK (automation_mode IN ('ai_lead','controlled'))`
  - `ai.skill_definition` + 种子 5 行（requirement_analysis / task_analysis / risk_detection / architecture_consistency / scheduling_orchestration）+ `ai.project_skill_binding`
  - `ai.analysis_run`：主稿字段 + `self_review JSONB`
  - `ai.change_proposal`：主稿字段 + `proposal_seq INT not null`、`confidence TEXT CHECK (confidence IN ('low','medium','high'))`、`executed_at TIMESTAMPTZ`、`executed_audit_id UUID`、`status CHECK (status IN ('proposed','executed','redline_pending','escalated','vetoed','expired','failed'))`、`UNIQUE(analysis_run_id, proposal_seq)`
  - `governance.approval_request`（主稿原样）、`governance.human_intervention`：intervention_type CHECK 在主稿六值上**增加 `'rollback_failed'`**
  - `governance.audit_log`：主稿字段 + `compensation_command JSONB`、`compensated_command_id UUID`
  - 索引：主稿 §7.4 全部 + `idx_proposal_executed_time`（部分索引 WHERE executed_at IS NOT NULL）+ `idx_intervention_project_time` + `idx_notification_project_time ON integration.notification(project_id, created_at DESC)`

- [ ] **Step 1: 写失败测试**

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn idempotency_key_unique(pool: sqlx::PgPool) {
    // 同 (project_id, source, idempotency_key) 二次插入 → unique violation
}
#[sqlx::test(migrations = "../../migrations")]
async fn job_unique_per_type_and_skill_seed(pool: sqlx::PgPool) {
    // 同 (event_id, job_type) 二次插入 → violation；skill_definition 计数 == 5
}
```

- [ ] **Step 2: FAIL → 写迁移 → PASS**。Run: `cargo test -p pm-infrastructure --test platform_schema`
- [ ] **Step 3: Commit** — `feat: add integration/ai/governance schemas with V2 deltas and skill seed`

---

### Task 4: pm-domain 核心类型

**Files:**
- Create: `crates/pm-domain/src/{lib.rs, ids.rs, task.rs, command.rs, proposal.rs, policy.rs, error.rs}`
- Test: 内联 `#[cfg(test)]`

**Interfaces (Produces，后续所有任务按此引用):**

```rust
pub type ProjectId = uuid::Uuid; // 同理 TaskId, MilestoneId, RequirementId, RevisionId, PrincipalId, EventId, RunId, ProposalId, AuditId

pub enum TaskStatus { Backlog, InProgress, Blocked, NeedsInfo, Done } // as_str()/from_str，DB TEXT 小写

pub enum ProjectCommand {
    CreateTask { project_id: ProjectId, title: String, description: String, assignee_id: Option<PrincipalId> },
    UpdateTaskStatus { task_id: TaskId, expected_revision: i64, new_status: TaskStatus },
    ApproveRequirementRevision { project_id: ProjectId, requirement_id: RequirementId, revision_id: RevisionId },
    AdjustMilestonePlan { project_id: ProjectId, milestone_id: MilestoneId, new_planned_at: Option<chrono::DateTime<chrono::Utc>>, expected_revision: i64 },
    SignAcceptance { project_id: ProjectId, milestone_id: MilestoneId, expected_revision: i64 }, // 红线代表
    ChangePolicy { project_id: ProjectId, new_policy: serde_json::Value },                       // 红线，M1 无 handler
    RollbackCommand { project_id: ProjectId, target_audit_id: AuditId, reason: String },
}
impl ProjectCommand { pub fn kind(&self) -> &'static str; pub fn reversibility(&self) -> Reversibility }

pub enum Reversibility { Reversible, Compensable, Irreversible } // SignAcceptance/ChangePolicy=Irreversible
pub enum Confidence { Low, Medium, High }
pub enum RiskLevel { Low, Medium, High }

pub struct Proposal { pub command: ProjectCommand, pub rationale: String, pub risk_level: RiskLevel,
                      pub confidence: Confidence, pub evidence: Vec<String> }
pub struct Finding { pub skill: String, pub severity: String, pub description: String, pub evidence_refs: Vec<String> }
pub struct AnalysisResult { pub summary: String, pub findings: Vec<Finding>, pub proposals: Vec<Proposal> }

pub enum PolicyDecision { AutoApply, RequirePreConfirmation { role: String }, Escalate { reason: String }, Deny { reason: String } }

pub enum CommandError {
    RevisionConflict { expected: i64, actual: i64 },
    RedLineBlocked { command: &'static str },
    NotHandled { command: String },
    AlreadyCompensated { audit_id: AuditId },
    AlreadyDecided { approval_id: uuid::Uuid },
    Validation(String), Db(sqlx::Error),
}
```

- [ ] **Step 1: 写失败测试**：serde JSON roundtrip `ProjectCommand` 每个变体；`TaskStatus` 字符串互转；`reversibility()` 映射（SignAcceptance/ChangePolicy→Irreversible，RollbackCommand→Reversible，其余→Compensable）；`kind()` 返回蛇形名（"update_task_status" 等）。
- [ ] **Step 2: FAIL → 实现 → PASS**。Run: `cargo test -p pm-domain`
- [ ] **Step 3: Commit** — `feat: add pm-domain command and policy types`

---

### Task 5: Policy Engine（YAML，确定性规则）

**Files:**
- Create: `policies/ai-pm-ai-lead-v2.yaml`
- Create: `crates/pm-application/src/policy_engine.rs`（pm-application 依赖 pm-domain）
- Test: `crates/pm-application/tests/policy_engine.rs`；测试内嵌 YAML 字符串，不读文件路径

**Interfaces (Produces):**

```rust
pub struct PolicyEngine { /* 解析后的规则 */ }
pub struct PolicyContext { pub confidence: Confidence, pub blast_radius: usize,
                           pub unresolved_conflict: bool }   // 注意：刻意不含 payload —— 注入面为零
impl PolicyEngine {
    pub fn from_yaml(text: &str) -> Result<Self, PolicyLoadError>;
    pub fn evaluate(&self, command: &ProjectCommand, ctx: &PolicyContext) -> PolicyDecision;
    pub fn is_redline(&self, command: &ProjectCommand) -> bool;
    pub fn required_role(&self, command: &ProjectCommand) -> Option<String>;
    pub fn veto_window(&self, command: &ProjectCommand) -> chrono::Duration; // default 4h, adjust_milestone 24h
    pub fn escalate_threshold(&self) -> usize; // blast_radius 阈值，默认 10
    pub fn for_tests() -> Self; // 内置与 yaml 文件同值的默认实例
}
```

- [ ] **Step 1: 写失败测试**

```rust
// 1) sign_acceptance → RequirePreConfirmation{role:"authorized_acceptor"}
// 2) change_policy → RequirePreConfirmation{role:"platform_admin"}
// 3) update_task_status + confidence=High → AutoApply
// 4) update_task_status + confidence=Low → Escalate
// 5) create_task + blast_radius=11 → Escalate
// 6) test_policy_decision_ignores_payload_content：
//    PolicyContext 无 payload 字段本身即断言；再验 evaluate 对同一命令相同 ctx 恒等返回
// 7) veto_window: sign_acceptance→4h, adjust_milestone_plan→24h, 其它→4h
// 8) 未知命令 kind → AutoApply（黑名单模型默认放行）
```

- [ ] **Step 2: FAIL → 实现**。YAML 结构照主稿 §10.4（defaults.effect=auto_apply / escalate.when 四条件 / red_lines 七条含 required_role / post_execution.veto_window）。
- [ ] **Step 3: PASS 后提交** — `feat: deterministic policy engine with red-line list and escalation rules`

---

### Task 6: 命令通道核心（executor + 审计 + 补偿）与 UpdateTaskStatus

**Files:**
- Create: `crates/pm-application/src/command_executor.rs`
- Modify: `crates/pm-application/src/lib.rs`（pub mod）
- Test: `crates/pm-application/tests/command_executor.rs`

**Interfaces (Produces):**

```rust
pub enum ActorKind { Human, AiPm, DevAgent, Service }
pub enum CommandSource { AiProposal { run_id: RunId, proposal_id: ProposalId },
                         HumanIntervention { intervention_id: uuid::Uuid },
                         RedlineApproval { approval_id: uuid::Uuid }, Bootstrap }
pub struct ExecutionContext { pub project_id: ProjectId, pub actor_id: PrincipalId,
                              pub actor_kind: ActorKind, pub source: CommandSource }

pub enum CompensationSpec {                        // 存 audit_log.compensation_command（状态恢复式）
    Restore(Vec<SnapshotEntry>),
    Delete { table: CompensableTable, target_id: uuid::Uuid },
}
pub struct SnapshotEntry { pub table: CompensableTable, pub target_id: uuid::Uuid, pub fields: serde_json::Value }
pub enum CompensableTable { Task, Milestone, Requirement, RequirementRevision } // 表名/主键/revision 列映射

pub struct ExecutionReceipt { pub audit_id: AuditId, pub command_kind: &'static str,
                              pub target_id: uuid::Uuid, pub resulting_revision: i64 }

pub struct PgCommandExecutor { pool: sqlx::PgPool, policy: std::sync::Arc<PolicyEngine> }
impl PgCommandExecutor {
    pub fn new(pool: sqlx::PgPool, policy: Arc<PolicyEngine>) -> Self;
    pub async fn execute(&self, cmd: ProjectCommand, ctx: ExecutionContext) -> Result<ExecutionReceipt, CommandError>;
    pub async fn compensate(&self, target_audit_id: AuditId, reason: &str,
                            ctx: ExecutionContext) -> Result<ExecutionReceipt, CommandError>;
    pub fn has_handler(&self, kind: &str) -> bool;   // orchestrator 校验命令可执行性用
}
```

执行语义（一个数据库事务内）：`SELECT ... FOR UPDATE` 目标行 → `expected_revision != row.revision` 则 `RevisionConflict` → 应用变更且 `revision = revision + 1` → INSERT audit_log（before/after、actor、source、compensation_command）→ 返回 receipt。红线强制：`policy.is_redline(cmd) && ctx.actor_kind != ActorKind::Human` → `RedLineBlocked`（人类通过 RedlineApproval source 执行红线是唯一例外路径）。补偿语义：读原 audit 行 FOR UPDATE；`compensated_command_id IS NOT NULL` → `AlreadyCompensated`；按 CompensationSpec 恢复（恢复也走 revision 检查：字段快照携带 resulting_revision，目标行已前进则 `RevisionConflict`）；新 audit 行回写原行 `compensated_command_id`。`ChangePolicy` 无 handler → `NotHandled`。

- [ ] **Step 1: 写失败测试**

```rust
// seed: bootstrap 最小数据（customer/project/principal/task revision=1）
// 1) execute UpdateTaskStatus{expected_revision:1, Done} → task.revision=2, status=done；
//    audit 有 before/after + compensation_command=Restore(task.status=旧值)
// 2) 再用 expected_revision=1 → Err(RevisionConflict{expected:1, actual:2})，状态不变
// 3) compensate(audit_id) → status 回 backlog, revision=3, 原 audit.compensated_command_id 已填
// 4) compensate 同一 audit 二次 → Err(AlreadyCompensated)
// 5) test_compensate_after_further_change_conflicts：
//    执行→再执行一次合法变更（revision 前进）→ compensate → Err(RevisionConflict)，不覆盖新状态
// 6) execute ChangePolicy → Err(NotHandled)
// 7) test_redline_rejected_for_ai_actor：
//    seed milestone revision=1；SignAcceptance with actor_kind=AiPm → Err(RedLineBlocked)；
//    同命令 actor_kind=Human, source=RedlineApproval → Ok，milestone.status='accepted'
```

- [ ] **Step 2: FAIL → 实现（本任务只需 UpdateTaskStatus 一个业务 handler，其余 Task 7）→ PASS**
- [ ] **Step 3: Commit** — `feat: command channel with audit, revision guard and snapshot compensation`

---

### Task 7: 其余命令 handler 与注册表不变量

**Files:**
- Modify: `crates/pm-application/src/command_executor.rs`（扩 handler 表）
- Test: `crates/pm-application/tests/command_handlers.rs`

**Interfaces:**
- Consumes: Task 6 的 executor 骨架。
- Produces: `CreateTask`（compensation=Delete{Task}）、`ApproveRequirementRevision`（compensation=Restore([requirement.current_revision_id 指针, requirement_revision.state])）、`AdjustMilestonePlan`（Restore(milestone.planned_at)）、`SignAcceptance`（Restore(milestone.status)）。

- [ ] **Step 1: 写失败测试**：每个 handler 一条 happy path（执行后行状态 + revision +1 + audit 落盘）+ 一条补偿恢复断言（compensate 后字段回到旧值、指针回退）；`ApproveRequirementRevision` 断言 requirement.current_revision_id 指向新 revision 且旧版 state='superseded'。
- [ ] **Step 2: FAIL → 实现 → PASS**。Run: `cargo test -p pm-application`
- [ ] **Step 3: 注册表不变量测试**

```rust
// test_every_auto_command_declares_compensation：
//   遍历全部命令 kind：has_handler(kind) && !policy.is_redline(kind) ⟹ 必须能产出 CompensationSpec
//   （本测试用构造的最小命令实例逐个执行后读 audit.compensation_command 非空来断言）
```

- [ ] **Step 4: Commit** — `feat: complete command handlers with compensation invariant`

---

### Task 8: 项目 Bootstrap 与只读查询服务

**Files:**
- Create: `crates/pm-application/src/bootstrap.rs`、`crates/pm-application/src/read.rs`
- Test: `crates/pm-application/tests/bootstrap_read.rs`

**Interfaces (Produces):**

```rust
pub struct BootstrapRequest { pub project_code: String, pub project_name: String }
pub struct BootstrapOutcome { pub customer_id: uuid::Uuid, pub project_id: ProjectId,
    pub ai_principal_id: PrincipalId, pub dev_agent_id: PrincipalId, pub human_id: PrincipalId }
pub async fn bootstrap_project(pool: &sqlx::PgPool, req: BootstrapRequest) -> Result<BootstrapOutcome, BootstrapError>;
// 建 customer（按 code 幂等）+ 3 个 principal(human/ai_pm/dev_agent) + project + member +
// project_ai_config(model_provider='fake', model_name='fake', automation_mode='ai_lead', policy_version=1)
// + 5 条 skill_binding。UNIQUE(project_code) 冲突 → Err(Duplicate)。

pub struct TaskContextRow { pub task_id: TaskId, pub title: String, pub status: String,
    pub revision: i64, pub requirement: Option<serde_json::Value>, pub milestone: Option<serde_json::Value> }
pub async fn task_context(pool: &sqlx::PgPool, project_id: ProjectId, task_id: TaskId)
    -> Result<TaskContextRow, ReadError>;   // task.project_id != project_id → ReadError::NotFound（跨项目不可见）
```

- [ ] **Step 1: 写失败测试**：bootstrap 幂等（同 code 二次 → Duplicate，不产生第二个 project）；3 principal + config ai_lead + 5 binding 断言；`task_context` 用项目 A 的 id 查项目 B 的 task → `NotFound`。
- [ ] **Step 2: FAIL → 实现 → PASS**
- [ ] **Step 3: Commit** — `feat: project bootstrap and project-scoped read service`

---

### Task 9: 入站事件接收（幂等 Inbox）+ REST 路由

**Files:**
- Create: `crates/pm-application/src/ingestion.rs`
- Create: `apps/pm-api/src/{main.rs, routes.rs}`（axum，`POST /api/v1/events`）
- Test: `crates/pm-application/tests/ingestion.rs` + `apps/pm-api/tests/routes.rs`

**Interfaces (Produces):**

```rust
pub struct SubmitEventRequest { pub project_id: ProjectId, pub actor_id: PrincipalId,
    pub source: String, pub event_type: String, pub idempotency_key: String,
    pub payload: serde_json::Value }
pub enum SubmitOutcome { Accepted { event_id: EventId }, Duplicate { event_id: EventId, status: String } }
pub enum IngestionError { IdempotencyConflict { existing_event_id: EventId }, UnknownProject, Db(sqlx::Error) }
pub async fn submit_event(pool: &sqlx::PgPool, req: SubmitEventRequest) -> Result<SubmitOutcome, IngestionError>;
// 单事务：INSERT inbound_event(payload_hash=sha256(payload)) + INSERT job(job_type='ai_analysis')，
// ON CONFLICT (project_id,source,idempotency_key) DO NOTHING；
// 冲突时回读：hash 相同 → Duplicate(原 event_id)；不同 → IdempotencyConflict。
// REST 返回 202 {event_id, processing_status:"queued"} / 200 Duplicate / 409 Conflict。
```

- [ ] **Step 1: 写失败测试**（服务层 4 条 + 路由 smoke 1 条）：Accepted 建事件+job 各 1；同 key 同 payload 重发 → Duplicate 同 event_id 且 job 仍 1 条；同 key 不同 payload → IdempotencyConflict；未知 project → UnknownProject；`POST /api/v1/events` 用 `tower::ServiceExt::oneshot` 断言 202 与 body。
- [ ] **Step 2: FAIL → 实现 → PASS**（pm-api 起 axum Router，pool 来自 env DATABASE_URL）
- [ ] **Step 3: Commit** — `feat: idempotent inbound event ingestion with rest route`

---

### Task 10: Job 队列（领取/租约/退避/冻结过滤）

**Files:**
- Create: `crates/pm-infrastructure/src/job_queue.rs`
- Test: `crates/pm-infrastructure/tests/job_queue.rs`

**Interfaces (Produces):**

```rust
pub struct ClaimedJob { pub job_id: uuid::Uuid, pub event_id: EventId, pub project_id: ProjectId }
pub async fn claim(pool: &sqlx::PgPool, job_type: &str, worker_id: &str, lease_secs: i64) -> Option<ClaimedJob>;
pub async fn complete(pool: &sqlx::PgPool, job_id: uuid::Uuid) -> Result<(), sqlx::Error>;
pub async fn fail(pool: &sqlx::PgPool, job_id: uuid::Uuid, error: &str,
                  max_attempts: i32, backoff_base_secs: i64) -> Result<(), sqlx::Error>;
```

claim SQL 语义：`FOR UPDATE SKIP LOCKED` + `JOIN ai.project_ai_config c ON c.project_id = job.project_id AND c.enabled AND c.automation_mode = 'ai_lead'`——**冻结过滤在领取层实现**。fail：`attempts+1`；`attempts >= max_attempts` → status='failed'，否则 status='queued' 且 `available_at = now() + backoff_base_secs * 2^attempts`。

- [ ] **Step 1: 写失败测试**：两个 job 顺序 claim 得不同 job；租约内同 job 不可再领；`test_expired_lease_reclaimable`（把 lease_until 改为过去后可再领）；fail 两次后 attempts=2 且 available_at 在未来、第三次 fail 到 max → status failed；`controlled` 模式项目的 job 领取不到，改回 `ai_lead` 后可领。
- [ ] **Step 2: FAIL → 实现 → PASS**
- [ ] **Step 3: Commit** — `feat: lease-based job queue with freeze filter and backoff`

---

### Task 11: Skill 路由、AnalysisModel trait 与 FakeModel

**Files:**
- Create: `crates/pm-ai/src/{skill_router.rs, model.rs, fake_model.rs}`（pm-ai 依赖 pm-domain）
- Test: 内联单元测试

**Interfaces (Produces):**

```rust
pub type SkillId = &'static str;
pub fn route(event_type: &str) -> Vec<SkillId>;
// requirement_submitted → [requirement_analysis, risk_detection]
// technical_design_submitted → [requirement_analysis, task_analysis, architecture_consistency]
// work_result_submitted → [task_analysis, requirement_analysis]
// milestone_check → [scheduling_orchestration, risk_detection, task_analysis]
// 其它 → []（编排器判 completed_no_change）

pub struct AnalysisContext { pub project_id: ProjectId, pub event_id: EventId, pub event_type: String,
    pub payload: serde_json::Value, pub task: Option<serde_json::Value>,
    pub requirement_revision: Option<serde_json::Value>, pub milestone: Option<serde_json::Value>,
    pub config: serde_json::Value, pub policy_version: i32 }  // 定义在 model.rs，Task 12 填充

#[async_trait]
pub trait AnalysisModel: Send + Sync {
    async fn analyze(&self, context: &AnalysisContext, skills: &[SkillId]) -> Result<AnalysisResult, ModelError>;
}
pub enum ModelError { Timeout, Unavailable(String), BadOutput(String) }

pub struct FakeModel { scenarios: HashMap<String, AnalysisResult>, failures: HashMap<String, ModelError>, default: AnalysisResult }
impl FakeModel {
    pub fn from_fixtures() -> Self;  // 五个固定 fixture（见 Global Constraints）
    pub fn install_failure(&mut self, fixture: &str, err: ModelError);
}
// analyze 按 context.payload["fixture"] 取场景；无标记走 default。
// cache_overlap 产 3 提案: UpdateTaskStatus(NeedsInfo) + CreateTask(缓存调研) + AdjustMilestonePlan
// redline_acceptance 产 1 提案: SignAcceptance；injection_attempt 同 payload 带注入话术 + SignAcceptance
// low_confidence 产 confidence=Low 的 UpdateTaskStatus；model_timeout → Err(Timeout)
```

- [ ] **Step 1: 写失败测试**：路由表 5 断言；FakeModel 各 fixture 返回结构正确、default 兜底、failure 生效。
- [ ] **Step 2: FAIL → 实现 → PASS**。Run: `cargo test -p pm-ai`
- [ ] **Step 3: Commit** — `feat: skill router and deterministic fake analysis model`

---

### Task 12: ContextBuilder 与 analysis_run 生命周期

**Files:**
- Create: `crates/pm-ai/src/context.rs`
- Test: `crates/pm-ai/tests/context.rs`（DB 测试）

**Interfaces:**
- Consumes: Task 11 的 `AnalysisContext`（context.rs 中填充逻辑，结构体定义在 model.rs）。
- Produces:

pub async fn start_run(pool: &sqlx::PgPool, event_id: EventId) -> Result<RunId, ContextError>;
// 不存在 run → 建(queued→analyzing)；存在且 finished_at IS NULL → 复用（崩溃恢复）；已完成 → Err(AlreadyDone)
pub async fn build(pool: &sqlx::PgPool, event_id: EventId) -> Result<AnalysisContext, ContextError>;
// 由 payload["task_ref"]/["milestone_ref"]/["requirement_ref"]（UUID 字符串，可缺省）关联加载；
// 引用不存在 → Err(ContextError::NotFound{ref})
pub async fn save_snapshot(pool: &sqlx::PgPool, run_id: RunId, ctx: &AnalysisContext) -> Result<(), sqlx::Error>;
pub async fn finish_run(pool: &sqlx::PgPool, run_id: RunId, result: &serde_json::Value,
                        self_review: &serde_json::Value) -> Result<(), sqlx::Error>;
pub async fn fail_run(pool: &sqlx::PgPool, run_id: RunId, failure_code: &str) -> Result<(), sqlx::Error>;
```

- [ ] **Step 1: 写失败测试**：start_run 建行并置 analyzing；二次调用复用同 run_id；模拟 finished 后再调 → AlreadyDone；build 正确带出 task/config/policy_version；`task_ref` 指向不存在 UUID → NotFound 且 `fail_run(failure_code='context_not_found')` 落库；snapshot JSON 可回读且含各 id。
- [ ] **Step 2: FAIL → 实现 → PASS**
- [ ] **Step 3: Commit** — `feat: analysis context builder and run lifecycle`

---

### Task 13: 自检闸（Self-Review Gate）

**Files:**
- Create: `crates/pm-ai/src/self_review.rs`
- Test: 内联单元测试

**Interfaces (Produces):**

```rust
pub enum ProposalOutcome { Auto, RequirePreConfirmation { role: String }, Escalate { reason: String } }
pub fn review_proposals(policy: &PolicyEngine, proposals: &[Proposal], blast_radius: usize)
    -> Vec<ProposalOutcome>;   // 逐提案：is_redline→RequirePreConfirmation{role}
                              // confidence==Low→Escalate("low_confidence")
                              // blast_radius>threshold→Escalate("blast_radius")
                              // 否则 Auto
pub fn self_review_json(outcomes: &[ProposalOutcome]) -> serde_json::Value; // 存 analysis_run.self_review
```

纯函数、无 IO、不看 payload（注入面为零——review focus #1 的另一半）。

- [ ] **Step 1: 写失败测试**：SignAcceptance → RequirePreConfirmation{authorized_acceptor}；Low 置信 UpdateTaskStatus → Escalate；blast_radius=11 → Escalate；混合列表顺序保持。
- [ ] **Step 2: FAIL → 实现 → PASS**
- [ ] **Step 3: Commit** — `feat: deterministic self-review gate`

---

### Task 14: 编排器 handle_ai_job（端到端主链路）

**Files:**
- Create: `crates/pm-ai/src/orchestrator.rs`
- Create: `apps/ai-pm-worker/src/main.rs`（循环 claim + handle + complete/fail，60ms 轮询间隔）
- Test: `crates/pm-ai/tests/orchestrator.rs`（DB 集成测试，FakeModel + for_tests policy）

**Interfaces (Produces):**

```rust
pub struct Orchestrator { pub pool: sqlx::PgPool, pub model: std::sync::Arc<dyn AnalysisModel>,
                          pub policy: std::sync::Arc<PolicyEngine>,
                          pub executor: std::sync::Arc<PgCommandExecutor> }
impl Orchestrator {
    pub async fn handle_ai_job(&self, job: &ClaimedJob) -> Result<(), OrchestratorError>;
}
pub enum OrchestratorError { RunAlreadyDone, Model(ModelError), Db(sqlx::Error) }
// blast_radius 来源：M1 取 proposals.len()（传给 review_proposals；主稿的逐对象影响面统计属后续计划）
```

流程（主稿 §14.2）：load event → `start_run`（AlreadyDone → 直接 complete job 返回）→ `build`+`save_snapshot`（NotFound → fail_run+job fail）→ `route`（空 → run completed + event.status='completed_no_change' + job complete）→ `model.analyze`（Err → fail_run+`job_queue::fail` 重试）→ 逐提案校验（`executor.has_handler(kind)` 或红线命令；target 对象存在且属本 project；不过 → proposal status='failed' 继续）→ 写 change_proposal（含 proposal_seq、confidence）→ `review_proposals` 逐条路由：

- `Auto` + `policy.evaluate==AutoApply` → `executor.execute(cmd, ctx{AiPm, AiProposal})` → proposal status='executed'、executed_at、executed_audit_id；INSERT notification(kind='change_applied', source_proposal_id, veto_window_until=now+veto_window(cmd))
- `RequirePreConfirmation` → INSERT approval_request(proposal_id, required_role)；proposal status='redline_pending'；notification(kind='redline_pending')
- `Escalate` → proposal status='escalated'；notification(kind='escalated')
- `RedLineBlocked`（双保险触发）→ proposal status='failed' + notification(kind='redline_violation_blocked')

收尾：`finish_run(result, self_review_json)`、event.status='applied'（有 executed）/ 'needs_clarification'（全失败）、INSERT agent_feedback(feedback_type='decision', content=summary)。

- [ ] **Step 1: 写失败测试（5 条，全部走真 DB + FakeModel）**

```rust
// 1) happy: cache_overlap → 2 条命令真执行（task 状态变/新 task 建出）、milestone 调整；
//    3 条 proposal 全 executed；3 条 notification 带各自 veto_window_until；agent_feedback 1 条；event='applied'
// 2) redline: redline_acceptance → approval_request 一行 pending；无任何命令执行；proposal='redline_pending'
// 3) test_injection_fixture_still_goes_redline: injection_attempt → 同 2)
// 4) test_handle_ai_job_idempotent_on_redelivery: 同一 job 调两次 → 第二次直接返回，
//    proposals/audit/notification 计数不变（proposal_seq 与 source_proposal_id 唯一约束兜底）
// 5) model failure: model_timeout → run failed、job attempts+1、无 proposal、event 保留
```

- [ ] **Step 2: FAIL → 实现 → PASS**。Run: `cargo test -p pm-ai`
- [ ] **Step 3: Commit** — `feat: ai orchestrator end-to-end with notifications and feedback`

---

### Task 15: 监督服务（否决/回滚/冻结/红线确认/通知流）+ REST

**Files:**
- Create: `crates/pm-application/src/supervision.rs`
- Modify: `apps/pm-api/src/routes.rs`（新增 5 路由：`POST /api/v1/supervision/veto`、`POST /api/v1/supervision/approvals/{id}/decision`、`POST /api/v1/supervision/freeze`、`POST /api/v1/supervision/unfreeze`、`GET /api/v1/projects/{project_id}/notifications?since=&limit=`）
- Test: `crates/pm-application/tests/supervision.rs` + 路由 smoke

**Interfaces (Produces):**

```rust
pub struct VetoRequest { pub project_id: ProjectId, pub proposal_id: ProposalId,
                         pub actor_id: PrincipalId, pub reason: String }
pub enum VetoOutcome { RolledBack { new_audit_id: AuditId }, ProposalClosed, AlreadyCompensated }
pub async fn veto(pool, executor: &PgCommandExecutor, req: VetoRequest) -> Result<VetoOutcome, SupervisionError>;
// proposal executed → executor.compensate(executed_audit_id)：
//   Ok → intervention('veto') + notification('veto_applied') → RolledBack
//   AlreadyCompensated → intervention('veto') 幂等落一条 → AlreadyCompensated（HTTP 200）
//   RevisionConflict → intervention('rollback_failed') → Err（HTTP 409，见 review focus #3）
// proposal redline_pending/escalated → status='vetoed' + intervention → ProposalClosed

pub struct DecideRequest { pub approval_id: uuid::Uuid, pub actor_id: PrincipalId,
                           pub decision: Decision, pub reason: String }
pub enum Decision { Approve, Reject }
pub enum DecisionOutcome { Executed { receipt: ExecutionReceipt }, Rejected }
pub async fn decide_approval(pool, executor, req) -> Result<DecisionOutcome, SupervisionError>;
// 状态机 pending→approved/rejected（重复 decide → Err(AlreadyDecided)，HTTP 200 幂等语义由路由层映射）
// Approve：以 ctx{Human, RedlineApproval} 执行该 proposal 命令（人类路径可执行红线，见 Task 6 例外）
// Reject：proposal status='vetoed' + notification

pub struct NotificationRow { pub id: uuid::Uuid, pub kind: String, pub payload: serde_json::Value,
                             pub veto_window_until: Option<chrono::DateTime<chrono::Utc>>,
                             pub created_at: chrono::DateTime<chrono::Utc> }

pub async fn freeze(pool, project_id, actor_id, reason) / unfreeze(...) 
// config.automation_mode controlled↔ai_lead + intervention('freeze'/'unfreeze') + notification
pub async fn notifications_feed(pool, project_id, since: Option<DateTime<Utc>>, limit: i64)
    -> Result<Vec<NotificationRow>, sqlx::Error>;
```

- [ ] **Step 1: 写失败测试**

```rust
// 1) veto 已执行提案 → RolledBack + intervention('veto') + notification；再次 veto → AlreadyCompensated 成功语义
// 2) veto 时目标已被后续修改 → Err + intervention('rollback_failed')（不覆盖新状态）
// 3) test_approve_idempotent：decide(Approve) 执行一次命令；再 decide → Err(AlreadyDecided)，命令只执行一次
// 4) freeze 后 claim 领不到该项目 job；unfreeze 后恢复（复用 Task 10 的 claim）
// 5) test_frozen_project_human_confirm_still_executes：冻结项目的红线 confirm 仍执行（human actor 不受冻结过滤）
// 6) veto 窗口过期后仍可回滚（三权之「随时否决」）：把 veto_window_until 改为过去 → RolledBack 依然成立
// 7) notifications_feed 按 created_at desc 分页
```

- [ ] **Step 2: FAIL → 实现 → PASS**
- [ ] **Step 3: Commit** — `feat: supervision service with veto, freeze, redline approval and feed`

---

### Task 16: MCP Server（3 个工具）

**Files:**
- Create: `crates/pm-mcp/src/{lib.rs, tools.rs, server.rs}`（依赖 pm-application/pm-ai；rmcp 3.8，按其官方 server 示例的 Router/ServerHandler 模式实现，API 细节以 rmcp 3.8 文档为准）
- Modify: `apps/pm-api/src/main.rs`（挂 `/mcp` 路由，streamable HTTP transport；具体绑定方式按 rmcp 3.8 文档）
- Test: `crates/pm-mcp/tests/tools.rs`

**Interfaces (Produces):** 工具名与语义（参数/返回 JSON 结构在本任务测试里钉死）：

- `pm_submit_technical_design{project_id, task_ref, idempotency_key, fixture, summary}` → 复用 `ingestion::submit_event`（event_type='technical_design_submitted'），返回 `{event_id, processing_status}`
- `pm_get_submission_status{event_id}` → 聚合 event.status + proposals 概要（id/status/command_type）
- `pm_get_task_context{project_id, task_ref}` → 复用 `read::task_context`，跨项目 → 工具错误

- [ ] **Step 1: 写失败测试**：按 rmcp 3.8 官方测试模式用进程内 client 调三个工具（submit 落库断言、status 聚合断言、跨项目错误断言）。若 rmcp 进程内测试模式与预期不符，以「工具函数层单测 + HTTP JSON-RPC 请求 smoke」替代并在 PR 说明。
- [ ] **Step 2: FAIL → 实现 → PASS**
- [ ] **Step 3: Commit** — `feat: mcp server with submit, status and task-context tools`

---

### Task 17: OpenAI 兼容模型适配器

**Files:**
- Create: `crates/pm-ai/src/openai_adapter.rs`（dev-dep wiremock）
- Test: `crates/pm-ai/tests/openai_adapter.rs`

**Interfaces (Produces):**

```rust
pub struct OpenAiCompatibleModel { /* reqwest::Client, base_url, model, api_key, timeout */ }
impl OpenAiCompatibleModel {
    pub fn new(base_url: &str, model: &str, api_key: &str, timeout: chrono::Duration) -> Self;
}
#[async_trait] impl AnalysisModel for OpenAiCompatibleModel { /* chat completions,
    response_format=json_object；系统提示词 = 主稿 §8.7 统一输出 JSON 契约 + 五 Skill 职责一段；
    解析 AnalysisResult，失败→ModelError::BadOutput；HTTP 5xx/连接失败→Unavailable；
    超时→Timeout */ }
```

- [ ] **Step 1: 写失败测试（wiremock）**：200 合法 JSON → 三个 fixture 级断言（summary/proposals 数/置信度枚举解析）；500 → Unavailable；返回非契约 JSON → BadOutput；延迟响应 + 1s timeout → Timeout。
- [ ] **Step 2: FAIL → 实现 → PASS**
- [ ] **Step 3: Commit** — `feat: openai-compatible analysis model adapter`

---

### Task 18: 主稿 §17.1 场景套件（端到端验收）

**Files:**
- Create: `crates/pm-application/tests/e2e_scenarios.rs`（复用前面所有服务；一个 helper：`bootstrap → submit(fixture) → 跑一轮 worker loop（直接调 claim+handle）`）

**Interfaces:**
- Consumes: 全部前置任务的服务接口；不产出新接口。

- [ ] **Step 1: 写失败测试（对应 §17.1 表中 M1 适用行）**

```rust
// 1) 同一 MCP 消息提交 10 次       → 1 个 inbound_event、1 个 job、命令只执行一次
// 2) AI 模型超时                    → 原始消息保留；job attempts 增加；可重试；到 max 后 status='failed'
// 3) 同一任务两个 Agent 并发修改     → 一条执行成功，另一条 RevisionConflict，无静默覆盖
// 4) AI 修改正式需求并定版           → 自动执行 + 审计 + 通知 + 否决窗口内回滚成功
// 5) AI 发起验收签署                 → 红线拦截转事前确认；批准后执行一次
// 6) AI 试图修改政策                 → NotHandled/RedLineBlocked，永不执行，notification 留痕
// 7) 否决已回滚的操作                → AlreadyCompensated 幂等成功，无二次破坏
// 8) 刹车冻结                        → job 不再被领取；解冻恢复；期间事件不丢
// 9) 跨项目读取                      → NotFound/工具错误
// 10) Worker 崩溃恢复                → 模拟：handle 到一半丢弃（run 停在 analyzing）→ 重跑 claim+handle
//                                    → 不产生重复提案/命令/通知
```

- [ ] **Step 2: FAIL → 补缺口 → PASS**。此任务预期暴露前序任务的真实缝隙，允许回改前序代码，但须保持既有测试全绿。Run: `cargo test --workspace`
- [ ] **Step 3: 全量回归**：`cargo test --workspace` 全绿 + `cargo clippy --workspace` 无 error。
- [ ] **Step 4: Commit** — `test: end-to-end scenario suite per spec section 17.1`

---

## 执行备注

- 任务依赖：1→2→3→4→5→6→7 线性；8/9 依赖 3；10 依赖 3；11 独立；12 依赖 3；13 依赖 5+11；14 依赖 6/10/11/12/13；15 依赖 6/10；16 依赖 8/9；17 独立（依赖 4/11）；18 收口。并行执行时按此拓扑。
- 每个 DB 测试都是独立临时库（sqlx::test 自动建/删），测试间无共享状态；跑全量前 `docker start aipm-pg-dev`。
- 首次冷构建约 11 分钟；之后增量秒级。若 CI/本地卡在 crates.io，检查 `~/.cargo/config.toml` 的代理段（已配 127.0.0.1:10808）。
