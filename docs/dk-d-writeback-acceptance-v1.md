# DK-D 实施验收记录 · 对话编排写回通道（需求草案 + 任务分解 draft）

- 日期：2026-11-08
- 分支：`feat/dk0-dialog-orchestration-dkd`（worktree `ai-pm-dk0d`）
- 基点：`72cb7d5`（D-DK-1~11 裁决记录回写完成，进实施基线 DK-A~F）
- 设计权威：`docs/product/ai-pm-dialog-orchestration-design-v1.md`（§3 / §6 / §7 / §8.3 B·E 组 / §13）
- 迁移：**零迁移**（`content.basis_finding_ids` 是 JSONB 子键；`purpose` 是自由 TEXT；`audit_log.source_kind` 是 TEXT）
- 红线核对：MCP 四工具零改动；V-6 单写入（编排器零直写业务表）；AI 无确认权四道闸全在；禁第二套问答基建（零新表）；`task_requirement` PK 改造未实施（S02-I4 范围）

---

## 一、交付形态（7 个 commit）

| # | commit | 内容 |
|---|---|---|
| 1 | `93b8e20` | `basis_finding_ids` 提案级来源标记：可选字段 + 三条硬边界（零迁移） |
| 2 | `961f881` | `WriteBackHook` 接口定义：trait / 端口 / 开关 / 白名单（本批为定义方） |
| 3 | `d0f0878` | 需求起草 hook：AI 主体九命令链，止步 `pending_confirmation` |
| 4 | `1c0962c` | 任务分解：三源骨架 + 上限/验收/依赖/角色四道系统校验 |
| 5 | `7633491` | 人类物化端点 + 整批 ready（E5）+ `draft→ready` 真人闸（E2） |
| 6 | `0dd63ac` | B1~B5 + E1~E5 全绿——越权/边界/夹具三组共 27 例 |
| 7 | 本文档 | DK-D 验收记录 |

### 1.1 接口定义方职责（§范围 1）

`crates/pm-ai/src/discovery_writeback.rs` 定义：

```rust
#[async_trait]
pub trait WriteBackHook: Send + Sync {
    async fn on_review_required(&self, tenant_id: Uuid, session: &DiscoverySessionView)
        -> Result<DraftOutcome, WriteBackError>;
    async fn decompose_if_approved(&self, tenant_id: Uuid, session: &DiscoverySessionView)
        -> Result<DecomposeOutcome, WriteBackError>;
}
```

- **DK-B 合并动作**：把 `NoWriteBack`（本批同时交付的空实现）换成 `DiscoveryWriteBack`，签名不变（后合者适配）。
- **一处必要偏差**：`tenant_id` 显式入参。`DiscoverySessionView` 不带租户列，而所有既有单点服务以 `tenant_id` 为第一边界参数——边界摆在签名上，不让 hook 自己反查。
- **开关唯一定义**：`WriteBackMode::from_env()` 读 `AI_PM_DISCOVERY_ORCHESTRATOR`（默认 off，拼错 WARN + off）。DK-B 的循环复用该入口，避免两处定义漂移。

### 1.2 三落点 → 既有单点服务（§6.1 V-6）

| 产物 | 唯一入口 | 本批改动 |
|---|---|---|
| 需求草案 | `requirement::create_requirement` / `create_draft` / `update_draft` / `submit_revision` | 请求体增可选 `basis_finding_ids`；服务层校验后写 `content` 子键 |
| 需求确认/拒绝/治理 | `confirm_revision` / `reject_revision` / `transition_lifecycle` | 零改动（`require_human` 原样） |
| 溯源关联 | `link_finding` / `register_source_input` / `link_source_input` | 零改动；新增 `materialize_basis` **组合**它们，不新开写入路径 |
| 任务 | `pm_write::create_task`（+ 可选 `acceptance_criteria`）、**新增** `link_task_requirement` | `task_requirement` 首次有单点服务（此前唯一写入在 executor 内联 SQL） |
| 任务依赖 | `task_dependency::add_dependency` | 零改动 |
| 入调度 | **新增** `pm_write::mark_task_batch_ready`（走 `PgCommandExecutor` → `apply_task_transition` + 同事务 `TaskReady`） | 补齐「人类整批确认 → 入调度」断链（见 §四.3） |

---

## 二、basis_finding_ids（D-DK-5 / §3.3）

**载体（零迁移）**：`requirement_revision.content.basis_finding_ids`（与 `markdown`/`summary` 并列的 JSONB 子键），修订视图以 `rv.content->'basis_finding_ids'` 透出。

**硬边界三条，任一失败 → 整条草案拒绝生成**（不留带病提案）：

