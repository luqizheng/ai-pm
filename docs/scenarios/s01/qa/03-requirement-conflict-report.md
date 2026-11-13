# S01 需求冲突与 GAP 报告（03-requirement-conflict-report）

> Phase 1 交付 · 2026-11-06 · QA Test Architect
> 职责：登记文档间冲突、产品裁决与实现不一致、历史方案与当前方案不一致、缺少验收标准的需求、无法确认有效性的需求。
> 处理纪律：只记录，不自行裁决，不自行选择任何一方作为正确实现。

## 1. 冲突登记

### C-01 · REQUIREMENT_CONFLICT → **已收敛（R1，2026-11-06 审查决定）** — 创建成功后落地页口径演进

| 项 | 内容 |
|---|---|
| 涉及 | S01-FR-006、S01-UX-012 |
| 冲突方 | ① `s01-02-project-creation-ux-v1-1.md` §7（Q3=A：「自动进入 Project Initialization Workspace」）② `s01-02-project-creation-design-revision-v1-1.md` §2.3（修订：「自动进入 Project Command Center + 初始化进度内嵌」） |
| 现状 | 既有前端实施行为（创建后进初始化四态面板、ready 跃迁自动跳总控台）与两稿方向均部分同向（S14 附 A-2 自认须按本稿核对落地页与跳转时机） |
| **R1 收敛** | 按 Phase 1 审查决定拆解三个概念并按已确定的产品方向收敛：① 首个页面 = 创建成功后立即进入项目初始化工作入口（Command Center）；② 展示区域 = 初始化事项作为 Command Center 内的派生展示区域（InitItem 为读模型非数据库实体）；③ 导航行为 = 初始化不要求全部完成即可开始项目工作、完成后自动收缩、新待人工事项重新展开、**不强制 ready 才允许进入 Command Center**。落地：FR-006 重写为页面导航要求（「初始化记录」→「初始化读模型可查询」）、UX-012 重写为初始化状态展示要求（补 Authority Resolution），两条均改 TRACEABLE |

### C-02 · AUTHORITY_UNCLEAR — S01-05 Project Baseline 修订稿冻结条件未完成

| 项 | 内容 |
|---|---|
| 涉及 | S01-ADR-023、S01-BR-026 等 Baseline 需求 |
| 说明 | `s01-05-project-baseline-design-revision-v1-1.md` 内嵌 V1.1 = 已裁决产品设计基线（R-BL-01~06），但冻结须四项核验完成；§九 Figma 提示词正文缺失、§六 6.1/6.2「略」登记挂账；原始 Q1/Q2 裁决正文未找回（原稿自带诚实性声明） |
| 处理 | R-BL 六裁决按 A1 采信（收编即裁决）；实施冻结状态标记 AUTHORITY_UNCLEAR，Phase 3 验收以代码事实（0037）与裁决语义为准 |

### C-03 · AUTHORITY_UNCLEAR — S01-01 编号冲突与工作包改名未执行

| 项 | 内容 |
|---|---|
| 涉及 | S01-01（初始化总览 vs 基线确认双稿占号）；修订后顺序 01~07 的改名执行 |
| 说明 | `docs/s01-全局主菜单与信息架构设计基线/ui/README.md` 明示编号冲突待裁决；第一份修订稿 §九 更正编号顺序但「稿件改名执行仍待统一」；S01-08 存在双案（Personal Home vs Knowledge Git Backup） |
| 处理 | RTM 按修订后顺序（01 总览 / 02 创建 / 03 仓库 / 04 认知 / 05 基线 / 06 Bootstrap / 07 交接）引用来源；稿件改名动作不在测试范围 |

### C-04 · IMPLEMENTATION_GAP（非缺陷，登记确认）— 项目级显式授权机制不存在

