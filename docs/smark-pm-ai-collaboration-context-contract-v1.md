# AI Collaboration Workspace V1.0 — 对象级上下文与审批读接口（后端契约任务书）

| 项目 | 内容 |
|---|---|
| 日期 | 2026-11-11 |
| 任务性质 | **后端契约 + 实施设计**（先定契约，再实施） |
| 触发 | 四栏 Shell 迁移四阶段已落地（前端）；AI-02 的对象级上下文**卡在后端** |
| 关联 | [设计基线](product/ai-pm-ai-collaboration-workspace-v1.md)（附录 B 差距 G3/G4/G5/G9/G10）· [裁决 DR-C2/C3/C7/C8](ui/ai-collab/smark-pm-aic-95-decision-record-v1.md) · [壳迁移评估任务书](smark-pm-ai-collaboration-shell-migration-assessment-v1.md) |
| 产出 | 本文件即契约草案（含待裁决项）→ 评审通过后拆实施切片 |
| 纪律 | 所有代码引用已 grep 实证；迁移号按现状取 **0019**（0018 已被 team_llm_catalog 占用） |

---

## 〇、为什么需要这次后端任务

前端四阶段（① 四栏容器 ② 会话能力并入常驻列 ③ 右栏四 Tab + History Drawer ④ 切项目确认）已落地。当前右栏能展示的 Context 只有**项目 + 仓库**两级（来自线程锚点），因为：

- 锚点白名单只有 5 个键（`project_id` / `project_name` / `repository_name` / `default_branch` / `repository_role`），**装不下对象级上下文与版本引用**；
- 聊天注入只有两节（S1 项目画像 / S2 仓库事实），**没有对象节**；
- `ai.conversation_interaction` 只有写回（POST），**没有读端点**，右栏 Actions / History 的审批与执行记录目前只能客户端派生。

本任务把三件事一次定清：**① Context Set 建模 ② 注入链扩节 ③ 审批/执行读接口**，并顺带收口 DEF-P0-01 / DEF-P0-02（默认拒绝 + 真人审批守卫）。

---

## 一、现状事实（已实证）

### 1.1 线程锚点（项目 / 仓库作用域）

`migrations/0003_conversation_context_anchor.sql`：`ai.conversation.context_anchor` 为 **JSONB 单列**，白名单 5 键 + 跨项目 CHECK（锚点内 `project_id` 须等于列上的 `project_id`）。

Rust 侧：`crates/pm-application/src/ai_conversations.rs:58` `ContextAnchor`（5 个 `Option<String>` 字段）、`:71` `is_empty()`、`:82` `validate(project_id)`（project_id 须匹配 + 字段长度受限）。

锚点的定位很明确（`chat_context.rs` 模块注释）：

> **锚点不即事实**——锚点是**定位**不是**内容**；仓库名/分支从库里重读，锚点只用于「找哪个仓库」，伪造的锚点查不到即零注入。

### 1.2 聊天注入链（两节 + 独立预算）

`crates/pm-ai/src/chat_context.rs`：

| 常量 | 值 | 含义 |
|---|---:|---|
| `CHAT_TOTAL_BUDGET_CHARS` | 8,000 | chat 段总预算（与访谈 36,000 **独立**） |
| `CHAT_CAP_PROJECT_PROFILE` | 2,000 | S1 项目画像上限 |
| `CHAT_CAP_REPOSITORY_FACTS` | 4,000 | S2 仓库事实上限 |

分节键：`project_profile` / `repository_facts`。注入源 = 线程锚点 → 服务端**重新投影**实时事实（S1 `core.project`、S2 `integration.project_repository`）。

记账：`ChatContext::metadata()` → `{ sections_included, sections_dropped, chars_total }`，写入 `message.metadata.injection`（`conversation_reply.rs:438`）——**前端阶段③已真实消费**。

安全三闸（沿 DK-C §5.5）：注入隔离 · 不注入凭据（D1 静态断言）· 锚点不即事实。

### 1.3 对象域与版本字段

