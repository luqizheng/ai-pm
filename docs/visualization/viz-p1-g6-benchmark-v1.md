# Viz P1 · G6 5 包体与帧率实测（V-1）

> 对应任务：`ai-pm-viz-26-p1-execution-prompt-v1.md` §一.1 / AC-P1-1
> 冻结条件：ADR-VIZ-002 V-1（`ai-pm-viz-07-roadmap-v1.md` P1 段）
> 实测日期：2026-11-12　实测人：CodeArts Agent（本机 checkpoint 栈）
> 结论：**PASS**（四项阈值全部达标，Q2 可冻结）

## 一、结论速览

| 指标 | 阈值 | 实测 | 结论 |
|---|---|---|---|
| viz lazy chunk（gzip） | ≤ 350 KB | **306.3 KB**（raw 1098 KB） | ✅ |
| 150 节点交互帧率 | ≥ 50 fps | **53.1 / 54.4 fps**（both，两次） | ✅ |
| 主 bundle 污染 | 零 | 主 entry 无 G6 特征串 | ✅ |
| 布局算法按需注册 | 只注册用到的 | 仅 `antv-dagre` | ✅ |

交互分流：hover 54.1 fps、wheel 60.1 fps、hover+wheel 53.1~54.4 fps。

## 二、固定测试条件

| 项 | 值 |
|---|---|
| 浏览器 | Microsoft Edge **157.0.4341.46**（Playwright `channel: msedge`，headless） |
| 设备 | 13th Gen Intel Core i9-13900H（20 线程）/ 32 GB |
| 操作系统 | Windows 11 Enterprise |
| 视口 | 1440 × 900 |
| 前端 | Vite dev server :8848（依赖已 `vite optimize` 预打包） |
| 数据规模 | **150 节点 / 120 边**（5 层 × 30 节点分层 DAG，中英文 label） |
| 布局 | `antv-dagre`，`rankdir=LR`，`nodesep=16`，`ranksep=48` |
| 交互操作集 | hover 扫掠（6 轮 × 横向 40px 步进）+ 滚轮缩放（6 轮 × ±240） |
| 采样 | `requestAnimationFrame` 时间戳，持续 6 s |
| 统计方法 | `avg = frames / duration`；`p50/p95` = 帧间隔分位数 |

## 三、测量脚本与复现

| 用途 | 文件 | 命令 |
|---|---|---|
| 帧率基准 | `ai-frontend/e2e/viz-p1-fps-benchmark.spec.ts` | `npx playwright test e2e/viz-p1-fps-benchmark.spec.ts` |
| 分流实验 | 同上，`VIZ_FPS_MODE=hover\|wheel\|both` | `VIZ_FPS_MODE=hover npx playwright test ...` |
| chunk 审计 | `scripts/viz-p1-chunk-audit.mjs` | `pnpm build && node ../scripts/viz-p1-chunk-audit.mjs` |

帧率脚本通过 `#viz-bench-root` 容器注入真实指针事件（不依赖 `locator.hover()`
——它在 canvas/SVG 命中测试上会死循环重试），并对渲染器实现零耦合。

## 四、优化过程（含被否方案）

| 阶段 | 方案 | both fps | 处置 |
|---|---|---|---|
| 基线 | 内置 `hover-activate`，全量 `setState` | 33.2 | 弃 |
| ① | 增量 hover（邻接表差集）+ 砍边标签/节点 icon 文本 | 43.9 | 保留 |
| ② | 切 SVG renderer（预期绕过 canvas 全量重绘） | 32.5 | **否**（DOM diff 更慢，hover 32.8） |
| ③ | 回退 canvas + hover 单飞合并（latest-wins） | 47.8 | 保留 |
| ④ | 标签 LOD：>80 节点默认不渲染 label，仅 active 显示 | **53.1** | **采用** |

关键事实：

1. `@antv/g6/esm/runtime/canvas.js:273` 对 `CanvasRenderer` 实例硬编码
   `setConfig({ enableDirtyRectangleRendering: false })`，G6 5 下无法开启脏矩形，
   任何自定义 canvas 实例都会被覆盖。故「局部重绘」路线不可用。
2. 重绘大头是 **150 份文本标签的测量与绘制**：去掉 label 后同配置 53.4 fps，
   加回即回落到 47.8 fps。
3. 最终方案保留 hover trace 语义（1 度邻居高亮 + 其余降透明），只是大图默认
   不铺满标签；hover / lens / path 命中节点仍显示标签，可读性不受损。

## 五、包体明细

```
index-*.js            gzip 343.4 KB   （entry，G6 标记 0）
g6Adapter-*.js        gzip 306.3 KB   （唯一 G6 chunk）
_plugin-vue_export…   gzip  49.2 KB
```

- G6 只出现在 `g6Adapter` 惰性 chunk，主 entry 与共享 chunk 零泄漏（79 chunks 全扫）。
- `@antv/g6` 主入口 `preset` 会全量注册全部扩展（实测 405 KB gzip），
  故改从 `@antv/g6/esm/exports` 取符号 + 手动 `register` 最小集：
  `circle` / `line` / `drag-canvas` / `zoom-canvas` / `drag-element` /
  `optimize-viewport-transform` / `antv-dagre` / `get-edge-actual-ends` / `arrange-draw-order`。
- 未使用的 `@antv/g-svg` 已在方案②否决后移除，不留在依赖树。

> 注：`vite build` 控制台报告的 gzip 略高于审计脚本（316.4 vs 306.3 KB），
> 是压缩级别口径差异；两者均低于 350 KB，门禁以脚本结果为准。

## 六、复测记录

| 轮次 | 模式 | frames | avgFps | p50 | p95 |
|---|---|---|---|---|---|
| 1 | both | 318 | 53.1 | 16.7 ms | 17.7 ms |
| 2 | both | 326 | 54.4 | 16.7 ms | 19.9 ms |
| 3 | hover | 325 | 54.1 | 16.7 ms | 17.0 ms |
| 4 | wheel | 360 | 60.1 | 16.7 ms | 17.6 ms |

## 七、风险与后续

- 标签 LOD 阈值 `LABEL_BUDGET = 80` 为经验值；若后续真实图普遍 >80 节点且
  用户反馈"看不到名字"，应改为按缩放级别 LOD（放大后逐级显示），而非放宽阈值。
- 帧率为 dev server + headless Edge 口径；生产构建（minify + 无 HMR）通常更快，
  该口径视为保守下限。
- 若未来节点规模显著超过 150（如 >500），需重新评估：增量渲染 / WebGL renderer /
  服务端预布局，不得直接沿用本结论。