| 项 | 内容 |
|---|---|
| 涉及 | S01-SEC-010 / S01-SEC-091 |
| 说明 | S02-I6-A 核验（`docs/scenarios/s02/s02-i6a-permission-foundation-code-facts-v1.md`）确认：Project Lead 现表达=project_member.role='owner'；**项目级显式授权（Approver 授权表）机制不存在**（唯一先例 git_capability_grant 0032 建→0033 撤）。S01 范围内两档角色（owner/member）已满足需求；「确认权=Project Lead+显式 Approver」属 S02 范围 |
| 处理 | S01 测试按两档角色验证；显式 Approver 不作为 S01 失败项 |

### C-05 · REQUIREMENT_CONFLICT — Personal Home 两稿概览指标与 Watchlist 口径差

| 项 | 内容 |
|---|---|
| 涉及 | S01-UX-057 |
| 冲突方 | `s01-05-personal-workspace-ux-v1-1.md` vs `s01-05-personal-home-high-fidelity-ui-v1.md`（README §「S01-05 现存两份文档」明示：命名口径冲突已裁决为 Personal Home，但**概览指标与 Watchlist 归属两处口径差仍待裁决**） |
| 处理 | **R1（审查决定）**：两项列为待裁决功能；不阻塞 S01 核心业务验收；由产品负责人正式确定后再进入 UI 验收；QA 不自行选择其中一稿。Phase 2 用例避开分歧点，按已实施稿（高保真 §13）验证基线行为 |

### C-06 · IMPLEMENTATION_GAP — MCP 权限码收紧是跨仓协同项

| 项 | 内容 |
|---|---|
| 涉及 | S01-SEC-067（REST/MCP 一致）的边界 |
| 说明 | G-18：MCP 权限码只覆盖 5 个写工具；lmms 只签发 3 个 MCP 码、无发现族专用码。单仓收紧会让现网执行端 key 立即 403。须按「lmms 发码 → ai-pm 收紧 → 回归」顺序 |
| 处理 | S01 测试不因权限码粒度不足判 FAIL；登记为环境约束（Phase 3 若触发 403 记 BLOCKED 而非缺陷） |

### C-07 · ACCEPTANCE_CRITERIA_MISSING → **已补齐（R1）** — 刷新/重登状态一致性

| 项 | 内容 |
|---|---|
| 涉及 | S01-DATA-020 |
| 说明 | 任务书 S01-01 检查项包含「页面刷新、重新登录后状态保持一致」，但 I5/S01-01 各稿均无对应编号验收条目；架构上由「状态纯派生」结构性保证 |
| 处理 | **R1 已按审查建议补入可执行六步验收条件**（创建项目→完成部分初始化事项→刷新→重登→重开项目→逐项对比 InitItem 状态/required/source_state/expanded→确认无会话或缓存影响；全程相同项目相同领域数据）。状态改 TRACEABLE，Phase 3 UI 实证 |

### C-08 · AUTHORITY_UNCLEAR — R-S01-01-01 为「只记录未评审」裁决

| 项 | 内容 |
|---|---|
| 涉及 | S01-ADR-041 / S01-BR-011 |
| 说明 | Native AI-PM Independence 裁决记录 V1.0（2026-11-05）为专家裁决·方案 A 的忠实记录，「只记录未评审」，专家后续稿（Project Native AI-PM 等 4 稿）未到达；挂账三处细化项 |
| 处理 | 裁决正文方向与 V2.0 基线（2026-11-06 用户裁决）同向，采信为 A1 级红线；后续稿到达后须回改核对 |

### C-09 · IMPLEMENTATION_GAP — I5 性能项无压测基准

| 项 | 内容 |
|---|---|
| 涉及 | S01-NFR-087 的 L5 性能子项 |
| 说明 | 验收矩阵明示：性能项未做压测基准（无性能基线数据），建议补 p95 采样 + EXPLAIN 验证 0034 索引命中；登记为遗留不阻塞 S01 |
| 处理 | Phase 3 不执行性能压测（超出任务书范围）；登记遗留风险 |

