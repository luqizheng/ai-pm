# SeeCode 技术调研与集成可行性报告 V1.0

> 状态：V1.1 已修订（并入 2026-11-12 专家评审；决策基线见 ADR V1.0） · 日期：2026-11-12 · 调研对象：https://github.com/Aryanutkarsh/SeeCode（main 分支，2026-11 实测读取 README / SKILL.md / spec.md / package.json）
> 结论先行：**SeeCode 适合作为「AI 生成图形产物」的渲染 Adapter（推荐采用），不适合作为在线交互图的渲染引擎（不采用）**。详见 §5。

## 1. 项目事实档案（均经原始文件核实）

| 项 | 事实 | 证据 |
|---|---|---|
| 定位 | Agent Skill：把 30~150 行 JSON spec 渲染为"editorial 级"可交互动画图 | README；SKILL.md 开头 |
| 版本 | 0.6.3（package.json）；SKILL.md metadata 0.6.2 | package.json:3 |
| 发布形态 | **非 npm 库**（`"private": true`），经 Claude Code plugin marketplace / `npx skills add` / git clone 安装 | package.json:4；README Install 节 |
| 语言/运行时 | Node >= 20，ESM；**npm 零依赖** | package.json（无 dependencies 字段）；README 徽章 |
| 图类型 | 45 种（architecture/dependency/ER/db-schema/UML/flowchart/sequence/state/swimlane/gantt/sankey/treemap/isometric 等）+ 15 变体 | README Diagram types 表 |
| 输出 | 单文件自包含交互 HTML；SVG（仅需 Node）；PNG/JPEG/GIF/MP4（用本机 Chrome/Edge headless，或页面内 Export 菜单从 jsDelivr 加载编码器） | README Export 节 |
| 中文 | 支持 `lang:"zh-CN"`，中日韩标签按字符换行；RTL 支持 | spec.md lang 字段；README "Not just code" |
| 规模预算 | 默认 ≤9 节点 / ≤12 边，超出要求拆分；`budget:"faithful"` 上限 24 节点 | SKILL.md §2 Budget；spec.md budget 字段 |
| 溯源 | spec 级 `evidence:[{id,file,line?,note?}]`，repo 扫描可产出 file:line 溯源 | spec.md evidence；SKILL.md §5 |
| 校验回路 | render 返回 `{code,at,msg,fix}`（E 阻塞 / W 警告），`--patch` 小步修补，3 轮上限 | SKILL.md §3；spec.md |
| 导入 | Mermaid/Graphviz/PlantUML/D2/drawio/Excalidraw/Structurizr/BPMN/SQL/Prisma/DBML/OpenAPI/CSV 重绘 | README Bring 节 |
| 许可 | MIT；捆绑编码器与字体另列 THIRD_PARTY_LICENSES.md；有 SECURITY.md / CONTRIBUTING / CI | package.json:4；README License/Privacy |
| 隐私 | 无服务端/账号/遥测；但默认产物加载 Google Fonts、页面内导出加载 jsDelivr 编码器，均可关闭（`fonts:system`；CLI 导出用本机浏览器） | README Privacy；spec.md Fonts |
| 社区 | 约 900+ star（第三方 skillsllm 统计，非官方页面核实）；近期 GitHub trending；维护者信息未在页面核实（按个人项目风险处理） | WebSearch 结果 |

## 2. 架构与四层关系（回答调研提纲 1/2）

```
SKILL.md（协议层：教 LLM 何时/如何写 spec）
   ↓  LLM 产出 30~150 行紧凑 JSON spec（type/nodes/edges/row/col/motion/budget/lang/evidence）
SC = node scripts/seecode.mjs（CLI 渲染器，纯 Node，零依赖）
   ↓  SC render  →  ok / {code,at,msg,fix} 问题清单 → LLM --patch 小步修正（≤3 轮）
   ↓  SC export  →  SVG（纯 Node）/ PNG/GIF/MP4（调本机 Chrome/Edge）
产物：单文件自包含交互 HTML（hover trace / click focus / route walk / lens / 步进 / pan-zoom / 搜索 / #focus 深链）
```

关键设计判断（对 AI-PM 有直接借鉴价值）：
1. **模型从不画 SVG**——LLM 只声明"有什么、怎么连、大概在哪（row/col）"，几何/排版/动效全部由确定性渲染器负责。token 成本约 1.4k/图（对比模型直写 SVG 约 15k）。
2. **复杂度预算内建**：≤9/12 的硬预算从协议层阻止 LLM 产出混乱大图。
3. **evidence 字段**把"图上的节点"和"仓库里的证据"显式连接——与 AI-PM 溯源原则同构。

