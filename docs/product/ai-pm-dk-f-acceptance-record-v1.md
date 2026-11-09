# DK-F 验收记录 V1.0 —— V0.5 demo 真栈联调收官

| 项 | 值 |
|---|---|
| 基点 | master `968494d`（KP-2+DK-E 已合）→ 分支 `feat/dk-f-demo`（worktree `ai-pm-dkf`） |
| 本批 commit | `a12e290` 后端批（写回装配+缺陷修复×3+迁移 0004）→ `5766086` compose → `daaa590` Stage 2 前端 → `d027e88` clippy 清欠 |
| 真栈 | 本机 DK-F 演示栈（`docker-compose.dkf.yml`）：aipm-dkf-pg(16493)/pm_dkf 空库 → 迁移 **0001_baseline→0002→0004**；pm-api/pm-worker/ai-pm-worker 镜像 `:dkf`；身份面复用 .10 lmms（登录+JWKS+首管理员 JIT）；provider=DeepSeek `deepseek-chat`（真实外呼，非 fixture） |
| 门禁 | 全量 **1340/1340 PASS**（nextest 快档 128.6s；基线 1328 + 本批 12）+ `clippy.sh --all` **PASS** + 前端 vitest **969/969** + typecheck 干净 |
| 配套文档 | 第 0 步核实 `ai-pm-dk-f-step0-verification-v1.md`；演示脚本 `ai-pm-v05-demo-script-v1.md` |

---

## 一、结论

**DK-F 完成，V0.5 关键路径收官**：§8.3 全表 30 条——真栈实证 19 条全 PASS、测试实证 11 条全 PASS（标注 TC，均为既有自动化用例在 1340 全绿中复验）、**0 FAIL**。两场景全链在真 provider 下从口述/导入一路走到任务入调度与更正回流，时序与设计 §8.1/§8.2 一致（差异见 §四）。过程中发现并修复 **3 处真栈缺陷 + 1 处 DK-D 派生缺口（G-1）**，各附回归测试与变异检验。

## 二、§8.3 全表（A1~A6 / B1~B5 / C1~C7 / D1~D4 / E1~E5 / F1~F3）

图例：**L**=真栈实证（Live，DeepSeek 真调用+DB/日志证据）；**TC**=测试实证（自动化用例，1340 全绿内复验）。

### A. 载体口径

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| A1 | 需求句升级：链接置位 + 仅一条 system 边界消息 + chat 表无重复行（无双写） | **PASS (L)** | `full_run_s1_landing.json`：`landed=interview, trigger=implicit_intent, intent=requirement_intake, created_session=true`；DB：`ai.conversation_message` 仅 1 条 system（metadata.handoff），需求句只在 `discovery_message` seq=2 |
| A2 | 升级后同线程纯问答落 discovery（不回落 chat） | **PASS (L)** | 场景 1 六轮驱动全部 `landed=interview`（无显式 mode）；discovery 消息按 seq 累积 |
| A3 | 分类器不可用 → 需求句落 chat、不建 session、不写 finding | **PASS (L)** | `a3_final`：provider disabled → `landed=chat, discovery_session_id=NULL, thread_link=NULL`，新会话 0（fail-closed）。注意点：rule3 本地关键词续接未完结访谈属**正确行为**，探针须先完结会话再测 |
| A4 | 「已切换到需求澄清」可见；撤销 → session `PAUSED` + 链接回退 | **PASS (L)** | `full_run_probe_a4`：`undo → PAUSED`、`thread_link=NULL`；边界消息 `notice=已切换到需求澄清` |
| A5 | 会话列表只有一套（ai.conversation） | **PASS (L)** | `GET /conversations` = 5 条（全部 chat 线程）；discovery 会话只在 discovery 列表 |
| A6 | COMPLETED 后同线程纯问答 → chat，不新建 session、不写 finding | **PASS (L)** | `full_run_probe_a6`：`landed=chat, discovery_session_id=NULL` |