### C-10 · 历史方案（已废止）确认

| 历史方案 | 当前有效方案 | 依据 |
|---|---|---|
| I3 V1.0 六能力分级授权模型 | V1.1 简化：可访问仓库 = Task 关联仓 ∩ verified，恒 read_write | S8 §0.0（用户裁决原文「不区分，都可以读写」）+ D-C-03 |
| V1 场景稿 Project 四态/Repository 接入态/AI-PM 初始化态/Execution Environment 态 | 最终基线 V1.2 Q1~Q6（派生式初始化，不新增状态机） | S1 编者按 |
| `s01-ui-design-system-v1.md` 视觉基线 | 设计系统 V2（Slate & Muted Teal） | git 记录 8fe2f33/b9f77aa（S01-05 高保真稿已按新基线实施） |
| `owner-<code>` 合成 principal 兼容路径 | T-022：真实调用者写 owner | `bootstrap.rs:118` |
| 1 小时进程清理窗口 | 持久化清理账本（无时间窗） | 0035；I1 §9.4 |

### C-11 · IMPLEMENTATION_GAP — S01-06/S01-07 正式需求缺失

| 项 | 内容 |
|---|---|
| 涉及 | S01-FR-085 / S01-FR-086 |
| 说明 | S01-06（Codex/MCP Bootstrap）与 S01-07（初始化完成与交接）在 S01-01 总览稿编号表中登记「未建」；S01-06 实质由 Codex Bootstrap V1.3（C138~C172，已过稿）承载，S01-07 无稿（仅 S14 附 A-2 旁证既有行为） |
| 处理 | 按 S01-QA 任务书原则：记 DOCUMENT_MISSING，标 NOT VERIFIED，不自行猜测功能要求。S01-06 相关运行时验证仅在 S27 验收矩阵范围内进行 |

## 2. 裁决与实现一致性核对结论

| 检查项 | 结论 |
|---|---|
| AI-PM 独立可用（裁决 1） | 一致：实现不依赖仓库/MCP/外部 Agent |
| doc_key=project-goal（裁决 2） | 一致：`project_goal.rs:22` + 唯一约束 |
| 六态仅读模型（裁决 3） | 一致：DB 无六态列，`command_center.rs` 派生 |
| Finding 双源初态（裁决 4/5） | 一致：FindingCreator 双通道（G-23） |
| 多仓库/无 Primary（裁决 6） | 一致 |
| 变更检测须确认（裁决 7） | 一致：0038 双列分离 + update-version 端点 |
| Archive 规则（裁决 8/9） | 一致：I1 §17 十四条裁决已实施（V1.1/V1.2 回填，偏差 V-3/V-4 闭合） |
| 创建后进入初始化（裁决 10） | 语义一致、落地页表述演进（C-01，R1 已按审查决定收敛） |
| Dashboard 优先级（裁决 11） | 一致：I5-026/027 已验证 |
| 不引入 Workspace（裁决 12） | 一致：Personal Home 定名已实施 |
| Git 不拆权限层级（裁决 13） | 一致：0033 撤表，简化模型落地 |

## 3. Phase 1 审查收编记录（2026-11-06，R1 修订）

> 审查结论：**Conditional Acceptance（有条件接受）**。以下 8 项进入 Phase 2 前的必要修订已全部执行，RTM 升级为 R1 版。

