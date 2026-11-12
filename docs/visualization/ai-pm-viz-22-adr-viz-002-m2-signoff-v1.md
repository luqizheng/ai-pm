# ADR-VIZ-002 M-2 修订签收稿

> **状态**：待签收（**专家要求：M-2 必须先签收，才允许 vendor SeeCode**）
> **日期**：2026-11-12 · **修订对象**：ADR-VIZ-002「渲染 Adapter 策略」· **来源**：15 号安全审查（实测）+ 专家裁决 2026-11-12
> **签收范围**：Architecture Owner + Security Owner（**双签**）

## 1. 修订性质（为什么必须走签收，不能算实现细节）

专家明确：这是**架构定位变更**，不是实现细节——「不能将原 ADR-VIZ-002 对浏览器内 SeeCode 的安全假设，直接继承为服务端执行的安全结论」。

| 维度 | 原定位（ADR V1.0.1 前） | 新定位（本修订） |
|---|---|---|
| 执行位置 | 用户浏览器内（iframe） | **服务端渲染 Worker** |
| 执行环境 | 浏览器沙箱 + 独立 Origin + CSP | **进程隔离 + 资源限额**（本稿 §3） |
| 产物形态 | 交互式 HTML（内含 JS） | **SVG only**（静态，无脚本） |
| 信任边界 | 产物作为不可信主动内容隔离 | **输入不可信 + 执行环境需另建隔离** |
| 依赖注入 | 仅浏览器运行时 | **Node 20 运行时（仅渲染 Worker 镜像）** |
| 部署责任 | 静态产物域 | Worker 镜像维护 + Node 运行时安全更新 |

**核心论断**：原安全模型解决的是「不可信 HTML 在浏览器中运行」；新模型要解决的是「不可信输入在服务端 Node 中执行」。**两者不可互相继承**。

## 2. 修订正文（ADR-VIZ-002 新条款）

### 2.1 Adapter B 定位变更为「服务端 SVG 出图器」

- 名称：**SeeCode Adapter（服务端出图器）**
- 输入：已通过校验的 VizSpec（**不可信主动内容**，见 §3.1）
- 输出：**SVG only**（HTML/GIF/MP4 不在此通道产出）
- 运行位置：渲染 Worker 进程，以子进程方式调用 SeeCode CLI
- 非核心依赖：渲染失败 → job 落 Failed/Rejected → 前端降级表格；**Live 图与 Fallback 不受影响**

### 2.2 落仓约束（vendor 不等于成为核心产品代码）

| 要求 | 落地 |
|---|---|
| 固定上游版本与 Git commit | `tools/seecode/VERSION`（version + commit + 获取日期） |
| 保留许可与来源 | `LICENSE` / `NOTICE` / `UPSTREAM.md`（含获取方式与修改记录） |
| 与自有 Adapter 隔离 | 业务领域**不得**直接依赖 `tools/seecode/`；仅 Adapter 可调用 |
| 明确移除路径 | 删除 `tools/seecode/` + 注销 Adapter 注册即回退到 Fallback，Live 图零影响 |
| 依赖方向 | `pm-application` → Adapter 接口 →（实现层）`tools/seecode/` 子进程；**无反向引用** |

建议目录（可按实际仓库结构调整，**依赖边界必须保持**）：
```
tools/seecode/{vendor/,LICENSE,NOTICE,UPSTREAM.md,VERSION}
apps/ai-pm-worker/src/viz/  ← Adapter 实现（子进程调用 + 输出捕获 + 超时）
```

### 2.3 升级策略

**锁版本 + 每季度评估**。每次升级必须形成版本变更记录，含五项：

1. 上游版本与 commit
2. 修改文件与差异（若有本地 patch）
3. 新增依赖与许可证变化
4. **安全回归结果**（`viz_artifact_safety`：真实产物夹具 + 恶意 SVG + 主动内容 + 外部引用）
5. **Adapter 契约回归结果**（`verify()` 八项断言）

> 升级不是自动追随上游，而是一次经评估的依赖变更。

### 2.4 运行时边界

| 项目 | 裁决 |
|---|---|
| 渲染 Worker 镜像 | **允许 Node 20** |
| API 服务镜像 | 不引入 Node 运行时依赖 |
| 普通任务 Worker | 不引入 SeeCode |
| Chrome / Headless Browser | **不需要**（SVG 纯 Node，15 号实测） |
| SeeCode 输出格式 | SVG only |
| 渲染失败 | Failed / Rejected，不影响 Live 图 |

