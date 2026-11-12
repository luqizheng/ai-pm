# AI-PM Visualization V0-3 技术设计（权限投影 · 一致性语义 · 图形性质）V1.0

> 状态：设计稿（证据齐备，待 Architecture/Security Owner 签收） · 日期：2026-11-12
> 上游：[ADR V1.0.1](ai-pm-viz-09-architecture-decision-record-v1.md)（ADR-VIZ-009、闭合 R-06 / R-07 / R-08 部分）· [13 号 V0-2 数据模型](ai-pm-viz-13-v0-2-data-model-design-v1.md)· [12 号 V0-1 校验器设计](ai-pm-viz-12-v0-1-spec-validation-design-v1.md)
> 落地约定：本文所有"仓库事实"均来自 2026-11-12 对 `D:\kudi-project\templates\ai-pm` 的实证（路径:行号随附）。

## 1. 范围与闭合目标

**交付物**：图查询的**双层权限过滤**实现规范、**一致性语义**定义与实现路径、**nature 粒度**定义、ObjectResolver 的实现落点与 SQL 草案、泄露面清单与对应用例。

**闭合**：
- **R-06** nature 粒度（Artifact / Node / Edge 各层语义）
- **R-07** `consistency=current` 的实际语义与降级规则
- **Q11 / ADR-VIZ-009** 权限过滤在查询层与投影层双层执行，边存在性不泄露

**非目标**：渲染（V0-9）· 前端（V0-8）· 领域表结构变更（**图形层不得要求业务加表**，ADR-VIZ-005）。

## 2. 权限模型现状（仓库事实）

| 事实 | 证据 |
|---|---|
| 可见性唯一事实源 = `core.project_member`（同租户内成员可见；跨租户与同租户非成员一律按不存在处理；项目 `status='deleted'` 双重排除） | `apps/pm-api/src/support.rs:175-196` |
| `require_project_visible` 不可见即 **404**（注释："不泄露存在性"） | `support.rs:199-210` |
| 错误体只有 `{"message"}`，不带 code | `support.rs:1-6,14-49` |
| 机器凭据（`auth_mode=="api_key"`）访问确认类端点 → **403** | `support.rs:88-98`；调用点 `pm_interactions.rs:65` |
| 真人门在服务层二次断言（`core.principal.kind='human'`） | `crates/pm-application/src/requirement.rs:203-226` |
| 角色枚举含 owner/member/ai_pm/architect/developer/tester，但**实际授权只认 owner 与 member**（ai_pm 等是 Agent 职能行，不得混入租户角色） | `migrations/0001_baseline.sql:1287-1293`；`crates/pm-domain/src/tenant.rs:4-6`；`approver.rs:36-44`；`pm_runs.rs:74-95` |
| 租户角色 owner/admin/member；唯一授权事实源是库内 `tenant_member.role` | `0001:1392-1400`；`apps/pm-api/src/auth.rs:503-512` |
| **知识库没有 visibility 字段**：KB 可见性 = 项目成员可见；draft 修订恒禁读；密级仅留钩子未实现 | `0001:2860-2871`（无 visibility 列）；`crates/pm-application/src/retrieval/validator.rs:41-47,58-66` |
| 双端隔离范式存在：关系两端端点都要过 `pc.endpoint_in_scope(...)`（按 ext_domain 分派到各域对象校验归属） | `migrations/0007_relationship_owner_domain_seeds.sql:5-8,82-151`；`crates/pm-application/src/retrieval/traversal.rs:65-68` |
| `pc.relationship_type.owner_domain`（pc/kb）**目前不参与任何查询过滤**，只用于注册表语义 | `0007:32-37,63-74`；`crates/pm-application/src/pc/mod.rs:244-274` |

### 2.1 关键缺口（直接决定 V0-3 的设计形态）

**既有读端点普遍不做项目级裁剪**：

| 读路径 | 实际过滤 | 缺口 |
|---|---|---|
| `design.list_design_requirements` | `WHERE dr.tenant_id=$1 AND dr.design_revision_id=$2` | join 的 requirement **无 project 裁剪**；`_design_id` 参数被刻意忽略（`crates/pm-application/src/design.rs:1122-1146`） |
| `delivery.task_requirement` traces | `WHERE tr.tenant_id=$1 AND tr.task_id=$2` | join 的 requirement **无 project 裁剪**（`crates/pm-application/src/task_read.rs:169-196`；count 同款 `:762-768`） |
| `requirement_impact`（跨表读） | 3 条独立 SQL，仅 tenant+requirement | design/task 侧**无 project 过滤**，且**无事务**（`crates/pm-application/src/requirement_read.rs:238-313`） |
| `pc.relationship` 遍历 | 有 tenant+project + 双端 `endpoint_in_scope` | ✅ 仅此一处做了双端隔离 |

