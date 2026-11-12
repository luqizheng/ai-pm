# Visualization Relation Registry V1.2 第一批会签执行版

> **状态**：待领域 Owner 会签（**专家架构裁决已通过，本文件不含任何已完成的领域授权**）
> **日期**：2026-11-12 · **基线**：[17 号词表 V1.2](ai-pm-viz-17-relation-registry-signoff-v1.md) · [20 号待决策项](ai-pm-viz-20-pending-decisions-v1.md) · 专家裁决 2026-11-12（分批激活批准）
> **专家明确声明**：架构裁决**不代替**领域 Owner 对具体 Relation 的正式签名；第一批只在对应 Owner 完成会签后才能真正写入 Active。

## 1. 本文件的作用（与专家裁决的分工）

| 层 | 由谁完成 | 状态 |
|---|---|---|
| 架构裁决：允许分批、Owner 分配、Q-1~Q-3 答复 | 专家 | ✅ 已完成（见 §2 裁决摘要） |
| **领域语义授权：R-01 / R-04 / R-05 是否可信** | **design-owner / delivery-owner** | ☐ **待会签** |
| 激活写入 Registry + 同步 lint / 测试 | 实施者（依签名执行） | ☐ 待前两项完成 |

**分批原则（专家裁决）**：每条关系**独立会签、独立 Active、独立生效**——不因同批中两条已签就自动激活第三条。

## 2. 专家裁决摘要（架构层，已生效）

| 编号 | 裁决 | 影响本文件 |
|---|---|---|
| Q1-1 | SeeCode 落仓形态 = A（vendor 进仓 `tools/seecode/`） | 属交付 B（M-2），不影响本文件 |
| Q1-2 | **必须先完成 M-2 签收再 vendor** | 见 22 号 |
| Q2-2 | **批准三批分批 Active 化**；第一批 = R-01 / R-04 / R-05 | **本文件核心** |
| Q2-1 | 九条 Owner 分配照准 | 见 §3 |
| Q2-3 Q-1 | Design 与 PC 的 ADR 关系**分层保留**，可视觉合并但保留双来源身份 | 第二批（R-02）前置 |
| Q2-3 Q-2 | PC **统一 `pc.related`**，不复制词表 | 第三批（R-06） |
| Q2-3 Q-3 | **方向不翻转**：Registry = dependent→prerequisite；DAG = prerequisite→dependent | 第一批（R-05）直接适用 |
| G-1/G-2/G-3 | 三条新增治理约束 | 见 §5 |

## 3. 第一批会签条目（逐条独立签名）

> **打印分发**：一页 A4 版本见 [23 号会签单](ai-pm-viz-23-batch1-signoff-form.html)（浏览器打开后 Ctrl+P，自包含无外网依赖）。


### R-01 `design.covers_requirement`

| 字段 | 值 |
|---|---|
| 责任 Owner | **design-owner** |
| 源表 | `design.design_requirement` |
| 端点 | `design_revision` → `requirement_revision` |
| 语义方向 | directed：design → requirement |
| 语义定义 | Design Revision 覆盖、实现或约束某一特定 Requirement Revision |
| 治理级别 | L2 |
| 为何进第一批 | 源表有明确复合键（design_revision_id + requirement_revision_id），语义歧义最小 |

**必须确认的三点**：
1. 语义确实是「覆盖 / 实现 / 约束」，而非仅「提及」或「弱关联」；
2. 图边读**绑定修订**（`requirement_revision_id`），不回退到 Requirement 当前版本；
3. 两端版本归属一致性由源数据保证（或读取时校验）。

**判定**：☐ Confirm　☐ Modify（改什么：______）　☐ Reject（理由：______）
**签署人 / 日期**：________________

---

### R-04 `task.implements_requirement`

| 字段 | 值 |
|---|---|
| 责任 Owner | **delivery-owner** |
| 源表 | `delivery.task_requirement` |
| 端点 | `task` → `requirement_revision` |
| 语义方向 | directed：task → requirement |
| 语义定义 | Task 被正式关联为实现某一特定 Requirement Revision 的交付任务 |
| 治理级别 | L2 |
| 为何进第一批 | `task_requirement` 以 (task_id, revision_id) 为主键，绑定关系显式 |

**必须确认的三点**：
1. 这是「实现范围绑定」，不是普通任务参考或弱关联；
2. 不得将历史绑定自动改写为 Requirement 当前 Revision；
3. **不得仅凭该关系推断 Task 已完成或 Requirement 已验收**（交付关联 ≠ 完成事实）。

**判定**：☐ Confirm　☐ Modify（______）　☐ Reject（______）
**签署人 / 日期**：________________

---

### R-05 `task.depends_on_task`

| 字段 | 值 |
|---|---|
| 责任 Owner | **delivery-owner** |
| 源表 | `delivery.task_dependency` |
| 端点 | `task` → `task` |
| **Registry 语义方向** | directed：`dependent → prerequisite`（`task_id → depends_on_task_id`） |
| **DAG 视觉方向** | `prerequisite → dependent`（与既有协作图方向一致，**不需要翻转**） |
| 语义定义 | Task A 依赖 Task B，表示 B 是 A 的前置任务 |
| 治理级别 | L2 |
| 为何进第一批 | 单表、CHECK 防自依赖、方向已与既有实现对齐（`execution_read.rs:568-570`） |

