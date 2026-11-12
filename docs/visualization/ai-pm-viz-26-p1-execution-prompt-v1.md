# Visualization P1 执行提示词（供新 session 使用）

> 把本文件全文作为新 session 的首个提示词粘贴即可。本文件自包含，不依赖任何历史对话。

---

## 任务

在 `D:\kudi-project\templates\ai-pm`（master 分支，起点提交 `86262eb2` 或更新）完成 **AI-PM Visualization Engine 的 P1：Live Graph 在线交互图**。

P0 已完成并合入 master：数据链路、权限投影、审批、安全闸、SeeCode 静态产物通道、九条关系全部 active。**P1 的唯一缺口是「前端交互画布」**——Live 数据端点已就绪（返回 `{data, meta}`），但前端还没有 G6 画布，只能显示静态表格。

## 一、P1 交付范围（三项，按序）

### 1. G6 5 引入与 V-1 验证（前置，必须先做）

引入 AntV G6 5 到 `ai-frontend`，并**实测**以下阈值（ADR-VIZ-002 冻结条件 V-1）：

| 指标 | 阈值 |
|---|---|
| viz lazy chunk（gzip） | ≤ 350 KB |
| 150 节点交互帧率 | ≥ 50 fps |
| 主 bundle 污染 | **零**（G6 不得进主 bundle） |
| 布局算法 | 按需注册（只注册用到的） |

**若任一项不达标**：不要硬上，停下来写实测报告并给出替代方案（Vue Flow / Cytoscape / 手写 SVG 分层布局），由产品负责人裁决。

### 2. 交互通道实现（Adapter A）

新增 `ai-frontend/src/components/viz/LiveGraphView.vue` + `src/lib/viz/renderers/g6Adapter.ts`，实现四项交互：

| 交互 | 行为 | 数据依赖 |
|---|---|---|
| hover trace | 悬停节点高亮入边/出边，其余降透明 | Spec edges |
| focus panel | 点击节点 → 打开已有 `VizTracePanel.vue` | Spec + source_ref |
| route journey | 选起点终点 → 服务端算路径 → 高亮 + 步骤列表 | 需后端支持（见 §5） |
| lens | 按 `edge_type`/`kind` 过滤高亮 | `view.lenses` |

**硬约束**：
- **禁用 G6 的 HTML shape**（`html` 类型允许自定义 HTML → 引入 XSS 面）。只用 SVG shape + 文本节点。
- 必须 `lazy` 加载（`defineAsyncComponent` 或路由级 dynamic import），**不得**从主入口静态依赖。
- 渲染失败 → 降级到已有 `VizSpecTable.vue`（表格），**不阻断页面**。

### 3. 三张 Live 图接线

| 图 | view_kind | 现有端点 |
|---|---|---|
| 追踪图 | `trace-graph` | ✅ 已实现 |
| Task DAG | `task-dag` | ✅ 已实现 |
| 架构图 | `architecture-graph` | ❌ **需新增**（见 §5） |

接入位置：`src/views/project/visualizations/VisualizationsView.vue`（已有，当前用 `VizSpecTable`），改为「交互图 + 表格切换」。

## 二、已就绪资产（不要重做）

### 后端（Rust，master 已有）

| 能力 | 位置 |
|---|---|
| VizSpec 类型 + 校验器（24 问题码） | `crates/pm-domain/src/visualization/` |
| Relation Registry（9 条 active，编译期嵌入） | `policies/viz-relation-registry.yaml` |
| 权限投影（双层过滤 + 计数重算 + 一致性标记） | `pm-application/src/visualization/projection.rs` |
| Live 图组装（trace / task-dag） | `pm-application/src/visualization/live_graph.rs` |
| Live 读侧 port + Pg 实现（单条 SQL） | `pm-application/src/repo/viz_read.rs`、`pm-infrastructure/src/repo_viz_read.rs` |
| Live 端点 | `apps/pm-api/src/viz_handlers.rs`（`GET .../live/{view_kind}`） |
| 产物通道（SeeCode vendor + 编译器 + Adapter） | `tools/seecode/`、`seecode_compiler.rs`、`viz_seecode_adapter.rs` |
| 渲染沙箱（M-2 §3 执行隔离） | `pm-infrastructure/src/viz_render_sandbox.rs` |

### 前端（Vue 3.5，master 已有）

```
src/lib/viz/types.ts          VizSpec 类型（与后端契约同源）
src/lib/viz/specAdapter.ts    后端 DTO → 前端模型（唯一转换点，不做静默默认）
src/lib/viz/export.ts         产物导出
src/api/visualizations.ts     getLiveGraph / getLiveSpec（后者已返回 VizSpec）
src/components/viz/           ArtifactFrame / SpecTable / TracePanel / VersionBar /
                              StaleBar / EmptyState / AiExplanation / FeedbackArea / GraphTab
```

**可参考的既有范式**：`src/views/project/tasks/planning/DependencyGraphView.vue`（手写内联 SVG 的 DAG，含箭头 marker 与边路径计算）。若 G6 不达标，这里是自研降级方案的起点。