### B. 需求落点

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| B1 | AI 主体 confirm → Forbidden 且确认零新增 | **PASS (TC)** | `pm-ai/tests/discovery_writeback.rs` B1 用例（AI 主体经九命令链被真人闸拒绝，`requirement_confirmation` 零新增）；live 不另探——「AI 无 REST token」本身即第 1 道闸 |
| B2 | 编排器止步 `pending_confirmation`；approved 只能由 Approver 产生 | **PASS (L)** | `full_run_s1_draft.json`：`state=pending_confirmation`、basis=7、`markdown_version_ref` 非空；⑨ 人类 confirm 后才 `approved` |
| B3 | 澄清（finding CONFIRMED）不产生 requirement；approved 不由 finding 推导 | **PASS (L+TC)** | live：裁决前 `delivery.requirement`=0 行；起草仅在人工裁决后由写回扫描面产生。TC：`writeback_sweep::sweep_drafts_after_human_confirms_findings` 步②断言 CONFIRMED 前零起草 |
| B4 | 溯源关联只能由真人产生（AI link_finding → Forbidden） | **PASS (TC)** | `pm-application/tests/requirement_basis.rs`（DK-D）；live 由「REST 物化/挂链由人执行」覆盖（⑧） |
| B5 | 未定稿下调任务生成 → 拒绝且文案指向需求页 | **PASS (L)** | worker WARN ×2：`requirement … is not approved — 请先在需求页确认该需求（approved）后再生成任务`（02:19:44 / 04:42:21 两轮复现） |

### C. 访谈编排

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| C1 | 有 open+blocking 问题时不驻留 REVIEW_REQUIRED | **PASS (L+TC)** | L：DEMO-2 更正轮在仍有 open 阻塞问题时保持 `NEEDS_USER_INPUT`（`s2r_correction_f123`）；TC：C1 单测绿 |
| C2 | 槽位未全绿，LLM 自称问完也不跃迁（WARN） | **PASS (TC)** | `model_claims_completion_but_missing_slots_keep_it_in_progress` |
| C3 | deferred 不阻塞但复核页必须列出 | **PASS (TC+L)** | TC：`deferred_questions_do_not_block_but_reach_the_prompt`；L：`[更正待知识应用]` 标记在会话详情/前端待办块可见 |
| C4 | 同 (session, last_message_id) 重放两轮行数不变 | **PASS (TC)** | `replaying_the_same_last_message_is_a_no_op` |
| C5 | 并发两次 append 无 UNIQUE(session,seq) 冲突 | **PASS (TC)** | DK-B C7 双入口行锁集成用例（`conversation_router.rs`） |
| C6 | 连续 3 轮同槽位失败 → NEEDS_USER_INPUT + 人工提示，不无限重试 | **PASS (TC)** | `the_same_missing_slots_for_three_rounds_escalate_to_human` + `three_unusable_rounds_escalate_and_stop_rescanning` |
| C7 | REST+编排器并发 20 轮零 seq 冲突零丢失 | **PASS (TC)** | DK-B C7 集成（双连接池 20×2 交错恰 1..=40）；本批 seq 定序修复后全绿 |

### D. 知识注入

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| D1 | 注入不含 provider/凭据字段（关键字扫描） | **PASS (TC)** | `discovery_context.rs` 静态扫描用例（SQL_*/投影正文 8 词禁列）；`capability_projection.rs` 同款只读断言（本批新增） |
| D2 | 超预算整节裁剪 + `llm_call_record.metadata` 记 `sections_dropped` | **PASS (L+TC)** | **L（本批迁移 0004 落地）**：`ai.llm_call_record` 实测行 `{"chars_total":2039,"sections_absent":[],"sections_dropped":[],"sections_included":[S1..S6]}`；TC：`budget_drops_sections_in_the_ruling_order_and_flags_them` |
| D3 | LLM 编造 evidence 被服务端丢弃并 WARN | **PASS (TC+L)** | TC：越权四类校验用例；L：更正目标杜撰时服务端拒收降级 `observed,unbound` + WARN（03:20:31，修复③前真实现场） |
| D4 | 访谈上下文无「20 轮窗口」裁剪 | **PASS (TC)** | `interview_context_keeps_structured_memory_and_six_turns`（S6 结构化记忆，最近 K=6 原始轮仅做尾部） |