**结论**：写侧有"同项目"硬约束（`crates/pm-application/src/pm_write/task.rs:595-612` 拒绝跨项目 requirement），但**读侧不复核**。V0-3 的投影层必须**自建第二层裁剪，不能依赖写侧不变量**（这是本设计的第一条硬约束）。

## 3. 双层过滤设计

### 3.1 L1 查询层（取数即裁剪）

每个对象/边的取数 SQL **必须**自带归属过滤：

| 层 | 规则 | 依据 |
|---|---|---|
| L1-a 租户/项目 | `WHERE tenant_id = $1 AND project_id = $2`（显式，不依赖调用点传参） | `0013:62-63`（事件行从 run 行快照 tenant/project，不信任调用点传参） |
| L1-b 端点双端隔离 | 边两端对象都要过可见性判定（跨项目引用不得渗入） | `0007:5-8`（"否则跨项目引用的脏数据会经端点连接渗入遍历结果"） |
| L1-c 生命周期 | 只取 `status='active'` / `published` / `state` 有效态；draft 恒禁 | `retrieval/validator.rs:41-47` |
| L1-d 关系类型白名单 | 只取已登记且 status='active' 的 relation（Registry） | `retrieval/traversal.rs:63-64` 同款 |

**V0-3 实现方式**：不复用既有未裁剪的读函数（`list_design_requirements` 等），而是**为图查询新建专用聚合模块**，其中每条 SQL 自带 L1 全部条件。原因：既有读函数的语义（对外 API 契约）不能改，而图查询的裁剪要求更强（§2.1）。

**SQL 草案（追踪图，Task→Requirement→Design→ADR）**：

```sql
-- L1：需求 + 当前修订
SELECT r.id, r.revision_id, r.title, r.status, r.content_hash
FROM delivery.requirement_revision r
JOIN delivery.requirement req ON req.id = r.requirement_id
WHERE req.tenant_id = $1 AND req.project_id = $2
  AND req.id = ANY($3)
  AND r.id = (SELECT current_revision_id FROM delivery.requirement req2 WHERE req2.id = req.id);

-- L1：设计↔需求绑定（带端点 project 校验，修复既有缺口）
SELECT dr.design_revision_id, dr.requirement_id, dr.requirement_revision_id
FROM design.design_requirement dr
JOIN delivery.requirement req ON req.id = dr.requirement_id
JOIN design.design_revision dv ON dv.id = dr.design_revision_id
WHERE dr.tenant_id = $1
  AND req.project_id = $2          -- ← 端点项目校验（既有读函数缺此条件）
  AND dv.project_id = $2           -- ← 端点项目校验
  AND dr.design_revision_id = ANY($4);
```
> 端点 project 校验条件是 V0-3 相对既有读函数的**关键增量**：它把"写侧不变量"变成"读侧强制"。

### 3.2 L2 投影层（组装图时再裁一次）

即便 L1 已过滤，投影层仍需二次裁剪，因为**权限可能在取数与渲染之间变化**，且图是跨域拼装的：

```
原始聚合结果
  → L2-a 逐节点复核可见性（resolver.exists_and_visible）
  → L2-b 移除不可见节点
  → L2-c 移除端点缺失的边（**不留半边**）
  → L2-d 重算所有计数（节点数/边数/度数/分组大小）
  → L2-e 标注悬空边为 W 级（供 UI 显式呈现）
  → L2-f 生成截断元数据（truncated + truncation_reason）
```

| 步骤 | 规则 | 依据 |
|---|---|---|
| L2-b | 无权节点整节点移除（连带其全部入边/出边） | ADR-VIZ-009 |
| L2-c | **整边丢弃，不输出半边**（不出现"有起点无终点"的悬空边） | `crates/pm-application/src/retrieval/assembly.rs:163-181`（`dropped_traversal_path_count` + 整边丢弃） |
| L2-d | 计数在裁剪**之后**重算 → 计数只反映"你能看到多少"，不因无权对象的存在而变化 | ADR-VIZ-009（关系计数不得泄露） |
| L2-e | 因权限裁剪产生的边 → `W_SPEC_DANGLING_EDGE`（12 号 §4.3），UI 明示"部分关系因权限未展示" | ADR-VIZ-009 |
| L2-f | 截断显式化：`truncated` + `truncation_reason` + 丢弃计数（范式：`traversal.rs:187-210` 的 `no_active_edge`/`candidate_budget_exceeded`/`traversal_depth_exceeded`；`assembly.rs:163-181`） | 同上 |

