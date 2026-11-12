# Visualization 两项待决策项审阅包 V1.0

> **状态**：待专家裁决 · 日期：2026-11-12
> **用途**：两个卡点的定向提问——① SeeCode 是否落仓（M-2 修订签收）；② 词表 9 条 relation 是否 active 化。
> **前置事实**：viz 全链路代码已落地并推 master（最新 `b3ae2e6d`），**当前唯二阻塞是这两个决策**，非技术实现问题。

---

## 决策一：SeeCode 是否 vendor 进本仓（M-2 修订签收）

### 已落地现状（不需重做）

| 项 | 状态 |
|---|---|
| 渲染编排 | ✅ `render_artifact`（Adapter 调用 → **SVG 必经静态化检查** → 落 `viz.render_output`） |
| Adapter 契约 | ✅ `RenderAdapter` trait + 八项契约断言（`verify()`） |
| 失败降级 | ✅ 三态 `Rendered`/`Rejected`/`Failed`；失败不落库；前端表格兜底 |
| 落库幂等 | ✅ 同 artifact+format `ON CONFLICT DO UPDATE` |
| **渲染器实现** | ❌ **唯一缺口**——未注册任何 Adapter，产物通道目前只能降级 |

### 需要裁决的五个问题

**Q1-1 落仓形态**：SeeCode 以什么形式进入 AI-PM？

| 选项 | 说明 | 代价 |
|---|---|---|
| A. vendor 进仓 `tools/seecode/` | 约 1720 行 JS + vendor/ 编码器（gifenc 等）+ MIT 声明 | 第三方代码在仓；升级需手工重 vendor |
| B. 构建期依赖（私有 npm/镜像） | 不落仓，镜像构建时拉取 | 需维护分发通道；内网构建需可达 |
| C. 独立渲染 Worker/服务 | 单独部署，HTTP 调用 | 违背 ADR-VIZ-001「不提前微服务」；多一套运维面 |
| D. 不采用 SeeCode，只用 Fallback（表格/Mermaid） | 零第三方 | 失去「编辑级静态图」产物能力（本项目立项时的产物质量来源） |

**倾向 A**（锁版本 vendor）。理由：唯一需要 Node 运行时的地方；B 在内网构建场景不可行；C 与已裁决的部署形态冲突。

**Q1-2 M-2 修订是否需要专家签收**：ADR-VIZ-002 原定 SeeCode 为「浏览器内产物渲染器」，15 号安全审查（实测）已改为「**服务端出图器（SVG only）**」。这是**定位变更**——vendor 落仓前是否必须走完 M-2 签收？还是可以先落仓（代码可随时移除）+ 后补签收？

**倾向**：先签收再落仓。落仓是「承担维护责任」的实质动作，签收是「承认这个责任边界」的形式动作——顺序反了会让责任人缺位。

**Q1-3 升级策略**：SeeCode 是个人 0.x 项目（v0.6.3）。
- 锁版本死守（安全回归可控，但放弃上游修复）
- 定期跟进（每次升级重跑 AC-7′ 注入用例 + 八项契约断言）
- **倾向**：锁版本 + 每季度评估；升级必须重跑 `viz_artifact_safety`（真实产物夹具）与 `adapter_contract`（八项断言）。

**Q1-4 容器 Node 运行时**：worker 镜像需加 Node 20（SeeCode CLI 依赖）。这是部署面变更——
- 是否接受？影响镜像体积与攻击面（Node 运行时本身）。
- **倾向**：接受，但仅在渲染 job 的 worker 镜像层；纯 SVG 出图（实测无需 Chrome）。

**Q1-5 安全回归责任**：vendor 后，AC-7′（产物静态化）的夹具需随渲染器版本重跑。谁负责？
- **倾向**：Adapter 变更即触发 `viz_artifact_safety` + `adapter_contract` 全绿，作为合并门禁（已可用脚本化）。

---

## 决策二：词表 9 条 relation 是否 active 化（17 号 V1.2 会签）

### 已落地现状

