# DK-A 对话编排底座（载体判定 + 单向升级）验收记录 V1.0

> 归档 2026-11-09。本记录是 `docs/product/ai-pm-dialog-orchestration-design-v1.md`
> 的 **DK-A 批**实施验收（对齐既有验收记录体例：形态 → 证据 → 偏差 → 合并须知）。
> 裁决 D-DK-1~11 见设计稿 §13；本批只做 D-DK-1/2/3 落地与 D-DK-9 附注的
> **C7 前半段**（REST 入口侧）。
> 实施分支：`feat/dk0-dialog-orchestration-design`（worktree `ai-pm-dk0`）。

## 一、实施形态

### 1.1 迁移（1 个：0002）

| 项 | 值 |
|---|---|
| 文件 | `migrations/0002_conversation_discovery_link.sql` |
| 变更 | `ai.conversation` ADD COLUMN `discovery_session_id UUID NULL` + 三元组外键 `(tenant_id, project_id, discovery_session_id) → discovery.discovery_session(tenant_id, project_id, id)` + 部分索引 `WHERE discovery_session_id IS NOT NULL` |
| 回填 | **无**（存量全部 NULL = 纯 chat 线程） |
| 取号史 | 设计稿预留 0063 → 实施时并行批依次取走 0063 / 0064 / 0065（三者均已进 master）→ 按台账纪律 max+1 顺移 0066 → **2026-11-09 冻结点基线合并（0001~0065 → `0001_baseline.sql`）后改取 0002**（台账 §〇） |

**外键比设计稿更严一处（有意偏差）**：设计稿 §2.3(4) 写 `(tenant_id, discovery_session_id)`；
实施用三元组，把「访谈会话与线程同项目」也钉住（`discovery.discovery_session` 的
`(tenant_id, project_id, id)` 唯一键由 0031 建）。MATCH SIMPLE 下 NULL 不触发外键检查，
故存量无影响。

### 1.2 服务层（`crates/pm-application/src/conversation_router.rs`，新增）

| 面 | 实现 |
|---|---|
| 判定 | `send_message()`：先读线程链接与会话状态，再按设计稿 §2.4 判定表自上而下命中：规则 1（已链且未 `COMPLETED/PAUSED` → 续接）→ 规则 2（显式 `mode="interview"` / `discovery_session_id`）→ 规则 3（开关 on + 本地变更意图 + 本人本项目未完结访谈 → 续接该访谈）→ 规则 4（开关 on + 本地意图 + LLM 分类 ≥ 0.7 → **新建**会话并升级）→ 规则 5（其余 → chat） |
| 开关 | `AI_PM_CONVERSATION_ROUTER=off\|on`，**默认 off**；拼错值 WARN 回 off（F22 传统）。off 时只有规则 1/2 生效 |
| 失败方向 | 分类器任一失败（无 provider / 超时 / 不可用 / 输出不合规 / 低于阈值 / 非升级型意图）→ **落 chat**，绝不建会话写 finding（写入侧 fail-closed） |
| 升级四步 | 同一事务内：① 建/取会话（WEB / TASK_DISCOVERY\|UPDATE\|INITIAL_ONBOARDING / IN_PROGRESS）→ ② 写一条 `discovery_message(speaker=ai, source=SYSTEM)` **交接摘要**（摘要，不逐条复制旧轮次）→ ③ 写一条 `conversation_message(role=system, metadata={"handoff":{…}})` 作 UI 边界 → ④ 置 `ai.conversation.discovery_session_id`；任一步失败整体回滚，不留半升级状态 |
| 落点 | landed=interview 时用户消息落 `discovery_message`（经既有 discovery 服务），**不进** `ai.conversation_message`（无双写）；landed=chat 时进 `ai.conversation_message` |
| 撤销 | `POST …/conversations/{cid}/handoff/undo`：30 分钟窗口内回退线程链接；**本次升级新建**的会话置 `PAUSED`（规则 3 续接的用户既有访谈不动其状态——撤销的是「这次切换」不是「此前已在进行的访谈」）；写一条 `handoff_undone` 边界消息 |
| 可见性 | 升级响应带 `handoff.notice = "已切换到需求澄清"` + `undo_until`（静默升级禁止，§2.4 / A4） |

