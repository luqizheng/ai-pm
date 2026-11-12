# Visualization Engine 系统架构设计 V1.0

> 状态：草案 · 日期：2026-11-12 · 前置：[D3 Spec 设计](ai-pm-viz-03-visualization-spec-v1.md)、[D2 SeeCode 调研](ai-pm-viz-02-seecode-research-v1.md)

## 1. 总体拓扑

```
┌─ ai-frontend (Vue 3.5 + Pinia + reka-ui + PRISM) ──────────────────────────┐
│  VizViewer（lazy chunk）                                                    │
│   ├─ LiveGraphView    ← Adapter A：AntV G6 5（P1 引入）                     │
│   ├─ ArtifactFrame    ← sandbox iframe（SeeCode HTML，P0）                  │
│   ├─ DegradedTableView（永远可用）                                          │
│   └─ TracePanel / VersionBar / ApprovalCard / StaleBar                      │
│  集成点：AI Chat Message 卡（envelope viz_artifact）· 右栏 Context Tab ·      │
│          各领域视图嵌入口 · ?project_id 钥匙模式                              │
└──────────────┬─────────────────────────────────────────────────────────────┘
               │ /v1/visualizations/*（独立路由分区，JSON；代码内部短名 viz 保留）
┌─ pm-api (axum) ────────────────────────────────────────────────────────────┐
│  viz_handlers（读/预览/审批/导出元数据）＋ require_project_visible 同门      │
│  viz_query（图聚合读端点：只读投影，直接查既有关系表）                        │
│  viz_spec_validate（E/W/I 校验器，Rust 侧单一实现）                          │
└──────────────┬─────────────────────────────────────────────────────────────┘
               │ job 队列（integration.job 既有机制）
┌─ ai-pm-worker ─────────────────────────────────────────────────────────────┐
│  viz_render_job：spec → Adapter B 编译（SeeCode spec）→ node SC render      │
│  → HTML 文本 → 存 artifact；失败回传 {code,at,fix} → 修补循环 ≤3 轮          │
└────────────────────────────────────────────────────────────────────────────┘
存储：PostgreSQL 新表（迁移 0020 起，见 §4）；渲染产物 HTML 存 PG（大对象/文本列），
不经外部对象存储——产物是派生物，可随时由 spec 重渲染重建。
```

## 2. 领域定位决策：基础设施，不是独立业务域

**结论：作为共享基础设施（shared kernel）挂在现有后端，不设独立 Visualization Domain。**

论证：
1. 图形**没有自己的业务事实**——所有节点/边的语义权威在原领域（需求归 delivery、认知归 pc/ai、知识归 kb）。独立领域会制造第二个权威，直接违反 P1 原则。
2. 引擎自有状态只有 artifact 元数据（版本/审批/stale），这是**展示层簿记**，与 PageIndex 同级，不与业务状态机平权。
3. 组织映射（DV-1 修订，采纳评审建议——**独立 crate ≠ 独立 Domain**：可有独立模块边界、数据访问接口、测试与依赖约束，但不拥有自己的业务事实模型）：
```
pm-domain        └── visualization/{spec, artifact, adapter, validation, projection}
pm-application   └── visualization services
pm-api           └── /v1/visualizations/*
```
   前端 `src/components/viz/`。不新增服务、不新增数据库实例。
4. **服务拆分时机（按部署与运行边界决定，不按团队规模/数据量）**：

| 条件 | 是否拆 |
|---|---|
| 仅图形查询与渲染 / Artifact 数量增长 / 前端图形变复杂 | 不拆（V1：同库同服务独立 crate） |
| SeeCode 静态渲染需要独立 Node Runtime | V1.x：可先独立渲染 Worker |
| Headless Chrome 资源压力 / 多产品共享渲染平台 / 独立扩缩容、故障隔离、安全边界成刚需 | V2：再考虑拆服务 |

   不为架构看起来完整而提前引入微服务。

## 3. 核心数据流

