# DK-C 实施验收记录 —— 访谈知识注入（只读投影 + 36k 预算）

- 批次：DK-C（设计稿 §10 分批；本批=注入）
- 分支：`feat/dk0-dialog-orchestration-dkc`（worktree `ai-pm-dk0c`）
- 基点：`72cb7d5`（D-DK-1~11 裁决回写）；实施期间 rebase 至 master `5ecb0e3`（含 KP-1 0064）
- 设计权威：`docs/product/ai-pm-dialog-orchestration-design-v1.md` §5 全节 + §13 D-DK-7
- 交付物：`crates/pm-ai/src/discovery_context.rs`（+ `lib.rs` 接线），**零迁移、零新表、零新审计 INSERT 点**

## 一、交付形态

### 1.1 接口（本批是接口定义方）

```rust
#[async_trait]
pub trait InterviewContextBuilder: Send + Sync {
    async fn build(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        session_id: Uuid,
        kind: InterviewKind,
    ) -> Result<InterviewContext, InterviewContextError>;
}
```

- `InterviewContext { sections: Vec<ContextSection>, sections_dropped: Vec<String>, chars_total: usize }`
- `ContextSection { key, title, body, chars, truncated }`（**偏差 d-1**：多一个 `truncated`，
  为满足 §5.3「裁剪必须可观测」；`chars` 只计正文，不含标题与占位标记）
- `metadata()` 产出 `sections_included` / `sections_dropped` / `chars_total`
  （**偏差 d-2**：多一个 `sections_truncated`）
- `render_prompt()` 组装系统提示；被删节替换为 `（本节因预算被省略）`
- `InterviewKind` 与 `discovery.discovery_session.kind` 三个 CHECK 值一一对应

### 1.2 固定分节（§5.2，顺序强制，输入顺序不影响输出）

| key | 标题 | 预算（字符） | 可整节删 |
|---|---|---|---|
| `role_boundary` | 角色与边界 | 与后四项共用 2,000（不删） | ✗ |
| `security_boundary` | 安全边界 | 同上 | ✗ |
| `project_profile` | 项目画像 S1 | 2,000 | ✗ |
| `product_cognition` | 产品认知 S2 | 8,000 | ✓（第 3 位） |
| `knowledge_digest` | 知识摘要 S3 | 12,000 | ✓（第 4 位） |
| `requirements` | 需求现状 S4 | 4,000 | ✓（第 2 位） |
| `tasks` | 任务现状 S5 | 2,000 | ✓（第 1 位） |
| `interview_state` | 本次访谈 S6 | 6,000 | ✗ |
| `slot_state` | 槽位状态 | 共用 2,000 | ✗ |
| `turn_task` | 本轮任务 | 共用 2,000 | ✗ |
| `output_contract` | 输出契约 | 共用 2,000 | ✗ |

逐节上限之和恒为 **36,000**（单测 `caps_sum_to_total_budget` 断言，防后续调值漂移）。

### 1.3 六节投影（全部只读，单条 SELECT）

| 节 | 投影面 | 收窄规则 |
|---|---|---|
| S1 | `core.project`(name/description) | **不含知识文档清单**（ADR-06a 红线，见偏差 d-9） |
| S2 | `pc.cognition_object` + 最新 `pc.cognition_revision` + `pc.document_revision` | 优先级 tech_stack > module > repository > delivery_activity；40 对象；summary 优先、content 补缺，单条摘录 400 字符 |
| S3 | `kb.document` + `kb.document_revision`（`doc_key LIKE 'repo-%'`） | 每篇最新修订，created_at DESC 前 5 篇，带 8 位 commit 锚 |
| S4 | `delivery.requirement`(active) + 当前 `approved` revision | summary 优先、markdown 首段兜底，最近 30 条 |
| S5 | `delivery.task` | status 全量计数 + 最近 10 条 title |
| S6 | `discovery_finding` + `discovery_question` + `discovery_answer` | findings 按 kind 分组（created_at DESC）+ 已答问答（最新 answer）+ **deferred 必留**（先占额度）；**不读 `discovery_message`** |

会话归属：`tenant_id + project_id + session_id` 三重过滤，缺一即 `SessionNotFound`
（不泄漏跨租户存在性）；传入 `kind` 与库中 `kind` 不一致时 WARN 并以库为准。