## 3. 逐条回答集成可行性问题（提纲 3~10）

| # | 问题 | 结论 |
|---|---|---|
| 3 | 能否作为 AI-PM 图形渲染基础？ | **部分可以**。作为 AI 生成图的"产物渲染器"适合；作为六领域在线交互图的引擎不适合（见 §4） |
| 4 | API/Adapter 集成？ | 无 HTTP API。集成方式 = 后端 worker 以子进程调 `node scripts/seecode.mjs render`（Node 20 可用即可）；或 vendor 其渲染脚本。**集成点天然薄**：输入 JSON、输出 HTML 文件 |
| 5 | 适合嵌入前端？ | 产物 HTML 适合 sandbox iframe 嵌入（自包含、无外部请求可用 fonts:system 关闭）；但其**渲染器本身不适合进前端 bundle**（设计为 Node CLI） |
| 6 | 中文/复杂关系/大图？ | 中文标签：✅（zh-CN + CJK 换行，需验收 UI 控件文案仍为英文的问题）。复杂关系：⚠️ 双向边/多语义边有表达力但受类型约束。**大图：❌ 硬预算 ≤9/12（faithful ≤24 节点）**，这是产品理念（editorial）而非缺陷 |
| 7 | HTML/SVG/PNG/GIF/MP4？ | ✅ 全支持。HTML 交互完整；SVG 零浏览器依赖；位图/视频走本机 Chrome/Edge 或页面内导出菜单 |
| 8 | 许可 | MIT ✅（代码自有）；捆绑字体/编码器需按 THIRD_PARTY_LICENSES.md 审查；vendor 进本仓合规（保留版权声明）。**规避项：改用 system fonts 后不引入 Google Fonts 分发问题** |
| 9 | 安全 | 产物含可执行 JS（交互所需）→ 必须 sandbox iframe（`allow-scripts` 且**不授 allow-same-origin**）+ 独立 CSP + spec 白名单校验 + 标签转义验证（其转义行为未在文档中承诺，列为 P0 验收测试项）。外网请求可归零（fonts:system + 服务端只产 HTML、位图导出交给用户浏览器页面内菜单） |
| 10 | 维护成本 | **高风险项**：0.x 版本、个人项目、非 npm 库（升级=重新 vendor）、无企业背书。缓解：Adapter 接口隔离 + 锁版本 vendor + spec 是 AI-PM 自有格式（SeeCode 只是编译目标，可替换可降级） |

## 4. 为什么不能作为在线交互图引擎

| AI-PM 在线图需求 | SeeCode 现实 |
|---|---|
| 数据实时来自 DB 图查询、随业务即时更新 | 产物是静态快照文件，更新=重新渲染文件 |
| 数百节点 Task DAG / 依赖图 | 硬预算 ≤9/12（faithful ≤24） |
| 服务端按调用者权限裁剪图数据 | 与 SeeCode 无关，但它无 API 渲染模式，产物固定 |
| 图内点击→AI-PM 路由/Context Panel 联动 | 产物深链机制（#focus）是文件内闭环，无法与宿主应用通信（sandbox 下也不应允许） |
| 版本 diff / stale 徽章 / 审批按钮与宿主 UI 一致 | 产物是独立 HTML 页面，宿主 UI 无法注入 |

**定性**：SeeCode 的产物模型是"**可携带的报告**"（one self-contained file you can send to anyone），AI-PM 在线图的需求是"**应用的活视图**"。两者互补不互替。

## 5. 与替代方案对比

