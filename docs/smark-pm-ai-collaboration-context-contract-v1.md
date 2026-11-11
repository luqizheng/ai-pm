# AI Collaboration Workspace V1.0 — 对象级上下文与审批读接口（后端契约任务书）

| 项目 | 内容 |
|---|---|
| 日期 | 2026-11-11 |
| 任务性质 | **后端契约 + 实施设计**（先定契约，再实施） |
| 触发 | 四栏 Shell 迁移四阶段已落地（前端）；AI-02 的对象级上下文**卡在后端** |
| 关联 | [设计基线](product/ai-pm-ai-collaboration-workspace-v1.md)（附录 B 差距 G3/G4/G5/G9/G10）· [裁决 DR-C2/C3/C7/C8](ui/ai-collab/smark-pm-aic-95-decision-record-v1.md) · [壳迁移评估任务书](smark-pm-ai-collaboration-shell-migration-assessment-v1.md) |
| 产出 | 本文件即**正式契约**（架构评审已通过 CX1~CX4；SQL-01~SQL-08 为实施前置约束） |
| 评审 | **架构评审通过（2026-11-11）**：CX1~CX4 全数裁定；SQL 与 API 契约按评审意见修订后实施 |
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

**表结构（评审修订版 · SQL-01~SQL-08 逐条落点）**：

```sql
-- 0019_conversation_context_item.sql
CREATE TABLE ai.conversation_context_item (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    -- ── 对象定位（多态；**不存正文**）────────────────────────────
    object_type text NOT NULL,
    object_id text NOT NULL,
    object_code text,
    -- ── 版本引用（**按类型解释**，见 SQL-01；无内容版本的对象留空）──
    revision_id uuid,             -- requirement_revision.id / design_revision.id / …
    version integer,              -- 该修订的版本号（**不得为乐观锁伪造**）
    content_hash text,             -- 引用时内容哈希（注入时服务端复核，见 SQL-07）
    row_version bigint,            -- 仅乐观锁类对象（如 task）记此，**不参与版本语义**
    -- ── 归属轴（**只有两态**）──────────────────────────────────
    attachment text NOT NULL DEFAULT 'attached',   -- attached | detached
    -- ── 生命周期轴 ────────────────────────────────────────────
    lifecycle text NOT NULL DEFAULT 'active',     -- active | inactive | pending | unavailable
    -- ── 可写性（**独立于 lifecycle**，不随 PATCH 切换）────────────
    read_only boolean NOT NULL DEFAULT false,
    -- ── 来源与不可用原因 ──────────────────────────────────────
    source text NOT NULL,                            -- page | manual | discovery | ai_suggestion
    unavailable_reason text,                         -- 进入 unavailable 必填（SQL-03 约束）
    added_by uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conversation_context_item_pk PRIMARY KEY (id),
    CONSTRAINT conversation_context_item_attachment_check
        CHECK (attachment = ANY (ARRAY['attached','detached'])),
    CONSTRAINT conversation_context_item_lifecycle_check
        CHECK (lifecycle = ANY (ARRAY['active','inactive','pending','unavailable'])),
    -- 离开 unavailable 时原因须清空（不得留误导性当前原因）
    CONSTRAINT conversation_context_item_unavailable_reason_check
        CHECK ((lifecycle = 'unavailable') = (unavailable_reason IS NOT NULL)),
    -- 同一会话不重复附加同一对象；revision_id 可空 → NULLS NOT DISTINCT（PG16）
    CONSTRAINT conversation_context_item_uniq
        UNIQUE NULLS NOT DISTINCT (conversation_id, object_type, object_id, revision_id),
    CONSTRAINT conversation_context_item_fk_conversation
        FOREIGN KEY (conversation_id) REFERENCES ai.conversation (id)
);
```

