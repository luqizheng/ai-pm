# AI-PM Visualization 会签材料包（发送稿）V1.0

> **状态**：待发送 · 日期：2026-11-12
> **用途**：两封可直接复制的定向材料——① 给架构/安全评审专家的增量说明；② 给领域 Owner 的词表会签材料。
> **纪律**：本包是**发送稿**，不是新的架构文档；所有结论均已在 [17 号词表 V1.1](ai-pm-viz-17-relation-registry-signoff-v1.md) 与 [09 号 ADR 附录 A](ai-pm-viz-09-architecture-decision-record-v1.md) 落定并归档。

---

# 第一封：致架构/安全评审专家（增量说明）

> **主题**：AI-PM Visualization —— 预审 V1.1 采纳意见与两处修正（无需新一轮复审，仅请确认）
>
> 专家老师您好：
>
> 您对 Registry 首批词表的架构预审（V1.1 稿）已收到。三个架构裁决**全部采纳并落定**：
> R-05 语义方向与执行流向分离、R-06 PC Registry 为唯一词汇权威、R-08 反向视图不等于新业务关系。
>
> 同时有两处我们基于**代码实证**对预审稿作了修正，需要您确认：
>
> **① R-07 的方向与端点（推翻预审稿表述）**
>
> 预审稿 R-07 写 `memory → business_object`、目标对象"由 memory_ref 的实际引用结构确定"。代码实证显示该语义不存在：
>
> - `crates/pm-application/src/kb_service/memory.rs:44` — `RefRole::Evidence` 的权威注释是「**来源依据：指向 L2 原文**（会话消息 / 任务交互 / 原始资料）」；
> - `crates/pm-application/src/kb_service/source.rs:297` — 实际查询恒为 `ref_role='evidence' AND target_domain='knowledge_base' AND target_type='source'`；
> - `crates/pm-application/src/kb_service/mod.rs:36` — `TYPE_SOURCE = "source"`。
>
> 即 evidence 指向的是 **kb.source（原始资料）**，语义是"该记忆以这条资料为依据"，而非"记忆支撑某业务对象"。按预审稿实现会在图上产生假边。
>
> 已修正为 **`kb.evidence_supported_by`（memory → evidence_source）**，并注明仍需 KB Owner 会签确认。
>
> **② R-05 方向裁决的落地矩阵（我方原表有误，已修正）**
>
> 您要求"关键路径和依赖闭包计算必须明确使用哪一种方向约定"。我们最初给的方向表有误（把依赖闭包与关键路径并作一行且方向写反），经代码实证后修正如下：
>
> | 计算 | 遍历方向 |
> |---|---|
> | 依赖闭包（A 依赖了谁） | 沿 Registry 方向 `dependent → prerequisite` |
> | 影响分析（改 B 影响谁） | 反向 `prerequisite → dependent` |
> | 关键路径 / CPM | `prerequisite → dependent` |
>
> **依赖闭包与影响分析方向相反**——若影响分析按 Registry 方向遍历，会得出"B 受 A 影响"的反向结论（实际是 A 受 B 影响）。
>
> 同时查到仓库既有约定与 Registry 语义方向相反，已加显式映射隔离：
>
> - `crates/pm-application/src/execution_read.rs:568-570` — 协作图边 `from = depends_on_task_id`（前置）、`to = task_id`（依赖方），即**既有图边方向 = prerequisite → dependent**；
> - `crates/pm-application/src/execution_read.rs:548-550` — `open_upstream_count` 统计前置任务未完成数，注释「未完成的上游依赖数」，即 **upstream = 前置任务**（术语陷阱：不是"受影响的上游"）。
>
> 实践结论：既有图边方向与影响分析方向一致 → **DAG 视觉无需翻转**，直接沿既有方向渲染；仅 Registry 存储的语义方向与之相反。
>
> **另**："I-PM"经核实为误写——仓库与文档中不存在该产品名，统一为 AI-PM。
>
> 以上不改变任何已裁决的架构方向，故未申请新一轮复审。若您认为上述两处修正有误，请直接指出，我们改后再报。
>
> 谢谢！

---

# 第二封：致领域 Owner（词表会签材料）

