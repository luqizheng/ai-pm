# AI-PM Visualization V0-6 安全方向审查（去 iframe 方案）V1.0

> 状态：**审查结论已定 —— 建议采纳"放弃 iframe"，改为服务端静态渲染 + 零脚本前端呈现**；待 Architecture/Security Owner 签收
> 日期：2026-11-12 · 审查范围：ADR-VIZ-008（五层隔离）、ADR-VIZ-010（转义）、V-2 / V-3 验证项、AC-6/7/8/9/18
> 变更性质：**安全姿态变更**（去掉浏览器端执行产物代码的整条路径），需修订 ADR-008 并重写 4 条验收项
> 证据基础：2026-11-12 对 SeeCode v0.6.3 的**本地实测**（Node v24.18.0，源码 clone 后实跑，非文档推断）

## 1. 审查结论（先行）

**建议采纳放弃 iframe。** 它不是安全妥协，而是把威胁面从"浏览器内执行不可信代码"整体移除——攻击者要利用产物，必须先攻破"产物根本不执行"这一前提。

但**不能只删 iframe 而不改渲染链路**：SeeCode 的交互能力（hover trace / focus panel / route journey）**全部由它的 viewer JS 提供**。去 iframe 后必须回答"交互能力去哪"，否则是用安全换掉了产品核心。

**推荐方案：双通道**

| 通道 | 产物形态 | 渲染位置 | 交互 | 安全边界 |
|---|---|---|---|---|
| **产物通道**（Artifact） | 服务端导出 **SVG**（静态） | 前端 `<img>` / `<object>` 引用 | 无（静态图） | **零脚本**：SVG 内无 `<script>`、无 `on*` 事件属性、无 `foreignObject`、无外部引用 → 浏览器不执行任何代码 |
| **交互通道**（Live） | VizSpec 数据 | 前端 G6 Adapter 自绘 | 完整 | 自有代码渲染自有数据，**无第三方产物代码参与** |

即：**交互由我们自己的前端承担（Viewer Adapter A），"可分享的编辑级图形"由 SeeCode 服务端出静态 SVG。** SeeCode 从"浏览器内运行"降为"服务端出图"，角色从"渲染引擎"变为"出图引擎"。

## 2. 实测证据（本次审查的硬依据）

对 SeeCode v0.6.3 本地实测（`git clone` 后实跑，非文档推断）：

### 2.1 SVG 导出：纯 Node、无浏览器、零外链

```
$ node skills/seecode/scripts/seecode.mjs export attack.html --formats svg
{"ok":true,"files":[{"format":"svg","bytes":298417}]}     # 无 Chrome/Edge 参与
```

| 检查项 | 结果 |
|---|---|
| `<script>` 标签 | **0** |
| `foreignObject` | **0** |
| 真实外链（排除 xmlns 与 data:） | **0** |
| 事件属性（可执行上下文） | **0**（`onerror` 仅出现在**已转义的文本节点内容**与 aria-label 属性值中：`&lt;img src=x onerror=alert(1)&gt;`） |
| 注入 payload | **已转义**（`<script>` → `&lt;script&gt;`） |
| 字体 | **base64 内嵌**（`src:url(data:font/woff2;base64,…)`），无外部字体请求 |
| 体积 | 292 KB（4 节点 / 1 边，含内嵌字体） |

> **官方口径一致**：export.md 明确 "SVG is built in plain Node, so it works with no browser"；PNG/JPEG/GIF/MP4 才需 Chrome。**这意味着服务端出 SVG 不需要任何浏览器基础设施** —— ADR-VIZ-007 的"不部署 headless Chrome"决策在 SVG 通道上完全成立。

### 2.2 转义行为：实测有效（非文档承诺）

用含注入 payload 的 spec 实跑 `SC render`：

| payload | 产物中的形态 | 判定 |
|---|---|---|
| `<script>alert('XSSL')</script>` | `&lt;script&gt;alert('XSSL')&lt;/script&gt;` | ✅ 转义 |
| `"><img src=x onerror=alert(1)>` | `&quot;&gt;&lt;img src=x onerror=alert(1)&gt;` | ✅ 转义 |
| `javascript:alert(1)` | 作为纯文本 | ✅ |
| `A & B < C > D " E ' F` | `&`/`<`/`>`/`"` 已转义；**单引号 `'` 未转义** | ⚠️ 见 §5.3 |

源码核对：`skills/seecode/scripts/lib/svg.mjs:3-9` 与 `lib/viewer/viewer.client.js:20` 均有 `esc()`，转义 `& < > "`；viewer 的两处 `innerHTML`（`:313` `panel.innerHTML = html`）传入的动态字段**全部经 esc**（`:405-424`、`:470-479` 的 label/sub/tag/evidence 逐项 `esc(...)`）。