> **归属字段口径（评审要求：服从既有属主隔离模式，不为查询方便加冗余）**：
> 照 `ai.conversation_interaction` 的做法——保留 `tenant_id` / `project_id` 列，
> FK 只钉 `conversation_id`，**不建** (tenant,project,id) 三元组唯一键；
> 「item 与其 conversation 同租户同项目」由**服务层 join 校验**（SQL-06），
> 跨属主访问按既有先例 **404 同形**。

### SQL-01 ~ SQL-08（实施前置约束，缺一不得开工）

| # | 修改项 | 落地要求 |
|---|---|---|
| **SQL-01** | **多态版本引用** | 不得在一个通用表里假装所有对象都有 `revision_id UUID + version INTEGER`。按下表**分类型**解释与校验；**没有正式内容版本的对象不得伪造内容版本** |
| **SQL-02** | **唯一约束** | `UNIQUE NULLS NOT DISTINCT (conversation_id, object_type, object_id, revision_id)`（PG16 支持），解决 `revision_id` 可空时普通 UNIQUE 放行重复的问题 |
| **SQL-03** | **双轴状态** | `attachment`（attached/detached）与 `lifecycle`（active/inactive/pending/unavailable）**分列**；`read_only` 独立列；**Available 不入库**（是候选查询结果）；`unavailable_reason` 与 `lifecycle='unavailable'` **同生同灭**（离开即清空，不留误导性当前原因） |
| **SQL-04** | **软移除** | DELETE 语义 = 移除出当前 Conversation（`attachment='detached'`），**不删原始业务对象**、**不丢版本引用与操作记录**；移除事件进 History |
| **SQL-05** | **权限校验** | 所有对象操作在**服务端**校验存在性 / 项目归属 / 权限；**不信任客户端传入的权限快照**（`permission_scope` 只作记录，不作判据） |
| **SQL-06** | **项目隔离** | Context Item 的 `project_id` / `tenant_id` 须与其 Conversation 一致（服务层 join 校验）；跨项目/跨属主访问 **404 同形** |
| **SQL-07** | **版本一致性** | 注入时**重新读取**被引用修订并**复核版本与 content_hash**；不采信 Context Item 里存的历史 hash |
| **SQL-08** | **审计** | 附加 / 启用 / 停用 / 移除 / 状态异常均可追溯（写入 `ai.conversation_interaction` 或独立审计表，见下） |

**SQL-01 分类型版本引用矩阵**（本仓已实证的版本载体）：

| object_type | 定位 | 版本载体 | 注入正文的前提 |
|---|---|---|---|
| `requirement` | `delivery.requirement.id` | `delivery.requirement_revision`（`version` + `state` + `content_hash`） | 该修订 `state='approved'` 才注入正文；否则只给标题/版本/状态 |
| `design` | `design.design.id` | `design.design_revision`（同构） | 同上 |
| `adr` | `design.adr.id` | `design.adr_revision` | 同上 |
| `knowledge` | `ai.project_knowledge.id` | `version integer` + `status` + `confirmed_by/confirmed_at` | 已确认（`confirmed_by` 非空）才注入正文 |
| `document` | `docproc.document.id` | `docproc.document_version`（`version_number` + `checksum`；**append-only**） | 引用指定版本并复核 checksum |
| `task` | `delivery.task.id` | **无内容版本**（`revision bigint` 是乐观锁）→ 记 `row_version`，**不参与版本语义** | 注入任务标题/状态/验收摘要；**不伪造内容版本** |
| `repository` | `integration.project_repository.name` | 无（实时投影） | 走 S2 仓库事实节，**不进对象节** |
| `project` | `core.project.id` | 无（实时投影） | 走 S1 项目画像节，**不进对象节** |

> **审批约束的适用范围（评审修正）**：「未批准正文不注入」**只对存在正式审批要求的对象**
> 成立（requirement / design / adr / knowledge / document 的正式修订）。Project 与
> Repository 事实本身没有 approved 修订状态，**不得把审批状态机套到它们身上**。

### D-CX2 · 注入链扩节：对象节（S3+）

