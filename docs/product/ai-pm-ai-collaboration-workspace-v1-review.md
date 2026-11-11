# 审批意见 — AI Collaboration Workspace V1.0 设计基线

| 项目 | 内容 |
|---|---|
| 被审文档 | `docs/product/ai-pm-ai-collaboration-workspace-v1.md`（master @ `996df5fa`） |
| 评审日期 | 2026-11-11 |
| 核对方式 | 附录 B 全部代码引用逐条 grep/sed 实证（基准 `a3f92f7c` 内容同当前 HEAD） |
| **结论** | **有条件通过** — 修正 F1、F2 后可进入实施拆分；F3、F4 顺手改 |

---

## 一、事实核对结果（附录 B 引用逐条实证）

| 文档引用 | 实证结果 |
|---|---|
| `stores/shell.ts:147` assistantContext / `:200` queue / `:211` take | ✅ 行号、语义均符 |
| `migrations/0003_conversation_context_anchor.sql` 白名单 + 跨项目 CHECK | ✅ 白名单 5 键（project_id / project_name / repository_name / default_branch / repository_role），CHECK 防跨项目锚点注入 |
| `ai_conversations.rs:82` `ContextAnchor::validate` | ✅ project_id 匹配校验在场 |
| `chat_context.rs:100` `build_chat_context` / `conversation_reply.rs:438` 注入记账 | ✅ 均在引用行 |
| `ai_interaction_envelope.rs` 8 类 + 11 态 + `failed` 终态 | ✅ `RESPONSE_TYPES: [&str; 8]`、`INTERACTION_STATUSES: [&str; 11]`、`TERMINAL_STATUSES` 含 failed |
| `migrations/0005_conversation_interaction.sql` 写回闭环 | ✅ 表存在 |
| G2「`metadata.injection` 前端零消费，仅 i18n 标签」 | ✅ 精确：前端仅 `ContextPanel.vue:218` 命中 `injectionSources` 标签 |
| G11「`ai.conversation.project_id` NOT NULL」 | ✅ `0001_baseline.sql:818` |
| 三份关联文档 | ✅ 均存在 |

**附录 B 的代码事实全部实证通过，无虚构引用。**

---

## 二、发现与意见

### F1 · P1 — 图片断链（阻塞合入态自包含）

文档引用 9 张 `docs/figures/ai-collab-v1-*`，均未入库（本次按指示只提交了 md）。已提交的基线对任何 clone 者图片全 404，且 `docs/figures/` 不在 .gitignore，属遗漏而非有意排除。
**处置**：紧随补一个 figures 提交（9 张一并入库）。

### F2 · P1 — 差距清单缺两条已知缺陷挂账

§5.3 把「AI 不得 AutoApply 正式 Requirement，不得自动审批」写作**已确定约束**，但代码现状与之相悖：

- **DEF-P0-01**：`auto_accept` 在「无任何层表态时默认允许」（`ai_team_policies.rs:150` 规则自述），默认放行缺口已登记未修；
- **DEF-P0-02**：`confirm_baseline` 无真人守卫，与场景 6 第 6 步「等待用户确认」、§0.3 硬边界的承诺直接相关。

B.2 差距清单是实施拆分的输入，漏掉这两条会让「权限/审批」类工作在拆分时失锚。
**处置**：B.2 或 B.3 增加「既有缺陷挂账」小节引用 DEF-P0-01 / DEF-P0-02。

### F3 · P2 — §5.1 卡名词表与信封实现不一致

§5.1 列 7 种卡（Analysis / Proposal / Risk / Question / Decision / Execution / Progress），信封实现是 8 类 `RESPONSE_TYPES`（informational / recommendation / proposal / question / decision / artifact / action-request / execution-result）。**Risk、Progress 无对应类型；artifact、action-request 在 §5.1 无落点**。AI-03（Response Cards）页面设计前必须给映射表，否则 Figma 与代码两张词表。
**处置**：§5.1 补「卡名 ↔ RESPONSE_TYPES」映射（Risk → recommendation/informational，Progress → execution-result 等，映射本身需设计裁决）。

### F4 · P3 — G9 笔误

「当前锚点为 5 字段（project/repo/branch/role）」——数对、名漏：5 字段为 project_id / project_name / repository_name / default_branch / repository_role，括号漏 project_name。顺手改。

---

## 三、正面确认

- §0.3 硬边界与权威边界（Markdown+Git / PostgreSQL 分工）表述准确，全文「Accept ≠ 生效」贯穿一致。
- 场景 4 的 Available / Attached / Read-only 三态 + 「页面切换不静默改上下文」与已落地快照机制（`queueAssistantPrompt` 提交时刻定格）方向吻合。
- §7 三对象分离（Conversation / Context Set / AI Action）+ 输入上下文快照，正确指出了当前锚点机制的架构缺口（G10）。
- 附录 B 自曝差距（13 条）且归类合理，「诚实性断点（G1/G2）最高优先」的排序正确。

---

## 四、结论

**有条件通过。** 设计基线的骨架、硬边界、场景优先级（先 AI-01~04）均可确认；放行条件：

1. F1：补提交 9 张 figures；
2. F2：B.2 挂账 DEF-P0-01 / DEF-P0-02；
3. F3：AI-03 开工前补卡名映射表（可与 F1/F2 同批修订）。