**结论**：ADR-VIZ-010 的硬门槛（"SeeCode 不过安全渲染测试则不允许接收不可信 Spec"）**实测通过**（V-3 的核心部分），可解除。

### 2.3 HTML 产物的外链（仅影响被弃用的通道）

HTML 产物含 4 类外部请求：

| 外链 | 来源 | 可否关闭 |
|---|---|---|
| `fonts.googleapis.com` / `fonts.gstatic.com` | 字体 stylesheet | 可（`config set fonts system` / spec `"fonts":"system"`） |
| `cdn.jsdelivr.net/npm/gifenc@1.0.3` | 页面内 GIF 导出编码器 | **CLI 导出路径不受影响**：`lib/export/export.mjs:22-25` 的 `libSource()` 读 `skills/seecode/scripts/vendor/` 下的**本地** `gifenc.js`/`mp4-muxer.js`/`webm-muxer.js`（`doctor.mjs:21` 校验其存在），jsDelivr 仅是 HTML 页面内的 fallback |
| `cdn.jsdelivr.net/npm/mp4-muxer`、`webm-muxer` | 同上 | 同上 |

> 即：**服务端 CLI 导出可完全离线**（本地 vendor 编码器 + 内嵌字体）；只有"用户在自己浏览器里点页面内 Export 菜单"才会触达 jsDelivr。若我们不再向浏览器投递 HTML 产物，这条路径也随之消失。

## 3. 安全模型重构：从"隔离不可信代码"到"不执行不可信代码"

### 3.1 两种范式对比

| 维度 | 原方案（iframe 隔离） | 新方案（不执行） |
|---|---|---|
| 假设 | 产物不可信 → 用沙箱隔离 | 产物不可信 → **不让它运行** |
| 防线数 | 5 层（独立 Origin / sandbox / CSP / 零凭证 / postMessage 白名单） | 3 层（静态化 / 零外链 / 自绘交互） |
| 残余风险 | 产物 JS 在用户浏览器内运行：绕过 CSP、挖矿、钓鱼 UI、资源耗尽 | 产物无脚本执行；残余风险=解析器缺陷（浏览器 SVG 解析器） |
| 配置依赖 | 部署必须提供独立 Origin + host-only Cookie + Header CSP（**依赖运维配合**） | 无部署依赖 |
| 交互能力 | 完整（由产物 JS 提供） | 产物通道无交互；交互由我们自绘（Viewer） |
| 验收成本 | 需实测沙箱逃逸、CSP 生效、postMessage 协议 | 静态化断言 + 外链断言 + 自绘通道无第三方代码断言 |

### 3.2 关键论断

> **"不执行"严格强于"隔离后执行"。** 沙箱是对已知会运行的代码做限制；不执行直接消除了该代码路径。即便沙箱配置正确，被入侵的产物仍可在用户浏览器内运行（DoS、钓鱼、指纹）。本方案让"产物被攻破"的最坏后果退化为"一张错误的图"。

### 3.3 新三层防线

| 层 | 措施 | 验收 |
|---|---|---|
| **L1 静态化** | 产物通道只产出 SVG；入库前经**产物类型断言**（无 `<script>`、无 `foreignObject`、无可执行 `on*` 属性、无外部引用、无 `<use>`/`<image>` 外链） | AC-7' |
| **L2 零外网** | 字体 base64 内嵌（实测已内嵌）；SVG 无任何真实外链；服务端导出走本地 vendor 编码器 | AC-6' |
| **L3 自绘交互** | 交互图由前端 G6 Adapter 渲染**VizSpec 数据**（自有代码 + 自有数据），产物代码不参与交互 | AC-8'/AC-18' |

## 4. 架构影响：SeeCode 角色降级

| 维度 | 原 ADR-VIZ-002 | 新方案 |
|---|---|---|
| SeeCode 定位 | 浏览器内产物渲染器（Adapter B） | **服务端出图器**（SVG only） |
| 调用位置 | 浏览器 iframe | worker（`SC render` → `SC export --formats svg`） |
| 运行环境 | 需独立 Origin + 浏览器 | **纯 Node**（无需 Chrome，无需独立服务器） |
| 交互 | 产物自带 | 见 §5「交互能力归属」 |
| 保留理由 | — | 保留：它提供"编辑级静态图"（内嵌字体、自适应尺寸、45 图形类型）与 SVG 纯 Node 出图能力，是**出图质量**的来源 |

### 4.1 对"SeeCode 独立服务器"的回答

用户曾提议 SeeCode 独立成服务器 + iframe 套入。**在新方案下该提议整体取消**：

- 不需要 iframe → 不需要为 iframe 准备独立 Origin；
- SVG 导出纯 Node → 不需要常驻服务，worker 内一次性调用 CLI 即可；
- 保持 ADR-VIZ-001 的"V1 同库同服务、不提前微服务"（避免为一个 CLI 工具引入常驻服务与它的运维面）；
- 若未来 GIF/MP4 或渲染并发成为瓶颈 → 按 ADR-VIZ-007 的 V2 条件项另立渲染 Worker。

