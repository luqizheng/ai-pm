# AI-PM Visualization Relation Registry 首批词表会签表 V1.1（架构预审修订稿）

> **状态**：架构与语义预审完成，**待领域 Owner 会签**（V0-1 唯一硬人工依赖） · 日期：2026-11-12
> **上游依据**：[ADR-VIZ-004](ai-pm-viz-09-architecture-decision-record-v1.md)（受控字符串、命名空间、Owner 会签）· [12 号 V0-1 设计](ai-pm-viz-12-v0-1-spec-validation-design-v1.md) §5 · [14 号 V0-3 投影](ai-pm-viz-14-v0-3-projection-consistency-design-v1.md)
> **修订来源**：V1.0 首批词表会签表 → 架构预审（专家）→ **V1.1（本文）**，含产品经理两处代码实证修正
> **本稿性质**：架构预审建议稿，**不代表任何领域 Owner 已完成正式会签**。未会签条目必须保持 `draft` 且不可投影。

## 0. V1.1 修订摘要

| # | 修订 | 来源 | 性质 |
|---|---|---|---|
| V-01 | R-01/R-03 目标类型统一为 `requirement_revision`（修正总表与详细定义不一致） | 架构预审 | 纠错 |
| V-02 | R-05 语义方向统一为 **dependent → prerequisite**（`task_id → depends_on_task_id`）；DAG 视觉流向由**显式 presentation transform** 反转，Registry 不反转 | 架构预审 | **架构裁决** |
| V-03 | R-06 确立 **PC Registry 为唯一词汇权威**（不复制 20+ 类型），新增 8 条跨 Registry 强制校验 | 架构预审 | **架构裁决** |
| V-04 | R-08 不作为独立 active 关系键，降为 R-01 的**反向视图别名**（`reverse_view_of`），避免重复计数与双向重复边 | 架构预审 | **架构裁决** |
| V-05 | Registry 状态模型补 `rejected`；明确 `deprecated` 只适用于"曾生效后被替代"，否决未启用者用 `rejected` | 架构预审 | 补全 |
| V-06 | YAML 字段结构定版（13 字段）；来源标识用 `schema.table.column` 稳定标识，迁移文件行号仅作审查依据 | 架构预审 | 补全 |
| V-07 | Lint 检查项扩至 15 条 | 架构预审 | 补全 |
| V-08 | Q-1/Q-2/Q-3 正式答复（分层保留 / `pc.related`+交叉校验 / 方向分离） | 架构预审 | 裁决 |
| **V-09** | **R-07 方向与端点修正**：`ref_role='evidence'` 指向 **kb.source（L2 原文/原始资料）**，不是任意业务对象；语义为 `memory → evidence_source`，关系名改为 `kb.evidence_supported_by` | **产品经理代码实证** | **纠错** |
| **V-10** | R-05 方向裁决的**算法层落地约束**：依赖闭包（沿 Registry 方向）与影响分析（反向）方向相反，必须各自显式声明，禁止各层各自约定 | 产品经理 | 补全 |
| **V-11** | **与仓库既有方向/术语约定对齐**：既有图边方向 = `prerequisite → dependent`（`execution_read.rs:568-570`）、`upstream` = 前置任务（`:548-550`）；Registry 语义方向不随之改，但投影层须提供显式方向映射 | 产品经理（代码实证） | 补全 |

### V-09 证据（本次代码实证）

`kb.memory_ref` 的 `ref_role` 语义在代码中有权威定义，非推测：

| 证据 | 内容 |
|---|---|
| `crates/pm-application/src/kb_service/memory.rs:40-48` | `RefRole::Evidence` 注释：**「来源依据：指向 L2 原文（会话消息 / 任务交互 / 原始资料）」**；`RefRole::Subject` 注释：**「讨论主题：按 Task 检索的锚（task / requirement）」** |
| `crates/pm-application/src/kb_service/source.rs:297` | evidence 实际用法：`mr.ref_role='evidence' AND mr.target_domain='knowledge_base' AND mr.target_type='source'` → **target 落在 kb.source** |
| `crates/pm-application/src/kb_service/retrieval.rs:76` | subject 用法：按 `(domain,type,id)` 反查 memory（`ref_role='subject'`），方向与 evidence 相反 |
| `crates/pm-application/src/kb_service/mod.rs:36` | `pub const TYPE_SOURCE: &str = "source";` |