### 3.1 AI 生成图形（Artifact 通道，P0）
```
用户在对话中要求图形
→ LLM 起草 VizSpec（对象 ID 只允许来自本会话 Context Item / 工具查询结果）
→ 服务端校验器跑 E/W/I（权限前置过滤）
→ E → {code,at,fix} 回给模型修补（≤3 轮，对齐 SeeCode 纠错回路）
→ 入库 viz_artifact(state=draft)
→ 对话卡片预览（sandbox iframe）→ Accept/Reject（复用 conversation_interaction 语义）
→ Accept → 走槽位审批（若该 slot 配置了审批；否则直接 published）
→ viz_render_job 产出 HTML → published 可见
```

> SeeCode Adapter 为**实验接入，非核心运行依赖**（DV-11）：渲染失败 → 卡片降级为表格视图，生成/审批链路照常运转。

### 3.2 规则拼装图形（Live 通道，P1）
```
前端打开图视图 → GET /v1/viz/projects/{pid}/live/{view_kind}?scope=…
→ viz_query 从既有关系表聚合（task_requirement / design_requirement /
  adr_requirement / adr_design_revision / task_dependency / pc.relationship / kb.memory_ref）
→ 服务端按权限裁剪（查询+投影双层，Q11）→ 组装 Live 响应（{data: VizSpec 核心, meta: 查询语义}，DV-6）
→ 前端 Adapter A 直渲染。不落库、无审批、随数据实时
```

**一致性边界（DV-12，Q12 裁决）**：Live 响应 meta 必须带 `consistency` 标记——可取一致快照时为 `current`；跨对象读取无法保证同一时点时为 `mixed-revision` 并附各对象 revision refs，UI 明示"图基于混合时点数据"。禁止把新旧版本数据无标记地混成"同一时点的完整关系图"。读取隔离级别与无法取得一致快照时的行为在 V0-2 任务中定死。

### 3.3 Stale 检测
```
GET artifact 时携带 X-Viz-Source-Hash 校验请求，或后台周期任务
重算 source_hash → 失配 → 状态置 stale（原版本保留）
→ Viewer 顶部黄条 + "重新生成"入口（回到 3.1 流程）
```

## 4. 数据模型（迁移 0020 取号，登记 docs/ai-pm-migration-ledger.md）

| 表 | 关键列 | 说明 |
|---|---|---|
| `viz.artifact` | id, project_id, slot_key, version_no, state, spec jsonb, spec_hash, source_hash, renderer_used, created_by/… | 版本不可变；`UNIQUE(project_id, slot_key, version_no)`；pending 审批部分唯一索引 |
| `viz.approval` | artifact_id, state, target_hash, requested_by, decided_by, decision_reason | 对齐 `design.design_approval`（0010:259）形态 |
| `viz.render_output` | artifact_id, format(html/svg), content text, content_hash | 仅存成功产物；可重渲染重建 |
| `viz.slot_config` | project_id, slot_key, view_kind, require_approval, auto_stale | 每槽位策略；缺省行给默认策略 |

不做：双写、对既有表的回填改造、兼容视图（AGENTS.md 规则 1：直接建新表）。

## 5. 权限与安全