**冻结前待办**：正式冻结前须确认 Node 20 实际依赖兼容性与运行时安全维护状态（专家附加条件）。

### 2.5 安全回归责任（双测试门禁）

**两个测试套件必须同时在 Adapter 变更时通过：**

| 套件 | 覆盖 |
|---|---|
| `viz_artifact_safety` | 真实产物夹具、恶意 SVG、主动内容（script/事件属性/foreignObject）、外部引用 |
| `adapter_contract` | 八项契约断言 + 失败状态 + 幂等性 + 确定性 |

**责任划分**：
- Adapter 开发者：变更与测试
- Visualization 技术 Owner：合并门禁
- Security Owner：安全规则与高风险用例基线维护

> **安全回归责任不能完全交给 Adapter 开发者本人。**

## 3. 服务端执行隔离（专家要求必须重新确认的六项）

原浏览器侧安全约束（独立 Origin、iframe sandbox、CSP、postMessage 白名单）**仍然适用**（产物在浏览器端展示时）；但**服务端执行隔离需另外建立**，六项逐一确认：

### 3.1 输入属于不可信主动内容

- VizSpec 的 label / status / locator 等字段一律按不可信文本处理（模型生成或用户输入）。
- 不得把 spec 当可信配置拼接进命令行参数——**必须走 stdin 或临时文件传参**，避免命令注入。
- 输入长度/结构上限由校验器前置拦截（`SpecLimits` 已实现）。

### 3.2 服务端执行必须隔离，不继承任何凭据

| 凭据 | 要求 |
|---|---|
| API 凭据（Bearer / JWT） | **不注入**渲染子进程环境 |
| 数据库 DSN | **不注入** |
| 业务权限上下文 | **不传递**——渲染器不需要知道调用者是谁 |
| 文件服务 token | **不注入** |

渲染子进程环境变量白名单化（仅 `PATH`、`NODE_PATH`、`HOME` 到临时目录）。

### 3.3 渲染过程不得访问外网

- 网络出口：渲染 Worker 层面默认阻断（或容器网络策略限制）。
- SeeCode 侧配置：`fonts: system`（不加载 Google Fonts）。
- HTML 页面内导出路径不经服务端（GIF/MP4 Deferred，不在本通道）。

### 3.4 限制 CPU / 内存 / 执行时间 / 文件系统

| 资源 | 限制要求 |
|---|---|
| 执行时间 | 硬超时（建议 30s，超时 kill 子进程 → Failed） |
| 内存 | 子进程内存上限（容器/ulimit 层面） |
| CPU | 并发度限制（Worker 内串行渲染，或信号量限流） |
| 文件系统 | 仅临时目录可写；**临时目录每次渲染后清理** |
| 进程数 | 禁止渲染进程 fork 额外长期子进程 |

### 3.5 SVG 输出必须经过独立安全静态化检查

- 渲染产物**入库前**必须过 `inspect_svg`（已实现，`artifact_safety.rs`）。
- 检查项：`<script>`、可执行 `on*` 事件属性、`foreignObject`、`iframe`/`object`/`embed`、外部引用（`http(s)://`、协议相对、CSS `url(http`、`@import`）、DOCTYPE/ENTITY。
- 允许项：`data:` URI（内嵌位图与字体）、片段引用（`#id`）。

### 3.6 检查失败必须拒绝产物，不得以未检查的 SVG 降级入库

**硬规则**：静态化检查失败 → `RenderOutcome::Failed` → **不落 `viz.render_output`**（已实现，`render_job.rs`）。

**禁止**的三种"降级"：
- ❌ 跳过检查直接入库
- ❌ 检查失败后改写 SVG 再入库（改写本身可能引入新问题）
- ❌ 降级为「未经检查的 SVG」

允许的唯一降级：**前端表格视图**（`VizSpecTable`，不涉及 SVG）。

## 4. 专家附加的重要澄清（写入本修订）

> **SVG 是静态文件格式，不代表 SVG 内容天然安全。**

必须检查脚本、事件处理器、foreignObject、外部资源引用、危险 CSS 等主动内容——**不能仅凭扩展名或 MIME 类型判定安全**。

本修订据此明确：`format='svg'` 不是安全证明；安全证明只能来自 `inspect_svg` 的检查结果。

## 5. 签收表