| # | 审查修订要求 | 执行结果 | 落点 |
|---|---|---|---|
| 1 | 修正 TRACEABLE 统计（按明细 86，非 82） | ✅ 已修正并重算：FR-006 收敛 + DATA-020 补验收 + UX-012 补权威解析后 **TRACEABLE=87** | 01-RTM 统计摘要；04 §1/§5 |
| 2 | 重新核算 P0（按明细 29，非 24） | ✅ 逐条重数 **P0=29**（明细清单见 RTM 统计摘要）；类型分布同步修正（FR22/BR38/SEC11/DATA9/UX6/ARCH3/ADR2/NFR1） | 01-RTM 统计摘要；04 §3/§4 |
| 3 | 处理 FR-006 与 UX-012 的冲突 | ✅ 拆解为页面导航要求（FR-006）与初始化状态展示要求（UX-012）；按已确定产品方向收敛（见 §1 C-01）；FR-006「初始化记录」改「初始化读模型可查询」 | 01-RTM M1；C-01 |
| 4 | 明确 UX-057 待裁决状态 | ✅ 列为 P2 待裁决功能，不阻塞 S01 核心业务验收，产品负责人裁决后进入 UI 验收；QA 不自行选稿 | 01-RTM M5；C-05 |
| 5 | 补充 DATA-020 验收条件 | ✅ 补入六步可执行场景（含「相同项目相同领域数据、不得只验证页面可打开」约束） | 01-RTM M2；C-07 |
| 6 | 明确 ARCH-090 Platform 架构边界 | ✅ 按审查表述修订：Control Plane 不重复建设外部工程执行 Runtime；内部 AI 管理能力仍属 Platform；统一 Task/Run/Delivery 协议协作；角色/调度/Runtime 三概念不得混同 | 01-RTM M9 |
| 7 | 增加 Evidence Level | ✅ 新增 E0~E5 六级证据强度列，92 条逐条标注（E0=3/E1=5/E2=28/E3=15/E4=41/E5=0）；E4 注明「开发交付测试记录≠独立 QA 验收结论」 | 01-RTM §0.1 + 各表 EV 列 |
| 8 | 补充 A1~A6 权威等级定义 | ✅ 权威等级定义表前移至 RTM 头部 §0 | 01-RTM §0 |

**配套质量检查（审查 §五）**：来源完整性（来源编号经 02-index 解析版本与日期）✅ / 实现证据补强（EV 分级 + 关键条目补测试名）✅ / 权威等级解释 ✅ / TC ID 留空待 Phase 2 且需求 ID 不变 ✅。模块划分 M1~M9 保留，92 条未重组 ✅。

**架构关系澄清（审查 §三）**：① S01-ARCH-090 与 V2 裁决关系已精确化（上表 #6）；② S01-BR-011 与 S01-ADR-041 建立关联注记（同一红线：BR=创建后业务可用性 / ADR=平台架构约束），Phase 2 只维护一套核心测试场景；③ S01-BR-044 与 S01-BR-047 建立关联注记（共享知识治理原则、分别测试，Goal 提案通过不代表通用提案流程通过）。
## 4. Phase 1 第二轮审查收编记录（2026-11-06，R2 修订）

> 审查结论：**Conditional Acceptance · 第二轮修订**。8 项小范围修订已全部执行，RTM 升级为 R2 版。