## 三、验收标准（可测）

| # | 标准 | 方式 |
|---|---|---|
| AC-P1-1 | G6 三项阈值达标（chunk/帧率/零污染） | 构建产物体积 + 性能测量脚本，**写进仓库** |
| AC-P1-2 | 三张图在真实数据下渲染 | Playwright e2e（真后端栈） |
| AC-P1-3 | hover / focus / lens 交互可用 | vitest 组件测（断言行为，非类名） |
| AC-P1-4 | route journey 路径正确 | 单测：给定图与两端点，断言路径节点序列 |
| AC-P1-5 | 渲染失败降级到表格，页面不报错 | vitest：mock 渲染器抛错 |
| AC-P1-6 | 未 active 的关系不出边 | 已有单测 `draft_relations_are_not_projected` 覆盖后端；前端断言响应边数 |
| AC-P1-7 | 现有门禁全绿 | 见 §4 |

## 四、硬约束（违反即返工）

### 4.1 架构约束

1. **零业务写路径**：viz 模块不得 import 业务写 service，不得写 `delivery/design/pc/kb/ai/plan/core/governance` schema（`scripts/viz-no-write-path.sh` 强制）。viz 自己的四表可写。
2. **pm-api 零 SQL**：handler 不得直写 SQL，一律下沉 pm-application（`scripts/guard-pm-api-sql.sh` 强制）。
3. **不进主 bundle**：G6 必须 lazy chunk。
4. **不得改既有业务表**：图形层需要新数据时走聚合查询，**不加业务表**。
5. **Spec 零渲染器字段**：VizSpec 不得出现 `row/col/skin/motion`（`scripts/viz-spec-lint.js` 强制）。

### 4.2 门禁（提交前必跑）

```bash
bash scripts/check-viz.sh          # registry + spec 纯净性 + 零业务写路径
bash scripts/guard-pm-api-sql.sh   # pm-api 零 SQL
cd ai-frontend && pnpm check:prism-tokens   # 全仓扫，存量违规 121 条属基线，新增必须零
cd ai-frontend && pnpm check:i18n           # 45 命名空间 / key 集合双语对齐
cargo test -p pm-domain -p pm-application -p pm-infrastructure -p pm-api
cd ai-frontend && npx vue-tsc --noEmit && npx vitest run && npx eslint <改动路径> --max-warnings 0
```

### 4.3 前端约定

- 页面根必须 `PageFrame`；项目上下文走 `?project_id=` 钥匙模式（**哨兵必须用 `SCOPE_ALL` 常量比对**，值是 `"__all__"`，写 `"all"` 是 bug）。
- 颜色/尺寸/时长必须用 `tokens.css` 变量；新增令牌**深浅两主题都要写**（`tokens.test.ts` 校验）。
- 文案走 `locales/{zh-CN,en}/viz.yaml`，两语言 key 集合必须完全一致。
- 测试：首行 `// @vitest-environment happy-dom`；用 `src/lib/test-utils.ts` 的 `mountShell/settle`；**Sheet/Teleport 组件需 `attachTo: document.body` 并从 `document.body` 取值**（`wrapper.text()` 是空的）。
- `.vue` 文件**用 python 脚本改**，不要用 Edit 工具（会破坏模板）。

## 五、需要新增的后端能力（仅一处）

`architecture-graph` 的 Live 查询：

- port：`pm-application/src/repo/viz_read.rs` 加 `cognition_edges(...)`
- 实现：`pm-infrastructure/src/repo_viz_read.rs`（**单条 SQL**，保证 `consistency=current`）
- 数据源：`ai.cognition_item`（domain ∈ identity/repository/domain/architecture/convention/evolution）+ `ai.project_baseline` / `baseline_item`
- 组装：`live_graph.rs` 加 `build_architecture_spec(...)`，边关系用 `pc.related` 或 `derived_trace`（**看实际数据决定，并在 Registry 中确认该关系已 active**）
- 端点：`viz_handlers.rs` 的 `live_graph` 加一个 `match` 分支

**route journey 的路径计算**：建议服务端算（复用图遍历），或前端在 spec 上做 BFS（≤500 节点可接受）。若服务端算，需新增端点；若前端算，需保证只走已投影的边。

## 六、已知坑（避雷，均实测踩过）

### 6.1 Rust / 数据库