**关键性质**：L2-d 的计数重算使**不同权限用户看到的同一张图，其节点数/边数只反映各自可见部分**——但这本身可能通过"计数差异"泄露"存在某些不可见对象"。因此：

> **决策（需 Security Owner 确认，D-2）**：默认**不返回**因权限裁剪而减少的计数。即响应中的 `total_nodes` / `total_edges` 表示"**你可见范围内**的图规模"，而不是"全量图规模"；**不提供** "全量规模"字段。否则计数差异将成为探测通道。

### 3.3 错误与不可见的统一语义

| 情形 | 对外表现 | 依据 |
|---|---|---|
| 对象无权 | **等同不存在** → 图中不出现；校验阶段命中 `E_SPEC_UNKNOWN_OBJECT_REF`（**不得出现"无权"字样**） | `pm_requirement_read.rs:77-93`（404 同形）；12 号 §4.4 |
| 跨项目请求 | 404 `{"message": "project not found"}` | `support.rs:199-210` |
| 机器凭据访问写侧 | 403 `{"message": "…requires a trusted interactive context"}` | `support.rs:88-98` |
| 真人门 | 服务层 `require_human`（`core.principal.kind='human'`）二次断言 | `requirement.rs:203-226` |

**禁止**：在响应体、日志、审计中区分"不存在"与"无权"。审计记录只写"目标不在可见范围"，不写"存在但无权"。

## 4. 泄露面清单（AC-17 T-23 的实现依据）

| # | 泄露通道 | 处置 | 验收方式 |
|---|---|---|---|
| 1 | 节点存在性 | L1-b + L2-b 移除 | 响应中不含无权对象任何字段 |
| 2 | 边存在性 | L2-c 整边丢弃 | 边数与边内容均不含 |
| 3 | Tooltip / Focus Panel | 只渲染 L2 后的对象 | 前端断言无无权 label |
| 4 | 关系计数 | L2-d 裁剪后重算，且**不返回全量规模** | 两组权限用户对比断言 |
| 5 | 聚合数量（按 kind/status 分组） | 分组在裁剪后统计 | 同上 |
| 6 | 搜索结果总数 | 搜索在裁剪后的集合上执行 | 同上 |
| 7 | 截断信息 | 截断原因**不得包含"因权限被裁剪 N 条"** → 统一记为"已达规模上限" | 断言截断文案不含数量差 |
| 8 | 布局预留位置 | 布局在裁剪后的集合上计算，不预留被删节点位置 | 截图/坐标断言 |
| 9 | 错误消息 | 404 同形，不区分无权/不存在 | 状态码与文案同形断言 |
| 10 | 日志与缓存 | 日志只写"不在可见范围"；**响应不设置可复用缓存**（或缓存键含 principal 维度） | 日志内容审计 + 缓存头断言 |
| 11 | **响应时间侧信道** | 不做"按无权对象数线性延迟"的差异化处理（默认无差异化即无泄露） | — |

**第 11 项说明**：本仓既有实现里存在"404 防枚举"的一致性意识（`kb_memory_api.rs:105-108` 非法 UUID 也返回 404）。V0-3 沿用同口径：所有不可见情形耗时特征一致，不引入按权限差异化的分支。

## 5. 一致性语义（R-07 闭合）

### 5.1 仓内事实

| 事实 | 证据 |
|---|---|
| **全仓无任何隔离级别设置**：无 REPEATABLE READ / SERIALIZABLE / set_config；247 处 `pool.begin()` 全为写路径事务 | grep 零命中（仅 `tenant_members.rs:289` 注释提及 READ COMMITTED） |
| 存在**单语句一致快照**范式：`acceptance_overview.rs:223-288` 用单条 SQL + CTE（pending/manifest/latest_delivery/latest_review/blockers）覆盖 5 张表，`$1=project_id / $2=tenant_id` 收口 → 一致性由单语句快照天然保证 | `crates/pm-application/src/acceptance_overview.rs:1-10,223-288` |
| 存在**跨语句无事务**反例：`requirement_read.rs:238-313` 三条独立查询 | 同上 |
| 固定 revision 读取机制已存在：`design.design_requirement.requirement_revision_id`、`delivery.task_requirement.revision_id`、`pc.relationship_revision.current_revision_id` 指针 | `0010:120-133`、`0001:1884`、`0001:3310` |
| 投影漂移必须可观测（Conflict/Stale 而非静默服务旧值） | `requirement_projection.rs:287-325`（`rebuild_projection` + 漂移 Conflict）；`knowledge.rs:566-568`（`KnowledgeError::Stale`） |