**结论**：专家 V1.1 稿 R-07 写的「`memory → business_object`，目标对象类型由 memory_ref 的实际引用结构确定」**不成立**——evidence 的 target 是 **kb.source（原始资料）**，语义是「该记忆以这条原始资料为依据」，而非「记忆支撑某业务对象」。若按专家稿实现，图上会出现「记忆 → 任意业务对象」的假边。

**修正后定义**：`kb.evidence_supported_by`，source=`memory`（具体为 kb.memory_ref.revision_id 所指的记忆修订），target=`kb_source`，方向 `memory → evidence_source`，过滤 `ref_role='evidence'`。

> ⚠️ **仍需 KB Owner 会签确认**：此修正依据代码注释与查询实证得出，KB Owner 是该语义的权威 Owner。若 KB Owner 确认语义为其他形态，应修改关系名/端点/方向，而非只改 UI 文案。

### V-10 证据（方向裁决的算法层落地）

R-05 采纳「语义方向 = dependent → prerequisite」后，**影响分析的方向与语义方向相反**：

```
Task A depends_on Task B   →  Registry 语义边 A → B（dependent → prerequisite）
                              影响分析沿 B → A 传播（改 B 影响 A）
```

若影响分析层直接按 Registry 方向遍历，将得到**反向结论**（A 依赖 B，却报"B 受 A 影响"）。因此：

- 依赖闭包 / 关键路径计算：**显式使用 `prerequisite → dependent`**（沿 DAG 执行方向）；
- 影响分析 / 下游影响：**显式使用反向遍历**（`dependent → prerequisite` 的逆）；
- 两处必须在代码与文档中**各自显式声明**方向，不得依赖"默认约定"。

已在 [14 号 V0-3 设计](ai-pm-viz-14-v0-3-projection-consistency-design-v1.md) §7 增补对应约束。

---

## 1. 会签结论摘要

### 1.1 逐条预审建议

| 关系 | 预审建议 | 核心意见 |
|---|---|---|
| R-01 `design.covers_requirement` | **确认，需修正总表** | 保留 Design Revision 到 Requirement Revision 的正式绑定语义 |
| R-02 `design.decided_by_adr` | **确认，补充边界** | 保留 Design 领域正式绑定；与 PC 认知关系分层，不直接合并 |
| R-03 `adr.covers_requirement` | **确认，需修正总表** | 明确目标为 Requirement Revision |
| R-04 `task.implements_requirement` | **确认** | 保留 Task 对特定 Requirement Revision 的实现关系 |
| R-05 `task.depends_on_task` | **需修改** | 统一语义方向为 dependent → prerequisite；视觉流向独立声明 |
| R-06 `pc.related` | **确认，附条件** | 不复制 PC 的 20+ 类型；必须引用并校验 PC 关系类型注册表 |
| R-07 `kb.evidence_supported_by` | **确认，需修正方向与端点**（V1.1 修正） | 只投影 `ref_role='evidence'`；target 为 kb.source（L2 原文） |
| R-08 `requirement.refines_design` | **需修改** | 不作为独立业务事实关系，改为 R-01 的反向视图别名 |
| R-09 `derived_trace` | **确认** | 保留为 Visualization 专属合成关系，必须显式标记间接推导 |

**预审结论：9 项中 6 项建议确认（其中 4 项附条件或需修正），2 项建议修改，0 项建议直接否决。**

这里的"确认"是架构预审意见，不是领域 Owner 会签结论。R-08 的修改建议不意味着删除其表达能力，而是将它从独立关系事实调整为 R-01 的反向投影视图。

### 1.2 激活前必须闭合的事项

1. R-01、R-03 总表中的端点类型与详细定义统一。
2. R-05 的语义方向统一，禁止总表和详细定义相互矛盾（V1.0 确有此矛盾，已修正）。
3. R-06 建立与 PC 关系类型注册表的交叉校验。
4. **R-07 核实 `memory_ref` 的证据引用方向和对象类型约束（V1.1 已由代码实证修正，仍需 KB Owner 确认）**。
5. R-08 从独立业务关系调整为反向视图定义。
6. Registry 状态模型明确区分 `draft`、`active`、`deprecated`、`rejected`。
7. 关系 Owner、会签日期、版本及来源约束字段完整。

---

## 2. Registry 设计原则

### 2.1 Registry 的职责边界

Visualization Relation Registry 是图关系的**受控映射与校验机制**，不是新的业务关系权威来源。

**它负责**：声明 Visualization 可识别和投影的关系类型；指明来源领域、来源表或领域注册表；定义端点与语义方向；声明 Owner 与会签状态；校验关系是否已注册、端点类型是否匹配、定义是否获批；为图展示提供稳定的关系语义标识。

