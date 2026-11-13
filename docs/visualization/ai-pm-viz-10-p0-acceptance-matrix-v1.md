# AI-PM Visualization P0 验收矩阵 V1.0

> **状态**：已发布（产品经理编制，待 QA Owner 与复审方签收） · **日期**：2026-11-12
> **基线**：[ADR V1.0.1](ai-pm-viz-09-architecture-decision-record-v1.md)（附条件通过）· AC 编号沿用 [D8](ai-pm-viz-08-risks-acceptance-v1.md) 原始定义，不重新编号
> **作用**：闭合复审阻断项 **R-02**（AC-1~21 阶段归属）并提交 **R-01** 核查证据（§6）；PNG（R-03）按条件处理不闭合。本矩阵是 M1 验收与 P0 测试任务拆分的直接依据。

## 1. 阻断等级定义

| 等级 | 语义 | 处理 |
|---|---|---|
| **B1 = M1 阻断** | P0 验收未通过则 M1 不通过、不得进入 P1 | 逐条配测试场景与证据，CI 强制 |
| **B2 = M1 条件** | 允许按**已决策的降级承诺**通过，但降级本身必须有决策记录 | 每条注明所依赖的降级决策 |
| **P1 = 移交** | 依赖 P1 能力（G6/Live），不阻断 M1 | 列入 §5 移交清单，P1 前必须闭合 |

安全类条目（AC-6/7/8/9/18）同时是**发布阻断**：无论 M1 与否，发布构建前必须全过。

## 2. AC 阶段归属总表（关闭 R-02）

21 条全部登记。AC-5/10/11/13/14/15 依据 D8 原始文本核对后归属（补齐 ADR V1.0.1 §ADR-011 未列的 6 条）：

| AC | 主题 | 阶段 | 实施任务 | 阻断 | 依赖 |
|---|---|---|---|---|---|
| AC-1 | Spec 校验 E/W/I + Golden | P0 | V0-1/V0-4 | B1 | R-06/07/09（设计闭合）；实现方案见 [12 号 V0-1 设计](ai-pm-viz-12-v0-1-spec-validation-design-v1.md) |
| AC-2 | Adapter 多目标 + 降级 | P0 基础 / P1 完整 | V0-4/V0-9 | B1（P0 部分） | V-4 |
| AC-3 | Artifact 状态机全路径 | P0 | V0-2 | B1 | R-04 |
| AC-4 | stale 指纹矩阵 | P0 | V0-2/V0-3 | B1 | R-04/05/07 |
| AC-5 | AI 生成链路 E2E | P0 | V0-5 | B1 | V-3（Fallback 路径即可过） |
| AC-6′ | 零外网 + 服务端 SVG 导出链实测 | P0（SVG）；PNG 条件 | V0-9/V0-10 | **B2**（PNG 部分） | R-03；见 15 号修订 |
| AC-7′ | 产物静态化断言（无脚本/无外链） | P0 | V0-6 | B1 | 已实现（`artifact_safety.rs`）；真实 SeeCode 产物作夹具验收 |
| AC-8 | 权限 404 同形/403 | P0 | V0-2/V0-3 | B1 | — |
| AC-9 | 注入防线（验收对象=SVG 产物） | P0 | V0-7 | B1 | V-3；补单引号用例 |
| AC-10 | 中文用例集 | P0 | V0-10 | B1 | — |
| AC-11 | 规模分档 | P0=分档行为+拒绝+基线定义；P1=帧率达标 | V0-1/V0-8 | B1（P0 部分）/ P1 | V-1（P1 部分） |
| AC-12 | viz 可拔 + 关闭态全隔离 | P0 | V0-8 + 专项回归 | B1 | R-10 |
| AC-13 | 零 mock 合规 | P0 | V0-8（release-guard 既有） | B1 | — |
| AC-14 | i18n viz.* | P0 | V0-8 | B1 | — |
| AC-15 | 溯源可达 + unlinked | P0（Artifact 通道）；Live 侧 P1 | V0-2/V0-8 | B1 | — |
| AC-16 | 零业务领域写路径 | P0 | V0-2/V0-5 | B1 | R-04（按 ADR §2.1 语义验证） |
| AC-17 | 权限投影（边存在性不泄露） | P0 | V0-3 | B1 | R-08 |
| AC-18′ | 无第三方产物代码参与前端交互 | P0 | V0-6 | B1 | 见 15 号修订 |
| AC-19 | 一致性语义 | P0 | V0-3 | B1 | R-07 |
| AC-20 | business/explanatory 区分 | P0 | V0-2/V0-5 | B1 | R-06 |
| AC-21 | Contract 八项 + Golden 入 CI | P0 基础 / P1 Adapter 扩展 | V0-4 | B1 | V-3/V-4 |