**必须确认的三点**：
1. `task_id` = 依赖方，`depends_on_task_id` = 前置方（这是库内定义，Registry 沿用它）；
2. **Registry 语义与 DAG 展示方向是两个约定**——不得为统一图形方向而改 Registry 语义（专家 Q-3 裁决）；
3. 影响分析与依赖闭包的遍历方向**相反**（依赖闭包沿 Registry 方向；影响分析沿反向）——两处代码必须各自显式声明。

**判定**：☐ Confirm　☐ Modify（______）　☐ Reject（______）
**签署人 / 日期**：________________

## 4. 未进第一批的条目（保持 Draft，不得提前激活）

| 条目 | Owner | 前置条件 |
|---|---|---|
| R-02 `design.decided_by_adr` | design-owner | Q-1 分层保留的落地确认（与 PC 关系并存规则） |
| R-03 `adr.covers_requirement` | design-owner | 约束语义强度确认（覆盖 / 约束 / 仅背景说明） |
| R-06 `pc.related` | pc-owner | 交叉校验规则确认；**不得成为绕过 PC 权威注册表的通用入口** |
| R-07 `kb.memory_supported_by_source` | kb-owner | 按代码实证修正后的目标实体（kb.source）作为会签对象 |
| R-08 `requirement.refines_design` | design-owner | 确认作为 R-01 反向视图别名（**不产生第二份业务关系数据、不独立 Active**） |
| R-09 `derived_trace` | architecture-owner | 合成边来源与推导规则；**不得成为任意用户/AI 可自由写入的关系** |

## 5. 三条新增治理约束（G-1/G-2/G-3）的落地

### G-1 激活必须可审计

每条 Relation 从 draft → active 必须留下**七项**记录（已扩展进 Registry schema 与 lint）：

| 字段 | 含义 |
|---|---|
| `key` | Relation ID |
| `registry_version` | 激活时的 Registry 版本 |
| `signoff_by` | 会签人（角色 + 姓名） |
| `signoff_at` | 会签时间 |
| `signoff_record` | 会签依据/记录引用（链接或会议纪要号） |
| `semantic_version` | 语义定义版本（语义变更须升版，不覆盖旧定义） |
| `activated_at` | 激活写入时间 |

**语义变更规则**：不得直接覆写已 Active 的 `semantics`——须升 `semantic_version` 并重新会签。

### G-2 Active 不代表历史数据自动合法

三个层次必须分开（**本文件只解决第 1 层**）：

| 层 | 含义 | 当前状态 |
|---|---|---|
| ① 定义已批准 | Registry 条目 Active | 第一批会签后完成 |
| ② 数据满足关系约束 | 库内历史数据是否都符合该关系定义 | **未验证**——需专门核查 |
| ③ 数据可进入图查询 | 通过权限与一致性校验 | 投影层已实现（双层过滤 + 双端隔离） |

**实施要求**：第一批激活后，需对存量数据做一次符合性抽样（尤其跨域引用），不得因 Registry Active 就假定历史数据合法。

### G-3 分批不破坏统一 Registry

- **一个 Registry 真源**（`policies/viz-relation-registry.yaml`）
- **一套 lint 规则**（`scripts/viz-registry-lint.js`）
- **一套 Active 治理机制**（本文 §5 G-1）
- 不同领域独立签收；未签收关系继续 draft
- **禁止**通过前端配置或任何旁路绕过后端校验

## 6. 激活执行清单（Owner 签名后由实施者执行）

- [ ] 收齐 R-01 / R-04 / R-05 三份签署（逐条独立，不合批）
- [ ] 逐条更新 YAML：`status: active` + G-1 七字段齐备
- [ ] `node scripts/viz-registry-lint.js` 通过（active 必须带会签记录）
- [ ] 关系校验：`cargo test -p pm-application --test viz_golden`（Golden 语料需同步把 draft 用例改为 active 用例）
- [ ] Live 图集成验收：
  - [ ] `GET .../live/trace-graph` 返回 Requirement→Design 与 Requirement→Task 边
  - [ ] `GET .../live/task-dag` 返回依赖边，方向 = `task_id → depends_on_task_id`
- [ ] **G-2 存量数据符合性抽样**（跨域引用、绑定修订一致性）
- [ ] 回填台账与 17 号会签记录表
- [ ] 第二、三批保持 draft，**不因本批通过而自动激活**

## 7. 解锁范围（第一批 Active 后的实际能力）

| 能力 | 解锁 |
|---|---|
| Requirement → Design 追踪图 | ✅ |
| Requirement → Task 实现追踪 | ✅ |
| Task DAG（含依赖方向） | ✅ |
| Requirement → Task 跨层追踪 | ✅（经 design_requirement + task_requirement 二跳，标 `derived_trace`） |
| ADR 相关图 | ❌ 待第二批 |
| PC 认知关系图 | ❌ 待第三批 |
| KB 证据溯源图 | ❌ 待第三批 |
| AI 生成静态产物（SeeCode） | ❌ 独立于本批，见 22 号 |
