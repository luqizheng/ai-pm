# DK-B 访谈编排器（worker 第八循环）验收记录 V1.0

> 归档 2026-11-08。本记录是 `ai-pm-dialog-orchestration-design-v1.md` §10
> **DK-B 批**的实施验收（对齐 D-1=b / D-2=c 验收记录体例）。裁决依据
> D-DK-6（三条件问完毕）/ D-DK-7（36k 预算）/ D-DK-9（第八循环，默认 off）/
> D-DK-11（独立 purpose）+ **C7 双入口并发硬验收**（§13 附注）。
>
> 实施 commit（分支 `feat/dk0-dialog-orchestration-dkb`，基点 `72cb7d5`）：
> `91024f1`（C7 锁修复）→ `2f2f44e`（服务层 + 编排器逻辑体）→
> `3f286b1`（worker 第八循环）→ `6954b98`（测试面）→ 本记录。
> **零迁移、零新表、MCP 四工具未动。**

## 一、最终形态

### 1.1 分层（V-6：编排器零直写）

```
worker 第八循环（apps/ai-pm-worker/src/main.rs::spawn_discovery_orchestrator）
  └─ pm-ai discovery_orchestrator.rs   扫描 / 提示词 / 输出校验 / 三条件 / 降级 / 记账
       └─ pm-application discovery::apply_orchestrator_round   ← 唯一写入口（一次事务）
            ├─ discovery_session 行 FOR UPDATE（与 REST 追加同一把锁）
            ├─ add_finding_in_tx / question INSERT / answer INSERT / transition / message INSERT
            └─ COMMIT（任一步失败 → 整轮回滚，会话状态不变）
```

### 1.2 扫描谓词

```sql
SELECT s.id, s.tenant_id, s.project_id, s.kind, last.id
FROM discovery.discovery_session s
JOIN LATERAL (SELECT id, speaker, created_at FROM discovery.discovery_message m
              WHERE m.tenant_id = s.tenant_id AND m.session_id = s.id
              ORDER BY m.created_at DESC, m.id DESC LIMIT 1) last
  ON last.speaker = 'user'
WHERE s.status IN ('IN_PROGRESS', 'NEEDS_USER_INPUT')
ORDER BY last.created_at ASC, last.id ASC LIMIT $1;
```

回复后最新消息变 `ai` → 自动离开扫描集；崩溃重启最多重跑一轮（服务层
「最新消息 id 未变」前置比较 + 正文归一化去重兜住，§4.6）。

### 1.3 一轮的七个写入步骤（单事务）

| # | 动作 | 幂等/并发 |
|---|---|---|
| 1 | 会话行 `FOR UPDATE` | 与 REST `append_message` 同一把锁 → 跨进程串行 |
| 2 | 最新消息 id 前置比较 | 不符 → `StaleRound`，零写入（整轮幂等） |
| 3 | findings 落库（槽位标签写 `actor_role='slot:<id>'`） | 归一化正文去重 |
| 4 | questions 落库 | 对既有 open + 本轮新增归一化去重 |
| 5 | 消费用户这一轮回答的问题 | 仅 `open` 才作答；已被 REST 答过 → 静默跳过 |
| 6 | 状态跃迁 | 只用既有边；`NEEDS_USER_INPUT→REVIEW_REQUIRED` 走两跳（见 §三 勘误 2） |
| 7 | ai 消息追加 | `seq` 在同一把行锁下计算（C7） |

### 1.4 开关与参数（全部常量，可调不迁移）