**它不负责**：新建业务关系；修改业务关系；替代 Design/Delivery/PC/KB 等领域的关系定义；将 AI 推导关联提升为正式业务事实；绕过领域权限检查或正式业务确认流程。

### 2.2 权威来源与投影关系

每条关系必须保留来源信息。Visualization 只能读取已有关系并形成图边。

- 来自业务表的关系 → Registry 必须引用来源领域的**正式数据结构**；
- 来自领域注册表的关系 → 必须**引用其权威注册表**，不能复制一份不受控的关系词汇表；
- 合成关系 → 必须明确标识它不是数据库中的业务关系事实。

### 2.3 关系方向

Registry 的 `source_type → target_type` 表示**业务语义方向**，不是布局方向，也不是执行流程的视觉方向。

- 语义方向不能因 UI 布局需要被静默反转；
- 图布局可采用不同视觉流向，但必须保留语义方向标识；
- 若 UI 展示方向与语义方向不同，必须由**显式的 presentation transform** 实现；
- 不得因视觉反转改变影响分析、路径追踪或权限过滤的语义。

### 2.4 状态模型

| 状态 | 含义 | 是否允许投影 |
|---|---|---|
| `draft` | 已提出，尚未完成 Owner 会签 | 否 |
| `active` | 已完成会签并正式启用 | 是 |
| `deprecated` | 曾经启用，现已被替代或废弃 | 否（历史解释场景除外） |
| `rejected` | 经会签明确否决，从未正式启用 | 否 |

**未会签不等于默认允许。** `draft`、`rejected`、`deprecated` 均不能进入正常图投影。

从未启用的条目被否决 → `rejected`（非 `deprecated`）；`deprecated` 只适用于曾经生效、后来被替代或废弃的定义。

---

## 3. 首批关系预审结果

### R-01 `design.covers_requirement`

**预审建议：确认，修正总表端点类型。**

- 来源领域：Design · 来源：`design.design_requirement`
- 来源字段：`design_revision_id`、`requirement_id`、`requirement_revision_id`
- 语义方向：`design_revision → requirement_revision` · Owner：design-owner · 建议状态：`active`（待会签）

**语义定义**：Design Revision 覆盖、实现或约束某一特定 Requirement Revision。

**必要约束**：
1. 图边必须绑定 `requirement_revision_id` 指定的版本，不能替换为 Requirement 当前版本。
2. `requirement_id` 与 `requirement_revision_id` 的归属一致性必须由来源数据约束或读取时校验保证。
3. 权限过滤必须同时覆盖 Design Revision 和 Requirement Revision。
4. 来源记录不完整或版本绑定无法确认时，**不得退化为指向当前 Requirement 的边**。

**修订点**：V1.0 总表中目标类型写为 `requirement`，统一改为 `requirement_revision`。

**Owner 会签重点**：确认语义确实是"覆盖/实现/约束"，而非仅"提及"或"关联"。

---

### R-02 `design.decided_by_adr`

**预审建议：确认，补充与 PC 认知关系的边界。**

- 来源领域：Design · 来源：`design.adr_design_revision`
- 语义方向：`design_revision → adr_revision` · Owner：design-owner · 建议状态：`active`（待会签）

**语义定义**：某个 Design Revision 受到特定 ADR Revision 的决策约束或技术决策依据约束。

**与 PC 关系的区分**：PC 领域存在相似名称的关系 `design-constrained-by-adr`（`migrations/0007_relationship_owner_domain_seeds.sql:69-74`，`owner_domain='kb'`）。**不要因为两个关系指向相同的 Design 和 ADR 对象，就认定它们是同一条权威关系**——Design 关系是 Design 领域的正式绑定，PC 关系是 Product Cognition 中记录的认知关系/认知表达。保留各自来源与领域归属。

**同图展示规则**：
1. 两个来源表达同一对对象、同一语义 → UI 可合并为一条视觉边，并在来源面板展示两个来源。
2. 语义或版本指向不同 → 必须分别保留。
3. 视觉合并不能删除来源证据，也不能把 PC 认知关系伪装成 Design 正式绑定。
4. **权限过滤必须先于视觉合并**，不能通过合并暴露用户无权查看的来源。

**Owner 会签重点**：Design Owner 确认 `design.adr_design_revision` 的权威语义；PC Owner 确认 PC 侧相似关系的含义与来源。**两方不应相互代签。**

---

### R-03 `adr.covers_requirement`

**预审建议：确认，修正总表端点类型。**