### 5.2 `current` 的判定（严格定义）

> **只有当一次响应中的全部数据来自同一条 SQL 语句（或同一事务内的同一快照）时，才允许标记 `consistency="current"`。**

| 场景 | 标记 | 理由 |
|---|---|---|
| 单条 SQL/CTE 聚合（追踪图、Task DAG、架构图均**必须**写成单条 SQL 或单条带 CTE 的 SQL） | `current` | 单语句快照天然一致（`acceptance_overview.rs` 范式） |
| 多条独立语句且分属不同权威来源 | `mixed-revision` + 各对象 revision refs | 跨时点风险（`requirement_impact` 反例） |
| 同一权威来源内的多条语句（同一事务） | 视是否需要跨源一致：需跨源 → `mixed-revision` | 事务不跨权威源时无法证明统一时点 |
| 无法证明一致性 | **必须** `mixed-revision`（fail-closed），不得默认 `current` | ADR-VIZ-006「不得仅凭一次数据库事务就声称跨多个权威来源已达到统一时点一致性」 |

### 5.3 响应中的 revision refs

`meta` 必带：

```json
"meta": {
  "consistency": "current | mixed-revision",
  "source_revisions": [
    { "domain": "delivery", "type": "requirement", "id": "R-042", "revision_id": "…", "content_hash": "…" },
    { "domain": "design",  "type": "design_revision",  "id": "D-017", "revision_id": "…", "content_hash": "…" }
  ],
  "generated_at": "…",
  "truncated": false,
  "truncation_reason": null
}
```
- `source_revisions` 与 spec 的 `provenance.source_revision_refs` 同构 → Live 与 Artifact 可做结构化 Diff（DV-6）。
- 每个 revision ref 的 `content_hash` 为裸小写 hex sha256（与 `knowledge::content_hash` 同口径）。

### 5.4 固定 revision 读取（图查询的读法约定）

- 关系边**按绑定的 revision 读**（`design_requirement.requirement_revision_id`、`task_requirement.revision_id`），不读 current → 同一张图内不出现"同一对象两个版本"。
- 对象属性（label/status）读该 revision 对应的快照，而非对象当前态；对象的"当前态"另以 `is_current` 字段标注（范式：`requirement_read.rs:264,283` 的 `is_current` + `stale_binding_count`）。
- `mixed-revision` 时 UI **必须**提示"图基于混合时点数据"（14 号 → D6 交互行已在 D6 §3 落位）。

### 5.5 降级行为

| 情形 | 行为 |
|---|---|
| 规模超限（>500 节点） | 拒绝（`E_SPEC_BUDGET_EXCEEDED`），**不静默截断** |
| 查询深度/候选超限 | 截断但显式：`truncated=true` + `truncation_reason`（`traversal.rs:187-210` 三值）+ 丢弃计数（`assembly.rs:163-181`） |
| 一致性无法证明 | 降级为 `mixed-revision` 并标注，不放弃响应 |
| 权限裁剪 | 整边丢弃（L2-c），**不计入截断**（截断与权限是两个独立维度，UI 分开展示） |

## 6. nature 粒度（R-06 闭合）

### 6.1 定义

| 层级 | 取值 | 含义 |
|---|---|---|
| **Artifact.nature** | `business` / `explanatory` | 整张图的性质。`explanatory` = AI 生成的假设/示意内容，**恒显「AI 生成 · 非正式认知」**，不可 published 到业务槽位（13 号 §3.2 的 DB CHECK 强制） |
| **Node.nature** | 可选，缺省继承 Artifact | 单个节点是否为业务事实。缺省继承 → 简化绝大多数场景 |
| **Edge.nature** | 可选，缺省继承 Artifact | 同上 |
| **Edge.derived** | `DerivedTrace { direct: bool, via: Vec<String>, semantic_confidence }` | **推导方式维度，与 nature 正交** —— 不把"间接推导"混进"事实性质" |

### 6.2 混合规则