| 层 | 措施 |
|---|---|
| 读权限 | viz_handlers 全部先 `require_project_visible`；对象级裁剪在 viz_query/校验器内完成；跨项目引用 404 同形；api_key → 403（对齐 pm_requirement_read / pm_interactions 先例） |
| 生成权限 | LLM 起草时可用对象集合 = 会话 Context Item ∪ 权限内工具查询结果；引用越权对象 → E 级 |
| 渲染隔离 | Artifact 经 `sandbox` iframe（`allow-scripts`，**不授** allow-same-origin/top-navigation）+ **独立产物 Origin**（两层隔离并用：独立 Origin 隔离主应用 Cookie/存储，opaque origin 再隔离 iframe 自身）；CSP 一律经 **HTTP Response Header** 下发（meta 标签不可替代，frame-ancestors 仅 header 生效）：`default-src 'none'; connect-src 'none'; form-action 'none'; object-src 'none'; base-uri 'none'; frame-ancestors <app-origin>`，script/style/img 指令按产物实际内联资源收紧；默认禁 Worker/顶层导航/弹窗 |
| 通信 | iframe **不携带任何身份凭证、不直接调用 AI-PM API**；postMessage 白名单协议（`node_selected`/`route_selected` 等有限类型）+ source 校验 + 随机会话标识；节点 ID 回传后由宿主执行权限检查与业务查询 |
| 输入安全 | Spec 白名单 schema；label/字段一律当不可信文本。**转义策略（Q9 裁决）**：① 输入层按纯文本约束（限长、拒控制字符、URL/图片/CSS 属性分别验证）；② 渲染层须实测确认 SeeCode 用安全文本 API（textContent 类）而非 innerHTML 拼接；③ **禁止盲目预转义**（防双重转义 `A & B` → `A &amp;amp; B`）；④ 注入用例覆盖全部输出位置（节点名/边标签/tooltip/focus 面板/导出 SVG）+ Unicode/RTL/零宽字符。**SeeCode 不过安全渲染测试，则不允许接收不可信 AI Spec——安全不外包给 iframe 沙箱** |
| 数据不出域 | `fonts:system` 零外网请求；服务端不装浏览器；页面内导出编码器来自 jsDelivr（用户浏览器行为，PRIVACY.md 已列明；内网部署场景禁用页面内导出并提供服务端 SVG 通道） |
| 发布合规 | 零 mock；renderer 资源不进 dist 主包；release-guard 不需改动（无 fixtures/demo 引入） |

## 6. 降级与业务独立性（P4 原则的工程保证）

1. 前端：`VizViewer` 为独立 lazy chunk + 动态 import；加载/渲染失败 → `DegradedTableView`（nodes/edges 表格 + 溯源列），业务页面不报错。
2. 后端：viz 路由分区独立挂载；关闭分区（feature flag）后运行全量既有测试必须绿——进入 CI 作为常态回归项（"viz 可拔"测试）。
3. worker：viz_render_job 队列独立 job type；积压/停摆只影响产物渲染，不影响对话与执行链路。
4. SeeCode 不可用：Adapter B 返回明确错误码，卡片显示降级表格；不影响 Adapter A 在线图。

## 7. Adapter 接口（防绑定的物理隔离）

```typescript
// 前端（Adapter A / 降级），packages 层接口，业务组件不得 import 具体实现
interface VizRenderer {
  render(spec: VizSpec, container: HTMLElement): VizHandle; // 返回 hover/click/lens 句柄
  capabilities: { maxNodes: number; exportFormats: string[] };
}
```
后端 Adapter B 为独立编译模块（`viz_render_job` 内），输入 VizSpec 输出 SeeCode spec；唯一允许 import SeeCode 字段名的位置，由架构 lint 守卫（见 D3 §7.2）。

## 8. 与 AI 协作工作台的融合（回应问题 5）

| 现有能力 | 融合方式 |
|---|---|
| Response Envelope 白名单（`ai_interaction_envelope.rs` 8 类，fail-closed） | 新增第 9 类 `viz_artifact { artifact_id, slot_key, preview_url, source_hash, nature }`；白名单外仍落纯文本，fail-closed 语义不变；`nature=explanatory` 的解释图**仅存在于对话通道**，不得发布至业务槽位（DV-13） |
| `ai.conversation_interaction`（accept/reject/submit/defer/decision，落库终态） | 图卡 Accept/Reject 复用该表；Reject reason 必填 |
| `ai.conversation_context_item`（多态对象引用 + revision/content_hash） | 生成图形的"可用对象集合"来源；图形选中节点可反向注入为 Context Item（把"图上看到的关系"变成对话上下文） |
| 右栏四 Tab / History Drawer | 新增 "Graph" Tab：槽位列表 + 当前版本 + stale 徽章 |
| 专注模式 / ?project_id | Viewer 全屏态复用专注模式容器；钥匙模式延续 |

## 9. 部署与运维

- worker 镜像加 Node 20 运行时（SeeCode CLI 依赖）；版本 vendor 进仓，构建期固定。
- 新配置项：`VIZ_ENABLED`（分区开关）、`VIZ_RENDER_TIMEOUT`、vendor 版本号上报（health 端点）。
- 可观测：job 时长/成功率、校验 E 率（生成质量信号）、stale 率、降级视图命中率。