- 来源领域：Design · 来源：`design.adr_requirement`
- 语义方向：`adr_revision → requirement_revision` · Owner：design-owner · 建议状态：`active`（待会签）

**语义定义**：某个 ADR Revision 覆盖、约束或影响特定 Requirement Revision。

**必要约束**：
1. 关系必须绑定明确的 Requirement Revision，不得自动替换为当前版本。
2. ADR Revision 与 Requirement Revision 的绑定不明确时，**不得创建推断边**。
3. 图中应区分 ADR 的决策约束关系与 Design Revision 对 Requirement 的实现关系。

**Owner 会签重点**：确认 ADR 对 Requirement 的语义是覆盖/约束/影响，还是仅用于说明决策背景。若实际语义较弱，应调整关系名称或定义。

---

### R-04 `task.implements_requirement`

**预审建议：确认。**

- 来源领域：Delivery · 来源：`delivery.task_requirement`
- 语义方向：`task → requirement_revision` · Owner：delivery-owner · 建议状态：`active`（待会签）

**语义定义**：Task 被正式关联为实现某一特定 Requirement Revision 的交付任务。

**必要约束**：
1. 关系必须使用 `revision_id` 指向指定 Requirement Revision。
2. 不得将历史绑定自动改写为当前 Requirement Revision。
3. Task 与 Requirement 的关系是**交付关联**，不代表 Task 已完成，也不代表 Requirement 已验收。
4. 不得仅凭该关系推断 Requirement 已实现或满足验收标准。

**Owner 会签重点**：确认表达的是"实现范围绑定"，而非普通任务参考或弱关联。

---

### R-05 `task.depends_on_task`

**预审建议：需修改，统一语义方向。**

- 来源领域：Delivery · 来源：`delivery.task_dependency`
- 来源字段：`task_id`（依赖任务）、`depends_on_task_id`（前置任务）
- Owner：delivery-owner · 建议状态：`active`（方向修订后待会签）

**语义定义**：Task A 依赖 Task B，表示 B 是 A 的前置任务。

**Registry 规范语义方向**：

```
dependent_task → prerequisite_task        即  task_id → depends_on_task_id
```

例：Task A 依赖 Task B → 数据记录 `task_id=A, depends_on_task_id=B` → Registry 语义边 `A → B`。

**必须修正的原文矛盾**：V1.0 总表写"前置任务 → 后置任务"，详细定义写"依赖任务 → 前置任务"，两者不能同时成立。**本稿选择后者为 Registry 规范语义方向。**

**视觉展示规则**：Task DAG 通常更容易按执行顺序展示为 `prerequisite → dependent`，这可作为 UI 的**布局或 presentation transform**，但不能改变 Registry 语义方向。

- 关系事实仍是 `A depends_on B`；
- DAG 可将 B 置于 A 上游，视觉显示 `B → A`；
- **变换必须显式声明**，不得让查询层、影响分析层和布局层各自采用不同方向；
- 关键路径与依赖闭包计算必须明确使用哪一种方向约定。

**⚠️ 算法层落地约束（V1.1 新增，V-10）**：

| 计算 | 遍历方向 | 理由 |
|---|---|---|
| **依赖闭包**（A 依赖了谁） | 沿 Registry 方向 `dependent → prerequisite` | 从 A 顺边走即得全部前置任务 |
| **影响分析**（改 B 影响谁） | Registry 方向的**反向**（= `prerequisite → dependent`） | 改 B 影响 A：A 在 B 的下游 |
| **关键路径 / CPM** | 沿 `prerequisite → dependent` | 顺执行顺序推进 |

> ⚠️ **影响分析与依赖闭包方向相反**：若影响分析直接按 Registry 方向遍历（A→B），将得出"B 受 A 影响"的**反向结论**（实际是 A 受 B 影响）。两处必须各自显式声明方向，禁止依赖"默认约定"。

**⚠️ 与仓库既有约定对齐（2026-11-12 代码实证，V-11）**：

| 证据 | 既有约定 | 对 Registry 的要求 |
|---|---|---|
| `crates/pm-application/src/execution_read.rs:568-570` | 协作图边 `from = depends_on_task_id`（前置）、`to = task_id`（依赖方）→ **既有图边方向 = prerequisite → dependent**，与 Registry 语义方向相反 | Registry 语义方向不因既有实现而改；但**投影层必须提供显式方向映射**，不得让两者混用 |
| `crates/pm-application/src/execution_read.rs:548-550` | `open_upstream_count` 统计前置任务未完成数，注释「未完成的上游依赖数」→ **upstream = 前置任务** | 术语沿用既有义（upstream = prerequisite）；**不得反向使用** |