| # | 审查项 | 审查意见 | R2 执行结果 |
|---|---|---|---|
| C-06 | S01-BR-003 实现证据不足 | 「无修改端点」只能证明无修改入口，不能证明受控修改和审计机制已实现 | ✅ 改 **IMPLEMENTATION_NOT_FOUND**；验收条件拆 4 条（① Name 变 Key 不变 ② 未授权路径不可改 ③ 如提供修改能力须授权+审计 ④ 如永久不可修改须改写需求并冻结）；证据限定「前端守卫仅覆盖①；产品规则（受控修改 vs 永久不可修改）未确认」 |
| C-07 | S01-DATA-020 不能凭「无会话态」认定 TRACEABLE | 验收条件完整 ≠ 实现证据充分，两维度不可互相替代 | ✅ 改 **IMPLEMENTATION_NOT_FOUND**；保留六步验收条件；EV=E0 保留；证据改写「无会话态=设计依据非一致性证明 + 无刷新/重登测试记录」，补数据来源与查询链（command_center 每请求实时查询领域表） |
| C-08 | S01-BR-026 E1 证据不足 | CHECK 约束只能证明状态值存在，不能证明完整行为 | ✅ 核查发现确认门禁行为代码：`pm_baseline.rs:5`（无 proposed 条目才可确认）、`:246-247`（defer/investigate 决策映射）、`:293`（deferred/investigation 作为 documented uncertainties 保留）→ EV 升 E2；证据限定「确认不阻断语义有业务代码依据；展示层不误导消费者的行为无测试证据，列 Phase 2/3 验证」 |
| C-09 | S01-UX-012 Authority Resolution 不严谨 | 用户对「进入初始化工作入口」的裁决不构成对全部面板视觉行为的正式批准 | ✅ 两层拆分：L1 产品规则（A1，由 FR-006 承载）/ L2 具体 UI 行为（A4 待冻结，确认前不作为已批准产品需求）；权威等级表新增 A4 使用约束 |
| C-10 | 部分 E4 标记需重新核实 | 不得因整套测试全绿就把关联需求标 E4 | ✅ 全量核定：补用例名（SEC-010/035/054/091、DATA-037、SEC-089、FR-045/046/047、FR-069/072、BR-070/074/076/079/080/082/083/084、FR-018/UX-027/028、FR-043、FR-053、UX-056 等）；**UX-052 降 E2**（651 全绿无登录落地断言）；**BR-073/081 降 E3**（record_termination/lease 清理专项用例名未核定）；**FR-021 升 E4**（`proposal_decision_and_confirm_flow`） |
| — | A4 使用约束 | A4 不能单独支持「正式产品规则已批准」 | ✅ RTM §0 权威等级表追加约束行（涉及产品行为的最终验收必须追溯到 A1/A2/A3） |
| — | ARCH-090 E2 含义 | 保留 TRACEABLE，E2=部分架构实现证据 | ✅ 实现证据列加注 |
| C-11 | UX-057 优先级自相矛盾（正文 P2 vs 统计 P3） | 统一 P3 | ✅ 统一为 P3（04 §5 的 P2 表述一并修正） |
| C-12 | FR-006/UX-012 实现证据混同 | 两条拆分后证据仍指向同一不完整链 | ✅ FR-006 补真实路由跳转证据（`ProjectCreateView.vue:118-126`，`router.push("/project/command-center")` + Q3 注释「创建成功即工作——直接进入项目总控台」）；UX-012 补初始化区域组件证据（`ai-frontend/src/views/project/command-center/`） |
| — | TRACEABLE 统计 | 按当前明细应为 89（R1 写 87 系加法起点错误），总数须=92 | ✅ R1 明细实数 89 确认；R2 经 BR-003/DATA-020 转列后 **TRACEABLE=87**，87+2(INF)+1(CONFLICT)+2(DM)=**92** ✅ 交叉校验闭合 |

**复审决定落实**：92 条需求及模块划分接受 / A1~A6 基本接受（+A4 约束）/ E0~E5 接受（部分条目重新核定）/ FR-006·UX-012 拆解接受（补证据）/ UX-057 接受（统一 P3）/ DATA-020 接受（状态重核）/ ARCH-090 接受 / P0=29 接受 / TRACEABLE 重新核算闭合。**Phase 1 待复审确认后正式冻结；Phase 2 按审查给出的风险优先级（P0-1 隔离 → P0-7 基线 → P1 → P2/P3）启动测试策略设计，每个 Test Case 反向追溯需求 ID，安全测试含正向/反向/越权/并发/重放场景。**
## 5. R2.1 终校记录（2026-11-06，第三轮修订指令）

> 本节仅追加 R2.1 本轮变化（指令范围：只修正证据等级、需求状态和追踪记录中的不一致；不重新设计需求、不扩大范围、不修改裁决）。