**建议**：在 `chat_context.rs` 增第三类节 `object_context`，来源 = **会话的 Context Set 中 `attachment='attached' AND lifecycle='active'` 的条目**（不是锚点）。

| 项 | 建议 |
|---|---|
| 节键 | `object_context`（可多对象合并为一节，或按对象分节——建议**按对象分节**，键 `object:<type>:<id>`，便于记账与预算裁剪） |
| 投影内容 | 对象标题 + 当前/指定修订正文摘要 + 版本引用（version + hash） |
| 预算 | 新增 `CHAT_CAP_OBJECT_CONTEXT`（建议 2,000）——现 8,000 预算里 S1+S2 上限合计 6,000，**余量 2,000 正好可用**，总预算不改（保持「chat 8k / 访谈 36k 独立」的既有口径） |
| 裁剪 | 超预算按 `updated_at DESC` 保留最近；被裁的进 `sections_dropped`（**前端已消费该字段**，阶段③已接真） |
| 安全 | 沿用三闸：注入隔离（`<context>` 内按数据对待 + 来源标注）· 不注入凭据 · **不注入未确认/未批准的对象正文**——`revision.state` 非 `approved` 时只注入标题与版本，不注入正文 |

**评审裁定（CX2 = A，通过）**：采用**按对象分节**，**不提高现有 8,000 总预算**。

**注入节与预算（终裁）**：

| 节 | 键 | 预算 |
|---|---|---:|
| 总预算 | — | **8,000（不变）** |
| S1 项目画像 | `project_profile` | 2,000 |
| S2 仓库事实 | `repository_facts` | 4,000 |
| S3+ 对象上下文 | `object:<type>:<id>` | **2,000**（原余量，不新增） |

**注入规则矩阵（终裁）**：

| 对象状态 | AI 可获得的内容 |
|---|---|
| Attached + Active，且对象可读 | 按权限投影对象内容（已批准不可变修订可注入正文摘要） |
| Inactive | **不注入** |
| Pending | **不注入正式对象正文** |
| Unavailable | **不注入**，但记录原因（进记账/事件） |
| Detached | **不注入** |
| 受正式审批约束、修订未批准 | **只注入**允许公开的标题 / 版本 / 状态元数据，**不注入草稿正文** |

> 「未批准不注入正文」**只适用于存在正式审批要求的对象**（requirement / design /
> adr / knowledge / document）；Project 与 Repository 事实无 approved 修订状态，
> 不得套用同一审批状态机。

**超预算裁剪（评审追加的两条约束）**：
1. 按 `updated_at DESC` 保留较新对象，**且必须有稳定的第二排序键**（建议
   `context_item.id`）——保证同样输入下结果**可重复**；
2. **每个对象节独立记账**：被裁剪的对象进 `sections_dropped`（前端已消费该字段，
   阶段③已接真），不能只记总字符数。

**版本一致性（SQL-07 落到注入层）**：`content_hash` **必须与实际注入的修订内容对应**。
服务端**重新读取**被引用的修订并**复核版本与 hash**，不采信 Context Item 里保存的
旧 hash——避免「Context 引用与对象内容不一致」。

**安全原则不变**：注入隔离 · 不注入凭据 · 锚点不即事实（三闸均不因扩节而放松）。

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

**评审裁定（CX3 = A，通过）**：采用独立 Context Item API，并把 **interactions 读接口**
与**候选对象聚合查询**一并纳入本任务。

**契约细节（评审补充）**：

| 方法 | 路径 | 契约要点 |
|---|---|---|
| GET | `.../context-items` | 返回归属、生命周期、只读属性、版本引用、来源、不可用原因 |
| POST | `.../context-items` | **支持批量附加**；逐项校验存在性 / 权限 / 项目归属 / 版本 |
| PATCH | `.../context-items/{item_id}` | **状态机白名单**（见下），不接受客户端任意 lifecycle 字符串 |
| DELETE | `.../context-items/{item_id}` | **软移除** → `attachment='detached'`，保留审计 |
| GET | `/v1/pm/projects/{project_id}/context-candidates` | 类型筛选 · 关键词 · **分页** · 已附加标识 · 可用性 · 只读标识 · 简短摘要（**服务端聚合**，前端不得分别请求五个业务域再自行拼权限） |
| GET | `.../interactions` | 审批/交互**权威**读数据源 |

