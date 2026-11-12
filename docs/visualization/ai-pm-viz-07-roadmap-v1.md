# 实施路线图及任务拆解 V1.1

> 状态：已并入 2026-11-12 专家评审（有条件通过）· 关键调整：**P0 不含 Live 图**，SeeCode 降为实验接入非核心依赖，安全与契约测试前移（DV-14）。

## P0 Foundation & Contract

| 任务 | 内容 | 验收锚点 |
|---|---|---|
| V0-1 Spec 核心与校验器 | VizSpec 核心模型（四段式）+ JSON Schema + Rust 校验器（E/W/I）+ source_hash 算法 + **Relation Registry**（命名空间词表 `domain.relation` + lint + 领域 owner 会签流程，DV-9） | AC-1 |
| V0-2 Artifact 元数据与存储 | 迁移 0020（viz.artifact/approval/render_output/slot_config，含 **provenance 与 nature 字段**，DV-7/DV-13）+ `/v1/visualizations/*` 端点 + 审批状态机 + stale 判定 | AC-3/4 |
| V0-3 权限投影与一致性 | 查询+投影双层权限过滤（**边存在性不泄露**，Q11）+ Live 响应 `consistency` 标记与混合时点行为（Q12） | AC-17/19 |
| V0-4 Adapter Contract | Adapter Contract Tests（合法 HTML/SVG、零意外外网请求、无注入、溯源保留、节点/边身份保留、失败可处理）+ **Spec Golden Tests（与 SeeCode 无关）** + Fallback Adapter（表格/Mermaid）+ 降级能力矩阵文档 | AC-2/21 |
| V0-5 AI 生成链路（实验） | envelope 第 9 类 `viz_artifact`（含 nature）+ 起草 prompt 规范（含"何时不画图"与 Business/Explanatory 分流）+ 修补循环 ≤3 轮 + 对话卡 Accept/Reject | AC-5/20 |

## P0 Security & Viewer

| 任务 | 内容 | 验收锚点 |
|---|---|---|
| V0-6 安全基座 | 独立产物 Origin + sandbox iframe（无 same-origin/top-navigation）+ CSP Header 全套（connect/form/object/base-uri/frame-ancestors）+ postMessage 白名单协议（source+会话标识）+ iframe 零凭证零 API | AC-7/8/18 |
| V0-7 注入与转义 | 输入层纯文本约束 + SeeCode 安全渲染实测（textContent/innerHTML）+ **禁盲目预转义**（双重转义用例）+ 注入向量全覆盖（节点/边/tooltip/focus/导出 SVG + Unicode/RTL/零宽） | AC-9 |
| V0-8 Viewer 基础组件 | VizViewer/ArtifactFrame/DegradedTableView/TracePanel/VersionBar/ApprovalCard/StaleBar + PRISM 令牌扩展 + i18n（`viz.*`）+ 降级表格 | AC-12/14 |

## P0 First Renderer（SeeCode，实验接入）

| 任务 | 内容 | 验收锚点 |
|---|---|---|
| V0-9 vendor 评估与接入 | `tools/seecode/` 锁版本 vendor（MIT 声明保留）+ Spec→SeeCode 编译器 + worker 渲染 job（HTML/SVG）+ `fonts:system` 零外网配置 + Node 镜像层；**失败降级为表格，不阻断任何链路** | AC-6 |
| V0-10 中文与导出验证 | 中文 spec 用例集（换行/图例/SVG 内嵌字体/深色一致）+ SVG/PNG 导出链零外网**实测** + CLI 本地 Chrome 出 GIF/MP4 是否依赖 jsDelivr（V2 决策输入，不阻塞） | AC-6/10 |

P0 三段顺序依赖：V0-1 → (V0-2/V0-4/V0-5 并行) → V0-9；V0-6/7 与 V0-4 并行，V0-8 收口。

## P1 Live Graph（G6 5）

| 任务 | 内容 | 依赖 |
|---|---|---|
| V1-1 G6 引入与验证 | 动态加载/独立 chunk/布局按需注册；**构建包体与运行性能实测后 Q2 方可冻结**；Vue Flow 仅在未来节点编辑需求出现时再评估 | P0 |
| V1-2 追踪图 | trace-graph + 追踪四表聚合 + **Direct/Indirect 影响区分**（DV-8，derived_trace 虚线） | V1-1 |
| V1-3 Task DAG | dag kind + task_dependency 查询 + 规模分档 | V1-1 |
| V1-4 架构图 | architecture-graph + cognition_item/baseline 聚合 | V1-1 |
| V1-5 覆盖与影响分析 | impact-tree 反向展开 + 覆盖缺口标注 | V1-2 |
| V1-6 图上反哺上下文 | 图选中节点 → 注入 conversation_context_item | P0 |

## P2 AI Collaboration Visualization

执行时序图（task_run_event 回放）· Multi-Agent 协作图 · ACP 会话恢复路径 · 审批/权限流程图（Artifact 通道）。

## P3 Project Intelligence Visualization

关键路径（DAG+CPM）· 风险传播（定义先过领域评审）· Commit 影响分析（G-4 解除后）· 多 Repo 关系 · **V2 条件项：独立渲染 Worker 支持 GIF/MP4**（确有客户需求 + 本地编码器验证通过后立项，DV-10）。

## 明确不做（本期，防蔓延）

图形编辑画布 / 实时协同编辑 / 独立渲染微服务（V2 前置条件未满足）/ BI 报表 / 移动端专门布局 / Spec 全量 Git 化 / headless Chrome 基础设施。

## 里程碑门槛

| 门槛 | 内容 |
|---|---|
| M1（P0 完） | Foundation+Security 验收全过（含 AC-17~21 新增项）；SeeCode 实验接入可演示、可降级；全量既有测试绿 + viz 可拔测试绿 |
| M2（P1 完） | 追踪图/Task DAG/架构图上线；G6 包体与性能达标（150 节点交互基线）；Q2 正式冻结 |
| M3（P2 完） | 时序回放图可用；上下文反哺打通 |
| M4（P3 完） | 智能分析类图按数据成熟度逐个放行；GIF/MP4 按条件项独立决策 |

## 评审状态与前置条件

**专家评审（2026-11-12）：有条件通过。**
- 可冻结：Q1/Q3/Q4/Q5/Q6/Q8（已回填 D1~D5）。
- 技术验证后冻结：Q2（G6 包体/性能、SeeCode Adapter 实测）、Q7（沙箱+CSP 实测）、Q9（全输出路径注入测试）、Q10（vendor 构建+降级测试）——对应任务 V0-4/V0-6/V0-7/V0-9。
- 新增强制项：Q11 权限过滤、Q12 一致性快照、Q13 解释图区分——已并入 V0-3/V0-5/D1 FR-9。
- 进入开发前：词表 owner 会签（V0-1 内）、迁移号确认（0020 起）、内网导出策略确认（DV-10 已按"V1 接受降级"冻结，V2 再议）。