## 3. P0 逐条验收矩阵（测试场景 · 证据 · 阻断）

场景编号 T-01~T-24；证据列均为**可存在的工件**（测试文件/CI 任务/审计输出），不接收口头确认。

### A. Spec 与校验

| AC | 测试场景 | 实施任务 | 验收证据 | 阻断 |
|---|---|---|---|---|
| AC-1 | T-01 合法样例集（每 kind ≥1）全过校验；T-02 非法样例逐条返回预期 `{code,at,fix}`（未知引用/缺 source_ref/超预算/未登记 rel）；T-03 Golden：固定输入→校验输出与规范化结构快照比对，任何人改 Spec 行为必须显式更新快照并过审 | V0-1/V0-4 | `crates/pm-domain/src/visualization/validation` 内联单测 + `validation/golden/{cases,expected}` 手写比对 + `scripts/viz-spec-lint.js` | B1 |
| AC-4 | T-04 指纹变更矩阵：对 revision_id/content_hash/关系版本/槽位配置各自变更 → stale 判定逐一断言；T-05 published 内容不可变（UPDATE 拒绝） | V0-2/V0-3 | Rust 集成测 `apps/pm-api/tests/viz_stale.rs` | B1 |

### B. Artifact 生命周期（AC-3，依赖 R-04 状态机分离设计）

| AC | 测试场景 | 实施任务 | 验收证据 | 阻断 |
|---|---|---|---|---|
| AC-3 | T-06 全路径：draft→published→stale；draft→rejected（reason 必填）；T-07 并发：同 slot 第二条 pending 审批触发 409（数据库约束级，非应用层先查后写）；T-08 审批通过仅变更 Artifact 生命周期与 Approval 状态，两者字段互不混写 | V0-2 | `tests/viz_artifact_lifecycle.rs`（含并发用例） | B1 |

### C. AI 生成链路（AC-5）

| AC | 测试场景 | 实施任务 | 验收证据 | 阻断 |
|---|---|---|---|---|
| AC-5 | T-09 生成→校验 E→修补（≤3 轮）→Accept→发布全链路 E2E（渲染走 Fallback 亦可过，SeeCode 环节按 V-3 结果）；T-10 Reject 必带 reason，Reject 后图对非作者不可见；T-11 explanatory 图无发布入口（服务端 4xx + UI 断言） | V0-5 | playwright `e2e/viz/ai-draft-chain.spec.ts`（真后端栈） | B1 |

### D. 安全（发布阻断）

| AC | 测试场景 | 实施任务 | 验收证据 | 阻断 |
|---|---|---|---|---|
| AC-6 | T-12 网络层断言：HTML 渲染与 SVG 导出全程零外网（无 Google Fonts/jsDelivr/CDN/远程图片）；T-13 导出失败不触发外网回退 | V0-9/V0-10 | e2e 网络拦截断言 + 报告工件 `viz-offline-audit.md` | B1（HTML/SVG） |
| AC-6-PNG | T-14 PNG 路径七问核查（组件/运行时/截图依赖/编码器/零外网/中文字体/失败降级）——**未闭合前 PNG 不对外承诺** | V0-10 | R-03 决策记录（能力矩阵表更新或降级声明） | **B2** |
| AC-7 | T-15 opaque origin：iframe 内访问 parent/localStorage/cookie 全失败；T-16 CSP Header 逐指令生效（header 下发，meta 无效性反证）；T-17 主应用 Cookie host-only，产物 Origin 零凭证 | V0-6 | `e2e/viz/security.spec.ts` + CSP 响应头快照 | B1 |
| AC-8 | T-18 权限矩阵：跨项目/越权对象/api_key 三类请求 404 同形或 403；T-19 被裁剪引用转 W 级悬空边（不静默丢弃） | V0-2/V0-3 | `tests/viz_authz.rs` + e2e | B1 |
| AC-9 | T-20 注入向量 × 全输出位（节点/边/tooltip/focus/HTML 导出/SVG 导出）：`<script>`、事件属性、`javascript:`、外部资源、SVG 注入、Unicode/RTL/零宽、超长、双重转义（`A & B` 不得显示 `&amp;amp;`）；T-21 textContent/innerHTML 实测记录进 ADR V-3 证据 | V0-7 | `e2e/viz/injection.spec.ts` + 向量清单文件 + V-3 签收 | B1 |
| AC-18 | T-22 postMessage：非白名单类型/伪造 source/缺失或过期 session ID/超长/超频 → 全部丢弃并审计；宿主收到 node_selected 后重新查权限（iframe 提交内容不直接信任） | V0-6 | `e2e/viz/postmessage.spec.ts` + 审计表查询输出 | B1 |