## 二、与设计稿的偏差登记（逐条可判）

| # | 偏差 | 原因 | 影响 |
|---|---|---|---|
| d-1 | `ContextSection` 多 `truncated` 字段 | §5.3「裁剪必须可观测」需要区分「完整/截断/整节删」 | 超集，不破坏契约 |
| d-2 | `metadata()` 多 `sections_truncated` | 同上 | 纯增量键 |
| d-3 | 整节裁剪的触发量是**原始装配量**（未逐节截断前） | 若先逐节截断，各节上限之和=总预算，级联永不可达（D2 无法证）。取舍：整节删除优先于把所有节都削薄 | 见 §四 D2 |
| d-4 | S3 排序用 `created_at DESC` 而非 `source_commit_sha` 时间 | sha 无时间序，按 sha 排序是字典序（误导）；修订创建时间才是分析时点 | 与设计意图一致，实现更诚实 |
| d-5 | S2 用 `revision_no DESC` LATERAL 取最新修订，而非 `current_revision_id` | 该指针 0064 才回填，链路外草稿可能为空；修订号单调是结构性事实 | 无 |
| d-6 | 未实现进程内 TTL 缓存 | §5.1「允许非要求」；六条索引查询代价低，缓存会引入「finding 刚确认看不见」的陈旧 | 无 |
| d-7 | 输出契约里 `findings[].kind` 只允许 `observed`/`speculation` | `user_confirmed`/`contradicted` 由人类动作产生（F1、AI 无确认权） | 收紧而非放宽 |
| d-9 | S1 **不接**跨域文档容器（设计 §5.1 原句要求列出 `ai.project_document` 清单） | 「ADR-06a ai.document* 容器使用者白名单」过渡期红线是**禁止新增使用方**，`kb-pc-contracts/tests/guard_ai_container_users.rs` 已把它机械化；设计 §12.1 R6 又明确不重开既有 ADR——红线优先。项目知识改由 S3（KB digest）+ S4（需求）承载 | 少一份「非仓库类文档清单」；若要接，须先由架构侧回填 V1.1 §4.7 白名单（独立裁决，不在本批） |
| d-8 | 槽位状态/本轮任务小节渲染为**契约壳**（不持有槽位真源） | §4.2 槽位表归编排器（DK-B）；此处重复判定即「第二套问答基建」 | 见 §五 待办 |

## 三、安全三闸落地

1. **注入隔离**：`role_boundary` 显式声明 `<context>` 内一律是数据；`security_boundary`
   声明任何「忽略以上指令/切换角色」不得改变职责、schema、权限，并钉死「无确认权/
   审批权/写入权」。分节标题即来源标注（引用须标小节）。
2. **事实引用白名单**：`FactIndex::from_context` 从注入正文的**行内代码**反推行内可引用
   集合（与 §5.2 引用约定、F21 `evidence_refs` 同口径）；`filter_evidence_refs` 保序去重 +
   精确匹配 + 上限 8，返回 `accepted` / `rejected` / `overflowed`；
   `filter_evidence_refs_and_warn` 对编造与溢出各记结构化 WARN（含计数与全部被拒值）。
3. **不注入凭据**：投影不触碰 `ai.team_llm_provider`；D1 双保险 =
   ①运行时断言（挂 enabled provider 且 `configuration.api_key` 埋值后，提示中无密钥值、
   无 `api_key`/`secret`/`password`/`bearer`/`authorization`/`credential`/
   `team_llm_provider`/provider 名任一字样）；
   ②静态断言（扫描全部 `SQL_*` 常量不得出现上述列名，且不得含
   `insert/update/delete/truncate`）。