> **主题**：AI-PM Visualization 关系词表首批条目 —— 请会签（V0-1 交付前置）
>
> 各位 Owner 您好：
>
> AI-PM 即将落地统一可视化引擎，需要把"业务库里已存在的关系"映射成图上可读的边。为避免造出第二套业务关系权威，这批关系需由**各领域 Owner 会签**后才能启用。
>
> **为什么需要您签字**：关系词表的语义归业务领域所有，Visualization 只负责映射与校验，不定义业务事实。我们已按既有库结构预填了语义、方向与两端类型，请核对是否准确。
>
> **判定的三档**：
> - ☑ **确认** — 定义准确，可启用；
> - ☐ **需修改** — 请写明改什么（语义 / 方向 / 两端类型）；
> - ☐ **否决** — 请写明理由。
>
> **说明**：否决不是失败——错误的词条比没有词条更危险（会让影响分析、覆盖检查失真）。
>
> **附件**：[17 号词表会签表 V1.1](ai-pm-viz-17-relation-registry-signoff-v1.md)（逐条定义 + 会签记录表）
>
> **分工**：
>
> | Owner | 负责条目 | 重点 |
> |---|---|---|
> | design-owner | R-01 `design.covers_requirement` | Design Revision 对 Requirement Revision 的正式覆盖语义 |
> | design-owner | R-02 `design.decided_by_adr` | Design 与 ADR Revision 的正式绑定语义（另需 PC Owner 分别确认 PC 侧相似关系） |
> | design-owner | R-03 `adr.covers_requirement` | ADR Revision 对 Requirement Revision 的约束语义 |
> | design-owner | R-08 `requirement.refines_design` | 是否接受作为 R-01 的反向视图别名 |
> | delivery-owner | R-04 `task.implements_requirement` | Task 与指定 Requirement Revision 的实现关系 |
> | delivery-owner | R-05 `task.depends_on_task` | 依赖字段语义与箭头方向 |
> | pc-owner | R-06 `pc.related` | 统一投影入口与 PC Registry 的交叉校验 |
> | kb-owner | R-07 `kb.evidence_supported_by` | **evidence 引用方向与目标对象类型（已按代码修正为指向 kb.source，请确认）** |
> | architecture-owner | R-09 `derived_trace` | 合成关系、推导元数据与安全约束 |
>
> **需要共同回答的三个问题**：
>
> 1. **Q-1**（Design + PC Owner）Design 域的 ADR 绑定与 PC 域的 `design-constrained-by-adr` 指向相同对象，是否视为同一事实？预审建议：**分层保留**（权威来源不同），允许同图视觉合并但须保留双来源证据，且权限过滤先于合并。
> 2. **Q-2**（PC Owner）PC 的 20+ 关系类型是否全部映射进 Visualization Registry？预审建议：**不复制**，统一 `pc.related` + 引用 PC 权威注册表并做交叉校验。
> 3. **Q-3**（Delivery Owner）依赖箭头方向？预审建议：**Registry 语义方向 = 依赖任务 → 前置任务**；Task DAG 的执行流向是**显式展示变换**，不改语义方向（仓库既有图边方向为前置 → 依赖方，与影响分析方向一致）。
>
> **会签记录请填在 17 号的 §7.2 表格**（Key / 决定 / 修改内容或否决理由 / Owner / 日期），或直接回复本邮件。
>
> **时限与影响**：会签未完成的条目将保持 `draft` 且**不可投影**（不会默认放行）。这批条目是 V0-1 的唯一硬人工依赖，直接影响首批可用的追踪图与任务依赖图。
>
> 谢谢！

---

## 附：发送清单与状态跟踪

| # | 材料 | 接收方 | 状态 | 追踪位置 |
|---|---|---|---|---|
| 1 | 第一封（增量说明） | 架构/安全评审专家 | ☐ 待发 | 17 号 §0 的 V-09/V-10/V-11 |
| 2 | 第二封（会签材料）+ 17 号附件 | design / delivery / pc / kb / architecture Owner | ☐ 待发 | 17 号 §7.2 会签记录表 |
| 3 | M-1~M-8 修订签收 | Architecture Owner + Security Owner | ☐ 待发 | 09 号附录 A |
| 4 | 取号 0061 签收 | Migration Owner | ☐ 待发 | 10 号 §6.1（真实 master 复核） |
| 5 | P0 验收矩阵签收 | QA Owner | ☐ 待发 | 10 号全文 |

**收齐后的下一步**（产品经理已就绪，不需再决策）：

1. 汇总 Owner 决定 → 处理 Modify/Reject → 写入 `policies/viz-relation-registry.yaml`；
2. 标记 `active`（仅已会签条目）+ 更新 Registry Lint；
3. 执行 R-01~R-09 的 Golden Tests；
4. 归档会签记录与 Registry 版本，作为 V0-1 完成证据；
5. **开启 V0-1 编码**（类型 + 校验器 + Registry 加载器 + Golden 语料 + 三个 lint 脚本）。