| 项 | 值 | 位置 |
|---|---|---|
| `AI_PM_DISCOVERY_ORCHESTRATOR` | `off`（默认）/ `on`；拼错 WARN + off | `DiscoveryOrchestratorMode` |
| 每轮会话数 / 扫描余量 | 5 / 25 | `MAX_SESSIONS_PER_ROUND` / `SCAN_CAP` |
| 每轮问题上限 | 3（超额截断 + WARN） | `MAX_QUESTIONS_PER_ROUND` |
| 访谈上下文 | S6 + 最近 K=6 轮原始问答（**不用 20 轮窗口**） | `RECENT_TURNS_K` / §5.4 |
| 提示词预算 | 36,000 字符；超预算按 S5→S4→S2→S3 整节裁并标注 | `TOTAL_PROMPT_CHARS` / `DROP_ORDER` |
| 单轮失败冷却 | 30s（测试可注入 0） | `RETRY_COOLDOWN` / `with_retry_cooldown` |
| 同槽位连续失败 | 3 轮 → `NEEDS_USER_INPUT` + WARN | `MAX_SLOT_STREAK` |
| 契约连续不合规 | 3 轮 → `NEEDS_USER_INPUT` + 停扫至用户再说话 | `MAX_INVALID_STREAK` |
| 记账 purpose | `discovery_orchestration` | `PURPOSE_DISCOVERY_ORCHESTRATION` |

### 1.5 AI 无确认权的五道闸（本批新增第 5 道）

1. 服务层 `require_human`（既有，`confirm_revision` 等）；
2. REST `ensure_trusted_context`（既有）；
3. MCP 不暴露确认工具（冻结面未动）；
4. 编排器命令白名单（常量，本批只用到 `add_finding`/`add_question`/`append_message`/`transition`）；
5. **本轮新增**：`apply_orchestrator_round` 只接受 `next_status ∈
   {IN_PROGRESS, NEEDS_USER_INPUT, REVIEW_REQUIRED}`——`COMPLETED`/`PAUSED`
   一律 `Validation` 拒（用例 `the_service_refuses_ai_completion`）。

## 二、验证证据

### 2.1 C7（本批硬验收）：双入口并发 20 轮

| 项 | 结果 |
|---|---|
| 用例 | `pm-application/tests/discovery.rs::concurrent_appends_from_two_pools_never_collide_on_seq` |
| 形态 | **两个独立连接池**（= pm-api 进程与 ai-pm-worker 进程），20 轮交错并发追同一 session（user/ai 交替，各 20 条） |
| 断言 | `seq` 恰为 1..=40 连续（零重复零缺号）+ 40 条消息零丢失 + 正文零重复 |
| 实测 | PASS（5.32s） |
| **反向实证** | 临时去掉 `FOR UPDATE` 后同用例立即失败：`duplicate key value violates unique constraint "discovery_message_tenant_id_session_id_seq_key"`（`Key (tenant_id, session_id, seq)=(…, 1)`）→ 用例有真杀伤力，锁是有效因 |

> 说明：基线期 discovery **没有**「追加消息」REST 端点（`pm_discovery.rs` 只有
> session/finding/question/status 八端点）。C7 的语义是「两个进程并发写同一
> `append_message`」，双池即等价证明；端点接线属 DK-A/DK-D，届时两个入口已共用
> 同一函数与同一把锁。

### 2.2 验收清单（设计 §8.3 C 组 + 本批附加）

