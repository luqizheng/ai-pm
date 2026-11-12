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
> **① R-07 的方向与端点（基于代码实证修正）**
>
> 预审稿 R-07 使用了 `memory → business_object`，并将目标对象定义为由 memory_ref 的实际引用结构确定。经代码核查，该语义不成立：
>
> - `crates/pm-application/src/kb_service/memory.rs:44`：`RefRole::Evidence` 的权威注释为"来源依据：指向 L2 原文（会话消息 / 任务交互 / 原始资料）"；
> - `crates/pm-application/src/kb_service/source.rs:297`：实际查询限定 `ref_role='evidence' AND target_domain='knowledge_base' AND target_type='source'`；
> - `crates/pm-application/src/kb_service/mod.rs:36`：`TYPE_SOURCE = "source"`。
>
> 因此，Evidence 引用的目标是 KB Source（原始资料），而非普通业务对象。其语义应为"该 Memory 以这条 Source 作为来源依据"。
>
> 关于命名：我们曾用 `kb.evidence_supported_by`，经审校发现该英文语序易误读为"Evidence 被某对象支持"。已定名为：
>
> - **Key：`kb.memory_supported_by_source`**（待 KB Owner 确认命名）
> - Source Type：`memory` · Target Type：`source` · 方向：`memory → source`
> - 来源：`kb.memory_ref` · 过滤条件：`ref_role='evidence'`
> - **一对多已实证**：`memory_ref` 无 UNIQUE(revision_id) 约束且插入侧为 refs 循环（`memory.rs:325-335`）→ 一条记忆修订可引用多条 Source，图上允许一对多证据边
>
> 该修正避免将 Memory 与业务对象之间不存在的关系投影为图边。最终语义仍由 KB Owner 会签确认。
>
> **② R-05 方向裁决的落地矩阵（已基于代码实证修正）**
>
> 您要求"关键路径和依赖闭包计算必须明确使用哪一种方向约定"。我们最初给的方向表有误（把依赖闭包与关键路径并作一行且方向写反），经代码实证后修正为三个层次：
>
> | 层次 | 方向 |
> |---|---|
> | ① 关系存储语义（Registry） | `dependent → prerequisite`（A depends on B 记为 A → B） |
> | ② 图查询计算 | 依赖闭包沿 ① 方向；**影响分析与 CPM 沿反向（`prerequisite → dependent`）** |
> | ③ DAG 视觉方向 | 沿既有 `prerequisite → dependent` **直接展示，不需要额外翻转** |
>
> 依赖闭包与影响分析方向相反，不能共用未经区分的遍历方向——若影响分析按 Registry 方向遍历，会得出"B 受 A 影响"的反向结论（实际是 A 受 B 影响）。
>
> 仓库既有协作图方向为 `prerequisite → dependent`，已与执行流向和影响传播方向一致：
>
> - `crates/pm-application/src/execution_read.rs:568-570`：协作图边 `from = depends_on_task_id`（前置）、`to = task_id`（依赖方）；
> - `crates/pm-application/src/execution_read.rs:548-550`：`open_upstream_count` 统计前置任务未完成数，既有术语中 **upstream 指前置任务**。
>
> 因此，Task DAG 沿用既有图边方向直接展示，不需要额外翻转；两种方向的差异仅在 Registry 投影与图查询适配层显式处理。
>
> **另**："I-PM"经核实为误写——仓库与文档中不存在该产品名，统一为 AI-PM。
>
> 上述修正不改变既有架构裁决，也不新增架构决策项，因此本次不申请重启完整架构评审。我们仅提交两处代码实证修正，**请您确认其语义及落地映射是否与既有架构约束一致**。若您认为修正有误，请直接指出，我们改后再报。
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
> **附件**：[17 号词表会签表 **V1.2**](ai-pm-viz-17-relation-registry-signoff-v1.md)（逐条定义 + 会签记录表）
>
> **分工**：
>
> | Owner | 负责条目 | 重点 |
> |---|---|---|
> | design-owner | R-01 `design.covers_requirement` | Design Revision 对 Requirement Revision 的正式覆盖语义 |
> | design-owner | R-02 `design.decided_by_adr` | Design 与 ADR Revision 的正式绑定语义（另需 PC Owner 分别确认 PC 侧相似关系） |
> | design-owner | R-03 `adr.covers_requirement` | ADR Revision 对 Requirement Revision 的约束语义 |
> | design-owner | **R-08 反向视图别名（非独立 Active 关系）** | 确认 `requirement.refines_design` 是否仅作为 `design.covers_requirement` 的反向查询视图；若是，**不生成独立 Active 关系键、不新增业务关系事实、不重复计数** |
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
> 3. **Q-3**（Delivery Owner）依赖箭头方向？裁定：**Registry 规范语义方向为 dependent → prerequisite**；既有协作图方向为 prerequisite → dependent，与执行流向和影响传播方向一致——**Task DAG 沿用既有图边方向直接展示，不需要额外翻转**；方向差异仅在 Registry 投影与图查询适配层显式处理。
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
| 2 | 第二封（会签材料）+ **17 号 V1.2 附件** | design / delivery / pc / kb / architecture Owner | ☐ 待发（**附件必须为 V1.2**，V1.1 含已推翻的 R-07 旧语义） | 17 号 §7.2 会签记录表 |
| 3 | M-1~M-8 修订签收 | Architecture Owner + Security Owner | ☐ 待发 | 09 号附录 A |
| 4 | 取号 0061 签收 | Migration Owner | ☐ 待发 | 10 号 §6.1（真实 master 复核） |
| 5 | P0 验收矩阵签收 | QA Owner | ☐ 待发 | 10 号全文 |

## 发送前最终检查清单（2026-11-12 审校后核对）

| # | 检查项 | 状态 |
|---|---|---|
| 1 | 17 号词表升 **V1.2**，R-07 已替换为 kb.source 语义 | ✅ 已完成（本包同批更新） |
| 2 | R-07 Key / Source Type / Target Type / 方向已统一（`kb.memory_supported_by_source`，memory → source） | ✅ 已完成 |
| 3 | R-05 依赖闭包 / 影响分析 / CPM 方向在两封邮件与附件中一致（三层表述） | ✅ 已完成 |
| 4 | DAG 沿既有 prerequisite → dependent 展示，**不再描述为需要额外翻转** | ✅ 已完成 |
| 5 | R-08 明确为反向视图别名，不作为独立 Active 关系 | ✅ 已完成 |
| 6 | 第一封请求限定为**增量修正确认**，不宣布专家已通过 | ✅ 已完成 |
| 7 | 09 号附录 A M-1~M-8 修订项与签收记录核对 | ☐ 发送时人工核对 |
| 8 | 10 号 §6.1 取号 0061 与真实 master 复核记录核对 | ☐ 发送时人工核对 |
| 9 | 收件人、Owner 角色及负责条目确认 | ☐ 发送时人工核对 |

**收齐后的下一步**（产品经理已就绪，不需再决策）：

1. 汇总 Owner 决定 → 处理 Modify/Reject → 写入 `policies/viz-relation-registry.yaml`；
2. 标记 `active`（仅已会签条目）+ 更新 Registry Lint；
3. 执行 R-01~R-09 的 Golden Tests；
4. 归档会签记录与 Registry 版本，作为 V0-1 完成证据；
5. **开启 V0-1 编码**（类型 + 校验器 + Registry 加载器 + Golden 语料 + 三个 lint 脚本）。