### E. 写回与任务

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| E1 | `task_requirement.revision_id` 指向 approved | **PASS (L)** | `s1_db_evidence`：`pinned_revision_state=approved`（8 行） |
| E2 | AI 任务全 draft；ready 只由人类产生 | **PASS (L)** | 分解落 8×draft；`mark-ready`（人类 REST）后 8×ready |
| E3 | 依赖落 `task_dependency` 而非标题 | **PASS (L)** | `dependency_edges=9`；抽样边=前置选型→短信服务→接口→页面→测试 |
| E4 | 无验收口径的任务生成被拒 | **PASS (TC)** | `validate_task_batch` E4 整批拒绝用例 |
| E5 | 整批确认后 TaskReady outbox = 确认任务数 | **PASS (L)** | `TaskReady=8 = ready=8`；`delivery.task_run` queued=8（入调度） |

### F. 场景 2 更正回流（DK-E 口径：零提案行+标记可见）

| # | 判据 | 结果 | 证据 |
|---|---|---|---|
| F1 | 更正产生 contradicted+user_confirmed 双行，旧 finding 不删不改 | **PASS (L×2)** | 两次独立真栈复跑均复现：`contradicted` 行（evidence `contradicts:{旧id}`）+ 更正后 `user_confirmed`；旧 observed JWT 行原样 PENDING（DB 直查） |
| F2 | KB 行人工决定前零变化（F2 已按 DK-E 勘误=零提案行+标记可见） | **PASS (L×2)** | `kb_revisions 2→2 / pc_revisions 12→12`、`discovery_proposal=0`；`[更正待知识应用]` deferred 标记落库且携带 `target_document`（实测精确到 `repo:…:module:README.md`） |
| F3 | PC 对象侧零写入（KP-2 已合：P4/P5 在位） | **PASS (L)** | `pc.*` 零变化；PC 侧人工应用路径=P5（AI 起草的提案人工 approve），本 demo 不虚构更正直写 |

**统计：30/30 PASS（L=19，TC=11，0 FAIL）。**

## 三、本批修复的真栈缺陷（回归测试+变异检验随批）

| # | 缺陷 | 根因 | 修复 | 回归测试（变异检验） |
|---|---|---|---|---|
| ① | **写回断头路**：场景 1 走到 REVIEW_REQUIRED 后永不起草 | 起草输入=人工 CONFIRMED finding，而轮次内 finding 必为 PENDING；人工裁决后会话不在编排扫描集 → 无第二轮。DK-D 单测直接造 CONFIRMED 终态，掩盖了「轮次内触发」前提不成立 | `sweep_write_back` 独立扫描面（REVIEW_REQUIRED ∧ ∃CONFIRMED，60s 冷却，幂等=写回侧 AlreadyDrafted）+ worker 接线 | `writeback_sweep.rs` ×3；禁用谓词必红 ✅ |
| ② | **同事务消息定序**：访谈静默卡死（stale_rounds 连发） | 升级同事务写 [ai 摘要, user 消息] 共享 `created_at`，`created_at DESC, id DESC` 次级键=随机 UUID → 扫描面与轮次幂等检查各自选中不同「最新消息」 | 扫描面与 `orchestrator_round` 幂等检查改按 `seq`（会话行锁下权威顺序，C7） | `latest_message_is_ordered_by_seq_not_by_timestamp_tiebreak`（ai 行 id 字典序更大，确定性复现；旧排序必红 ✅）；修复后 master 上 20 个偶红用例归零 |
| ③ | **更正目标不可解析**：每轮更正降级 `observed,unbound`，F1 永不成立 | 输出契约要求 `target_finding_ref` 照抄 uuid，但 S6 渲染不含 id → 模型只能抄正文 | S6 渲染补 `(id=…)`；模型实测随即能精确解析（连带 `target_document` 解析出模块级 doc_key） | `existing_finding_ids_are_visible_in_the_prompt`（去 id 必红 ✅） |
| G-1 | **生产写回未装配**（DK-D §244 派生缺口）：worker 默认 NoWriteBack，且 `WriteBackModel` 无生产实现 | DK-D 只落端口与骨架 | `pm-ai::writeback_model`（三级解析+JSON 契约）+ `capability_projection`（只读投影，凭证零接触）+ worker `.with_write_back(...)` | writeback_sweep + 全链 live |
| G-2 | **D2 无处落账**：`llm_call_record` 无 metadata 列（DK-C/B 挂账） | 0041 无该列 | **迁移 0004**（初取 0003 撞号顺移，台账登记）+ `record_call_with_metadata`（编排轮次含 DK-C 投影层丢弃合并） | D2 live 证据 |