1. id 必须存在于本 `(tenant_id, project_id)` 的 `discovery.discovery_finding`（跨项目/跨租户按不存在处理，不泄露存在性）；
2. `verification_status` 必须 `CONFIRMED`；
3. 清单必须落在一个 `discovery_session` 内。

> 第 3 条的落地方式：请求体**不引入 session 参数**（不扩大设计面），以「清单自身单一 session」表达「本 session 内」；跨 session 混合即拒绝。这是本批对设计稿的一处实施解释，登记在此。

**语义纪律**：非权威——不进 `requirement_finding`、不参与任何校验、AI 不得借此登记来源（S02D-20 不动）。正式溯源仍由真人一步物化。

**`update_draft` 三态**：`None` = 保留原标记（`content` 是全量重建的，不显式保留会静默丢）、`Some([])` = 显式清空、`Some(ids)` = 校验后替换。

---

## 三、需求起草 hook（§8.1 前半）

时序（AI 主体，`actor = ai.project_ai_config.ai_principal_id`）：

```text
create_requirement（含 basis） → update_draft（含 basis） → submit_revision（→ pending_confirmation）
```

- **到此止步**：`confirm` / `reject` / `transition_lifecycle` 不在白名单，服务层 `require_human` 兜底（第 1 道闸）；REST 拒 `api_key`（第 2 道）；MCP 不暴露（第 3 道）；白名单常量（第 4 道）。
- **幂等**：由「basis 交集」认定本 session 已有需求 → `AlreadyDrafted`，不重复建。交集扫**全部修订**而非仅最新（否则人类续改一次归属就丢，会重复建需求）。
- **前置跳过**（零写入）：开关 off / 会话非 `REVIEW_REQUIRED` / 本 session 无 `CONFIRMED` finding。
- **请求编号**：模型建议 → 归一化（大写、`[A-Z0-9-]`、折叠、截 32）→ 同项目重号顺延 `-2`/`-3`…，空则退化 `DISC-<session 前 8 位>`。
- **记账**：`purpose='discovery_orchestration'`（D-DK-11，常量零迁移）；端口返回 `ModelCallFailure { error, provider, latency_ms }`，**失败调用同样入账**（DEC-LLM-004.7），provider 未解析时只 WARN 不伪造 FK。

---

## 四、任务分解（§7 / D-DK-8）

### 4.1 触发与幂等

- 候选：本 session 起草（basis 交集）且 `lifecycle_status='active'` 的需求，取 **当前正式修订**（`current_revision_id`，只可能是 approved）。
- 排他：该 approved 修订已存在 `task_requirement` 行 → 本轮 `NothingToDo`（重放天然 no-op）。
- B5 前置门：任一候选未定稿 → `Err(RequirementNotApproved)`，文案含「请先在需求页确认」（fail-closed，不挑"能拆的那条"）。

### 4.2 三源（§7.1）

| 源 | 取法 | 本批状态 |
|---|---|---|
| 需求现状 | approved 修订 `content->>'markdown'` + `requirement_acceptance_criterion`（0056，按 position 序） | 完整 |
| PC / KB 能力面 | `CapabilityProjection` 端口（`CapabilityFacts { objects, digests }`） | **本批空实现** `EmptyCapabilityProjection`；DK-C 合并后替换（不碰 DK-C 的注入投影模块） |
| 角色候选 | `ai.ai_role_version`（本项目 enabled 的最新版本，`DISTINCT ON (role_id)`） | 完整 |

### 4.3 系统骨架 vs LLM 回填（§7.3）

系统定（`validate_task_batch`，纯函数 + 常量）：

| 规则 | 违规处理 |
|---|---|
| 前置任务 ≤ 3、主任务 ≤ 8 | **截断**（计数 `truncated`，WARN） |
| title 非空、批内 `key` 非空且唯一 | **整批拒绝** |
| 每个任务（含前置）至少一条非空验收口径 | **整批拒绝**（E4） |
| `depends_on` 只能指向批内**更早出现**的键 | **整批拒绝**（未知键/前向引用/自引用；天然无环） |
| `role_ref` 只能命中候选 `role_key` | **丢弃该建议**并计数（不报错、不编造） |

LLM 只回填：title / 描述 / 验收口径文本 / 依赖建议 / 角色建议。

**落库**：`create_task(status='draft')`（+ 验收口径同事务）→ `link_task_requirement`（E1 执法点）→ `add_dependency`（前置→主，E3）。**AI 永不置 ready**（E2）。

---

## 五、人类半步（D-DK-5 物化 / D-DK-8 整批确认）

### 5.1 一键物化

`POST /v1/pm/requirements/{requirement_id}/basis/materialize`