**PATCH 状态机（终裁；`read_only` 不在此列——它由来源对象权限计算，不随 PATCH 切换）**：

| 转换 | 允许条件 |
|---|---|
| Active → Inactive | 允许授权用户执行 Disable |
| Inactive → Active | **必须重新检查**对象可读权限与版本有效性 |
| Pending → Active | **必须满足相应人为确认要求**，不得由普通 AI 操作直接完成 |
| 任意 → Unavailable | 必须有**服务端判定依据与原因**（`unavailable_reason` 必填） |
| Unavailable → Active | 必须重新检查对象可用性、权限与版本 |
| Detached → 恢复 | **不允许**通过 Enable 直接恢复；须**重新附加并重新校验** |

**软移除语义（终裁）**：移除 = 从当前 Conversation 移除，**不是**删除原始
Requirement / Design / Knowledge 等业务对象。移除后：不再注入 AI · 不再出现在
Attached 列表 · History 中可见移除事件 · **不删原始业务对象** · **不丢版本引用与操作记录**。

**interactions 读端点（终裁）**：返回真实交互记录——操作者 · 原始 Action · 处理状态 ·
处理时间 · 处理理由 · 关联消息 · 关联对象 · 审批/拒绝结果；**保持既有属主隔离**：
跨 Project / 跨 Conversation / 跨属主访问不得泄露资源是否存在，按既有约定 **404 同形**。

### D-CX4 · DEF-P0-01 / DEF-P0-02 收口（与本任务同区域）

| 缺陷 | 位置 | 现状 | 要求 |
|---|---|---|---|
| DEF-P0-01 | `crates/pm-application/src/ai_team_policies.rs:150` 规则自述「能力类字段取所有表态层交集；**无任何层表态时默认允许**」 | 默认放行 | 改**默认拒绝**：无明确表态 → DENY |
| DEF-P0-02 | `confirm_baseline` | 无真人守卫 | 加真人审批守卫（不可绕过） |

补充要求（DR-C2）：`auto_accept` 语义拆分（可进非正式 Draft / 不可正式审批）；权限取交集（Team ∩ Project ∩ Action Policy）；**修复后必须服务端自动化测试验证**，不以 UI 按钮限制替代。

**评审裁定（CX4 = A，通过）**：与本任务**同批修复**，且 **B1 必须排在最前面**。

- **DEF-P0-01 默认拒绝**：无任何策略层表态时默认允许 → 改为「**没有明确授权 = DENY**」。
  有效权限 = `Team Policy ∩ Project Policy ∩ Action Policy`；**任何一层拒绝即最终拒绝；
  任何一层没有明确授权，也不得被解释为允许**。
- **DEF-P0-02 真人审批守卫**（服务端强制）：Requirement 正式确认/发布 · Revision 正式批准 ·
  Knowledge 正式确认/发布 · Decision 正式确认。AI 可提出 / 分析 / 生成草稿 / 提交审批请求，
  但**不能批准自己的提案**，也不能经其他接口绕过人工审批。
- **auto_accept 语义拆分**：① 接受 AI 建议进 Draft（按策略授权）② 正式审批并发布
  （**必须真人执行**）。
- **存量策略**：新项目、无存量策略数据 → **直接改默认语义，不写一次性校正脚本**；
  但**必须对所有策略配置路径做回归测试**，避免其他代码路径继续默认放行。
- **验收**：不得只测 UI —— 必须有**服务端自动化测试**证明默认拒绝与真人审批守卫不可绕过。

## 三、实施切片（评审终裁顺序）