| 组合 | 允许？ | 处理 |
|---|---|---|
| Artifact=business，节点全 business | ✅ 默认 |
| Artifact=business，个别节点 explanatory | ✅ | 该节点 UI 单独标注「AI 假设」；**校验器要求 explanatory 节点必须带 `derived` 或明确的假设标记**（12 号 §4.3 的 W 级提示） |
| Artifact=explanatory，含 business 节点 | ⚠️ | 允许（对话中"在假设图上标出真实模块"），但 **Artifact 整体仍为 explanatory**，不可发布 |
| Edge.business 两端为 explanatory 节点 | ❌ | 校验器 Blocker（业务关系不能建立在假设节点上） |
| `derived` 与 `nature` 同时出现 | ✅ | 语义正交：`nature=business` + `derived.direct=false` = 事实性但间接推导（这正是二跳拼接的语义） |

### 6.3 落地

- Spec 类型：`meta.nature` + 节点/边级可选 `nature` + `derived`（12 号 §3 已按此定义）。
- DB：`viz.artifact.nature` + CHECK（13 号 §3.2）。
- UI：`explanatory` 徽标（恒显）；`derived` 边虚线 + 「经 X 推导」提示（D6 §3 已落位）。
- 校验：`E_SPEC_NATURE_MISMATCH`（Edge.business 挂在 explanatory 节点上）——需补入 12 号问题码表。

## 7. ObjectResolver 实现落点与 SQL 草案

### 7.1 落点

`crates/pm-application/src/visualization/resolver.rs`，实现 12 号定义的 `ObjectResolver` trait（pm-domain 定义接口，pm-application 连 DB 实现 —— 依赖倒置边界，不让 pm-domain 依赖 DB）。

### 7.2 解析规则（按 domain/type 分派）

| domain / type | 可见性判定 | 依据 |
|---|---|---|
| `delivery` / requirement, requirement_revision | `delivery.requirement` 的 tenant+project + 项目成员 + revision 状态 | `pm_requirements.rs:32-50` |
| `delivery` / task | `delivery.task` tenant+project + 成员 | `pm_execution.rs:49-70`（task→project→成员） |
| `design` / design, design_revision, adr, adr_revision | design/adr→project→成员；revision 三元组校验 | `pm_designs.rs:32-95` |
| `kb` / document, document_revision | 项目成员可见；**draft 恒禁** | `retrieval/validator.rs:41-47`；`kb_service/document.rs:402-480` |
| `pc` / cognition_object, relationship | tenant+project + `pc.endpoint_in_scope` 双端校验 | `0007:82-151` |
| `ai` / cognition_item, project_document | tenant+project + published 修订 | `knowledge.rs:481-500` |
| `plan` / task_plan | plan→project→成员 | `pm_plans.rs:29-50` |

**统一实现**：所有分派最终汇到 `require_project_visible` 的可见性 SQL（`support.rs:184-195`）—— **V0-3 不自建第二套可见性 SQL**（硬约束）。

### 7.3 关系边判定

- `relation_exists(from, to, relation)`：查 Registry 登记的关系表 + L1 条件 + 端点可见性。
- **Registry 的 `source_type/target_type` 决定查哪张表**（`task.implements_requirement` → `delivery.task_requirement`；`design.covers_requirement` → `design.design_requirement`；`task.depends_on_task` → `delivery.task_dependency`；`design.decided_by_adr` → `design.adr_design_revision` + `design.adr_requirement`；`pc.*` → `pc.relationship`；`kb.supported_by_memory` → `kb.memory_ref`）。
- **映射表属于 Visualization 层**（不引入业务表变更，符合 ADR-VIZ-005）。

## 8. 角色与人类门

| 动作 | 门 | 依据 |
|---|---|---|
| 查看 Live 图 / Artifact | 项目成员 | `support.rs:199-210` |
| 创建 draft | 项目成员 + `ensure_trusted_context`（api_key → 403）+ 服务层真人门 | `support.rs:88-98`；`requirement.rs:203-226` |
| 请求审批 | 同上 | 同上 |
| **发布到共享槽位** | **限定 `core.project_member.role='owner'`**（建议，待 D-1 签收） | 仓内实际授权只认 owner/member（`approver.rs:36-44`、`pm_runs.rs:74-95`）；发布是项目级公共事实，应由项目 owner 承担 |
| 裁决审批 | 真人门 + 非本人请求（可选） | `pc.approval` 同款 decided_by 语义 |

> Agent 职能角色（`ai_pm`/`architect`/`developer`/`tester`）**不是授权角色**（`crates/pm-domain/src/tenant.rs:4-6`），不得因"是 AI 团队成员"而获得发布权。

## 9. 测试设计