**这同时解掉了上一轮悬而未决的部署问题**（独立主机名 / DNS 权限 / Cookie host-only / 内网形态）——新方案一个都不需要。

## 5. 交互能力归属（去 iframe 的真实代价与解法）

### 5.1 代价

SeeCode viewer 的交互——hover trace（悬停高亮邻接）、focus panel（点击节点开详情面板）、route journey（两点间路径播放）、lens（按类型过滤高亮）——**全部由产物内 JS 提供**。放弃 iframe = 放弃这四项在"产物通道"中的表现。

### 5.2 解法：交互归 Viewer，数据归 Spec

关键洞察：**交互所需的数据全在 VizSpec 里**（节点、边、focus、lenses）。这些交互不需要第三方代码——它们是**图算法 + UI 状态**，用 G6 或纯 SVG 自绘即可实现：

| SeeCode 交互 | 自绘实现 | 依赖 |
|---|---|---|
| hover trace | G6 事件：悬停节点 → 高亮入边/出边 | G6（P1） |
| focus panel | 点击节点 → 右栏 TracePanel 显示 source_ref / 状态 / 关联对象 | G6（P1）+ 已有 TracePanel 组件（D6 §2） |
| route journey | 路径计算（节点级 BFS/Dijkstra）→ 高亮 + 步骤列表 | G6（P1）+ 服务端可算（P3 关键路径复用） |
| lens | 按 edge_type/kind 过滤高亮（Spec 已定义 lenses 字段） | G6（P1） |

**结论**：交互能力**不会丢失**，只是从"产物通道"平移到"Live/交互通道"（G6，P1）。P0 的产物通道 = 静态 SVG（可分享、可导出、可嵌入 `<img>`），P1 的 G6 通道 = 完整交互。

### 5.3 单引号未转义的处置

实测 `esc()` 不转义单引号 `'`。在 SVG 文本节点与 aria-label 属性值中，单引号无害（属性用双引号包裹）。但若将来出现**单引号包裹的属性**或 HTML 模板路径，则需补转义。

**处置**：在 AI-PM 侧的输入约束（D3 §4）中，把单引号纳入"允许但记录"清单，并在 V0-7 的注入用例集中增加"单引号 + 上下文"用例（`V-3` 的补充项）。

## 6. 新方案下的四条验收项（替换原 AC-7/8/18 + AC-6 修订）

| 原编号 | 原内容 | 新编号 | 新内容 | 阻断等级 |
|---|---|---|---|---|
| AC-7 | 沙箱隔离（opaque origin / 零凭证 / CSP Header） | **AC-7'** | **产物静态化断言**：入库 SVG 产物中 `<script>` / `foreignObject` / 可执行 `on*` 属性 / 外部引用（`http(s)://`、`//host`、`url(http`）/ `<use>`/`<image>` 外链 计数均为 0；`<img>` 引用产物时浏览器不执行任何脚本（无 CSP 违规、无 console 注入） **已实现**：`crates/pm-application/src/visualization/artifact_safety.rs`（`inspect_svg`），以**真实 SeeCode 产物**为夹具验收（`crates/pm-application/tests/fixtures/seecode_real_artifact.svg` + `tests/viz_artifact_safety.rs`）。 B1 + 发布阻断 |
| AC-8 | 权限 404 同形 / 403 | AC-8 | **不变**（权限维度与渲染形态无关） | B1 |
| AC-18 | postMessage 协议安全 | **AC-18'** | **无第三方产物代码参与前端交互**：交互通道的前端执行路径中不加载任何 SeeCode 产物脚本（构建产物断言：无产物 JS 引入；源码断言：交互组件不 import 产物 HTML）；产物 SVG 以 `<img>`（或 `<object>`+无脚本）引用 | B1 + 发布阻断 |
| AC-6 | 零外网 + 导出链实测 | **AC-6'** | **服务端 SVG 导出链零外网**：实测导出过程与产物均无外部请求（字体 base64 内嵌已验证）；删除"浏览器内页面 Export 菜单"路径的依赖声明 | B1 |

**AC-9（注入防线）**：保留，但验收对象改为 SVG 产物（实测已过，见 §2.2）；补单引号与上下文用例。

## 7. 残余风险登记