| 切片 | 内容 | 依赖 | 验收 |
|---|---|---|---|
| **B1** | **安全边界修复**：DEF-P0-01 默认拒绝 · DEF-P0-02 真人审批守卫 · auto_accept 语义拆分 | — | 策略层单测：默认拒绝 / 交集任一层否决 / 真人审批不可绕过 / **所有策略配置路径回归** | ✅ 已落地（`27fad9b6`） |
| **B2** | **Context Item 数据模型与 CRUD**：迁移 0019 · Rust 模型 · 双轴状态机（PATCH 白名单）· 批量附加 · 权限与归属校验 · 软移除 · Context API | CX1/CX3 | 集成测试：附加/启停/移除/软删留痕/**越权**/**跨项目 404 同形** | ✅ 已落地（`57203cec`） |
| **B3** | **对象级 AI 注入**：S3+ 对象节 · 版本校验（SQL-07 重读复核）· 内容投影 · 预算裁剪（稳定次排序 + 独立记账）· 注入记账 | B2 | 注入测试：各状态注入矩阵 / 超预算裁剪可重复 / 未批准只给元数据 / 三闸不破 | ✅ 已落地（本次提交） |
| **B4** | **真实读取接口**：interactions GET · 候选对象聚合查询（类型/关键词/分页/已附加/可用性/只读/摘要）· 权限过滤 · 属主隔离 | CX3 | 集成测试：字段完整 / 候选过滤 / 404 同形 | ✅ 已落地（本次提交） |
| **B5** | **前端对接**：AI-02 Context · Sources · Results · Actions · History **全部改读真实后端数据，不再客户端派生权威状态** | B2/B3/B4 | 前端门禁全绿 + 右栏派生视图降级为**非权威展示** | ✅ 已落地（`0667cab8`） |

> B1 独立在最前：它是**已确认的安全边界**，与上下文建模无依赖，先收口收益最大、风险最小。

### B5 实施结果（2026-11-12 · `0667cab8`）

| 项 | 落地 |
|---|---|
| 权威读面 API | `api/contextItems.ts`：GET context-items / interactions / context-candidates（ContextItem 双轴类型 · InteractionEntry 权威字段 · Candidate 聚合形状） |
| store 权威化 | `assistant.actions` 改为权威 `InteractionEntry[]`；新增 `contextItems`；deriveView 收敛为 deriveResults（Results 仅展示面，非权威状态） |
| 会话机制接线 | publish 拉权威读面（与消息轮询同节奏 + 写回后即时刷新）；失败不打断对话（保留旧态） |
| History 五类 | ai_action / approval ← interactions（**accepted/rejected → approval 类**）；conversation ← 会话列表；context_change ← 注入记账；**execution 如实留空**（V1 无执行读端点，不用推断冒充——契约 90 Part 8） |
| 右栏渲染 | Actions：`action · reference · 处理者`（权威字段）；Context Tab：attached 条目 + 版本引用（`v{version}` / `row {row_version}`），detached 不出现 |
| 门禁 | 前端 187 文件 / **1074 例全绿** · vue-tsc · eslint · i18n（4990 key） |

**B1~B5 全部完成。** AI-02 对象级上下文链路（数据模型 → 注入 → 权威读 → 前端）端到端打通。

---
### B4 实施结果（2026-11-12）

| 项 | 落地 |
|---|---|
| interactions GET | `GET /v1/pm/projects/{pid}/conversations/{cid}/interactions`——操作者 / 原始 action / 处理状态 / reason / option_id / answer / 记录号 / 关联消息摘要（160 字符）；**属主隔离 404 同形**；Actions / History 的权威读源 |
| candidates GET | `GET /v1/pm/projects/{pid}/context-candidates?type=&q=&page=&size=&conversation_id=`——**type 必选单值**（AI-02 按类型分页签，单域一条 SQL，不做跨六域 UNION 分页）；ILIKE 关键词；分页 {items,total}；conversation_id 给出时逐项算 attached（detached 不算，可再次附加）；version_label 带 max(revision) |
| 域过滤 | requirement/design lifecycle=active · adr 排除 superseded/deprecated/rejected · knowledge active · task 排除 cancelled/superseded · document 按项目归属 |
| 测试 | +4（交互权威字段完整 / 跨属主 404 同形 / 候选过滤+分页+attached 标识+未知类型 400 / 多域覆盖） |
| 门禁 | 全仓 **1527 例通过 / 0 失败** |