### 1.3 REST（`apps/pm-api/src/ai_conversations.rs`）

- **唯一发送入口**仍是既有 `POST /v1/pm/projects/{pid}/conversations/{cid}/messages`；
  body 扩 `mode?` / `discovery_session_id?`；响应 = 落点消息视图平铺 + `{landed, discovery_session_id, handoff}`。
  chat 落点的消息字段与旧契约**完全一致**（`role` 恒 `user`），旧前端不破。
- 新增 `POST …/conversations/{cid}/handoff/undo`（撤销升级）。
- 判定在**服务端、写入用户消息之前**；前端不判落点、不拼两套 API（§2.4 铁律）。

### 1.4 意图分类 LLM 端口（`crates/pm-ai/src/conversation_router_model.rs`，新增）

照 `snapshot_model` / `conversation_reply` 先例：逐调用三级 provider 解析；JSON 契约
`{"intent": "requirement_intake|kb_correction|project_onboarding|project_query|chit_chat", "confidence": 0..1}`；
每次真实调用（成功或失败）落一条 `ai.llm_call_record`，purpose = **`conversation_router`**
（自由 TEXT，零迁移）；无 provider → 不记账、只 WARN。分类同步路径超时上限 8s
（不吃 provider 长超时——同步 REST 不能被长超时拖住）。

### 1.5 单写入与红线遵守

- 判定、升级、撤销全部在 `pm-application` 服务层；handler **零直写 SQL**（V-6）。
- discovery 写入经既有 discovery 服务的事务内变体（`create_session_in` /
  `lock_session_in` / `append_message_in_tx` / `transition_session_in`），池版原函数改为薄封装，**既有调用方零改动**。
- **零新增生产审计 INSERT 点**（R5）：升级路径全部经既有服务落审计，无需登记
  `tests/audit_actor_coverage.rs` 新清单。
- 零新表（禁第二套问答基建）；MCP 四工具契约未动；AI 无确认权（本批不产生任何确认）。
- 本批**不实现编排器**（DK-B）：升级后的访谈消息暂无 AI 回复属预期，未用临时方案补。

## 二、验证证据

测试库：专用容器 `aipm-dk0-pg`（`postgres:16`，端口 **16483**，
`DATABASE_URL=postgres://aipm:aipm@127.0.0.1:16483/aipm`，127.0.0.1 禁 localhost）。

### 2.1 §8.3 A1~A6 逐条判定

| # | 判据 | 用例 | 结果 |
|---|---|---|---|
| A1 | 升级后 `discovery_session_id` 被置且**只有一条** system 边界消息；`conversation_message` 无该需求句重复行 | `explicit_interview_upgrade_is_atomic_and_single_written`（断言 chat 段恰 2 行 = user + system、system 计数 = 1、需求句零出现于 chat）+ `repeat_explicit_send_does_not_stack_boundaries`（重发显式指定同一会话 → 规则 1 短路，边界仍 1 条） | PASS |
| A2 | 升级后同线程纯问答落 `discovery_message`，不回落 chat | `upgraded_thread_keeps_landing_in_interview`（开关 **off** 也成立；chat 段行数不变；访谈 seq 递增） | PASS |
| A3 | 分类器不可用 → 需求句落 chat、不建 session、不写 finding | `classifier_failure_lands_in_chat_fail_closed`（**6 种失败/低置信逐个断言**：NoProvider / Timeout / Unavailable / BadOutput / 置信 0.3 / 非升级型意图；末断言全项目 **0 session、0 finding**） | PASS |
| A4 | 「已切换到需求澄清」可见；撤销 = session 置 `PAUSED` + 链接回退 | `undo_handoff_pauses_and_unlinks_within_window`（提示文案断言、撤销后 session=PAUSED、链接 NULL、撤销后消息回落 chat、二次撤销 409、窗口过期 409 且状态不动）+ `undo_blocked_once_session_advances`（会话已推进 → 409，链接不动） | PASS |
| A5 | 会话列表只有一套（来自 `ai.conversation`），访谈不出现为列表项 | `conversation_list_is_the_only_thread_directory`（列表恰 1 项 = 线程；链接以**列**暴露） | PASS |
| A6 | 访谈 `COMPLETED` 后同线程纯问答 → `landed="chat"`，不新建 session、不写 finding（D-DK-1 附注） | `completed_interview_falls_back_to_chat`（规则 1 须看状态、不能只查列存在） | PASS |