| 域 | 主表 | 版本/修订载体 |
|---|---|---|
| 需求 | `delivery.requirement`（`current_revision_id`、`row_revision`） | `delivery.requirement_revision`（`version integer`、`state`、`content jsonb`、`content_hash`、`markdown_version_ref`） |
| 设计 | `design.design`（`current_revision_id`） | `design.design_revision`（`version integer`、`state`、`content jsonb`、`content_hash`、`base_current_revision_id`） |
| ADR | `design.adr` | `design.adr_revision` |
| 任务 | `delivery.task`（`revision bigint` —— 乐观锁，**不是内容版本**） | 无独立内容版本表 |
| 知识 | `ai.project_knowledge` + `kb` 分节索引（`0008`） | 分节级引用 |

**结论**：需求/设计有**不可变修订**（version + state + hash），任务只有行版本——引用任务时**不能声称内容版本**，只能给 `revision bigint`。

### 1.4 交互写回（只有写，没有读）

`migrations/0005_conversation_interaction.sql` → `ai.conversation_interaction`：`action`（accept/reject/submit/defer/decision）、`status`（accepted/rejected/deferred/submitted/resolved）、`reason`、`option_id`、`answer jsonb`、`decided_by`、`reference`（人类可读记录号 `INT-XXXXXXXX`）、`UNIQUE(message_id)`。

端点：`apps/pm-api/src/ai_conversations.rs` 只有 4 条路由，其中 interaction 仅 **POST** 写回；`apps/pm-api/src/pm_interactions.rs` 仅 POST `/v1/pm/interactions/{id}/responses`。**无任何 GET**。

---

## 二、契约决策（待裁决 D-CX1~CX4）

### D-CX1 · Context Set 用**独立表**承载，不用 JSONB 塞

**建议**：新增 `ai.conversation_context_item` 表；**锚点保持 5 键不变**（线程作用域仍归锚点）。

理由：DR-C7 要求每条 Context 有**独立生命周期**（Active / Inactive / Pending / Unavailable）+ Enable/Disable + 变更历史 + 审计。JSONB 单列做不了「按条目状态机 + 按条目审计 + 按条目版本引用」；新项目无存量数据，可直接建表（无迁移负担）。

**表结构建议**：

```sql
CREATE TABLE ai.conversation_context_item (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    -- 对象定位（跨域统一：type + id + 版本引用）
    object_type text NOT NULL,           -- requirement | design | adr | task | knowledge | document | repository | project
    object_id text NOT NULL,             -- uuid 或稳定 code
    object_code text,                    -- 展示用（REQ-001 / DESIGN-004）
    -- 版本引用（对象级；无版本的对象置空）
    revision_id uuid,                    -- requirement_revision.id / design_revision.id
    version integer,                     -- 该修订的 version
    content_hash text,                   -- 引用时的内容哈希（防「同版本不同内容」）
    -- 归属与生命周期（DR-C7 双轴）
    attachment text NOT NULL DEFAULT 'attached',  -- attached | detached
    lifecycle text NOT NULL DEFAULT 'active',    -- active | inactive | pending | unavailable
    read_only boolean NOT NULL DEFAULT false,   -- 可写性标记（Read-only）
    -- 来源与权限范围（基线 §7.2）
    source text NOT NULL,                -- page | manual | discovery | ai_suggestion
    permission_scope text NOT NULL,      -- 记录判定时的权限范围快照
    unavailable_reason text,             -- lifecycle=unavailable 时的原因（必填）
    added_by uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conversation_context_item_pk PRIMARY KEY (id),
    CONSTRAINT conversation_context_item_attachment_check
        CHECK (attachment = ANY (ARRAY['attached','detached'])),
    CONSTRAINT conversation_context_item_lifecycle_check
        CHECK (lifecycle = ANY (ARRAY['active','inactive','pending','unavailable'])),
    -- Pending 不得自动转 Active 的服务端保证：pending 态不得声称可用
    CONSTRAINT conversation_context_item_unavailable_reason_check
        CHECK ((lifecycle <> 'unavailable') OR (unavailable_reason IS NOT NULL AND length(btrim(unavailable_reason)) > 0)),
    CONSTRAINT conversation_context_item_uniq UNIQUE (conversation_id, object_type, object_id, revision_id)
);
```

**待裁决 CX1**：
- (a) 独立表（建议） vs (b) 扩展锚点 JSONB 加 `objects[]`；
- 若选 (a)：「生命周期」与「归属」是否合成一列（当前建议**两列**——DR-C7 双轴）；
- 唯一键是否含 `revision_id`（同一对象挂两个版本？建议**不允许**，版本变更走 replace）。