---
### B3 实施结果（2026-11-12）

| 项 | 落地 |
|---|---|
| 注入矩阵 | `load_injection_sources`：只取 attached+active；requirement/design/adr 仅 approved 修订注入正文（1,400 字符截断）；knowledge 行级已确认注入正文；document 仅元数据（正文在文件存储）；task 仅元数据 + 行版本漂移如实标注；未批准**只给元数据** |
| 重读复核（SQL-07） | 正文与状态一律服务端重读（修订实值 / 对象行实况），不采信条目快照；task 行版本漂移在 meta 标注「row version advanced」 |
| 装配与记账 | `build_chat_context_with_objects`：对象节键 `object:<type>:<id>`；预算 2,000（不提总预算）；放不下整节裁掉、**键进 sections_dropped（逐对象记账）**；排序 updated_at DESC + id ASC（稳定次排序，结果可重复） |
| 接线 | conversation_reply::build_injection 加载条目并装配；**无条目 = 行为与 B2 前一致**（空切片不产生对象节） |
| 测试 | +4（投影 2：approved/draft/knowledge/task/过滤；装配 2：预算裁剪逐对象记账 / 空切片旧行为逐字节一致） |
| 门禁 | 全仓 **1523 例通过 / 0 失败** |

---
### B2 实施结果（2026-11-12 · `57203cec`）

| 项 | 落地 |
|---|---|
| 迁移 0019 | `ai.conversation_context_item`：双轴分列（attachment 两态 / lifecycle 四态）· read_only 独立列 · `UNIQUE NULLS NOT DISTINCT`（SQL-02）· unavailable_reason 与状态同生同灭（SQL-03）· 版本列可空 + row_version 专列（SQL-01）· 软移除语义（SQL-04） |
| 对象校验 | `validate_object_reference` 按分类型矩阵逐类实现（requirement/design/adr 修订复核 version+hash · knowledge 行内版本 · document version_number+checksum · **task 仅行版本，拒绝伪造内容版本**）；写入即复核（SQL-05/07 写入半边）；对象必须归属本项目（SQL-06） |
| 端点 | GET 列表（含 include_detached 供 History）· POST 批量附加（≤20，逐项校验，重复 409）· PATCH lifecycle（**状态机白名单**；Detached 不可 Enable；unavailable 必填原因、离开即清空）· DELETE 软移除（不删业务对象、审计留痕） |
| 真人门 | **pending → active 必须 `core.principal.kind='human'`**（DR-C2；fail-closed） |
| 审计 | attach / lifecycle / remove 逐笔走 `pm_write::append_audit_with_after`（四身份列；SQL-08） |
| 测试 | `context_items.rs` 5 例：附加+列表+重复 409+不存在 400 · 任务拒绝内容版本字段+row_version 复核 · 状态机全流转（含真人门与原因同生同灭）· 软移除留痕+审计 2 条 · **跨属主 404 同形**（peer 激活+入项目成员后读写均 404，属主不误伤） |
| 门禁 | 全仓 **1519 例通过 / 0 失败**（较 B1 +5） |

### B1 实施结果（2026-11-11 · `27fad9b6`）