| # | 判据 | 结果 |
|---|---|---|
| C1 | 有 `open` 且 `why_blocking` 非空的 question → 不停在 `REVIEW_REQUIRED`（本批：产出阻塞问题即 `NEEDS_USER_INPUT`） | PASS（`blocking_question_moves_to_needs_user_input_and_is_accounted`） |
| C2 | LLM 输出 `questions=[]` 但槽位缺一 → 留 `IN_PROGRESS` + WARN | PASS（`model_claims_completion_but_missing_slots_keep_it_in_progress`） |
| C3 | `deferred` 不阻塞跃迁，但复核面必须列出 | PASS（`deferred_questions_do_not_block_but_reach_the_prompt`：迁 `REVIEW_REQUIRED` 且注入 S6 含该问题） |
| C4 | 同一 `(session_id, last_message_id)` 重放两轮，question/finding 行数不变 | PASS（`replaying_the_same_last_message_is_a_no_op`：第二次 `StaleRound`，三类行数全等） |
| C5 | 并发两次 `append_message` 不产生 `UNIQUE(session_id, seq)` 冲突 | PASS（2.1，40 条并发远强于 2 条） |
| C6 | 连续 3 轮同槽位失败 → `NEEDS_USER_INPUT` + 人工提示，不无限重试 | PASS（`the_same_missing_slots_for_three_rounds_escalate_to_human`） |
| C7 | REST + 编排器 20 轮并发零冲突零丢消息 | PASS（2.1） |
| 附加 | 无 provider → 零写入（不建行、不推进状态）、零记账 | PASS（`no_provider_writes_nothing` + 冷却不再重复扫） |
| 附加 | 单轮 token/延迟在 `llm_call_record` 可查 | PASS（purpose/status/latency 断言；token 列沿用 Batch H 口径——仅 provider 返回 `usage` 时填充，mock 无 usage 故为 NULL，真模型属 DK-F） |
| 附加 | LLM 越权 4 类全被拒 + WARN | PASS（`validation_rejects_the_four_overreach_classes`：增槽位 2 / 伪造 finding_ref 1 / 自标 user_confirmed 1 / 写 verification_status 1，另有编造证据 1 与超额截断 1） |
| 附加 | 服务层拒绝 AI 完成会话 | PASS（`the_service_refuses_ai_completion`） |
| 附加 | 长会话不丢结构化记忆（D4 的 DK-B 前置） | PASS（`interview_context_keeps_structured_memory_and_six_turns`：25 轮后仍 system+6 轮，早期 finding 在 S6） |

### 2.3 回归与门禁

| 项 | 结果 |
|---|---|
| `cargo test -p pm-application --no-fail-fast`（全部集成二进制） | **0 failed** |
| `cargo test -p pm-ai`（lib 78 + 三套件 9/6/6；含新增 20 例） | **99 passed / 0 failed / 1 ignored**（`real_deepseek_*` 由 `DEEPSEEK_API_KEY` 门控，属 DK-F） |
| `cargo test -p ai-pm-worker`（`main_loop_switch`） | 4/4 ✓ |
| `cargo test -p pm-api --no-fail-fast` | **0 failed** |
| discovery 既有套件（22 例，含新增 C7 例） | 22/22 ✓ |
| `cargo clippy -p pm-application -p pm-ai -p ai-pm-worker --all-targets -- -D warnings` | 干净 |
| 本批触及文件 `rustfmt --check` | 干净（另有 3 个**基线期既有**漂移文件未动：`analysis_model.rs` / `conversation_reply.rs` / `discovery/web.rs`） |
| 迁移 | **零**（本批零新增；记录撰写时迁移上限 0062——此后 0063/0064/0065 被并行批取走、0066 被本系列预留后又逢 2026-11-09 迁移基线合并，现行链 = `0001_baseline.sql` + DK-A `0002`，台账 §〇·二） |
| **集成树全量回归**（merge master 接 DK-A 底座后，2026-11-09，`bash scripts/cargo-test.sh`，专用库 16483） | **1251/1251 PASS**，4 skipped（= master 1226 + 本批 25；nextest 快档 114s）；`clippy.sh pm-application pm-ai pm-api ai-pm-worker` PASS |

## 三、设计勘误与实施偏差（必须登记，否则后续批次会按设计原文误判）