**实践结论**：既有图边方向（prerequisite → dependent）与影响分析方向一致 → **DAG 视觉无需翻转**，直接沿既有方向渲染；仅 Registry 存储的语义方向与之相反，二者由显式映射隔离。

**Owner 会签重点**：确认 `task_id` 表示依赖任务、`depends_on_task_id` 表示前置任务，并确认业务查询与 DAG 布局分别采用的方向约定。

---

### R-06 `pc.related`

**预审建议：确认，附带严格的跨 Registry 校验条件。**

- 来源领域：PC · 来源：`pc.relationship` + `pc.relationship_type`（权威注册表）
- 语义方向：由 PC 关系类型定义决定 · Owner：pc-owner · 建议状态：`active`（待会签）

**语义定义**：PC 对象之间存在由 PC 领域关系类型注册表定义的关系。

**采用统一 `pc.related` 的理由**：PC 关系类型本身已有正式定义与治理机制；复制一份会形成**双重权威**；PC 新增/修改类型时两套注册表易版本不一致；Visualization Registry 应管理"能否投影"，而非重新定义 PC 的全部业务词汇。

**因此**：Visualization Registry 注册 `pc.related`；实际类型通过 `type_key` 引用 PC 的 `pc.relationship_type`；边标签使用 PC 注册表定义的展示名称或稳定语义标识。

**强制校验条件**：
1. `type_key` 必须存在于 PC 权威关系类型注册表。
2. `source_object_types`、`target_object_types` 必须匹配。
3. 方向性必须遵循 PC 关系类型定义。
4. `governance_level` 必须保留，不得因统一为 `pc.related` 而丢失。
5. `owner_domain` 必须保留，不能把其他领域 Owner 的关系误归为 PC Owner。
6. PC 关系类型被禁用或废弃时，Visualization **不得继续将其作为正常关系投影**。
7. `type_key` 不得为空或使用未注册值。
8. 不能用通配符端点类型绕过 PC 自身的类型校验。

**Owner 会签重点**：确认 `pc.related` 只作为投影入口，PC 关系类型、方向、对象类型与治理等级仍由 PC 领域唯一维护。

---

### R-07 `kb.evidence_supported_by`（V1.1 修正名与方向）

**预审建议：确认，方向与端点按 V1.1 修正。**

- 来源领域：KB · 来源：`kb.memory_ref`
- 过滤条件：`ref_role='evidence'`
- 语义方向：`memory → evidence_source`（**V1.1 修正**）
- Owner：kb-owner · 建议状态：`active`（待会签）

**语义定义（V1.1 修正）**：某条 Memory 以某条**原始资料（kb.source）**为来源依据。

**为什么修正（代码实证）**：`RefRole::Evidence` 的权威注释为「**来源依据：指向 L2 原文（会话消息 / 任务交互 / 原始资料）**」（`kb_service/memory.rs:44`）；实际查询中 evidence 的 target 恒为 `target_domain='knowledge_base' AND target_type='source'`（`kb_service/source.rs:297`，`TYPE_SOURCE="source"` 于 `kb_service/mod.rs:36`）。因此**不存在"记忆支撑任意业务对象"的语义**——若按 V1.0 稿实现会产生假边。

**方向规则**：只投影 evidence 方向 `memory → evidence_source`；不得把 `subject` 角色（按 Task/Requirement 检索的锚）反向再作为另一条边同时展示，否则出现重复边或误导性双向关系。

**必要约束**：
1. 只允许 `ref_role='evidence'` 的引用进入该关系。
2. `subject` 及其他角色不得自动等同于 evidence。
3. 目标对象类型为 `kb_source`；若无法解析，**不得退化为无类型节点**。
4. Memory 的证据引用不代表该记忆内容已被正式确认。
5. UI 应区分"证据依据"与"正式业务关系"。

**Owner 会签重点**：KB Owner 确认 `ref_role='evidence'` 的含义与目标对象类型。若真实语义与本修正不符，应修改关系名/端点/方向，而不是只改 UI 文案。

---

### R-08 `requirement.refines_design`（反向视图，不作独立关系键）

**预审建议：需修改，不作为独立业务关系注册。**

- 原来源：`design.design_requirement` · 原方向：`requirement_revision → design_revision` · Owner：design-owner

**问题**：R-08 与 R-01 使用同一来源事实（R-01 为 `design_revision → requirement_revision`）。若两者作为独立 active 关系注册，将造成：同一事实两个关系键；关系统计与影响分析**重复计数**；同一张图出现两条相反方向的边；领域语义与展示方向混淆；未来修改 R-01 时可能遗漏 R-08，形成双重维护。