### 2.2 升级路径 3 例（DK-A 门槛要求）

| 例 | 用例 | 结果 |
|---|---|---|
| 显式进入 | `explicit_interview_upgrade_is_atomic_and_single_written`（mode="interview"；断言分类器**零调用**、会话 source/kind/status/created_by、摘要不复制旧轮次原文、finding=0） | PASS |
| 隐式命中（开关 on） | `implicit_intent_upgrade_when_router_on`（分类命中 kb_correction → 新建 UPDATE 会话、提示可见、分类器恰 1 次调用） | PASS |
| 分类失败落 chat（开关 on） | `classifier_failure_lands_in_chat_fail_closed`（6 种失败变体全落 chat、零 session、零 finding） | PASS |

### 2.3 补充不变量

| 不变量 | 用例 | 结果 |
|---|---|---|
| 升级后线程**脱离第七循环 `conversation_reply` 的 pending 集**（F19 扫描谓词=最新消息 role='user'） | `upgraded_thread_leaves_conversation_reply_scan_set`（用真实 worker `reply_round()`：升级前 pending=1，升级后 pending=0） | PASS |
| 规则 3 只续接**本人本项目**的未完结访谈；撤销不暂停用户既有访谈 | `rule3_continues_own_open_interview` | PASS |
| 开关 off + 本地规则未命中 → 分类器零调用（省 token） | `router_off_and_local_miss_never_call_classifier` | PASS |
| 边界：跨项目会话 404 / 已完结会话 409 / 非法 mode 400，且**失败请求零写入** | `explicit_session_boundaries` | PASS |
| **C7 前半段（DK-A 入口侧）**：两条线程并发续接同一访谈 → `seq` 连续零重复零丢失 | `concurrent_threads_on_one_session_never_collide_on_seq`（2 线程 × 12 轮交错并发，seq = 1..=N） | PASS |

### 2.4 变异检验（用例有效性反证）

| 变异 | 期望 | 实测 |
|---|---|---|
| 规则 1 去掉会话状态检查（`session_is_active` 恒 true） | A6 必红 | `completed_interview_falls_back_to_chat` **FAILED**、另 1 例 FAILED ✓ |
| 去掉会话行锁（`lock_session_in` → `get_session_in`） | C7 用例必红 | 连跑 3 潮次**全部 FAILED**，响应 500（并发 `seq` UNIQUE 冲突）✓ |

（变异均已还原；还原后全绿。）

### 2.5 回归与门禁

| 项 | 结果 |
|---|---|
| 本批新集成用例（`apps/pm-api/tests/conversation_router.rs`） | **14/14 PASS** |
| 本批单测（pm-application 4 + pm-ai 2） | 6/6 PASS |
| `ai_conversations` 既有 REST 例 | 8/8 PASS（**零回归**） |
| `ai_conversations` 既有服务例 | 6/6 PASS |
| `discovery` 既有 REST 例 / 服务例 | 6/6、21/21 PASS（**零回归**） |
| 全工作区回归（`bash scripts/cargo-test.sh`，基线合并同步后重跑，2026-11-09） | **1226/1226 PASS**，4 skipped（存量 `#[ignore]`：qa_b02_14/18/19 + 真实 DeepSeek 计费用例），**0 failed**（= master 基线 1206 + 本批新增 20；nextest 快档 111s）；存量白名单（consul 环境型 / qa_b05_02·03）未触发 |
| `cargo fmt`（本批 9 个文件） | 干净 ✓（全仓存量 42 处 fmt 漂移在**本批前后完全相同**，均在本批未触碰文件） |
| `cargo clippy`（pm-application / pm-ai / pm-api `--all-targets`） | 本批文件零告警 ✓（存量 1 处：`apps/pm-api/src/ai_team_policies.rs:197` 非本批引入） |
| 迁移干净库应用 | 专用库 16483（容器 2026-11-09 重建，旧链账本随之作废）全新链应用成功：`0001_baseline.sql` + `0002` ✓ |