| 用例 | 模板来源 | 断言 |
|---|---|---|
| 跨租户/非成员取图 | `apps/pm-api/tests/tenant_isolation.rs:144-201`（XT-1/XT-2 同款） | 列表不含；直查 → 404；**写不得留行** |
| 非成员项目过滤 | `apps/pm-api/tests/visibility.rs:258-308`（`project_scoped_lists_do_not_leak_other_projects`） | `?project_id=<他人>` → 404 |
| api_key 发布 | `apps/pm-api/tests/pc_proposals.rs:311-360`（P5 门） | 403；改 role 后放行 |
| **泄露矩阵 11 项**（§4） | 自建：同一图分别以"全量权限"与"受限权限"两个主体请求 | 逐项断言受限响应不含无权对象任何标识；计数只反映可见集合；截断文案不含权限差值 |
| 一致性 | 自建：构造跨源混合数据 | `consistency` 标记正确；`source_revisions` 完整；`mixed-revision` 时 UI 提示可见 |
| 端点项目校验 | 自建：插入跨项目脏引用（仿 `0007:5-8` 描述的场景） | 脏引用**不出现在图中**（L1-b 拦截） |
| 固定 revision 读 | 仿 `requirement_read.rs:264,283` | 同一对象在图内只有一个版本；`is_current` 标注正确 |
| 计数一致性 | 两主体对比 | 受限主体的 `total_nodes` = 其可见节点数；**响应不含全量规模字段** |

测试脚手架复用 `apps/pm-api/tests/visibility.rs` 的 `token_for(sub, tid)` / `router_with_auth_config(pool, test_jwks(), AuthConfig::default())` / `body_json()` / `ids_of()`（`visibility.rs:28-38,59-66,165-174,189`）。

## 10. 实施任务清单

| # | 任务 | 完成定义 |
|---|---|---|
| V0-3.1 | Resolver 实现 | `resolver.rs` 按 §7.2 全域分派；统一汇到 `project_visible` |
| V0-3.2 | 图查询聚合模块 | 每条 SQL 自带 L1-a~d；追踪图 + Task DAG 各写成**单条 SQL/CTE**（§5.2 current 前提） |
| V0-3.3 | 投影层 | L2-a~f 全部实现；计数裁剪后重算 |
| V0-3.4 | 截断与降级元数据 | `truncated` + `truncation_reason` + 丢弃计数；与权限裁剪分维度 |
| V0-3.5 | 一致性标记 | `consistency` + `source_revisions` 计算；无法证明 → `mixed-revision` |
| V0-3.6 | nature 粒度 | 校验器补 `E_SPEC_NATURE_MISMATCH`（12 号 §4.3 增补）；UI 徽标与 derived 虚线（D6） |
| V0-3.7 | 角色与人类门 | 发布限定 owner（D-1 签收后落）；api_key 403 |
| V0-3.8 | 泄露矩阵测试 | §4 的 11 项逐条断言 + 两主体对比 |
| V0-3.9 | 端点隔离测试 | 跨项目脏引用场景断言 |

## 11. 待签收与偏差

| # | 项 | 决策人 | 说明 |
|---|---|---|---|
| D-1 | 发布权限是否限定 `role='owner'` | Architecture Owner | §8；仓内实际授权只认 owner/member |
| D-2 | **计数策略**：只返回"可见范围内规模"，不提供全量规模 | Security Owner | §3.2；反之则计数成为探测通道 |
| D-3 | `current` 判定是否接受"单条 SQL/CTE"为充分条件 | Architecture Owner | §5.2；仓内无隔离级别可用 |
| D-4 | 知识库密级缺口 | Product/Security | KB 无 visibility 列，密级仅留钩子未实现（`retrieval/validator.rs:58-66`）→ 图查询对 KB 对象等同"项目成员可见"；若将来引入密级，图查询须同步接入 |
| D-5 | `owner_domain` 是否参与图查询过滤 | Architecture Owner | `pc.relationship_type.owner_domain` 目前不参与查询（`pc/mod.rs:244-274`）；建议图查询**不**按其裁剪（它是确认权威域，不是可见性），但需 owner 确认语义 |
| D-6 | Live 与 Artifact 的 revision refs 对齐方式 | Backend Owner | §5.3：两者同构以支持 Diff |
| D-7 | 端点 project 校验是否回补既有读函数 | Architecture Owner | §3.1：既有读函数缺 project 裁剪是**既有缺口**；V0-3 只在新模块补，不改既有 API 语义（改动影响面另议） |