| # | 确认项 | Architecture Owner | Security Owner |
|---|---|---|---|
| 1 | Adapter B 定位变更为服务端 SVG 出图器 | ☐ | ☐ |
| 2 | 落仓约束（版本锁定 / 许可 / 隔离 / 移除路径） | ☐ | — |
| 3 | 升级策略（锁版本 + 季度评估 + 五项记录） | ☐ | ☐ |
| 4 | 运行时边界（Node 20 仅渲染 Worker） | ☐ | ☐ |
| 5 | 双测试门禁 + 三方责任划分 | ☐ | ☐ |
| 6 | §3.1 输入不可信 + 传参不经命令行 | — | ☐ |
| 7 | §3.2 零凭据继承 | ☐ | ☐ |
| 8 | §3.3 零外网 | — | ☐ |
| 9 | §3.4 资源限额四项 | ☐ | ☐ |
| 10 | §3.5 独立静态化检查 | — | ☐ |
| 11 | §3.6 检查失败拒绝入库（禁止三种降级） | ☐ | ☐ |
| 12 | §4 SVG 格式 ≠ 内容安全 | — | ☐ |

**签署**：Architecture Owner ____________ 日期 ______　·　Security Owner ____________ 日期 ______

## 6. 签收后的执行顺序（专家裁决 P0-3 → P0-5）

```
1. M-2 双签完成（本文件）
2. vendor 落仓（tools/seecode/ + VERSION/UPSTREAM/NOTICE）
3. Worker 渲染隔离实现（§3.1~§3.4） + Adapter 注册
4. 双测试门禁全绿（viz_artifact_safety + adapter_contract）
5. 真实产物端到端：AI 生成 spec → 渲染 SVG → 静态化通过 → 入库 → 前端 <img>
```

**注意**：第 1 步未完成前，**不得**执行第 2 步（专家明示「不批准先 vendor、后补 M-2 签收」）。

## 7. 已就绪的技术前提（无需重做）

| 项 | 状态 |
|---|---|
| RenderAdapter 接口 + 八项契约断言 | ✅ `adapter_contract.rs` |
| SVG 静态化检查 | ✅ `artifact_safety.rs`（真实 SeeCode 产物夹具已验证） |
| 渲染编排 + 失败不落库 + 幂等落库 | ✅ `render_job.rs` |
| 产物落库表（format 分列 text/bytea） | ✅ 迁移 0061 |
| 前端 `<img>` 静态呈现 + 表格降级 | ✅ V0-8 |
| **本修订的 §3 执行隔离（四项）** | ✅ **已实现**：`pm-infrastructure/src/viz_render_sandbox.rs`（见 §7.1） |
| §3.3 零外网 / §3.5 静态化 / §3.6 拒绝入库 | ✅ 已有（部署网络策略 + `artifact_safety` + `render_job`） |

## 7.1 §3 执行隔离的实现状态（2026-11-12）

`crates/pm-infrastructure/src/viz_render_sandbox.rs` 已落地 §3 的**四项可代码化条款**，
使 M-2 签收时技术前提真实存在（不是纸面承诺）：

| 条款 | 实现 | 单测 |
|---|---|---|
| §3.1 输入不可信 + 传参不经命令行 | spec 一律经 **stdin** 传入；`args` 仅放渲染器固定参数 | `stdin_payload_is_passed_to_child` |
| §3.2 零凭据继承 | `env_clear()` + 白名单（PATH / SystemRoot 等基础项 + TMPDIR/HOME 指向沙箱目录）；**凭据类键名二次过滤**（token/secret/password/database_url/api_key） | `credentials_are_not_inherited`、`secret_looking_extra_env_is_rejected` |
| §3.4 资源限额 | 执行超时（`kill_on_drop` 强杀）、stdout 字节上限、独立临时目录且 **Drop 即清理** | `timeout_kills_child`、`oversized_output_is_rejected`、`workdir_is_cleaned_up_on_drop`、`nonzero_exit_is_reported` |
| §3.1 输入经 stdin 的完整性 | 非零退出带截断 stderr 上报（UTF-8 边界安全） | `truncate_respects_utf8_boundary` |

**明确不覆盖**（写清边界，避免误以为"签了就等于全防住"）：
- **§3.3 零外网**：由部署侧网络策略保证（本模块不做网络命名空间隔离）——渲染 Worker 须在网络层阻断出口。
- **§3.5/§3.6**：由 `artifact_safety::inspect_svg` 与 `render_job` 保证（渲染后、入库前的独立闸）。
- **CPU/内存硬限额**：由容器资源限制（cgroup）承担；本模块只做时间与输出量限制。

**安全责任分工（再次强调）**：本模块防「渲染进程越权」，**不防「渲染输出恶意」**——
后者是静态化检查的职责。SVG 是静态格式，不等于内容安全。