## 三、偏差登记（有意偏离设计稿之处）

| # | 偏差 | 理由 |
|---|---|---|
| 1 | 外键用**三元组** `(tenant_id, project_id, discovery_session_id)`（设计稿为二元组） | 钉「访谈与线程同项目」，比设计稿更严；0031 已有该唯一键 |
| 2 | 迁移号 **0002**（设计稿预留 0063） | 并行批依次取走 0063/0064/0065 → 顺移 0066 → 2026-11-09 迁移基线合并（0001~0065 squash 为 `0001_baseline.sql`，新链自 0002 起）后改取 0002（台账 §〇） |
| 3 | 撤销时**仅对本次升级新建的会话**置 `PAUSED`；规则 3 续接的既有访谈只回退链接 | 撤销的语义是「撤销这次切换」，不是终止用户此前已在进行的访谈；置 PAUSED 会破坏他人的访谈进度 |
| 4 | 规则 3 只续接**本人（created_by）本项目**的未完结访谈 | 他人的访谈不是本用户的对话段落；跨用户续接会造成归属混乱 |
| 5 | 分类器**同步**调用，超时上限 8s | REST 同步路径不能被 provider 长超时拖住；provider 自身 timeout 取 `min(cfg, 8s)` |
| 6 | `pm-ai` 从 pm-api 的 **dev-dependency 升为正式依赖** | 生产路径要注入 LLM 分类器（`ConversationRouting::from_env`） |
| 7 | 分类器**无 provider 时不记账** | 无 provider 行可挂账（同 `snapshot_model` 先例）；失败记账只在真实调用发生时 |

## 四、合并须知（合 master 前必读）

本批与并行批在同一分支族上并行开发，合并时有三处必须人工处置：

1. **`crates/pm-application/src/discovery/message.rs` 与 DK-B 重叠**：
   DK-B 的 `91024f1`（C7 双入口 seq 串行）已把池版 `append_message` 改为「事务内取会话行锁 + 调事务内函数」。
   本批的 `append_message_in_tx` **刻意与其同名同签名**（`tx: &mut Transaction<'static, Postgres>`），
   并新增 `session::lock_session_in` 作统一取锁点。合并时：**取 DK-B 的池版 `append_message`**，
   本批的事务内函数与 `lock_session_in` 可直接套用（冲突面 = 该文件的 `append_message` 函数体）。
   本批的 C7 用例 `concurrent_threads_on_one_session_never_collide_on_seq` 覆盖「REST 升级/续接入口」
   这一侧；DK-B 的用例覆盖「两个独立连接池」那一侧——两侧合起来才是 D-DK-9 附注的完整 C7。
2. **`crates/pm-ai/src/call_record.rs` 与 DK-B / DK-D 重叠**：本批加 `PURPOSE_CONVERSATION_ROUTER`，
   DK-B/D 加 `PURPOSE_DISCOVERY_ORCHESTRATION`——**同文件不同常量**，合并时两行都要留。
3. **迁移台账**：本批迁移已在台账 §〇·二「基线后在途迁移」以 **0002** 行登记
   （含完整取号史）；master 版台账已含基线合并上下文，合并无冲突面。

另：`ai.conversation` 新增列 → 若前端有会话列表的 `ConversationView` 契约消费方，
本批已在响应里多出 `discovery_session_id` 字段（**新增字段，不改既有字段语义**）；前端接线属 DK-F 范围。

## 五、未做（留给后续批）

- **DK-B 编排器**：访谈消息暂无 AI 回复属预期；第八循环、问题生成、槽位判定、状态跃迁均在 DK-B。
- **DK-C 注入 / DK-D 写回 / DK-F 真栈联调**：见设计稿 §10 批次表。
- 前端「访谈段拼接渲染 + 撤销按钮 + 分隔条」属 DK-F。

**文档结束**