**修订方案**：R-08 不作为独立业务关系键进入首批 Active Registry，保留为 R-01 的反向视图别名：

```
requirement.refines_design → reverse_view_of: design.covers_requirement
```

其作用是允许视图从 Requirement 出发查询相关 Design，**而不是新增一条业务事实**。

**投影规则**：
1. 底层只读取一次 `design.design_requirement`。
2. 查询视角需要时可反转源端与目标端。
3. 反向投影必须保留原始关系键 `design.covers_requirement` 及来源信息。
4. **同一图中，R-01 与 R-08 不得对同一条底层事实同时生成两条边。**
5. 若未来 Owner 证明"Requirement refines Design"是独立于"Design covers Requirement"的另一种业务语义，必须提供独立来源或明确业务定义，再重新申请注册。

**Owner 会签重点**：确认 R-08 是纯粹反向视图，还是存在独立业务语义。若为纯反向视图，接受别名方案即可。

---

### R-09 `derived_trace`

**预审建议：确认。**

- 来源领域：Visualization · 来源：VizSpec 合成，不对应业务关系表
- 端点：由实际推导路径中的对象决定 · Owner：architecture-owner · 建议状态：`active`（待会签）

**语义定义**：表示 Visualization 根据已存在的直接关系推导出的间接追踪关系。**它不是业务领域持久化的直接关系，也不代表两个端点之间存在直接业务绑定。**

**必要结构**：`derived.direct = false` · `derived.via = [...]` · `derived.semantic_confidence = "structurally_supported"`

**展示规则**：
1. 使用虚线或其他明确的间接关系视觉样式。
2. 标注"经 X 推导"或等价文案。
3. 在 Impact Analysis 中进入 Indirect 分组。
4. 不得与直接业务关系使用完全相同的视觉语义。
5. 路径上的每一条直接关系都必须经过 Registry 校验与权限过滤。
6. **如果中间路径存在未授权对象，不能通过合成边暴露其存在。**
7. 不得将 `derived_trace` 写回业务关系表。
8. 不得把推导关系当成正式业务事实用于审批或业务状态变更。

**推导约束**：只允许表达结构上有依据的路径推导（如 `Task → Requirement ← Design`）；不能仅因两个对象在图中相邻就自动断言它们存在业务关系。

**Owner 会签重点**：Architecture Owner 确认合成关系的结构、来源标记、间接分组与权限安全规则。

---

## 4. Q-1 / Q-2 / Q-3 正式答复

### Q-1：Design 领域 ADR 绑定与 PC `design-constrained-by-adr` 是否为同一事实？

**答复：按不同权威来源的关系保留，不直接合并。**

理由：Design 的 `design.adr_design_revision` 是 Design 正式关系来源；PC 相似关系是 PC 认知关系来源。即使描述相同对象对，权威性也不同。

| 场景 | 处理 |
|---|---|
| 对象相同、语义相同、版本也相同 | 可合并视觉边，保留两个来源 |
| 对象相同，但版本不同 | 分别展示或明确版本差异 |
| 对象相同，但语义不同 | 分别展示 |
| 仅 PC 有关系、Design 无正式绑定 | 只能作为 PC 认知关系展示 |
| 仅 Design 有正式绑定、PC 无关系 | 作为 Design 正式关系展示 |

**结论：不合并权威定义；允许同图视觉合并，但必须保留来源和版本证据。** 待 Design Owner 与 PC Owner 分别确认。

### Q-2：PC 关系是否应全部映射到 Registry？

**答复：采用一个 `pc.related`，不复制 20+ PC 关系类型**，并建立严格交叉校验：`type_key` 必须来自 PC 权威注册表；方向、源端类型、目标端类型使用 PC 定义；治理等级与 Owner Domain 保留；PC 注册表中的停用关系不得继续投影；Registry Lint 必须能检测非法 `type_key`。

**结论：Visualization Registry 统一注册投影入口；PC Registry 继续唯一维护 PC 关系词汇表。** 待 PC Owner 会签。

### Q-3：Task 依赖关系箭头方向？

**答复：Registry 语义方向采用"依赖任务 → 前置任务"**（`A → B`，A 依赖 B），因为 `task.depends_on_task` 描述的是 A 对 B 的依赖语义，且来源字段本身即 `task_id=A, depends_on_task_id=B`。DAG 视觉布局可用 `B → A` 表达执行顺序，作为显式 presentation transform。