### D-CX2 · 注入链扩节：对象节（S3+）

**建议**：在 `chat_context.rs` 增第三类节 `object_context`，来源 = **会话的 Context Set 中 `attachment='attached' AND lifecycle='active'` 的条目**（不是锚点）。

| 项 | 建议 |
|---|---|
| 节键 | `object_context`（可多对象合并为一节，或按对象分节——建议**按对象分节**，键 `object:<type>:<id>`，便于记账与预算裁剪） |
| 投影内容 | 对象标题 + 当前/指定修订正文摘要 + 版本引用（version + hash） |
| 预算 | 新增 `CHAT_CAP_OBJECT_CONTEXT`（建议 2,000）——现 8,000 预算里 S1+S2 上限合计 6,000，**余量 2,000 正好可用**，总预算不改（保持「chat 8k / 访谈 36k 独立」的既有口径） |
| 裁剪 | 超预算按 `updated_at DESC` 保留最近；被裁的进 `sections_dropped`（**前端已消费该字段**，阶段③已接真） |
| 安全 | 沿用三闸：注入隔离（`<context>` 内按数据对待 + 来源标注）· 不注入凭据 · **不注入未确认/未批准的对象正文**——`revision.state` 非 `approved` 时只注入标题与版本，不注入正文 |

**待裁决 CX2**：
- (a) 对象节按对象分节（建议） vs (b) 合并单节；
- (b) 总预算是否从 8,000 提到 10,000（建议**不提**，先用余量）；
- (c) 未批准修订是否只注入标题+版本（建议**是**——否则等于让 AI 看到草稿正文）。

### D-CX3 · 读接口（前端右栏与 AI-02 页面需要）

**建议新增端点**（挂 `apps/pm-api/src/ai_conversations.rs` 或新模块 `ai_context_items.rs`）：

| 方法 | 路径 | 用途 | 前端消费方 |
|---|---|---|---|
| GET | `/v1/pm/projects/{project_id}/conversations/{conversation_id}/context-items` | 列会话的 Context 条目（含状态/版本/来源/权限范围） | 右栏 Context Tab · AI-02 页面 |
| POST | 同上 | 附加对象（可多选；服务端校验对象存在 + 权限 + 归属项目） | AI-02 Add Context |
| PATCH | `.../context-items/{item_id}` | 生命周期流转（enable / disable / pending→active / 标记 unavailable+原因） | AI-02 Enable/Disable |
| DELETE | `.../context-items/{item_id}` | 移除条目（**软删**：置 `attachment='detached'` 留痕，还是物理删？建议**软删**，审计需要） | AI-02 Remove |
| GET | `/v1/pm/projects/{project_id}/context-candidates?type=&q=` | 可选对象（Available 列表；按当前项目 + 权限过滤） | AI-02 Add Context 选择器 |
| GET | `/v1/pm/projects/{project_id}/conversations/{conversation_id}/interactions` | 审批/交互记录（**权威**，替掉客户端派生） | 右栏 Actions / History 的审批类 |

**待裁决 CX3**：
- (a) Remove 软删（建议） vs 物理删；
- (b) 候选对象 GET 是否需要独立端点（建议**要**：跨 5 个域做统一候选聚合，放服务端比前端 N 次请求可靠）；
- (c) interactions 读端点是否并入本任务（建议**并入**：否则右栏 Actions/History 长期只能客户端派生）。

### D-CX4 · DEF-P0-01 / DEF-P0-02 收口（与本任务同区域）

| 缺陷 | 位置 | 现状 | 要求 |
|---|---|---|---|
| DEF-P0-01 | `crates/pm-application/src/ai_team_policies.rs:150` 规则自述「能力类字段取所有表态层交集；**无任何层表态时默认允许**」 | 默认放行 | 改**默认拒绝**：无明确表态 → DENY |
| DEF-P0-02 | `confirm_baseline` | 无真人守卫 | 加真人审批守卫（不可绕过） |

补充要求（DR-C2）：`auto_accept` 语义拆分（可进非正式 Draft / 不可正式审批）；权限取交集（Team ∩ Project ∩ Action Policy）；**修复后必须服务端自动化测试验证**，不以 UI 按钮限制替代。