| # | 风险 | 级别 | 说明 | 缓解 |
|---|---|---|---|---|
| SR-1 | **SVG 解析器缺陷**（浏览器 SVG 解析器漏洞） | S2 | SVG 不执行脚本，但解析仍由浏览器处理 | 只允许服务端生成、只读展示；CSP 仍下发（纵深）；产物来源可追溯（spec_hash） |
| SR-2 | **SVG 体积膨胀**（内嵌字体使 4 节点图达 292 KB） | S2 | 大图 SVG 可达数 MB，影响加载 | 产物大小上限（13 号 §5：SVG ≤ 8 MB）；`fonts:system` 可减小；前端懒加载；分档渲染 |
| SR-3 | **静态化丢失交互**（产品感知） | S1 | 产物通道无 hover/focus/route | §5.2 解法：交互归 G6（P1）；P0 期间产物通道在 UI 上标注"静态预览" |
| SR-4 | **SVG 内文本不可选中搜索**（`<img>` 引用） | S3 | 降级视图（表格）已覆盖可搜索需求 | 保留降级表格通道 |
| SR-5 | **产物仍含业务标签**（数据不出域的边界） | S0 | 产物在 PG 内、随权限裁剪后下发 | 权限过滤在服务端完成（14 号设计）；产物无脚本 → 无法外传数据（无 fetch/form/img 外链） |
| SR-6 | **用户下载 SVG 后本地打开** | S2 | 若 SVG 含脚本则风险；本方案已断言无脚本 | 静态化断言覆盖（AC-7'） |
| SR-7 | **未来若重新引入 HTML 产物** | S1 | — | 需重新走 ADR-008 全套（独立 Origin + sandbox + CSP + postMessage）；默认禁止 |

## 8. 修订清单（需 Architecture Owner 签收）

| # | 修订对象 | 修订内容 |
|---|---|---|
| M-1 | **ADR-VIZ-008** | 标题与结论改为「产物静态化与零脚本呈现」：删除五层 iframe 隔离方案，改为三层（静态化 / 零外网 / 自绘交互）；删除 postMessage 协议章节；新增"重新引入 HTML 产物需重走 ADR-008"条款 |
| M-2 | **ADR-VIZ-002** | SeeCode 定位从"浏览器内产物渲染器"改为"服务端出图器（SVG）"；删除"SeeCode 独立服务器 + iframe"选项；保留 Fallback Adapter |
| M-3 | **ADR-VIZ-007** | 导出能力矩阵更新：SVG = 服务端纯 Node 出图（V1 承诺）；HTML = 不向浏览器投递（仅可选下载，且须静态化检查）；PNG/GIF/MP4 维持 Deferred（需 Chrome） |
| M-4 | **ADR-VIZ-009 / D6** | 交互归属：产物通道静态；交互通道（G6）承担 hover/focus/route/lens；TracePanel 复用既有组件 |
| M-5 | **AC-6/7/8/18** | 按 §6 重写；AC-8 不变 |
| M-6 | **V-2 / V-3** | V-2（沙箱 CSP 实测）**取消**（不再需要）；V-3（注入测试）**转为** SVG 产物静态化断言 + 注入向量用例；V-4（vendor 构建与降级）保留且扩展为"SVG 出图链可复现 + 离线" |
| M-7 | **AC-11（性能）** | 产物通道不再有 iframe 渲染开销；新增 SVG 体积与前端渲染耗时指标 |
| M-8 | **D2 / D4 / D6 / D7** | 同步删除 iframe/独立 Origin/postMessage 相关描述，改为静态化 + 自绘交互 |

## 9. 与「交互能力归属」的排期影响

| 期 | 变化 |
|---|---|
| P0 | 产物通道改为**服务端 SVG**（不再有 iframe 与 CSP 工作）；产物在对话卡内以 `<img>` 静态呈现 + 标注"静态预览"；G6 引入提前到 **P1 开头**以尽快补回交互 |
| P1 | G6 交互通道上线，取代产物通道的交互职责；P0 的静态产物保留为"可分享版本" |
| P2/P3 | 不变 |

**风险提示**：G6 引入提前会让 P1 的包体/性能验证（V-1）更早成为关键路径。建议 V-1 验证在 P0 末即启动（不等 P1 开工）。

## 10. 结论

| 问题 | 结论 |
|---|---|
| 放弃 iframe 是否更安全？ | **是**，且是量级差异：从"隔离后执行"变为"不执行"，残余风险从"沙箱逃逸"降为"解析器缺陷" |
| 是否需要 SeeCode 独立服务器？ | **不需要**。SVG 出图纯 Node，worker 内一次性调用即可 |
| 交互能力是否丢失？ | **不丢失**，平移到 G6 通道（P1）；P0 期间产物为静态预览 |
| ADR-VIZ-010 硬门槛是否解除？ | **实测通过**（转义有效、SVG 无脚本无外链），可解除 |
| 是否需要外部专家复审？ | **不需要**（属既定方向内的安全姿态升级，非选型重开）。但 **M-1~M-8 修订需 Architecture + Security Owner 签收**，且 M-2 改变了 ADR-VIZ-002 的 SeeCode 定位，**建议知会原评审专家**（一句摘要即可，不需复审轮次） |