| 方案 | 类型 | 在线交互 | 大图 | 中文 | AI 友好（spec 驱动） | 输出 | 许可 | 维护 |
|---|---|---|---|---|---|---|---|---|
| **SeeCode** | Agent Skill + Node CLI | 产物内（沙箱文件） | ❌ ≤24 | ✅ | ✅✅（为此而生） | HTML/SVG/PNG/GIF/MP4 | MIT | ⚠️ 个人 0.x |
| **AntV G6 5** | 前端图可视化库 | ✅✅ | ✅（千级） | ✅ | ✅（数据驱动 JSON） | Canvas/SVG 在线 | MIT | ✅ 蚂蚁持续维护 |
| **Vue Flow** | 前端节点图组件（Vue3） | ✅✅（偏编辑器交互） | ✅ | ✅ | ✅ | SVG 在线 | MIT | ✅ 活跃 |
| **Mermaid** | 文本图 DSL | ⚠️ 弱交互 | ❌（几十节点劣化） | ✅ | ✅（文本即图） | SVG 在线；文档内嵌最自然 | MIT | ✅ 成熟 |
| **Cytoscape.js** | 前端图库 | ✅ | ✅✅ | ✅ | ✅ | Canvas | MIT | ✅ 老牌、API 陈旧 |
| **ECharts graph** | 图表库 | ⚠️ 泛用 | ✅ | ✅✅ | ✅ | Canvas | Apache-2.0 | ✅ |

**选型结论**（与 D3/D4 呼应）：
- **Adapter A（在线交互）**：P1 起引入 **AntV G6 5**（读向探索图、布局算法含 DAG/flow、中文生态、MIT、维护强）；Vue Flow 作为备选（若交互形态更偏"节点画布"再切换，接口隔离后成本可控）。
- **Adapter B（AI 生成产物）**：**SeeCode 实验接入，不作为 P0 核心运行依赖**（DV-11）——P0 完成 vendor 评估 + Adapter Contract Tests 后决定转正；V1 产出 HTML/SVG/PNG。另设 **Fallback Adapter**（表格/Mermaid）为常备降级目标。
- **Mermaid**：不进引擎，仅在 Markdown 文档内继续按现状渲染（若有），不作为 Adapter 承诺对象。
- 统一约束：三者都不允许被业务领域直接依赖；领域只产 AI-PM Visualization Spec（D3）。

## 6. 采用 SeeCode 的集成边界（约束清单）

1. **vendor 锁版本**：将 `skills/seecode/`（SKILL.md + scripts）以固定版本 vendor 进本仓 `tools/seecode/`（或独立 build 依赖仓库），保留 LICENSE 与 THIRD_PARTY_LICENSES.md；不跟随上游自动升级。
2. **只用 `render`（HTML）与 `export --for svg`**（纯 Node 路径）；V1 不在服务端生成 GIF/MP4（不部署 headless Chrome，DV-10）。两个**待实测项**：① CLI 能否用本地 Chrome/Edge 完成 GIF/MP4 而不经 jsDelivr 页面内编码器（V2 决策输入）；② SVG/PNG 导出链路是否真正零外网（实测断言，AC-6，不以格式名称推断）。
3. **全局配置固化**：`fonts: system`（零外网请求）、watermark 保留（溯源水印含槽位+版本信息）、skin 跟随 V2 深色体系。
4. **spec 是编译目标**：SeeCode spec 字段（row/col/budget/…）只存在于 Adapter B 编译器内部；AI-PM Spec 不暴露这些字段（防绑定，见 D3 §7）。
5. **Node 可用性**：worker 镜像需含 Node 20（`apps/ai-pm-worker` 已是容器栈，加一层基础镜像能力即可，属部署约束非代码改动）。
6. **上游弃维预案 = 降级能力矩阵（DV-11，替代"2 周内可切换"的笼统承诺）**：降级目标是信息仍可读、业务仍可用，而非保持全部视觉效果。

| 图形类型 | 降级方式 |
|---|---|
| Flowchart / Sequence | Mermaid（Fallback Adapter） |
| ER / Architecture | Mermaid 或结构化表格 |
| Traceability / Task DAG | 在线视图（G6）或表格 |
| 动画演示 / GIF / MP4 / 复杂 Isometric | 不保证 |

## 7. 借鉴清单（不集成代码也要吸收的设计）

| 理念 | AI-PM 落点 |
|---|---|
| 模型只声明语义，渲染器负责几何 | Spec 分层：view hints（意图）与渲染解耦（D3 §3） |
| 复杂度预算 + 分级（strict/balanced/faithful） | NFR 规模分档 S/M/L（D1 §6） |
| render→{fix}→patch 的纠错回路 | AI 生成图的自动修复循环（D4 §3.3） |
| evidence file:line | source_ref 溯源强制（D3 §4） |
| 一个文件可携带 | Artifact 导出作为一等能力（FR-6） |
| "什么时候不该画图"（README When not to use it） | 生成侧守则写进 prompt 规范（D7 任务 V0-3） |