| # | 指令项 | R2 状态 | R2.1 变更 | 依据 |
|---|---|---|---|---|
| 1 | DATA-020 证据等级与实现状态 | EV=E0、IMPLEMENTATION_NOT_FOUND | **EV→E2；状态→IMPLEMENTATION_GAP**。保留六步验收；区分「读取路径存在」（`command_center.rs` 每请求实时查询领域表=业务代码证据）与「一致性已验证」（无任何刷新/重登测试）；Phase 3 必须独立验证刷新、退出登录、重新登录及重新打开项目后的 InitItem 一致性。**不因缺 E5 认定实现不存在，也不因读取路径存在认定一致性已验证** | 审查指令 §1；证据核查 `command_center.rs` |
| 2 | BR-003 产品裁决与实现核验分离 | IMPLEMENTATION_NOT_FOUND（合并表述） | **保持 IMPLEMENTATION_NOT_FOUND + 新增 ⚑ DECISION REQUIRED 标记**；拆分两个独立事项：① 产品裁决（Key 允许受控修改 vs 永久不可修改——待产品负责人，QA 不自行决定，裁决后确定终版验收条件）② 实现核验（前端守卫覆盖 Name→Key 不变；受控修改+审计机制无实现证据，缺口已登记） | 审查指令 §2 |
| 3 | E4 全量重新核查 | 40 条（统计有误） | **核定 41 条**（R2 明细实数 43−UX-027−SEC-054+0）；重点 4 条：UX-027 **E4→E2**（无首屏顺序断言用例）、SEC-054 **E4→E2**（Personal Home 列表过滤无直接断言）、FR-053 **E4 维持**（断言 `push("/project/create")` 实测确认）、FR-021 **E4 维持**（`proposal_decision_and_confirm_flow`）；BR-017 经断言行号确证（`command_center.rs:146/:163`）。全量 41 条核定明细落 `04-evidence-level-audit.md` | 审查指令 §3 |
| 4 | FR-085 重新核定 E3 | EV=E3（含 C166 跨机实测作依据） | **EV→E2**：C166 跨机实测为**人工测试**（T-090 项），不计自动化证据；实际自动化证据=`onboarding.rs::health_is_public_and_me_is_not` + `pm_devices.rs` 设备层用例（部分覆盖，未与 S01-06 需求逐条映射）；DOCUMENT_MISSING 维持（**实现存在不等于正式需求已完整**） | 审查指令 §4 |

**R2.1 统计口径（全部由明细逐条汇总并交叉核对）**：
- 状态：TRACEABLE=**87** ＋ IMPLEMENTATION_NOT_FOUND=**1**（BR-003 ⚑）＋ IMPLEMENTATION_GAP=**1**（DATA-020）＋ REQUIREMENT_CONFLICT=**1**（UX-057，P3）＋ DOCUMENT_MISSING=**2** ＝ **92** ✅
- EV：E0=**3** ＋ E1=**4** ＋ E2=**31** ＋ E3=**13** ＋ E4=**41** ＋ E5=**0** ＝ **92** ✅
- P0=29 / P1=53 / P2=9 / P3=1 维持；模块与类型分布维持。
- **统计纪律**：本轮所有统计数字均由明细清单直接汇总（状态清单与 EV 清单见 RTM v2.1 统计摘要与 `04-evidence-level-audit.md` §5），无手工填写数字。

**R2.1 禁止事项遵守确认**：未新增未经批准的产品需求（92 条不变）／未修改 A1/A2/A3 正式裁决／未将 A4 设计建议提升为正式需求（UX-012 L2 维持 A4 待冻结）／未把代码存在视为需求验收通过（DATA-020 只升 EV 不改「未验证」定性、FR-085 维持 DOCUMENT_MISSING）／未提前编写 Phase 2 测试用例／未将 Phase 3 验收结果提前填入 Phase 1（全部 E5=0）。

**RTM 权威文件变更**：本轮起 `01-requirement-traceability-matrix-v2.1.md` 为现行版；`01-requirement-traceability-matrix.md`（R2）保留作修订留痕。