**待裁决 CX4**：默认拒绝的**爆炸半径**——现存策略数据是否需要一次性迁移/校正（若存量策略本就少，可直接改默认语义 + 回归测试；若多，需一次性校正脚本——**注意本仓规则：新项目无存量数据，不做迁移脚本**，改默认语义即可）。

---

## 三、实施切片建议（评审通过后）

| 切片 | 内容 | 依赖 | 验收 |
|---|---|---|---|
| **B1** | DEF-P0-01 默认拒绝 + DEF-P0-02 真人守卫（含服务端自动化测试） | — | 策略层单测：默认拒绝 / 交集否决 / 真人审批不可绕过 |
| **B2** | `0019_conversation_context_item.sql` + `ContextItem` 模型 + CRUD + 权限/归属校验 + 端点（CX3 前四行） | CX1/CX3 | 集成测试：附加/启停/移除/越权/跨项目 |
| **B3** | 注入链对象节（CX2）+ 记账扩字段 | B2 | 注入测试：对象节内容/预算裁剪/未批准只给标题与版本/三闸不破 |
| **B4** | interactions 读端点（CX3 最后一行）+ 候选对象聚合 GET | CX3 | 集成测试：属主隔离 404 同形 / 候选过滤 |
| **B5** | 前端对接（对象级 Context 面板、AI-02 页面、Sources 真实账） | B2/B3/B4 | 前端门禁全绿 + 右栏不再客户端派生 |

> 切片顺序刻意把 **B1 独立在前**：DEF-P0-01/02 是**已确认的安全边界**，与上下文建模无依赖，先收口收益最大、风险最小。

---

## 四、明确不做（本任务范围外）

| # | 事项 | 原因 |
|---|---|---|
| O1 | 改动 `ai.conversation` 的 `project_id NOT NULL` 约束 | DR-C6 已定：V1 会话必须绑定 Project |
| O2 | 跨项目上下文 / 团队级会话 | DR-C6 排除，需单独评审数据模型与权限架构 |
| O3 | 锚点 5 键的语义重构 | 锚点职责是「线程作用域定位」，对象级另表承载，两者不混 |
| O4 | 自动审批 / AI 自我批准 | DR-C2 红线：禁止 |
| O5 | 移动端 | 本轮桌面 |

---

## 五、给后端的三条提醒

1. **锚点不即事实**这条既有原则要延续到对象：**Context 条目里存的是定位 + 版本引用，不是正文**；正文与状态一律服务端重读（否则「同版本不同内容」会骗过 AI）。
2. **注入预算别动总口径**：chat 8k / 访谈 36k 是既有独立预算，S1+S2 上限合计 6,000，**余量 2,000 够对象节**；扩总预算会连带影响既有注入快照与测试基线。
3. **凡新增表都要属主隔离先例**：`ai.conversation_interaction` 的做法是「无冗余钉列，跨线程/跨项目/跨属主伪造由服务层 join 校验挡住（404 同形）」——新表照此办理，不要给每张表都加 (tenant,project,id) 三元组钉。

---

## 附：关联文档

| 文档 | 用途 |
|---|---|
| `docs/product/ai-pm-ai-collaboration-workspace-v1.md` | 设计基线（附录 B 差距清单 G3/G4/G5/G9/G10、§7.2 Context Set 记录粒度） |
| `docs/ui/ai-collab/smark-pm-aic-95-decision-record-v1.md` | DR-C2（默认拒绝/真人审批/auto_accept 语义）、DR-C3（四 Tab + History）、DR-C7（双轴状态）、DR-C8（四栏渐进迁移） |
| `docs/ui/ai-collab/smark-pm-aic-91-phase1-prototype-contract-v1.md` | 第一阶段契约（专注模式、渐进迁移、19 步演示、验收清单） |
| `docs/smark-pm-ai-collaboration-shell-migration-assessment-v1.md` | 壳迁移评估与实施结果（四阶段已落地，含存量缺陷记录） |
| `docs/ui/ai-collab/smark-pm-aic-ui-02-context-management-v1-3.md` | AI-02 页面设计稿（双轴状态、Add Context 多选、摘要优先、详情入 Drawer） |