| # | 设计原文 | 代码事实 | 本批处理 |
|---|---|---|---|
| **D-B-1** | §4.1 扫描谓词 `WHERE s.status='IN_PROGRESS'` | 编排器产出阻塞问题后会话就是 `NEEDS_USER_INPUT`，用户再回话时若只扫 `IN_PROGRESS` 会**永久卡死**（§4.4 的 `NEEDS_USER_INPUT→IN_PROGRESS` 没有别的执行体） | 扫描 `IN_PROGRESS` **或** `NEEDS_USER_INPUT`；轮次内按需落 `IN_PROGRESS`/`NEEDS_USER_INPUT`/`REVIEW_REQUIRED` |
| **D-B-2** | F2「合法边已含 `NEEDS_USER_INPUT→REVIEW_REQUIRED`」，§4.4 注「都在既有转移函数内」 | `session.rs` 的边表**没有**这条边（只有 `NEEDS_USER_INPUT→IN_PROGRESS/COMPLETED`） | 不补边（零迁移）：服务层用 `session_transition_path` 走 `NEEDS_USER_INPUT→IN_PROGRESS→REVIEW_REQUIRED` 两跳，终态一致；边表抽成 `SESSION_EDGES` 单一来源 |
| **D-B-3** | §4.2「槽位全绿从 findings 的 kind+槽位标签派生」 | `discovery_finding` **无槽位列**；零迁移下要让判定可由已持久化行纯函数复算，必须有载体 | 槽位标签写既有预留列 `actor_role='slot:<id>'`；解析只认槽位表内 id（人类/采集端写的角色名不会被误认，见 `slots.rs` 单测） |
| **D-B-4** | §5.3「每轮在 `llm_call_record.metadata` 记 `sections_included/sections_dropped/chars_total`」 | 0041 的 `llm_call_record` **没有 metadata 列** | 本批把三值放轮次报告 + 结构化日志（`AssembledPrompt` 字段可直接透出）；`sections_dropped` 落库方式留 DK-C 裁决（加列 = 迁移，或只留日志） |
| **D-B-5** | §4.2 输出契约 `findings[]` 只有 content/kind/confidence/evidence_refs | 归属（user_confirmed）与槽位覆盖都必须由服务端复算，需要用户原话锚点与槽位 | 契约增加**可选** `quote`（必须原样）与 `slot`；模型给的 `kind=user_confirmed` 一律拒绝计数，归属改由「quote 字面出现在用户本轮消息里」判定 |
| **D-B-6** | §4.5「用户跳答 → 匹配最新未答问题」 | 谁来把 question 置 `answered` 未定义；REST `answer_question` 要求人主体，而用户回答是**消息** | 编排器在轮次内以「最新 open 问题 + 创建时间早于本轮消息」为匹配对象，`answer_question` 侧写入 `answered_by=session.created_by`（用户自己说的话）；REST 侧已答过 → 静默跳过（良性竞争） |
| **D-B-7** | §4.5「无 provider/解析失败 → WARN + 跳过 + 30s 冷却」+「连续 3 次 → NEEDS_USER_INPUT」 | 两者叠加时，三次不合规要跨越 60s+ 冷却才触发升级 | 行为照设计保留；冷却时长做成可注入（`with_retry_cooldown`），验收用例注入 0 只验证升级语义 |

## 四、失败用例清单（跑了但红/跳过的必须列出）

本批**无跳过**用例。开发期出现过 3 个红，全部是**用例自身缺陷**（不是产品缺陷），已修：

| 用例 | 首次失败现象 | 根因 | 处置 |
|---|---|---|---|
| `budget_drops_sections_in_the_ruling_order_and_flags_them` | `dropped` 多了 `S3_kb_digest`（4 节全裁） | 夹具把 `"turn-N"` **重复 2000 次**（每轮 ~14k 字符）而非生成 2000 字符 | 夹具改成定宽填充；修复后 dropped 恰为 `[S5,S4,S2]` |
| `slot_coverage_needs_kind_and_a_live_verdict` | `green` 顺序与期望相反 | 断言写成插入序，实现按**槽位表顺序**输出 | 期望改为表序（`users_and_roles` 先于 `core_behavior`） |
| `three_unusable_rounds_escalate_and_stop_rescanning` | 第 2 轮 `invalid_rounds=0` | 第 1 轮失败后写入 30s 冷却 → 第 2 轮直接跳过（这正是设计的「不烧 token」语义） | 增加冷却注入点，用例置 0 |