| 项 | 状态 |
|---|---|
| 词表真源 | ✅ `policies/viz-relation-registry.yaml`（9 条，**全 draft**） |
| 治理门禁 | ✅ `viz-registry-lint.js`（15 条规则；active 必须带会签记录） |
| 校验器行为 | ✅ 对 draft 关系产出 `E_SPEC_RELATION_NOT_APPROVED`（**阻断**） |
| Live 图 | ⚠️ 端点已通，但因关系全 draft，**实际图不可用**（被校验器阻断） |

**这是当前唯一阻塞业务可用性的项**：代码全通，但一条关系都没启用。

### 需要裁决的三个问题

**Q2-1 五个域 Owner 的会签责任**（17 号 §7 的逐条表）

| 条目 | Owner | 需确认 |
|---|---|---|
| R-01 `design.covers_requirement` | design-owner | 覆盖/实现/约束语义 |
| R-02 `design.decided_by_adr` | design-owner | 与 PC 同名关系分层（Q-1） |
| R-03 `adr.covers_requirement` | design-owner | 约束语义强度 |
| R-04 `task.implements_requirement` | delivery-owner | 实现绑定（非弱关联） |
| R-05 `task.depends_on_task` | delivery-owner | 方向（dependent→prerequisite） |
| R-06 `pc.related` | pc-owner | 统一投影入口 + 交叉校验 |
| R-07 `kb.memory_supported_by_source` | kb-owner | **已按代码实证改为指向 kb.source**（原稿 business_object 不成立） |
| R-08 `requirement.refines_design` | design-owner | 反向视图别名（非独立关系） |
| R-09 `derived_trace` | architecture-owner | 合成关系身份与安全约束 |

**Q2-2 是否允许分批 active 化**（关键提议）

当前是「全 draft = 全不可用」的二元状态。建议改为分批：

| 批次 | 条目 | 理由 |
|---|---|---|
| 第一批 | R-01 / R-04 / R-05 | 源表结构最确定、语义歧义最小（`design_requirement` / `task_requirement` / `task_dependency` 三表均有明确键约束）；启用后追踪图与 Task DAG 即可用 |
| 第二批 | R-02 / R-03 / R-08 | 涉及 ADR 与反向视图，需 design-owner 先答 Q-1/Q-3 |
| 第三批 | R-06 / R-07 / R-09 | 跨 Registry（PC）、代码语义修正（KB）、合成边（架构），需各自 Owner 独立确认 |

**倾向**：允许分批。理由：全 draft 让已完工的链路空转；分批不降低治理标准（每条仍需签收才能 active）。

**Q2-3 Q-1~Q-3 三个共答问题的答复**（17 号 §4 已有预审建议，需 Owner 确认）
- **Q-1**：Design 的 ADR 绑定 vs PC 的 `design-constrained-by-adr` 是否同一事实？（预审建议：**分层保留**，同图可视觉合并但保留双来源）
- **Q-2**：PC 的 20+ 关系类型是否全量映射？（预审建议：**不复制**，统一 `pc.related` + 引用 PC 权威注册表）
- **Q-3**：依赖箭头方向（预审建议：Registry 语义 = dependent→prerequisite；DAG 视觉=prerequisite→dependent，**已对齐既有协作图方向，无需翻转**）

---

## 两个决策的依赖关系（重要）

```
决策二（词表 active 化）  →  解锁 Live 图 / 追踪图 / Task DAG 的业务可用性
                              （代码已就绪，只差"关系可投影"）
决策一（vendor SeeCode）  →  解锁产物通道（AI 生成的静态图）
                              （不阻塞 Live 图；产物通道当前降级为表格）
```

**两者互不阻塞**：决策二先落，Live 图立刻可用；决策一后落，产物通道再补齐。

## 不决策的后果

| 现状 | 后果 |
|---|---|
| 词表全 draft | 所有图查询被校验器阻断 → 引擎等于未上线（尽管代码全绿） |
| 无渲染器 Adapter | 产物通道只能降级表格 → AI 生成图能力不可用，但 Live 图不受影响 |