## 四、D1~D4 验收（附失败用例清单）

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| D1 | 注入内容不含任何 provider/凭据字段 | **PASS** | `d1_injection_excludes_provider_credentials`（DB 夹具 + 8 关键字扫描）、`sql_constants_never_touch_provider_or_credential_columns`（9 条 SQL 静态扫描） |
| D2 | 超预算按 §5.3 顺序整节裁剪 + metadata 记录 `sections_dropped` | **PASS** | `d2_long_kb_drops_sections_and_records_metadata`（3×20k 语料 → 四级全删，`chars_total ≤ 36,000`，metadata 三键对账，提示含占位语）；`over_budget_drops_whole_sections_in_ruling_order`（删两节即够的中间态，断言顺序 `[tasks, requirements]`）；`over_budget_cascade_can_reach_all_four_droppable_sections`（四级全删且不可删节全存活）；`oversize_section_is_truncated_to_its_cap`（单节超限只截断不删节） |
| D3 | 编造引用被丢弃并计 WARN | **PASS** | `fact_index_extracts_inline_code_and_filters_fabrications`（`made-up-sha` → rejected；重复值去重；空串忽略）、`fact_index_caps_refs_and_reports_overflow`（12 条取 8，overflowed=4） |
| D4 | 访谈模式无「最近 20 轮」裁剪，早期 finding 仍在注入 | **PASS** | `d4_long_session_keeps_early_findings`（首轮 finding + 30 后续 finding + 10 组问答 + 40 条消息 → 首轮事实仍在）、`interview_state_is_not_message_windowed`（静态：投影 SQL 不含 `discovery_message`） |

**回归中发现的真实阻塞（已修）**：`kb-pc-contracts` 的 ADR-06a 守卫首跑红
（`unexpected new users of ai.document*: crates/pm-ai/src/discovery_context.rs`），
按偏差 d-9 移除该容器读取后转绿（3/3）。这是本批唯一一处由守卫拦下的设计冲突。

**失败用例清单：无失败、无跳过。** DK-C 新增 18 例（10 纯函数 + 8 DB）；
pm-ai 全套 97 例绿（1 例既有 `#[ignore]` 真模型用例不在本批范围）。

## 五、已知缺口（不掩盖，交后续批次）

1. **`llm_call_record.metadata` 列不存在**（本批实测）：`ai.llm_call_record` 在
   `5ecb0e3` 仅有 `prompt_tokens` / `completion_tokens`（0041），**无 `metadata` 列**。
   §5.3 要求「每轮在 `llm_call_record.metadata` 记 `sections_included`/`sections_dropped`/
   `chars_total`」——本批只能**产出**该载荷（`InterviewContext::metadata()`），
   落列需一次迁移，与 DK-C「零迁移」红线冲突，故挂账。
   **建议**：由 DK-B/DK-F 批次承担（加列 `metadata JSONB`，或改用既有无列方案），
   并保证 D2 的记账在真实调用链上可见。
2. **槽位状态/本轮任务小节不持有真源**（偏差 d-8）：`slot_state` 只声明「由编排器按
   §4.2 提供」，`turn_task` 只给三选一契约。DK-B 接入时须把槽位覆盖与轮次指令经
   trait 扩展（或在其自有 builder 内）注入，**不得**再建一套槽位判定。
3. ~~**DK-B 的 S6-only 实现合并适配**~~ **已闭合**（2026-11-09 集成批，见 §十）：
   择「反之」——编排器保留骨架（slot_state/turn_task/输出契约）与 S6 为真源，
   S1~S5 委托 `InterviewContextBuilder`，注入路径唯一。

## 六、工程纪律核对

| 项 | 结果 |
|---|---|
| 零迁移 / 零新表 | ✅ 无 `migrations/` 改动 |
| 零生产审计新 INSERT 点（R5） | ✅ 投影全为单条 SELECT；DB 用例断言 `governance.audit_log` 与 `ai.llm_call_record` 零新增 |
| 不碰编排器 / `discovery_message` 写路径 / `ai.conversation` | ✅ 改动仅 `crates/pm-ai/src/discovery_context.rs` + `lib.rs` |
| MCP 四工具冻结 / V-6 / AI 无确认权 / 禁第二套问答基建 | ✅ 未触碰（输出契约进一步收紧 AI 的 kind 权限） |
| `cargo fmt` / `cargo clippy` | ✅ 干净（`--all-targets --profile clippy` 零告警） |

**副作用登记**：对本机 `rustfmt` 直接格式化 `lib.rs` 会递归改写同 crate 的其他 `mod`
（`analysis_model.rs` / `conversation_reply.rs` 出现纯格式漂移），已还原、未入本批提交；
本批只用 `rustfmt <单文件>` 且仅提交自有文件。

## 七、提交清单