| 项 | 落地 |
|---|---|
| DEF-P0-01 默认拒绝 | `policy_composition::resolve_capability` 改为默认拒绝（适用层内沉默 = 未授权；无策略行 = 拒绝）；**policy schema 升 `composed-v2`**（语义变更 → 同 hash 不得不同含义）；对外规则文案同步改写 |
| DEF-P0-02 真人守卫 | `confirm_baseline` 加确认权三重门（interactive / 本地开发 off 放行；api_key、unknown 拒绝）+ `core.principal.kind='human'` + 项目可见性 404；**后端强制，不可绕过** |
| auto_accept 语义拆分 | 自动接受进草稿与正式审批分列判定（`auto_accept && !require_human_approval` 保持；正式审批另走真人门） |
| 回归证据 | 单测（默认拒绝两例 + 空层集改口径）· `b1_policy_guard.rs` 三例（api_key 403 / 真人 200 且确认人是真人 / AI 主体永不得为确认人）· `review` C100 显式授权 + **C102 新增默认拒绝走审批门** · `run_dispatch` 显式授权 + 默认拒绝快照用例 · 读面空态断言改默认拒绝并校验对外文案 |
| 顺手修的存量缺陷 | ① `devenv` 设备并发登记：INSERT 只以 `(id)` 为仲裁索引，而表同时有 `PK(id)` 与 `UNIQUE(tenant_id,id)` → 并发冲突可能先命中复合唯一约束而错误逃逸；改为不指定仲裁目标 ② 审计清单补登 `team_llm_catalog.rs` 新增审计点 |
| 门禁 | 全仓 **1514 例通过 / 0 失败**；我改的文件 rustfmt 干净 |
> B5 的硬要求：**Actions / History 不得继续用客户端推断的状态充当权威数据**。

## 四、明确不做（本任务范围外）

| # | 事项 | 原因 |
|---|---|---|
| O1 | 改动 `ai.conversation` 的 `project_id NOT NULL` 约束 | DR-C6 已定：V1 会话必须绑定 Project |
| O2 | 跨项目上下文 / 团队级会话 | DR-C6 排除，需单独评审数据模型与权限架构 |
| O3 | 锚点 5 键的语义重构 | 锚点职责是「线程作用域定位」，对象级另表承载，两者不混 |
| O4 | 自动审批 / AI 自我批准 | DR-C2 红线：禁止 |
| O5 | 移动端 | 本轮桌面 |

---

## 五、评审裁决汇总（2026-11-11）

| 裁决项 | 最终选择 |
|---|---|
| CX1 Context Set | **A：独立表**（不扩锚点 JSONB） |
| CX1 状态模型 | **双轴保留**（attachment 与 lifecycle 分列，read_only 独立） |
| CX1 Available | **候选查询状态，不入 Context Item**（存储态只有 attached / detached） |
| CX2 对象注入 | **A：按对象分节**（`object:<type>:<id>`） |
| CX2 总预算 | **保持 8,000**（S1 2000 / S2 4000 / S3+ 2000） |
| CX2 未批准正文 | **不注入**（仅对存在正式审批要求的对象适用） |
| CX3 Remove | **软移除**（→ detached，保留审计，不删业务对象） |
| CX3 Candidate GET | **纳入**（服务端聚合，前端不自拼五域） |
| CX3 Interactions GET | **纳入**（Actions / History 的权威读源） |
| CX3 PATCH | **状态机白名单**（read_only 不随 PATCH 切换） |
| CX4 默认拒绝 | **必须修复**（无明确授权 = DENY） |
| CX4 真人审批 | **必须服务端强制**（不可绕过） |
| CX4 存量迁移 | **不做一次性校正脚本**（新项目无存量策略） |

**架构方向通过；SQL 与 API 契约修订后实施**（SQL-01~SQL-08 为开工前置，缺一不得动手）。

### 评审追加的防复发要求（务必写进实现）

1. **不得为迁就 UI 把对象正文 / 状态 / 权限塞进 JSONB** —— 否则会重新出现
   「前端显示 ≠ AI 实际注入」的不一致。Context Item 只存定位与版本引用。
2. **Context Item 存的是定位与版本引用，不是正文** —— 正文与状态一律服务端重读。
3. **不采信客户端传入的权限快照**（SQL-05）—— 权限判定只在服务端。

---

## 六、给后端的三条提醒

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
