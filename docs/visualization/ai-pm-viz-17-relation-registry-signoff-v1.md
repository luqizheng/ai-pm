# AI-PM Visualization Relation Registry 首批词表会签表 V1.0

> 状态：**待领域 Owner 会签**（V0-1 唯一硬人工依赖） · 日期：2026-11-12
> 上游：[ADR-VIZ-004](ai-pm-viz-09-architecture-decision-record-v1.md)（受控字符串 + 命名空间 + owner 会签）· [12 号 V0-1 设计](ai-pm-viz-12-v0-1-spec-validation-design-v1.md) §5
> 用途：会签通过后，本文 §3 的词表写入 `policies/viz-relation-registry.yaml`（status: active）；未会签的条目保持 `status: draft`，**校验器对其产出 Blocker**（未会签 = 不可用，而非默认放行）。

## 1. 会签须知（给领域 Owner 的三句话）

1. **Registry 是映射与校验机制，不是新的业务关系权威。** 本表不定义任何业务事实——每条 relation 只是**把已经存在于业务库里的既有关系映射成图上可读的边**。若你认为某条关系"业务上不成立"，那是业务表的问题，不是本表的问题。
2. **每条词条只需回答四个问题**：语义对不对？方向对不对？两端类型对不对？Owner 是谁？（其余字段由我们按既有库结构预填）
3. **判定分三档**：☐ 确认 / ☐ 需修改（写明改什么）/ ☐ 否决（写明理由）。**否决不是失败**——错误的词条比没有词条更危险（会让影响分析失真）。

## 2. 词条总览（8 条 + 1 条合成边）

| # | relation key | 中文语义 | 两端 | 源表（既有） | 领域 | 提案 Owner |
|---|---|---|---|---|---|---|
| R-01 | `design.covers_requirement` | 设计覆盖需求 | design_revision → requirement | `design.design_requirement`（0010:120） | design | design-owner |
| R-02 | `design.decided_by_adr` | 设计由 ADR 决策 | design_revision → adr_revision | `design.adr_design_revision`（0010:204） | design | design-owner |
| R-03 | `adr.covers_requirement` | ADR 覆盖需求 | adr_revision → requirement | `design.adr_requirement`（0010:220） | design | design-owner |
| R-04 | `task.implements_requirement` | 任务实现需求 | task → requirement_revision | `delivery.task_requirement`（0001:1881） | delivery | delivery-owner |
| R-05 | `task.depends_on_task` | 任务依赖于任务 | task → task | `delivery.task_dependency`（0001:1754） | delivery | delivery-owner |
| R-06 | `pc.related` | 产品认知对象关系 | pc object → pc object | `pc.relationship` + `pc.relationship_type`（0001:3293/3369） | pc | pc-owner |
| R-07 | `kb.supported_by_memory` | 知识条目被记忆支撑 | knowledge/memory → 任意对象 | `kb.memory_ref`（0001:2914，`ref_role='evidence'`） | kb | kb-owner |
| R-08 | `requirement.refines_design` | 需求细化设计（design.covers_requirement 的反向视角） | requirement → design_revision | 同 R-01（反向投影） | design | design-owner |
| R-09 | `derived_trace` | **间接推导**（合成边，非业务事实） | 任意 → 任意 | 无（VizSpec 层合成） | visualization | architecture-owner |

## 3. 逐条会签表

### R-01 `design.covers_requirement`

| 字段 | 值 |
|---|---|
| 语义 | 某条设计修订覆盖（实现/约束）了某条需求修订 |
| source type | `design_revision` |
| target type | `requirement_revision` |
| 方向 | directed |
| 既有源表 | `design.design_requirement`（`design_revision_id` + `requirement_id` + `requirement_revision_id`；0010:120-133） |
| 治理级别 | L2 |
| 备注 | 该表**绑定到具体修订**（含 `requirement_revision_id`）→ 图上应读**绑定修订**而非当前修订（14 号 §5.4 固定 revision 读法） |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

### R-02 `design.decided_by_adr`

| 字段 | 值 |
|---|---|
| 语义 | 某条设计修订受某条 ADR（架构决策）约束/裁决 |
| source type | `design_revision` |
| target type | `adr_revision` |
| 方向 | directed |
| 既有源表 | `design.adr_design_revision`（0010:204） |
| 备注 | 与 pc 域同名关系 `design-constrained-by-adr`（0007:69-74，owner_domain='kb'）**语义相近但不同源**——pc 的是认知侧边，本条是 design 域的权威绑定。图上两者是否都出现，请一并判定（见 §5 问题 1） |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

### R-03 `adr.covers_requirement`

| 字段 | 值 |
|---|---|
| 语义 | 某条 ADR 覆盖（约束/影响）了某条需求修订 |
| source type | `adr_revision` |
| target type | `requirement_revision` |
| 方向 | directed |
| 既有源表 | `design.adr_requirement`（0010:220） |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

### R-04 `task.implements_requirement`

| 字段 | 值 |
|---|---|
| 语义 | 某任务实现了某条需求修订 |
| source type | `task` |
| target type | `requirement_revision` |
| 方向 | directed |
| 既有源表 | `delivery.task_requirement`（`task_id` + `revision_id`，主键；0001:1881-1888） |
| 备注 | 该表绑定 `revision_id`（需求修订），不是需求当前态 |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

### R-05 `task.depends_on_task`