- 读最新修订的 basis → 逐条 `link_finding` → 登记并关联 `source_input(kind='discovery')`（一个访谈会话一条，按 `project + source_ref` 复用）。
- 首行 `require_human`；每个子服务各自也有真人闸 → AI 调用 403（B4）。
- 幂等：已关联进 `already_linked_findings`；`finding_ids` 只允许是清单子集（超出即拒绝，不猜意图）。

### 5.2 整批 `draft → ready`（E5）

`POST /v1/pm/projects/{project_id}/tasks/mark-ready` → `pm_write::mark_task_batch_ready`

**发现并补齐的一处真实断链**：`pm_write::update_task` 的直写**不生产 outbox 事件**，而 `TaskReady` 是入调度的唯一触发。若人类整批确认走 PATCH 直写，AI 生成的任务永远不会进调度——E5 在既有面上必然为 0 ≠ N。

修法（不新开状态写入路径）：整批确认走 `PgCommandExecutor::execute(UpdateTaskStatus)` → `apply_task_transition` + 同事务 `TaskReady`，因此「事件数 = 确认任务数」由结构保证。为此新增 `CommandSource::HumanAction`（`source_kind='human_action'`，零迁移）——人类动作不得伪装成 bootstrap/system。

### 5.3 `draft → ready` 真人闸（E2 的服务层那道）

`pm_write::update_task` 在 `draft → ready` 这条边上加 `require_human_conn`（同状态 no-op 不取闸）。为此把 `require_human` 抽出连接版（判定逻辑仍只有一处），供事务内复用。

**兼容性核查**：既有 `update_task` 到 ready 的两处用例（MCP contract、run_context）都是 `ready → ready` 的 no-op 直写，不被闸门拦截；REST 用例（`pm_writes`）主体是 `kind='human'`，照常通过。

---

## 六、验收清单逐条对照

### B 组（需求落点）

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| B1 | AI 调 `confirm_revision` → Forbidden，`requirement_confirmation` 零新增 | **PASS** | `ai_principal_cannot_confirm_link_or_materialize`（含 lifecycle 同拒） |
| B2 | 编排器产出终止于 `pending_confirmation`；`approved` 只由 Approver 真人产生 | **PASS** | `draft_stops_at_pending_then_decompose_pins_approved_revision`（回包状态 + 修订行状态 + 人类 confirm 后才 approved） |
| B3 | 澄清（finding CONFIRMED）不产生任何 requirement 行；approved 不由 finding 推导 | **PASS** | `confirmed_finding_alone_creates_no_requirement`（含走真 `record_finding_verification` 的一例） |
| B4 | 溯源关联只能真人产生（AI `link_finding` → Forbidden），UI 以建议清单呈现 | **PASS** | 同上 + `materialize_basis` 403、`requirement_finding` 零行 |
| B5 | 未定稿需求下调任务生成 → 拒绝，文案指向「请先在需求页确认」 | **PASS** | `draft_stops…`（断言错误文案）+ `RequirementNotApproved` 变体 |

### E 组（写回与任务）

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| E1 | `task_requirement.revision_id` 指向 `state='approved'`（DB 断言） | **PASS** | 3 行钉定全部 approved；`link_task_requirement` 对非 approved 修订返回 Validation |
| E2 | AI 生成任务全 `draft`；`ready` 只由人类产生 | **PASS** | 3/3 draft；AI 批量确认 403；AI PATCH→ready 403 |
| E3 | 前置→主依赖落 `task_dependency`，非标题字符串 | **PASS** | 边表 2 条且方向为主→前置；依赖键校验在 `validate_task_batch` |
| E4 | 无验收口径的任务生成请求被拒绝 | **PASS** | `task_without_acceptance_rejects_the_whole_batch`（零任务落库） |
| E5 | 整批确认后 `TaskReady` 事件数 = 确认任务数 | **PASS** | `batch_ready_emits_exactly_one_task_ready_per_task`（3 任务 = 3 事件，重放不重复） |

### 附加（§范围 / §7.3 夹具）

| 项 | 结果 |
|---|---|
| basis 边界 2 例：引用未确认 / 不存在 finding 整条拒 | **PASS**（另加跨 session 1 例；三种都断言零需求零修订） |
| LLM 分解夹具：超上限截断 / 无验收口径拒绝 / 角色越界丢弃 | **PASS**（3+8 截断 truncated=5；角色越界 dropped=1） |
| 开关 off 零写入（默认 off 的 fail-closed） | **PASS** |
| 白名单常量不含任何禁止项（第 4 道闸） | **PASS** |
| 记账 `purpose='discovery_orchestration'` | **PASS**（`llm_call_record` 行断言） |
| 手工构造：重复 key / 空 key / 前向引用 / 自引用 / 编号归一化 | **PASS**（pm-ai 单测 6 例） |

