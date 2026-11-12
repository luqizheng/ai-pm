# Visualization Viewer UI/UX 设计方案 V1.0

> 状态：草案 · 日期：2026-11-12 · 视觉基线：V2 深色 + 雾蓝青（s05 锁定），PRISM 令牌，禁止紫色与硬编码色值；i18n 走 `viz.*` 命名空间。

## 1. 宿主与入口（三个一级入口）

| 入口 | 位置 | 形态 |
|---|---|---|
| 对话卡 | AI Chat 消息流（envelope `viz_artifact`） | 卡片内嵌 ArtifactFrame + Accept/Reject/Feedback 操作条 |
| 右栏 Tab | 协作工作台右栏（现四 Tab）新增 **Graph** Tab | 槽位列表（含 stale 徽章）→ 点开进入 Viewer |
| 领域视图嵌块 | 各领域页面（追踪图入口在需求/设计/任务视图） | 内嵌 LiveGraphView（懒加载），失败降级表格 |

`?project_id=` 钥匙模式延续；专注模式下 Viewer 支持全屏（Esc 退出，对齐专注模式交互惯例）。

## 2. Viewer 布局（桌面优先）

```
┌────────────────────────────────────────────────────────────┐
│ 顶栏：标题 · slot · 版本 v3 (stale) · [版本切换 ▾] [对比] [导出 ▾] │
│ ⚠ 黄条（仅 stale 时）：源数据已变更 · [重新生成]              │
├───────────────────────────────────────────────┬────────────┤
│                                               │ 溯源侧栏     │
│              画布（图主体）                     │ · 节点详情   │
│   hover trace / click focus / lens / route    │ · source_ref│
│                                               │ · [打开原文档]│
│                                               │ · 关联审批    │
├───────────────────────────────────────────────┴────────────┤
│ 底栏：图例（受控词表渲染）· 缩放 · 节点/边计数 · 规模分档徽标  │
└────────────────────────────────────────────────────────────┘
```

- 画布与侧栏宽度沿用协作工作台既有栅格节奏（188/518/446/288 体系的就近比例），不新造布局语言。
- ArtifactFrame（对话卡内）：固定高度卡内预览 + "展开全屏"按钮；iframe sandbox 无 same-origin，卡内不提供操作按钮（操作在宿主卡面上）。

## 3. 关键交互规范

| 交互 | 行为 | 权限/安全语义 |
|---|---|---|
| Hover | 高亮邻接（出边 accent / 入边 secondary，借鉴 SeeCode trace） | 纯视觉 |
| Click 节点/边 | 打开溯源侧栏：label、kind、status、source_ref、`unlinked` 灰标 | 只显示调用者有权对象（服务端已裁剪） |
| 打开原文档 | 跳转 source_ref.locator（文档锚点/领域详情页） | 常规路由权限 |
| Lens | 按 edge_type/kind 过滤高亮（词表驱动） | 纯视觉 |
| Route | 起点终点间路径动画（dag/trace 专用） | 纯视觉 |
| 版本切换/对比 | 版本下拉 + 两版本 spec diff 面板（节点/边增删改三色） | 读权限 |
| Accept/Reject | 仅对话卡与审批面板出现；Reject 必填 reason | 复用 interaction 审批语义，落库终态 |
| Stale 提示 | 黄条 + 版本徽标变色；"重新生成"开启新 draft（不覆盖旧版本） | 无静默覆盖 |
| 导出 | SVG/PNG/GIF/MP4/HTML 菜单；文件名含 slot+版本+时间；内嵌来源水印 | 内网部署禁页面内位图导出（D2 §6.3） |
| 空态 | 无数据：显示"暂无关系数据"+ 去建立链接的引导链接（**不是演示图**） | AGENTS.md 规则 4 |
| 降级态 | 渲染失败：表格视图 + "图形渲染不可用"说明 | 业务零阻断 |

## 4. 视觉与令牌

| 项 | 规范 |
|---|---|
| 色彩 | 全部 PRISM 令牌引用；节点 kind 色、边 rel 色、状态色在 `viz` 令牌子集内扩展（走 PRISM 扩展流程，不私加 hex） |
| 深浅色 | 跟随应用主题；Artifact HTML 固定深色皮肤（V2）+ 系统 fonts |
| 字体 | 中文标签优先级最高：CJK 换行验证、最小 12px 可读、导出内嵌字体（SVG embed fonts） |
| 密度 | 复杂度预算可视化：接近 budget 上限时画布角落出现密度徽标（对齐 SeeCode "pushes back on clutter" 理念） |
| 动效 | 跟随 `prefers-reduced-motion`（SeeCode 产物已尊重，宿主动效同标准） |

## 5. 组件与代码组织（前端）

```
src/components/viz/
  VizViewer.vue            // 编排：加载 spec/artifact → 选 renderer → 挂载
  ArtifactFrame.vue        // sandbox iframe 封装（唯一允许 iframe 的组件）
  LiveGraphView.vue        // Adapter A（G6，P1）
  DegradedTableView.vue    // 降级表格（永远可用）
  TracePanel.vue / VersionBar.vue / ApprovalCard.vue / StaleBar.vue / LegendBar.vue
src/lib/viz/
  types.ts                 // VizSpec TS 类型（与后端契约同源）
  useVizArtifact.ts / useLiveGraph.ts   // pinia/axios 数据流
  renderers/               // adapter 注册表（接口见 D4 §7）
```
- `ArtifactFrame` 之外禁止出现指向 artifact HTML 的 iframe（lint 规则）。
- 所有文案走 `viz.*`，过 `check:i18n`；测试禁 findComponent 内部断言按既有坑位惯例（Teleport/emit 先例）。

## 6. 可用性与无障碍

- 键盘：Tab 遍历节点（Live）；方向键步进（Artifact 步进模式天然支持）；Esc 清 focus。
- 屏幕阅读器：spec `meta.lang` 传入产物页 lang 属性；侧栏文本为读屏主通道。
- 性能感知：>150 节点默认折叠为聚合并提示"点击展开"；加载骨架屏用 PRISM 骨架令牌。

## 7. 状态一览（数据驱动，不写死）

| 状态 | 表现 |
|---|---|
| draft（作者预览） | 顶栏灰徽 "草稿 · 未生效"，禁导出 |
| pending 审批 | 黄徽 "待审批"，Approve/Reject 仅审批角色可见 |
| published | 正常 |
| stale | 黄条 + 徽标（§3） |
| rejected | 仅历史可见，灰显 |
| 渲染失败/引擎关闭 | 降级表格 |