| 字段 | 值 |
|---|---|
| 语义 | 某任务依赖于另一任务先完成 |
| source type | `task` |
| target type | `task` |
| 方向 | directed（**被依赖 → 依赖方**，图上箭头指向依赖方） |
| 既有源表 | `delivery.task_dependency`（`task_id` / `depends_on_task_id`；0001:1754-1763，CHECK 防自依赖） |
| 备注 | 方向约定需明确：表中 `task_id` 是"依赖方"，`depends_on_task_id` 是"被依赖方"。**图中箭头方向请判定**：建议 依赖方 → 被依赖方（读者直觉："A 依赖 B"） |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

### R-06 `pc.related`

| 字段 | 值 |
|---|---|
| 语义 | 产品认知对象之间的**已登记类型**关系（具体类型由 pc.relationship_type 决定） |
| source type | `pc_object`（通配） |
| target type | `pc_object`（通配） |
| 方向 | 继承 `pc.relationship_type.directionality`（directed / undirected） |
| 既有源表 | `pc.relationship`（0001:3293）+ `pc.relationship_type`（3369，含 `directionality` / `source_object_types` / `target_object_types` / `governance_level` / `owner_domain`） |
| 备注 | **不建议逐条映射 20+ 个 pc 类型到 Registry**（会与 pc 域词表双轨）。建议：图上把 pc 关系统一呈现为 `pc.related` + 边上带 pc 的 `type_key` 作为**标签**。是否接受此简化，见 §5 问题 2 |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

### R-07 `kb.supported_by_memory`

| 字段 | 值 |
|---|---|
| 语义 | 某业务对象的认知被知识记忆条目作为证据支撑 |
| source type | `memory` |
| target type | `*`（任意业务对象） |
| 方向 | directed |
| 既有源表 | `kb.memory_ref`（`ref_role='evidence'` 时成立；`ref_role='subject'` 是反向——记忆的主体是目标对象；0001:2914-2929） |
| 备注 | `ref_role` 二值（evidence / subject）决定方向；建议图上**只呈现 evidence 方向**（记忆 → 对象），subject 方向是同一事实的反向表述，避免同图双箭头 |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

### R-08 `requirement.refines_design`

| 字段 | 值 |
|---|---|
| 语义 | 某条需求修订细化了某条设计修订（= R-01 的反向视角） |
| source type | `requirement_revision` |
| target type | `design_revision` |
| 方向 | directed |
| 既有源表 | 同 R-01（**反向投影，非新表**） |
| 备注 | 词表允许同一事实的多个方向视角（与 `pc.relationship_type` 的"规则→需求用既有 constrains 反向覆盖"先例一致，0007:44-46）——但**同一张图内不得同时出现 R-01 与 R-08 的同一条边**（去重规则：反向边存在时，正向边由该视角替代） |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

### R-09 `derived_trace`（合成边，非业务事实）

| 字段 | 值 |
|---|---|
| 语义 | **经中间对象推导的间接关系**；不代表业务上的直接实施关系 |
| source type | `*` |
| target type | `*` |
| 方向 | directed |
| 源表 | 无（VizSpec 层合成，字段 `derived: {direct:false, via:[…], semantic_confidence:"structurally_supported"}`） |
| 归属域 | visualization（由 Architecture Owner 治理，非业务域） |
| 强制标注 | 图上必须虚线 + 「经 X 推导」；影响分析中归入 **Indirect** 组（14 号 §6、ADR-VIZ-005） |

**判定**：☐ 确认　☐ 需修改：______　☐ 否决：______　Owner：__________

## 4. 会签后落地步骤（V0-1.5）

1. 把确认条目写入 `policies/viz-relation-registry.yaml`，`status: active`；否决条目 `status: deprecated`（保留历史解释能力）。
2. 每条填 `owner`（角色名）+ `signoff_date` + `introduced_in: "1.0"`。
3. 跑 `scripts/viz-registry-lint.js`（字段完整性、未登记、类型不匹配、draft 未会签、语义变更未升版）。
4. `derived_trace` 若被否决 → 二跳拼接边改为**只带 `via` 标注的普通边**，且必须在 UI 明示推导路径（否则违反 ADR-VIZ-005 的"不得把二跳推导展示成真实直接关系"）。

## 5. 需要 Owner 共同回答的三个问题

| # | 问题 | 背景 | 答复 |
|---|---|---|---|
| Q-1 | **design 域的 ADR 绑定**（R-02）与 **pc 域的 `design-constrained-by-adr`**（0007:69，owner_domain='kb'）语义相近，是否视为同一事实？ | 若视为同一事实 → 图上需去重，避免同图两条等价边；若视为不同事实 → 需在边上区分来源（design 权威绑定 vs pc 认知边） | |
| Q-2 | **pc 关系是否逐条映射**（R-06）：20+ 个 `pc.relationship_type` 全量入 Registry，还是统一为 `pc.related` + 边上带 type_key 标签？ | 全量映射可做严格类型校验，但词表双轨维护成本高 | |
| Q-3 | **R-05 箭头方向**：依赖方 → 被依赖方（读者直觉）还是反向？ | 影响 Task DAG 的可读性与关键路径计算方向 | |

## 6. 回执

| relation key | 判定 | 修改/否决说明 | Owner | 日期 |
|---|---|---|---|---|
| `design.covers_requirement` | | | | |
| `design.decided_by_adr` | | | | |
| `adr.covers_requirement` | | | | |
| `task.implements_requirement` | | | | |
| `task.depends_on_task` | | | | |
| `pc.related` | | | | |
| `kb.supported_by_memory` | | | | |
| `requirement.refines_design` | | | | |
| `derived_trace` | | | | |

**Q-1 / Q-2 / Q-3 答复**：Q-1 ______　Q-2 ______　Q-3 ______