**新增测试合计**：pm-ai 集成 9 + pm-ai 单测 6 + pm-application 集成 12 = **27 例，全绿**。

---

## 七、门禁与回归

| 门禁 | 结果 | 备注 |
|---|---|---|
| 新增测试 | 27/27 绿 | `pm-ai`（hook + 单测）、`pm-application`（basis / 整批 ready） |
| 全量回归 | 见 §七.1 | nextest + doctest，`scripts/cargo-test.sh` |
| `cargo fmt` | 本批 19 个文件单文件 `rustfmt --check` 干净 | 用单文件 rustfmt（`cargo fmt -p` 会波及全 crate）。**存量漂移**：`crates/pm-ai/src/{analysis_model,conversation_reply}.rs` 在基点即非 fmt-clean，已回退不纳入本批 |
| `cargo clippy --all-targets` | 本批 crate 零新增警告 | **存量**：`apps/pm-api/src/ai_team_policies.rs:197` `needless_borrow`（基点 `72cb7d5` 同一行已存在），未在本批改动面内，不做顺手修改 |
| 审计点（C174/C180） | 无需登记 | 新增写路径全部复用 `pm_write::append_audit`（`pm_write/audit.rs` 的 INSERT 点计数不变），无新审计 INSERT 文件 |
| 迁移 | 零迁移 | 见文首 |

### 7.1 全量回归

> 结果与失败用例清单见本节末（运行中/完成时回填）：
>
> - 命令：`DATABASE_URL=postgres://aipm:aipm@127.0.0.1:16503/aipm bash scripts/cargo-test.sh`
> - 结果：**待回填**

---

## 八、已知偏差与登记（不掩盖）

1. **`tenant_id` 入参**（§1.1）：`DiscoverySessionView` 无租户列，签名比设计稿的示意多一个参数。
2. **「本 session」的表达方式**（§二）：请求体不引入 session 参数，以「清单自身单一 session」+ CONFIRMED + 同项目三条落地；跨 session 混合拒绝。
3. **`update_draft` 在起草链里是同内容重写**（§三）：为对齐 §8.1 时序保留 `create → update_draft → submit`，`update_draft` 与 `create` 内容等价。收益是「basis 随草稿更新」这条唯一能改 draft 正文的合法面被真实覆盖。
4. **`task_requirement` 仍是 `(task_id, revision_id)` PK**（§6.1 表）：按 S02-I4 范围未改造，`link_task_requirement` 按目标语义（同一 Task 对同一 Requirement 默认一个执行依据）写，不引入与将来 PK 改造冲突的语义。
5. **写入阶段的原子性**（任务分解）：逐条落库，失败即停并返回错误；已建行保持 `draft`（人工可见、可整批否决或删除）。不做跨服务大事务——`create_task` / `link_task_requirement` / `add_dependency` 各自是既有单点的原子单位。
6. **`CapabilityProjection` 为空实现**：三源之二的贡献本批退化，等待 DK-C 合并（不碰其注入投影模块）。
7. **`verification_kind` 固定 `manual`**：AI 生成的验收口径不猜验证方式（LLM 只回填文本），人工可在任务页改。
8. **E5 的补齐属于本批范围扩张**（§5.2）：设计稿没有写"人类整批确认走哪条命令链"，实施时发现既有直写不产事件，故新增 `HumanAction` 来源与批量确认服务。这是 §7.4「入调度门」成立的必要条件，不是可选项。

---

## 九、给 DK-B / DK-C / DK-E 的接口约定

| 对端 | 约定 |
|---|---|
| **DK-B** | 把 `NoWriteBack` 换成 `DiscoveryWriteBack::new(pool, model, capability, knowledge, mode)`；`on_review_required` 在 `REVIEW_REQUIRED` 跃迁后调、`decompose_if_approved` 在扫描轮里调；开关复用 `WriteBackMode::from_env()`。`DecomposeOutcome::NothingToDo{reason}` 是正常空转，不应记 WARN 级错误 |
| **DK-C** | 实现 `CapabilityProjection::project(project_id) -> CapabilityFacts { objects, digests }`（只读；不含 provider/凭据字段——D1 的过滤责任在注入侧，本端口只负责传结构） |
| **DK-E** | 更正类 finding 同样进 basis 校验（`CONFIRMED` 才可引用）；知识提案走 `discovery_proposal`，与本品无交叉 |

---

## 十、一句话结论

DK-D 的六个范围项全部落地且零迁移：**basis 两层溯源**（AI 提案级标记 + 人类一步物化）、**需求起草止步 pending_confirmation**、**任务分解三源 + 四道系统校验 + draft 落库**、**整批 ready 补上入调度断链**、**AI 越权四例全 Forbidden**；B1~B5 + E1~E5 全绿，全量回归见 §7.1。