另：开发期自伤两处（`left(text,bigint)` 类型错、子查询未选出 `seq` 列）均在联调中由自身日志/测试捕获修复，不计产品缺陷。

## 四、与设计时序图的差异登记（§8.1/§8.2 对照）

1. **§8.1 步 21「ORC→REQ create+submit」时点后移**：设计图把起草画在跃迁 REVIEW_REQUIRED 的同一编排轮内；真栈口径=轮次内**触发**（无 CONFIRMED 即跳过），实际起草发生在**人工裁决后的写回扫描轮**（本批缺陷①的修复面）。语义不变（AI 止步 pending、人确认后分解），时序图补注即可。
2. **§8.2 步 3「start_session_from_import」**：实现为 `kb.analysis.completed` 消费侧 `ensure_onboarding_from_analysis`（DK-E），会话首条用户消息仍需人发起（编排器扫描谓词要求最新消息=user）。
3. **「更正→提案→KB/PC 回流」段**（§8.2 步 6~8）按 DK-E 裁定 (b) 不执行：零提案行，止步双 finding+标记；KB 应用经人工记忆评审（K4）演示，digest/PC 真正回流挂账 KP-3/KP-2 续作（第 0 步核实，不虚构）。
4. **Runner claim 真执行**（场景 1 彩蛋）：本栈 `AI_PM_RUN_EXECUTOR` 缺省 noop——Run 入调度（queued×8）为止，不接真执行体（验收必要项之外的彩蛋，如实登记）。

## 五、Stage 2（前端）与协同

- **现状核实**（第 0 步）：Batch G 会话页对 `landed/handoff/discovery 段` 零消费，system 边界消息渲染为 AI 气泡——按设计 §2.5 走「最小接线」分支。
- **本批交付**（`daaa590`，分支内）：`SendMessageOutcome` 落点信封契约、两段时间线（chat+discovery）、边界分隔条、待办块（open/待知识应用/已确认事实）、轮询同节奏刷新；测试 21/21、全量 969/969 零回归、typecheck 干净。
- **协同裁决（用户定）**：AI 对话框整合会话（sess_05e4b2f0，Batch 3B 信封协议）**先定型** AssistantView，本批增量其后按 diff 移植——后端契约面（0002 列/落点回包/会话详情）**全部已在 master**，该会话不依赖本分支。渲染边界无冲突：线程链接态下 system 边界行不进 chat 面板（本批滤除），信封卡照常渲染其他 AI 卡。
- **迁移取号**：0003=对方会话（conversation_context_anchor，台账已登记在途）、0004=本批（llm_call_record_metadata，取号史在案）。

## 六、Stage 3（.10 部署窗口）决策登记

**本批不做，demo 在本机栈进行**（用户 2026-11-09 协同裁决：前端以整合会话为先，DK-F 以本机栈收官）。.10 窗口另行单批：镜像重发（含全部 DK/KP 产物）→ **重锚**（基线合并后旧库 checksum 不兼容，drop 重建或 dump/restore）→ R3 最小 recreate（pm-api/pm-worker/ai-pm-worker；token-service/lmms 不动）→ env 配三开关 on + provider → R1~R8 回归 + 两场景冒烟。演示栈配方已固化（`docker-compose.dkf.yml`），迁移即 `0001→0002→0004`。