**结论：关系语义方向与 DAG 视觉流向分开定义；Registry 不反转，UI 如需执行流向使用显式 presentation transform。** 详见 R-05 的算法层落地约束（V-10）。待 Delivery Owner 会签。

---

## 5. 修订后的首批关系清单

| Key | Source Type | Target Type | Source | Semantic Direction |
|---|---|---|---|---|
| `design.covers_requirement` | `design_revision` | `requirement_revision` | `design.design_requirement` | Design → Requirement |
| `design.decided_by_adr` | `design_revision` | `adr_revision` | `design.adr_design_revision` | Design → ADR |
| `adr.covers_requirement` | `adr_revision` | `requirement_revision` | `design.adr_requirement` | ADR → Requirement |
| `task.implements_requirement` | `task` | `requirement_revision` | `delivery.task_requirement` | Task → Requirement |
| `task.depends_on_task` | `task` | `task` | `delivery.task_dependency` | **Dependent → Prerequisite** |
| `pc.related` | PC object | PC object | PC relationship registry（`pc.relationship_type`） | 由 PC type_key 决定 |
| `kb.evidence_supported_by` | `memory` | `kb_source` | `kb.memory_ref`（`ref_role='evidence'`） | **Memory → Evidence Source** |
| `derived_trace` | `*` | `*` | VizSpec synthetic | Derived direction |

**R-08 反向视图定义**：`requirement.refines_design` → `reverse_view_of: design.covers_requirement`（不新增独立业务关系事实，不参与底层关系计数）。

---

## 6. Registry YAML 与 Lint 修订要求

### 6.1 字段结构

每条关系至少包含：`key` · `status` · `domain` · `owner_role` · `source` · `source_type` · `target_type` · `direction` · `semantics` · `governance_level`（适用时）· `introduced_in` · `signoff_date`（正式会签后）· `signoff_record`（会签记录引用）。

来源信息应优先使用稳定的 `schema.table.column` 或领域注册表标识。**迁移文件及行号可保留为审查依据，但不应作为唯一的运行时来源标识。**

### 6.2 特殊关系字段

- **R-06**：`type_key_source: pc.relationship_type` + 必须执行跨 Registry 校验。
- **R-08**：`view_alias: true` + `reverse_view_of: design.covers_requirement`；不得作为独立业务事实关系进入 Active Registry。
- **R-09**：`synthetic: true` + `source: vizspec` + `business_fact: false` + 必须校验 `derived` 元数据。
- **R-07（V1.1）**：`ref_role_filter: evidence` + `target_type: kb_source`。

### 6.3 Lint 必须覆盖的检查（15 条）

1. Active 关系必须有 Owner 和有效会签记录。
2. Draft、Rejected、Deprecated 关系不得进入正常投影。
3. 所有关系键必须唯一。
4. 所有关系键必须符合命名空间规范。
5. 来源表、字段或领域注册表必须存在。
6. Source Type 与 Target Type 必须符合来源结构。
7. Requirement Revision 必须按绑定版本读取，不得替换为 Current Revision。
8. R-05 必须使用统一的语义方向。
9. R-06 必须与 PC 关系注册表交叉校验。
10. R-07 必须校验 `ref_role='evidence'` 且 target 为 kb_source。
11. R-08 不得重复生成 R-01 的同一底层事实。
12. R-09 必须携带完整推导元数据。
13. 关系语义发生变化时必须升级 Registry 版本。
14. 领域来源被废弃或变更时必须检测影响关系。
15. 任何未注册关系必须 fail closed。

### 6.4 版本变更规则

- 新增关系：至少变更 Registry 次版本。
- 修改关系端点、方向或语义：必须升级版本并**重新会签**。
- 修改展示标签但不改变语义：可按项目约定采用较低级别版本变更，但必须保留变更记录。
- 废弃关系：保留历史解释能力，不再作为普通 Active 投影。
- 修改 `type_key` 的 PC 语义：以 PC 领域的变更流程为准，并触发 Visualization 兼容性检查。

---

## 7. 领域 Owner 会签清单

架构预审不替代领域 Owner 的业务语义签字。