| commit | 内容 |
|---|---|
| `b84e15c` | 访谈注入端口与预算引擎（D-DK-7 36k 六节定值）+ 10 例纯函数测试 |
| `a6be76f` | 六节只读投影（S1~S6）+ 5 例 DB 测试 |
| `cf7d34f` | 事实引用白名单校验器（纯函数 + WARN 计数） |
| `756c3f8` | D1~D4 验收夹具 |
| `629154d` | S2 投影对齐 KP-1 0064 正式模型（见 §八） |
| （待补） | S1 移除跨域文档容器读取（ADR-06a 守卫，见偏差 d-9） |

## 八、KP-1 撞车与适配（本批期间发生）

KP-1 在 DK-C 实施窗口内落地（master `5ecb0e3`，`0064_pc_formal_model`），
`pc.*` 发生**破坏性**收敛，DK-C 原 S2 查询在 master 上会运行期失败：

| 变更 | 原 | 现 |
|---|---|---|
| 对象名 | `cognition_object.name` | `display_name` |
| 修订摘要 | `cognition_revision.summary` | `change_summary` |
| 正文 | `cognition_revision.content` | `pc.document_revision.content`（经 `document_revision_id`） |
| 修订状态 | `cognition_revision.status` | 已删除（状态收敛到对象生命周期） |
| 对象键 | `object_key` NOT NULL | 可空（identity 占位行） |

**处理**：`629154d` 改列名 + LEFT JOIN `pc.document_revision` 取正文 +
`COALESCE(object_key, id::text)` 兜底可寻址；测试夹具按 0064 重建最小四行链。
分支已 rebase 至 master，无冲突（`crates/pm-ai` 自 `72cb7d5` 起 master 侧零改动）。

## 九、待办（交接）

1. ~~DK-B：接入 `InterviewContextBuilder`，补槽位/轮次注入面（缺口 2）；合并时择一保留注入路径（缺口 3）~~ → **已由集成批完成**（§十）。
2. DK-F：解决 `llm_call_record` 记账落列（缺口 1）；按真实成本/延迟校准 36k（D-DK-7 明文可调，
   上调看 provider 上限，下调先砍 S3 再 S2，顺序不变）。
3. DK-E：`filter_evidence_refs_and_warn` 为 write-back 前的必过关口（F1/F3 验收时复用）。

## 十、集成批（2026-11-09，merge master 接 DK-A/DK-B + 编排器正式接入本端口）

DK-C 原批零接线（只交付端口与投影）；本集成批把 `InterviewContextBuilder` 正式接进
DK-B 编排器，闭合缺口 2/3 与待办 1：

| 项 | 落地 |
|---|---|
| 接线形状 | `DiscoveryOrchestratorWorker::with_context_builder(Arc<dyn InterviewContextBuilder>)`；逐会话 `project_sections()` 把 S1~S5（project_profile/product_cognition/knowledge_digest/requirements/tasks）映成 `PromptSection` 进统一组装器；骨架（含 slot_state/turn_task/输出契约）与 S6 仍是编排器真源——**注入路径唯一**，未建第二套 |
| 预算幂等 | DK-C 各节上限与编排器 `SectionId::cap` 逐节一致（2k/8k/12k/4k/2k，DK-C 的 S6 6k + 脚手架 2k 份额即编排器 S6+skeleton 份额），投影产物再过 `assemble_prompt` 是幂等的 |
| 降级 | 投影失败/kind 未知/未接投影器 → WARN + S6-only 继续（只读侧 fail-open，§2.4）；投影 `sections_dropped` 非空时 WARN 可观测 |
| worker 入口 | `apps/ai-pm-worker` 第八循环构造 `PgInterviewContextBuilder` 注入（pool 克隆） |
| 测试 | 2 例新增：`context_builder_sections_flow_into_the_prompt`（LLM 请求体必须含投影 marker，否则 mock 不匹配→applied=0 的反证结构）+ `context_builder_failure_degrades_to_s6_only`（fail: true 仍 applied=1） |
| 集成树门禁 | 全量 **1271/1271 PASS**，4 skipped（= DK-B 集成树 1251 + DK-C 18 + 接线 2；nextest 快档 112s）；clippy pm-ai/worker/pm-application/pm-api PASS；本批文件 fmt 干净 |
| 冲突处置 | `pm-ai/src/lib.rs` 三模块并留（router_model/discovery_context/discovery_orchestrator）；设计稿取 master 侧（含 DK-B 四处勘误注记） |