## 七、一句话结论

DK-F 六范围（两场景真栈全链 + §8.3 全表 + 缺口清偿 + Stage 2 最小接线 + 交付文档）全部落地；**V0.5 只剩跨会话前端整合的移植步**（契约已定，非本批阻塞项）。

## 八、合并执行决定（2026-11-09，用户委托：后续合并全部由 DK-F 会话决断）

**现状**：`feat/dk-f-demo` = `eca4030`（已并入 master cb5f9e0，门禁 1340/1340 + clippy PASS + 前端 974/974）已推远端；master 本体仍 = cb5f9e0，被主 checkout 的整合会话在途工作（28 个未提交文件，与 DK-F 重叠）阻塞——git 拒绝在 dirty 重叠文件上合并，且强并会让该会话后续提交用旧内容静默覆盖 DK-F 文件。

**决断（定死，不再逐次确认）**：

1. **DK-F → master = fast-forward 到 `eca4030`**（分支已含 master，零新增冲突）。
2. **唯一安全触发点 = 整合会话把全部批次落盘**（master 出现其 commits 且重叠文件不再 dirty）。禁止 stash/代提交等绕行（用户已裁、且会打断在途会话的分批计划）。
3. **触发后由 DK-F 会话执行**（两种时序同一套动作）：
   - 它先落：把新 master 并入 `feat/dk-f-demo`（真合并——`pm-ai/src/lib.rs` 与 `pm-application/src/lib.rs` 模块注册取并集、台账各行并存、`AssistantView.vue`=**它定稿版 + 重放 `daaa590` diff（即 Stage 2 移植本体）**）→ 门禁（全量 nextest + clippy + 前端 vitest/typecheck）→ master `--no-ff` 到分支 tip → push。
   - 它声明暂停且 tree 暂净：先 ff master 到 `eca4030`，它余下批次基于新 master 自续（冲突清单已交付其会话）。
4. **移植验收口径**（=§五 Stage 2 的回归面）：`AssistantView.test.ts` 的 DK-F 三例（落点/边界消息/未链接不变）+ 既有用例 + 它的信封用例全绿；typecheck 干净。


## 九、合并执行记录（2026-11-09，触发条件满足后实际执行）

**触发核验**：整合会话 Batch1~4 已全部落盘（master `05eb38db..e99d28f3` 8 commits，用户已 push，origin/master=e99d28f3）；主 checkout 树净（仅 1 个未跟踪 docs 稿，不参与合并）。

**实际动作**（§八.3「它先落」时序）：
1. 真合并 `5369ee4`：master 并入 `feat/dk-f-demo`。冲突 4 处按 §八.3 口径解——`pm-ai/lib.rs` 并集（capability_projection ∥ chat_context）；`pm-application/lib.rs` 自动合并即正确并集（dkf 全模块 + ai_interaction_envelope/ai_interactions，逐模块核对无缺）；台账 0003/0004/0005 三行并存（0004 状态改已实施）；`conversations.ts` master 契约形状 + DK-F `SendMessageOutcome`/`ConversationHandoff` 并存；`AssistantView.vue` = 整合定稿版为底 + 重放 `daaa590` 九处移植本体（imports 并集/访谈段状态与映射/system 边界过滤/syncInterviewLink 五调用点/handleSend 落点判定/模板访谈段保留 master `:submit` 卡片链/样式块）。
2. **门禁全绿**：clippy `--workspace --all-targets -D warnings` PASS；nextest **1367/1367 passed**（4 skipped，doctest ok，DATABASE_URL→dkf 栈 16493）；前端 typecheck 干净 + vitest **997/997 用例全绿**（994=master 基线 + DK-F 三例与契约例；§八.4 口径逐项过）。3 个 e2e spec 文件级失败（vitest 误收集 Playwright spec）与 master 基线逐位一致——存量行为，非本合并引入，另行挂账。
3. master `--no-ff` ← 分支 tip，push master + 分支。