| 坑 | 说明 |
|---|---|
| `migrations/` 现状 | 0001_baseline + 0002~0020（0020 = viz 四表）。**旧链号段（0021~0065）不参与取号**，见台账 §三 |
| 测试库 | `DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm'`（**必须 127.0.0.1，禁 localhost**）。`aipm` 库与 `template1` 都是空的，测试走**每用例重放全量迁移**的慢路径 |
| 新增迁移后 | 必须 `bash scripts/seed-test-template.sh` 重播种（否则混跑报错） |
| 容器内 psql | `docker exec aipm-pg-gate psql -U aipm **-h localhost** -d aipm -c "..."`（不加 `-h` 报角色不存在） |
| 真实环境库 | 容器 `superpowers-full-postgres-1`，库名 `postgres`（不是 `aipm`） |
| core 表结构（写夹具必踩） | `delivery.requirement` 用 `lifecycle_status`（**无 status**）；`requirement_revision`/`design_revision` 内容列是 `content jsonb`，版本列 `design_revision` 是 `version`（**非 version_no**）；`design.design` 的 `code` 非空；`delivery.task` **无 created_by**；`core.principal` **无 tenant_id**（在 principal_identity）；`core.customer` 有 tenant_id NOT NULL |
| 审计 FK | `governance.audit_log.actor_id` 对 `core.principal` 有 FK → 测试 actor 的 principal 必须真实存在 |
| `delivery.task_dependency` | 有复合外键 `(project_id, depends_on_task_id) → task(project_id, id)` → 跨项目依赖**插不进去**（结构性保证） |
| sqlx | 无 `.sqlx/`、禁 `query!` 宏，一律运行时 `query*` + `.bind()` |
| 字符串陷阱 | Python 写 Rust 会破坏 `'\''` 字面量与 `r#"..."#`（含 `"#` 提前终止）→ 用 `r##"..."##`、优先用 Edit 工具 |
| clippy | `-D warnings`；参数 >7 个用 input struct 打包 |

### 6.2 前端

| 坑 | 说明 |
|---|---|
| `pnpm check:prism-tokens` | 全仓扫；存量 121 条违规属基线，只看**新增文件**是否零违规 |
| i18n YAML | 同层不能有同名键（重复键会让 vite i18n 插件在**构建期**报错，而 `check-i18n-namespace.js` 不报） |
| 并发测试 | vitest 并行时偶发超时（`entry-bootstrap` 单跑 9s、并发被 20s 杀）→ 先单跑确认 |
| 临时目录类断言 | 不要断言"全局目录数量"，并行测试必然竞态 → 直接测 Drop 语义 |
| 产物引用 | 一律 `<img>`（静态 SVG），**禁 `<iframe>`/`<object>`**（`<object>` 可执行 SVG 内脚本） |

## 七、执行顺序

```
1. 拉基线：git log -1（确认 ≥ 86262eb2），跑一遍 §4.2 门禁建立绿基
2. G6 引入 + V-1 测量（含测量脚本入库）
   └─ 不达标 → 停，写报告 + 替代方案
3. g6Adapter + LiveGraphView（四项交互，先 trace-graph 一条打通）
4. 接线 trace-graph 与 task-dag（VisualizationsView 加「图/表」切换）
5. 后端加 architecture-graph（port + SQL + 组装 + 端点 + 集成测）
6. 前端接 architecture-graph
7. 全量门禁 + e2e（三张图 + 降级路径）
8. 回填 docs/visualization/（D7 路线图 P1 勾选、新增实测报告）
```

## 八、交付物

| # | 内容 |
|---|---|
| 1 | `src/lib/viz/renderers/` 下的 G6 adapter（含 capabilities 声明） |
| 2 | `src/components/viz/LiveGraphView.vue`（四项交互 + 降级） |
| 3 | 三张图可用的 `VisualizationsView`（图/表切换） |
| 4 | 后端 `architecture-graph` 全链路（port/SQL/组装/端点/测试） |
| 5 | **V-1 实测报告**：`docs/visualization/viz-p1-g6-benchmark-v1.md`（含固定测试条件：浏览器版本、设备、节点边数、交互操作集、p50/p95 方法） |
| 6 | 测试：G6 adapter 单测、LiveGraphView 组件测、三图 e2e、降级路径测 |
| 7 | 文档回填：D7 P1 勾选、00 索引新增条目 |

## 九、边界（不要做）

- ❌ 不改既有业务表、不加业务写端点
- ❌ 不做图编辑器（拖拽改图并回写业务）
- ❌ 不引入第二个图形库（只 G6；备选方案仅在 V-1 不达标时作为替换，不是并存）
- ❌ 不动 SeeCode 产物通道（P0 已完成，`<img>` 静态呈现不变）
- ❌ 不为了「图好看」放宽 Registry 校验或权限裁剪
- ❌ 不在 P1 做实时协同编辑

## 十、关键设计提醒

**权限与一致性不能因交互而退化**：

- 交互图的数据来自 Live 端点，**服务端已裁剪**（权限双层过滤 + 双端隔离）。前端**不得**再向端点请求"全量数据"后自行隐藏。
- `meta.consistency = mixed-revision` 时 UI 必须提示（不要静默展示成同一时点）。
- 交互（展开、路径）只能改变**视图**，不得产生任何写操作。
- 图上选中节点要能反哺对话上下文（可选，若做：注入 `conversation_context_item`，走既有端点，不新增表）。

**为什么 G6 而不是 HTML 画布**：见 ADR-VIZ-002 / 15 号安全审查——产物的 HTML 内嵌脚本一律不执行；交互由图库用**自己的数据**渲染，这是我们自己的代码 + 自己的数据，安全边界清晰。