### E. 权限投影 · 一致性 · 图形性质

| AC | 测试场景 | 实施任务 | 验收证据 | 阻断 |
|---|---|---|---|---|
| AC-17 | T-23 泄露矩阵：无权对象的节点/Tooltip/**边存在性/关系计数/聚合数/截断数/布局占位/错误消息/日志/缓存行为**逐项断言响应中不含任何标识 | V0-3 | `tests/viz_projection_authz.rs`（字节级断言） | B1 |
| AC-19 | T-24 一致性：同事务场景 `consistency=current`；跨对象混合时点场景返回 `mixed-revision` + 各对象 revision refs，UI 提示可见 | V0-3 | `tests/viz_consistency.rs` + vitest 断言 | B1 |
| AC-20 | T-11（复用）：explanatory 服务端拒发布 + UI 恒显徽标，双重断言；nature 粒度按 R-06 设计结论落到 Spec schema 后补节点/边级用例 | V0-2/V0-5 | `tests/viz_nature.rs` + vitest | B1 |

### F. Contract · 降级 · 可拔 · 工程基线

| AC | 测试场景 | 实施任务 | 验收证据 | 阻断 |
|---|---|---|---|---|
| AC-2（P0 部分） | T-25 每已交付 kind ≥1 主呈现 + ≥1 Fallback；Contract 八项断言（合法 HTML/SVG、零意外外网、无注入、溯源保留、节点/边身份保留、失败可处理）；T-26 降级演练：模拟 SeeCode 不可用 → Fallback 按矩阵生效、链路不阻断。P0 **不要求** G6 第二渲染目标（完整语义 P1 闭合） | V0-4/V0-9 | `tests/viz_contract.rs` + 演练记录 | B1 |
| AC-12 | T-27 `VIZ_ENABLED=false`：API 404/403 语义、前端入口隐藏、后台任务不调度、既有链路回归、已有 Artifact 不阻塞业务请求（ADR-001 七项） | V0-8 + 专项 | `scripts/test.sh` 内 `viz-detach` 分支：关分区跑全量既有测试绿（仓内无 CI workflow，落本地脚本） | B1 |
| AC-13 | T-28 发布构建产物无 fixtures/demo/mock 引用 | V0-8 | release-guard 既有门禁输出 | B1 |
| AC-14 | T-29 `viz.*` 命名空间完整 + 硬编码扫描 | V0-8 | `check:i18n` 双脚本输出 | B1 |
| AC-15 | T-30 任一节点点击 → TracePanel 显示 source_ref 并可跳转 locator；无溯源对象显式 unlinked 灰标（UI 断言） | V0-2/V0-8 | vitest + e2e | B1 |
| AC-16 | T-31 接受图形后审计断言：业务域对象零变更（**viz 自身四表写入允许**，按 ADR V1.0.1 §2.1）；T-32 架构 lint：viz 模块 import 领域写 service 即构建失败 | V0-2/V0-5 | 审计查询输出 + `scripts/viz-no-write-path.sh` | B1 |
| AC-11（P0 部分） | T-33 预算执行：>500 节点 E 级拒绝；30/150/500 分档行为断言；150 节点帧率**基线定义与测量脚本**交付（实测达标 P1） | V0-1/V0-8 | 校验单测 + `docs/visualization/viz-perf-baseline.md` + 测量脚本 | B1（P0 部分） |
| AC-10 | T-34 中文用例集：长标签换行/图例中文/SVG 内嵌字体/深色一致（HTML+SVG 双路） | V0-10 | `e2e/viz/i18n-zh.spec.ts` + 快照 | B1 |

## 4. 场景-证据汇总

24+ 组场景（T-01~T-34，含复用），证据工件共 5 类：Rust 集成测（6 个新测试文件）、playwright 安全/E2E（独立 config，5 个 spec）、vitest（组件级）、**本地门禁脚本**（`scripts/check-viz.sh` 统一入口 → `viz-registry-lint.js`/`viz-spec-lint.js`/`viz-no-write-path.sh`，另加既有 release-guard/check:i18n —— **仓内无 CI workflow，"CI 任务"一律落为本地脚本**）、人工签收件（V-2/V-3/V-4 签收、`viz-offline-audit.md`、`viz-perf-baseline.md`、R-03 决策记录、降级演练记录）。

> 另：**Golden 快照无 insta/expect-test 可用**（仓内零快照 crate）→ 手写文件比对，方案见 [12 号 V0-1 设计](ai-pm-viz-12-v0-1-spec-validation-design-v1.md) §8.1。

## 5. P1 移交清单（不阻断 M1，P1 前必须闭合）

| 项 | 内容 | 关闭条件 |
|---|---|---|
| AC-2 完整 | G6 第二渲染目标 + 每 kind 交互增强 | V-1 通过 + Contract 扩展 |
| AC-11 帧率 | 150 节点交互 ≥50fps（按 V-1 固定的设备/浏览器/操作集/p50-p95 方法实测） | V-1 通过 |
| AC-15 Live 侧 | Live 图溯源点击与 unlinked 标注 | V1-2 交付 |
| V-1 | G6 构建包体/性能验证（M1 不阻断，P1 前通过） | ADR V1.0.1 §ADR-011 |

## 6. R-01 核查结果（证据提交，待签收）

按 ADR V1.0.1 要求于 2026-11-12 在本 checkout 执行核查：

| 证据 | 结果 |
|---|---|
| `ls migrations/`（本 checkout） | 最大号 **0019**（`0019_conversation_context_item.sql`） |
| `docs/reference/ai-pm-migration-ledger.md` | 最大**已应用 = 0054**（`0054_acp06_07.sql`，2026-11-08）；台账纪律 §三.1 明示"新取号以 `ls migrations/` 实况为准" |
| 结论 | **台账与 checkout 存在漂移**（0044~0054 为并行工作流在他处应用）；原 ADR 假设"0020"作废 |
| 暂定取号 | **0055** = max(实况 0019, 台账 0054) + 1 |
| 待办 | Migration Owner 于 V0-1 启动日在**真实 master** 上复核 `ls migrations/` 与台账后签收确认；若复核结果 >0054 则顺延；确认前任何 viz 迁移不得建文件 |

R-02 关闭声明：本矩阵 §2 已登记全部 21 条归属；请 QA Owner 签收。R-03 维持开放（AC-6-PNG=B2）。

### 6.1 R-01 补充核查（2026-11-12 第二轮，跨分支）

| 证据 | 结果 |
|---|---|
| `git ls-files migrations/`（master/origin/master 树） | 仅 **0001–0019**（19 个文件） |
| `git log --all --diff-filter=A -- 'migrations/*.sql'` | 跨分支出现的迁移号已达 **0064**（0044–0064 区间有 0029、0045、0048 缺号） |
| `0055` 占用情况 | **已被 `0055_device_name_active_unique.sql` 占用**（devenv 修复迁移），不可取用 |
| 相关提交是否在 origin/master | 5393e706 / cca7d070 / 0c05946e / 4a592d7c **均在 origin/master 历史中**，但对应文件不在 master 当前工作树 → 迁移文件存在"已合入历史、未留存工作树"的不一致 |
| 结论 | **本文件先前"暂定 0055"作废**；真实可用编号须以 master 实际迁移目录实况 + 台账双源核对后确定，且需确认迁移目录的真实归属（master 树仅 0019 vs 历史 0064 的不一致本身需先解释） |

**处置（依 2026-11-12 决策）**：编号核查**不阻断设计阶段与 V0-1 实施**；编号确认收敛到**建表任务（V0-2）启动前必须闭环**，届时由 Migration Owner 在真实 master 上给出：① 实际迁移目录归属；② 下一个可用编号；③ 台账登记行。三项缺一不可开始写迁移文件。

## 7. M1 验收清单（整合 ADR V1.0.1 §8）

- [ ] P0 归属 AC（§2 标 P0 的全部 B1 条目）测试全绿，证据工件齐备
- [ ] B2 条目（AC-6-PNG）持有降级决策记录
- [ ] V-2、V-3、V-4 签收（V-1 不阻断 M1，P1 前通过）
- [ ] 既有业务全量测试绿（含 `viz-detach` 关闭态回归）
- [ ] R-01 由 Migration Owner 在真实 master 复核签收；viz 迁移以签收号落文件
- [ ] R-04~R-10 设计闭合件归档（状态机/容量规则/nature 粒度/consistency 语义/Origin-Cookie 设计/Registry 治理/关闭范围）
- [ ] Relation Registry 领域 Owner 会签记录归档