另有 **2 类环境型失败**（非代码）：

1. **Docker VM/容器层重启**：本机 Docker 在一次门禁期间重启，专用测试库
   `aipm-dkb-pg`（16493）被整体清掉 → 一批 `PoolTimedOut`；改用长驻的
   `infrastructure-aipm-pg-1`（**127.0.0.1:16433**）后全绿。
2. **共享测试库 `template1` 被毒（11-06 遗留）**：`template1` 里留有 45 条
   迁移指纹 + 8 个业务 schema（core/delivery/integration/ai/governance/
   discovery/devenv/source）→ 任何新建测试库都会继承 → 迁移版本不匹配/整片红。
   按既定配方清理（drop 这些 schema + `public._sqlx_migrations`），并删除 2 个
   泄漏的 `_sqlx_test_*` 库；`_aipm_test.template_fingerprint` 属测试夹具，保留。
   —— 这条对并行会话同样受益，已在本记录留档。

另记一处**编译期回归**（本批自身，已修）：`DiscoveryError` 新增 `StaleRound`
后，`apps/pm-api/src/pm_discovery.rs::discovery_error` 与
`crates/pm-mcp/src/tools.rs::map_discovery_error` 两处穷尽 match 断裂 → pm-api
整包编不过（表现为「套件零输出」）。两处各补一条 `Conflict` 映射（工具契约与
枚举语义均未变），重新编译后全绿。

## 五、挂账与下一步

| # | 事项 | 归属 |
|---|---|---|
| 1 | S1~S5 六节投影 + 预算裁剪落库记账（D-B-4）+ D1~D4 | **DK-C**（接入点已留：`with_supplemental_sections` + `PromptSection`/`SectionId`） |
| 2 | 需求草案落点 / 建议关联清单 / 任务分解 draft（B1~B5、E1~E5） | **DK-D**（本批到 `REVIEW_REQUIRED` 为止，编排器永不调 `COMPLETED`） |
| 3 | UI → `append_message` 的入口接线（`landed` 语义、撤销升级） | **DK-A / DK-D** |
| 4 | 真 provider 端到端（token 列、真实延迟、成本） | **DK-F** |
| 5 | ~~设计稿勘误回填（D-B-1 §4.1 谓词 / D-B-2 F2 边 / D-B-4 metadata 列 / D-B-6 回答归属）~~ | **已闭合**（2026-11-09 DK-B 集成批：设计稿已随 DK-A 进 master、无抢文件冲突，四处勘误注记就地回填） |
| 6 | `.10` / 本地栈开 `AI_PM_DISCOVERY_ORCHESTRATOR=on` 的部署冒烟 | DK-F 前置 |
| 7 | 进程内态（30s 冷却 / 连续缺口计数 / 停扫标记）**不持久化**：重启即重置，最坏多跑一轮（设计允许的崩重语义）；长期运行内存规模受活跃 session 数约束 | 设计已接受，V1 不加表 |

## 六、红线核对

| 红线 | 本批 |
|---|---|
| 禁第二套问答基建 | 零新表零迁移；访谈正文只在 `discovery_message` |
| MCP 四工具冻结 | 未新增/未修改任何 MCP 工具 |
| AI 无确认权 | 服务层 `next_status` 白名单硬拒 `COMPLETED`/`PAUSED`；finding 归属服务端复算；`verification_status` 不可被模型写 |
| V-6 单写入 | 编排器零直写；全部经 `pm-application` 单点服务（一次事务） |
| 审计点 C174/C180 | 未新增生产审计 INSERT 点（复用既有服务路径） |
| 不重开已裁项 | D-DK-1~11、13 ADR、Q-PC 系、ACP V1.3.1、U-1=C/U-2=A 未动 |