| 关系 | 责任 Owner | 会签重点 | 预审建议 |
|---|---|---|---|
| R-01 | design-owner | Design Revision 对 Requirement Revision 的正式覆盖语义 | 确认 |
| R-02 | design-owner | Design 与 ADR Revision 的正式绑定语义 | 确认 |
| R-03 | design-owner | ADR Revision 对 Requirement Revision 的约束语义 | 确认 |
| R-04 | delivery-owner | Task 与指定 Requirement Revision 的实现关系 | 确认 |
| R-05 | delivery-owner | 依赖字段语义及箭头方向 | 修改后确认 |
| R-06 | pc-owner | 统一投影入口与 PC Registry 的交叉校验 | 附条件确认 |
| R-07 | kb-owner | **Evidence 引用方向与目标对象类型（V1.1 修正：指向 kb.source）** | 附条件确认 |
| R-08 | design-owner | 是否接受 R-01 的反向视图方案 | 修改 |
| R-09 | architecture-owner | 合成关系、推导元数据及安全约束 | 确认 |

### 7.1 会签决定

每个 Owner 对负责的条目选择：**Confirm**（接受本稿定义）/ **Modify**（提出修改内容及理由）/ **Reject**（否决并说明原因）。

会签结果必须记录：关系 Key · 决定 · 修改内容或否决理由 · Owner 角色 · 会签人 · 会签日期 · 关联的 Registry 版本。

### 7.2 会签记录表

| Key | 决定 | 修改内容 / 否决理由 | Owner | 日期 |
|---|---|---|---|---|
| R-01 | 待会签 | | design-owner | |
| R-02 | 待会签 | | design-owner | |
| R-03 | 待会签 | | design-owner | |
| R-04 | 待会签 | | delivery-owner | |
| R-05 | 待会签 | | delivery-owner | |
| R-06 | 待会签 | | pc-owner | |
| R-07 | 待会签 | | kb-owner | |
| R-08 | 待会签 | | design-owner | |
| R-09 | 待会签 | | architecture-owner | |

### 7.3 Q-1 / Q-2 / Q-3 会签记录

| 问题 | 建议决定 | Owner 确认 |
|---|---|---|
| Q-1 Design ADR 绑定与 PC 认知关系 | 分层保留，允许有来源证据的视觉合并 | Design Owner + PC Owner |
| Q-2 PC 关系类型管理方式 | `pc.related` + PC Registry 交叉校验 | PC Owner |
| Q-3 Task 依赖方向 | Registry 为 dependent → prerequisite；DAG 可显式反转视觉流向 | Delivery Owner |

---

## 8. 会签通过后的执行顺序

1. 收齐领域 Owner 对 R-01~R-09 的决定。
2. 处理所有 Modify 与 Reject 项，形成最终关系定义。
3. 将通过的条目写入 `policies/viz-relation-registry.yaml`。
4. 通过的条目标记为 `active`；未完成会签的保留 `draft`；明确否决的标记 `rejected`；曾 Active 后被替代的标记 `deprecated`。
5. 更新 Registry Lint，执行完整性、端点、方向、领域交叉注册表与重复关系检查。
6. 执行 R-01~R-09 对应的 Golden Tests。
7. 将会签记录、Registry 版本与测试结果归档，作为 V0-1 完成证据。

**V0-1 完成条件**：所有准备激活的关系均完成对应领域 Owner 会签；关系来源、端点、方向与语义定义一致；R-05 方向矛盾已消除；R-06 与 PC 权威关系注册表实现交叉校验；R-07 Evidence 语义经 KB Owner 确认；R-08 不产生重复业务关系；R-09 的间接推导标识与权限边界经过验证；Registry Lint 与 Golden Tests 通过。

**在以上条件满足前，未会签条目必须保持不可投影状态。**

---

## 9. 术语澄清（关于「I-PM」）

专家稿注明"原文标题使用了 I-PM，本稿统一写作 AI-PM"。**核实结论：AI-PM 仓库与本套文档中不存在名为「I-PM」的独立产品或模块**（`AGENTS.md`、`README.md` 与 `docs/visualization/` 全量检索均无此名称，产品名统一为 **AI-PM**）。因此统一写作 AI-PM 正确，无需恢复原名。

## 10. 最终架构预审意见

**建议：有条件通过架构预审，提交领域 Owner 会签。**

本批词表的整体架构方向与 ADR-VIZ-004 的受控关系注册原则一致。主要修订集中在关系语义精确化、领域权威边界与图投影方向一致性，而不是扩大首批关系范围。

本稿不建议将 Visualization Registry 变成第二套业务关系模型：PC 关系继续由 PC 领域唯一维护；R-08 作为反向视图而非重复注册的业务事实；`derived_trace` 必须保持合成关系身份。

**本次预审不等于 V0-1 已完成。** V0-1 的唯一硬人工依赖仍是对应领域 Owner 的正式会签。会签通过后，才可将批准条目转为 Active 并进入后续实施。
