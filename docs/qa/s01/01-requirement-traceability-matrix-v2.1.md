# S01 Requirement Traceability Matrix（01-requirement-traceability-matrix-v2.1）R2.1

> Phase 1 交付 · 2026-11-06 · QA Test Architect · **R2.1 终校版**（基于 R2，按第三轮修订指令完成 4 项校正；Phase 1 提交产品负责人最终审查）
> **R2.1 变更摘要**：① DATA-020 EV E0→**E2**（`command_center.rs` 每请求实时查询领域表=业务代码证据；不得因缺 E5 认定实现不存在）、状态 IMPLEMENTATION_NOT_FOUND→**IMPLEMENTATION_GAP**（部分实现依据存在，完整行为未验证）；② BR-003 增 **Decision Required** 标记，产品裁决与实现核验拆分；③ E4 重新核查——**UX-027 降 E2**（无首屏顺序断言用例）、**SEC-054 降 E2**（Personal Home 列表过滤无直接断言）、其余 E4 逐条核定（41 条，见 `04-evidence-level-audit.md`）；④ FR-085 EV E3→**E2**（C166 跨机实测为人工测试，不计自动化证据；`/health`+设备层自动化测试为部分覆盖），DOCUMENT_MISSING 维持。
> **TRACEABLE 只表示需求、实现与证据之间存在可追溯关系，不代表已通过验收**；验收条件完整 ≠ 实现证据充分（两维度不可互相替代）。
> 来源文件编号（S1~S31）解析到版本与日期见 `02-requirement-source-index.md` §2。
> 验证方法缩写：UI / API / DB / INT(Integration) / SEC。

## 0. 权威等级（Authority Level）定义

| 等级 | 名称 | 判定规则 |
|---|---|---|
| **A1** | 用户正式裁决 | 用户明示选择的方案/原话（含收编进设计稿的 Q/R 编号裁决） |
| **A2** | 正式实施基线 | `docs/s01-implementation-baseline-v1.md` 及六份 Registry 体系 |
| **A3** | 已过稿/已收编裁决的专项设计稿 | I1~I6 设计稿、最终基线 V1.2、已收编裁决的 UX 稿 |
| **A4** | 已记录待冻结的专家供稿/修订记录 | `docs/ui/s01/` 设计修订五份（2026-11-05 记录，待基线确认） |
| **A5** | 裁决记录（只记录未评审） | R-S01-01-01 等 |
| **A6** | 代码/迁移事实 | 只作 Implementation Evidence，不自动成为产品需求 |
| **H** | 历史方案（已被取代） | 保留作决策留痕，不得作为当前需求 |

> **A4 使用约束（R2）**：A4 仅作为设计建议和待冻结实施依据。涉及产品行为的最终验收，必须追溯到 A1/A2/A3 中已批准的规则；若某产品行为仅有 A4 来源而无 A1/A2/A3 对应权威，该行为标记待裁决，不得按已批准需求验收。

## 0.1 Evidence Level（证据强度等级）定义

| 等级 | 定义 |
|---|---|
| **E0** | 只有需求或设计文档，无实现证据 |
| **E1** | 数据库 Schema / Migration 证据 |
| **E2** | 业务代码 / API / Endpoint 证据 |
| **E3** | 对应自动化测试存在，但未核对断言映射或用例名 |
| **E4** | 有对应测试用例名称 + 开发测试通过记录（验收矩阵/设计稿回填为正式载体） |
| **E5** | 独立 QA 实例完成端到端验证（Phase 3 产出，当前为 0） |

> 纯数据库约束可由 E4 集成测试提供充分证据；不得因整套测试全绿就把关联需求标 E4。
> **E4 是开发交付记录，不等于本次独立 QA 验收结论**；**人工实测（如 C166 跨机实测）不计入自动化证据等级**（R2.1 核定规则）。
> E4/E3 全量核定明细见 `04-evidence-level-audit.md`。

---

## M1 · Project Creation（S01-02）

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-FR-001 | FR | 项目创建 Name 必填；Description/Type 可选 | S13 §3.1/§3.4 | A3 | P1 | API | 缺 Name/Key 返回 400 且零副作用 | `pm_lists.rs:131-144`（code/name 空值 400） | E2 | | TRACEABLE |
| S01-FR-002 | FR | Project Key 由系统建议、人类确认，提交时校验唯一 | S13 §3.2（Q1=A 用户裁决） | **A1** | P1 | UI+API | 输入 Name 后 Key 自动建议且可修改；重复 Key 被拒 | `ProjectCreateView.vue:58,68,81`（suggestKey+手动编辑保护）；`0001_core.sql:21`（code UNIQUE） | E2 | | TRACEABLE |
| S01-BR-003 | BR | Key 创建后不随 Name 自动变化；后续如允许修改，必须通过受控操作并留审计 ⚑ **DECISION REQUIRED（R2.1）** | S13 §3.2 规则段 | A3 | P2 | API+DB | **待产品负责人裁决后确定终版验收**。裁决分支 A（允许受控修改）：① 修改 Name 不改变 Key ② 未授权路径不能修改 Key ③ 修改必须经授权操作并留审计。裁决分支 B（永久不可修改）：需求改写为「创建后不可修改」并冻结。**两分支均待裁决，QA 不自行决定** | **实现核验（独立于产品裁决）**：① 前端 `keyEditedManually` 守卫（覆盖「Name 变 Key 不变」）✓；② 未发现 Key 修改端点（仅证明当前无修改入口，**不能证明受控修改与审计机制已实现**）；③ 「受控修改+审计」实现不存在或未被发现——**两项独立事项：产品裁决（待）＋实现核验（缺口已登记）** | E2 | | IMPLEMENTATION_NOT_FOUND ⚑ DECISION REQUIRED |
| S01-FR-004 | FR | Owner 默认当前用户、可修改 | S13 §3.3（Q2=A 用户裁决） | **A1** | P1 | API+DB | 创建者真实主体写入 project_member(role='owner') | `bootstrap.rs:118-119`（T-022 真实调用者占 owner 槽）；`tests/project_members.rs` 覆盖成员关系 | E3 | | TRACEABLE |
| S01-SEC-005 | SEC | Creator 与 Owner 分别记录；AI 不得指定 Owner、Agent 不得任 Owner | S13 §3.3；S14 §3.2 | A3 | P1 | SEC | 非 human 主体不能成为 owner；AI 建议与用户提交可区分 | `bootstrap.rs:16-18,282-286`（5 成员编制 ai_pm→ai_pm 非 owner） | E2 | | TRACEABLE |
| S01-FR-006 | FR | **页面导航要求**：创建成功后立即进入项目初始化工作入口（Command Center，初始化区域可见）；「初始化记录建立」= 初始化读模型可查询（InitItem 为派生读模型、非数据库实体）；不强制初始化完成（ready）才允许进入 | S13 §7（Q3=A）+ S14 §2.3/§四 + S18 §3.5；C-01 收敛口径 | **A1** | P1 | UI+API | 创建成功响应含 Project ID/Owner；真实路由跳转至 Command Center；ready 前后均可进入 | `ProjectCreateView.vue:118-126`——`router.push({ path: "/project/command-center", query: { project_id } })` + 注释「Q3：创建成功即工作——直接进入项目总控台，无成功停留页、无初始化等待」；`pm_lists.rs:131-`（BootstrapRequest） | E2 | | TRACEABLE |
| S01-BR-007 | BR | 自动进入≠自动执行：仓库连接/AI 分析/基线确认均需人工发起或授权 | S13 §7.2 | A3 | **P0** | INT | 创建后无未经授权的仓库访问、AI 分析或基线自动确认 | 0036 `started_by NOT NULL`；0037 `created_by NOT NULL`；无创建后自动链 | E2 | | TRACEABLE |
| S01-FR-008 | FR | 创建失败明确显示原因并保留已填信息 | S13 §7.3 | A3 | P2 | UI | 失败响应带业务原因；表单值不丢 | `pm_lists.rs:138-143`（400 message） | E2 | | TRACEABLE |
| S01-BR-009 | BR | Project Key 全局唯一，重复提交不产生重复项目 | S13 §3.2 步骤3/5 | A3 | P1 | DB | 同 Key 二次创建被唯一约束拒绝 | `0001_core.sql:21`（code UNIQUE） | E1 | | TRACEABLE |
| S01-SEC-010 | SEC | 项目访问权限按 project_member 过滤；非成员 404 不泄露存在性 | S25；S2 §5.1 | A2 | **P0** | SEC | 非成员列表不可见/详情 404 | `tests/project_members.rs` **15 例全绿**：`grant_makes_member_able_to_see_project` / `outsider_cannot_see_project_members` / `cross_tenant_project_is_not_found` 等 | E4 | | TRACEABLE |
| S01-BR-011 | BR | 项目创建后内置 AI-PM 立即可用，不要求先配置 MCP/Codex/外部 Agent（**关联 S01-ADR-041**：同一红线，BR=创建后业务可用性，ADR=架构约束；Phase 2 共用一套核心场景） | S20 §一/§三（R-S01-01-01 方案A） | A5 | **P0** | INT | 刚创建项目（零仓库零设备）即可用需求管理/AI 分析/知识整理/人工决策 | 认知/知识/需求端点均不依赖仓库（`pm_cognition.rs`/`pm_requirements.rs`）；V2.0 基线 S28 红线 | E2 | | TRACEABLE |
| S01-UX-012 | UX | **初始化状态展示要求**（两层）：**L1 产品规则（A1）**=创建后进入项目初始化工作入口、不得要求全部初始化完成后才能开始项目工作（S01-FR-006 承载）；**L2 具体 UI 行为（A4，待冻结）**=默认紧凑摘要、完成后收缩、新待办重新展开、具体页面跳转方式——产品负责人确认 A4 设计稿前，L2 不作为已正式批准的产品需求 | S14 §2.3/§四 + S18 §3.5（A4）；实施记录（S14 附 A-2，旁证） | A4 | P2 | UI | L2 行为与 S14/S18 口径一致；无 ready 门禁；收缩/展开正确；最终验收待产品负责人确认 A4 后执行 | 初始化区域组件：`ai-frontend/src/views/project/command-center/`（六大区块渲染有组件测试：`CommandCenterView.test.ts::注册路由后渲染六大区块与设计稿样本数据`）；收缩/展开 API 级断言见 acceptance 用例 | E2 | | TRACEABLE（L2 按 A4 待冻结处理） |

> **UX-012 Authority Resolution（两层）**：L1 产品规则 = 原始 Q3=A（S13 §13，A1）+ C-01 收敛口径（不强制 ready）→ S01-FR-006 承载；L2 具体 UI 行为 = S14 V1.1 / S18 V1.1 §3.5（A4，待冻结）→ 本条目承载。用户对「创建后进入初始化工作入口」的裁决不构成对全部初始化面板视觉行为的正式批准。

## M2 · Project Baseline & Initialization（S01-01 / S01-05 / I5 / Command Center）

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-ARCH-013 | ARCH | InitItem 是初始化事项的展示读模型，不是数据库实体，由七个领域实体状态派生 | S18 §4.1；S11 §4.2 | A4+A3 | **P0** | DB+API | 数据库无 InitItem/六态持久化列；读模型派生 | 迁移 0030~0038 无六态列；`command_center.rs` 派生查询 | E2 | | TRACEABLE |
| S01-BR-014 | BR | 六态（not_started/in_progress/completed/blocked/not_applicable/undetermined）仅为展示状态，不得写入领域实体、不得反向计算 | S18 §4.1/§4.2 | A4 | **P0** | API+DB | 展示层状态与领域原始状态（source_state）并存；无写回路径 | `command_center.rs`（status=展示层 + source_state 保留，契约 C-6） | E2 | | TRACEABLE |
| S01-FR-015 | FR | 七事项（project_basics/project_goal/repository_access/code_analysis/project_cognition/dev_environment/human_actions）各有可执行派生判定规则 | S11 §4.2 表 | A3 | P1 | API | 每事项按判定表产出状态且可追溯 source | `command_center.rs` 七判定实现；契约 §4.2；`tests/command_center.rs::cognition_without_pending_work_keeps_six_state_semantics` 等 | E3 | | TRACEABLE |
| S01-BR-016 | BR | 必需项仅三项（basics/goal/human_actions）；仓库/分析/认知/环境 required=false | S11 §4.2（D-C-06） | A2 | P1 | API | required 字段与裁定一致 | `command_center.rs` required 常量 | E2 | | TRACEABLE |
| S01-BR-017 | BR | 不产出初始化完成百分比；完成度输出 not_determinable，不宣称初始化已完成 | S11 C-7 + D-C-06；I5-022/029 | A2 | P1 | API+UI | 响应无 percent 字段；completion=not_determinable | `tests/command_center.rs::new_project_renders_all_blocks_and_zero_repository_is_legal` 内断言 `:146`（completion=not_determinable）+ `:163`（`assert!(!raw.contains("percent"))`） | E4 | | TRACEABLE |
| S01-FR-018 | FR | 收缩规则：存在必需事项未完成或任一阻塞 → expanded=true；否则收缩；收缩不隐藏阻塞/待办 | S11 §4.2 收缩规则；I5-027/028 | A3 | P2 | API | expanded 布尔正确翻转 | `command_center_acceptance.rs::initialization_progress_collapses_and_expands_again` | E4 | | TRACEABLE |
| S01-BR-019 | BR | 初始化状态以**最新**有效作业判定（成功后再次失败→阻塞；不看历史成功） | I5-006；S11 §4.2 | A3 | P1 | INT | 最新 attempt 口径 | 契约 §4.2（最新作业口径）+ `command_center.rs` | E2 | | TRACEABLE |
| S01-DATA-020 | DATA | 页面刷新、重新登录后初始化状态保持一致 | 任务书 S01-01 检查项；S22；R1 补验收 | A3 | P1 | UI | **六步验收条件（保留）**：① 创建项目并完成部分初始化事项；② 刷新页面重新读取初始化状态；③ 退出登录后重新登录；④ 再次打开同一项目；⑤ 逐项对比各 InitItem 的状态、required、source_state、expanded；⑥ 确认状态未因浏览器会话或前端临时缓存而改变。全程相同项目与相同领域数据，不得只验证页面可打开。**Phase 3 必须独立验证刷新、退出登录、重新登录及重新打开项目后的 InitItem 一致性** | **R2.1 改判**：读取路径业务代码证据成立——`command_center.rs` 每请求实时查询领域表（无会话态、无缓存层）→ EV=E2；**「读取路径存在」≠「刷新/重登一致性已验证」**——无任何刷新/重登一致性的测试记录 → 完整行为未验证 | **E2** | | **IMPLEMENTATION_GAP**（部分实现依据存在；完整行为 Phase 3 独立实测） |
| S01-FR-021 | FR | Project Baseline：AI 提案→人类逐项确认→形成版本不可变基线 | 0037 迁移头注；S19 | A3 | P1 | API+DB | 提案/条目/版本三实体生命周期可复现 | 0037 三表；`pm_baseline.rs` 4 端点；`tests/pm_baseline.rs::proposal_decision_and_confirm_flow` | E4 | | TRACEABLE |
| S01-BR-022 | BR | 同一项目同时只允许一个待确认基线提案 | 0037（`uq_baseline_proposal_pending`） | A6+A3 | P1 | DB | 第二个 pending 提案被唯一索引拒绝 | 0037 部分唯一索引 | E1 | | TRACEABLE |
| S01-ADR-023 | ADR | R-BL-01~06 六裁决：Requirement 只引用 Revision(B)/未决允许且标记(A)/无 Repository 可确认(A)/Cognition 不完整可确认(A)/完整逻辑快照(A)/确认权限与创建者分离(B) | S19 §2 六项正式用户裁决 | **A1** | P1 | API+DB | 六项逐一验证 | `pm_baseline.rs`；0037（confirmed_by 与 created_by 分列） | E2 | | TRACEABLE |
| S01-BR-024 | BR | 基线版本不可覆盖、当前有效基线唯一、正式确认必须由人类完成 | S17 README 冻结原则 1~6；S19 | A3 | P1 | DB | confirmed 后条目不可变；无 AI 确认路径 | 0037（confirmed 随即不可变；decided_by human 校验） | E2 | | TRACEABLE |
| S01-BR-025 | BR | 已完成事项正确保留；未完成事项可继续处理；不适用事项不阻塞初始化 | 任务书 §2 S01-01；S11 §4.2/§5 | A3 | P1 | API | 不适用事项 required=false 且不计入未完成 | `tests/command_center.rs::new_project_renders_all_blocks_and_zero_repository_is_legal`（I5-023：`code_analysis=not_applicable` 且 required=false） | E4 | | TRACEABLE |
| S01-BR-026 | BR | 基线允许未决事项且明确标记（blocking 更正/Unknown=documented uncertainties），不自动阻断 | S19 R-BL-02=A；S19 附 A-3 | **A1** | P2 | API | 未决条目保留 investigation/deferred 态且可见；不误导为已确认、不阻断基线确认（**展示层完整行为列 Phase 2/3 验证**） | 0037 `baseline_item.state CHECK`（状态值定义）+ 确认门禁行为代码 `pm_baseline.rs:5`、`:246-247`、`:293`（确认不阻断语义有业务代码依据；展示层行为无测试证据） | E2 | | TRACEABLE（完整行为 Phase 2/3 验证） |
| S01-UX-027 | UX | 首屏优先级：Needs Attention（待人工处理）优先于统计卡片与初始化进度 | S18 §3.2；S10 §3；I5-026 | A4+A3 | P1 | UI | 块顺序断言 | **R2.1 降级**：`CommandCenterView.test.ts` 仅 2 用例（`注册路由后渲染六大区块与设计稿样本数据` / `页头「项目设置」发出 settings 事件`）——**无「首屏优先级顺序」断言用例**；组件渲染证据存在 | E2 | | TRACEABLE（顺序断言列 Phase 3 UI 实证） |
| S01-UX-028 | UX | 初始化完成后模块自动收缩；新增待处理事项时重新展开 | S18 §3.5；I5-027/028 | A4+A3 | P2 | UI+API | 收缩/再展开复现 | `command_center_acceptance.rs::initialization_progress_collapses_and_expands_again` | E4 | | TRACEABLE |

## M3 · Repository Management（S01-03 / I3 / I6-B）

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-FR-029 | FR | 一个项目允许关联多个 Git Repository | S1 Q1（§125-173） | A3 | P1 | API | 多仓添加/列表正确 | `project_repository` 多行；`command_center.rs` repository_count | E2 | | TRACEABLE |
| S01-BR-030 | BR | 零仓库项目合法，不判失败、不阻塞项目工作 | S11 §4.2（repository_access required=false）；R-BL-03=A；I5-002 | **A1** | **P0** | API | 零仓库 → 仓库「未开始」+分析「不适用」 | `tests/command_center.rs::new_project_renders_all_blocks_and_zero_repository_is_legal` | E4 | | TRACEABLE |
| S01-FR-031 | FR | 仓库接入至少验证 URL/连通性/认证/读权限/Clone 五项；连接检查失败与分析失败必须区分 | S1 §6.1 | A3 | P1 | API | 认证失败≠分析失败；URL 解析成功≠接入成功 | `repository/` verify 流程；`integration.project_repository.status` | E2 | | TRACEABLE |
| S01-BR-032 | BR | 被历史引用的仓库不可物理删除（RESTRICT）+ 应用层 409 | 0031 第11项；G-07/G-16 | A2 | P1 | DB+API | 删除被引用仓 → DB 23503 / REST 409 | 0031 FK RESTRICT；`repository::delete_repository` HasHistory；`tests/repositories.rs` | E3 | | TRACEABLE |
| S01-BR-033 | BR | Confirmed Commit（selected_commit）只能经 `POST /repositories/{id}/update-version` 由人类确认写入；PATCH 不得触碰 | 0038 迁移头注；S15 修订稿 R-03-04 | A3+A4 | P1 | API+DB | PATCH 改 selected_* 被拒；确认历史 append-only | 0038 三列 + `repository_version_confirmation` 表 + 独立端点 | E2 | | TRACEABLE |
| S01-FR-034 | FR | 变更检测：Observed HEAD 由 verify 自动回写，检测到变化须请求用户确认后才写入 selected_commit | 0038 注释（§5.1）；S15 修订稿 | A3+A4 | P1 | API | HEAD 变化→提示确认；未确认不落地 | 0038 `head_commit_sha` vs `selected_commit` 分离 | E2 | | TRACEABLE |
| S01-SEC-035 | SEC | 仓库访问边界 = Task 关联仓 ∩ 仓库状态 verified；范围外/失验/归档/取消一律拒绝 | S8 §0.0.2 判定链 | A3 | **P0** | SEC+INT | 五级判定链逐一拒绝且零副作用 | `tests/repo_access_enforcement.rs` **8 例全绿**：`out_of_scope_repository_is_denied` / `repository_no_longer_verified_blocks_execution` / `archived_project_blocks_execution` / `missing_snapshot_denies_everything` / `snapshot_is_immutable_after_claim` 等 + `run_scope_snapshot.rs` 4 例 | E4 | | TRACEABLE |
| S01-ARCH-036 | ARCH | 不得将 Git 六操作拆分为不同产品权限层级（用户裁决：不区分，都可读写） | S8 §0.0.1 裁决原文；D-C-03 | **A1** | **P0** | SEC | 无能力分级表/端点存在；档位恒 read_write | 0033 撤下 `git_capability_grant`；六能力枚举废止 | E1 | | TRACEABLE |
| S01-DATA-037 | DATA | 跨项目仓库串扰 DB 级防护：两条归属链跨项目不可构造 | S12 §1.1 目标3；D-2a | A2 | **P0** | DB | 跨租户/跨项目引用违反组合 FK（23503） | `tests/i6b_db_constraints.rs` **13 例全绿**：`negative_1_evidence_cannot_cross_projects` / `negative_4_finding_project_must_match_session_project` / `negative_5_finding_cannot_attach_other_project_repository` 等（直接打库断言 23503/23505/23514） | E4 | | TRACEABLE |
| S01-BR-038 | BR | 逐仓失败隔离：单仓失败不中止其余仓库；全部失败才终止导入 | G-13；S1 场景稿 §6 | A2 | P1 | INT | 一仓失败其余继续且留 failed Attempt | `run_import_job` 逐仓隔离 + `all_repositories_failed`；`tests/import_contract.rs` 7 例 | E3 | | TRACEABLE |
| S01-BR-039 | BR | 同 Project 同时仅一个 active 导入；失败/取消释放名额，不卡死 | 0031；G-08/G-12 | A2 | P1 | DB | 并发导入唯一赢家；失败后可再发起 | G-12 修复（`fail_import` 同事务释放） | E2 | | TRACEABLE |
| S01-BR-040 | BR | 初始无 Primary Repository 可接受；`project_repository.role` 是类型标签非权限 | S1 Q1 实施约束；S4 3.2 | A3 | P2 | DB | role 值域不参与授权 | `0008_repositories.sql:23-24` | E1 | | TRACEABLE |

## M4 · AI-PM Cognition（S01-04 / I4 / R-S01-01-01）

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-ADR-041 | ADR | Native AI-PM 独立可用：不要求 Repository/MCP/Codex/Claude Code/外部 Agent；三层能力模型（**关联 S01-BR-011**） | S20 §一/§二/§三；S28 红线 | A5+A1 | **P0** | INT | 零外部接入下需求收集/分析/知识整理/规划辅助/决策支持可用 | `pm_cognition.rs`/`pm_requirements.rs`/知识端点零仓库依赖 | E2 | | TRACEABLE |
| S01-SEC-042 | SEC | 认知 Run 由人类启动，AI 不得自行启动 | 0036 注释（Q2-C）；S16 | A1+A3 | P1 | API+SEC | started_by 必填且为真人；AI 调用被拒 | 0036 `started_by NOT NULL`；`pm_cognition.rs` POST 在成员人类层之下 | E2 | | TRACEABLE |
| S01-FR-043 | FR | 认知条目按六领域产出并逐项人工确认 | 0036（domain CHECK）；S16 Q3-B | A3 | P1 | API | 每条独立确认/拒绝；AI 无决策入口 | 0036 domain CHECK；`pm_cognition.rs:287`；`tests/pm_cognition.rs::start_run_derives_items_and_confirm_produces_knowledge` / `reject_keeps_history_and_edit_confirm_updates_content` | E4 | | TRACEABLE |
| S01-BR-044 | BR | AI 建议/分析不得未经授权直接成为正式项目事实（**通用知识提案红线**；**关联 S01-BR-047**：共享治理原则、分别测试、不可互相替代） | S1 §1.2；S7 §12；S18 §3.2 | A3 | **P0** | INT | 未确认提案不进入正式知识/Goal/事实消费 | 知识提案 8 态管线；`discovery_proposal` 4 态审核 | E2 | | TRACEABLE |
| S01-FR-045 | FR | Project Goal 以知识对象承载：doc_key=project-goal；不新增 core.project 字段 | S1 Q3；I4 I4-D01/D02 | A3 | P1 | API+DB | Goal 创建/读取/更新走知识管线 | `project_goal.rs:8,22`；`command_center.rs:589`；`tests/project_goal.rs::goal_is_created_with_project_and_is_unique` | E4 | | TRACEABLE |
| S01-BR-046 | BR | 每 Project 唯一正式 Goal | I4 I4-D03；S11 §4.1 | A3 | P1 | DB | 重复 Goal 被唯一约束拒绝 | `ai.project_document` UNIQUE(project_id, doc_key)；`tests/project_goal.rs::goal_is_created_with_project_and_is_unique` | E4 | | TRACEABLE |
| S01-BR-047 | BR | Agent 的 Goal 提案强制红线挂起，不得自动发布（**Goal 特殊红线**；关联 S01-BR-044） | G-26（Q2）；I4 I4-D05 | A3 | **P0** | INT | Goal 提案只挂起不发布 | `submit_proposal` Goal 强制红线；`tests/project_goal.rs::goal_modification_requires_proposal_and_human_confirmation` | E4 | | TRACEABLE |
| S01-BR-048 | BR | 归档后 Goal 只读：四写入入口统一检查 active | I4 I4-D10；I4-F；G-26 | A3 | P1 | INT | 归档后四入口拒绝且与归档线性化 | `tests/project_goal_archive.rs` **5 例全绿**（归档拒提案/归档拒确认/历史可读/并发三轮无新 published/被拒写入零发布审计） | E4 | | TRACEABLE |
| S01-DATA-049 | DATA | Agent 上下文项目隔离：正式 Goal 是默认权威上下文；未确认提案不得污染；Run 记录所用 Goal 版本 | I4 §8；I4-E | A3 | P1 | INT | 上下文读取仅本项目 published Goal；RunContract.goal 快照 | `project_goal.rs`（load_official_goal）；RunContract.goal 快照（G-26） | E3 | | TRACEABLE |
| S01-BR-050 | BR | speculation 必须经授权人类确认才能升级 user_confirmed；Execution Agent 不得自行提升 | S1 Q5（F3 七条） | A3 | **P0** | SEC | Agent 确认被拒；确认动作有授权+审计 | I2 决定服务真人门；`tests/discovery.rs`（I2-025 场景） | E4 | | TRACEABLE |
| S01-BR-051 | BR | 已确认信息与 AI 推测信息在展示上明确区分 | S16；S1 Q5；S7 §12 | A3 | P1 | API+UI | kind/verification_status 双维度保留原始值 | `DiscoveryFindingView.display_state` + source_state（C-6） | E3 | | TRACEABLE |

## M5 · Personal Workspace / Personal Home（S01-05）

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-UX-052 | UX | 用户登录后默认落地个人工作入口 `/personal`（Personal Home） | S17 高保真稿 §13（已实施） | A3 | P1 | UI | 登录后落在 /personal | `PersonalHomeView.vue` + 路由配置（代码级）；`PersonalHomeView.test.ts` 7 用例**均无登录落地断言**——651 全绿不等于落地行为已验证 | E2 | | TRACEABLE（落地断言列 Phase 3 UI 实证） |
| S01-FR-053 | FR | Personal Home 提供项目快捷入口与 Create New Project 快捷操作 | S17；`PersonalHomeView.vue:263,282` | A3 | P1 | UI | 快捷入口可点且落到真实功能 | `createProject()` 实现；`PersonalHomeView.test.ts::「新建项目」进入 S01-02 项目创建页`（断言 `push("/project/create")`） | E4 | | TRACEABLE |
| S01-SEC-054 | SEC | 只展示用户可访问的项目（按角色/权限过滤） | S17；S25 | A2 | **P0** | SEC | 不可见项目不出现 | **R2.1 降级**：`PersonalHomeView.test.ts` 无「不可见项目不出现」断言；证据 = 后端过滤代码（`project_member` 可见性）+ 间接用例（`project_members.rs::outsider_cannot_see_project_members`、`command_center.rs::non_member_cannot_read_command_center`）——**Personal Home 列表过滤本身无直接测试** | E2 | | TRACEABLE（直接断言列 Phase 3 实证） |
| S01-BR-055 | BR | 命名裁决：Personal Home 定名；不引入未经裁决的通用 Workspace 业务实体 | S17（2026-11-04 用户命名裁决）；S21 | **A1** | P1 | UI+API | 无 Workspace 实体/入口/术语 | `personal/` 视图命名；S01-00 取代注 | E2 | | TRACEABLE |
| S01-UX-056 | UX | 空数据、加载、异常三类状态均有明确呈现 | S17 §11；S18 §6 | A3 | P2 | UI | 三态视觉与文案齐备 | `PersonalHomeView.test.ts::整页失败时给出可重试的错误态 / 没有任何项目时显示首次登录引导，不展示概览 / 待办清空时给出平静空态而不是告警` | E4 | | TRACEABLE |
| S01-UX-057 | UX | 概览指标与 Watchlist 是否进首页：两稿口径不同，**P3 待裁决功能；不阻塞 S01 核心业务验收** | S17 README 注；审查决定 | A3 | P3 | UI | 待裁决：不阻塞核心验收；产品负责人裁决后进入 UI 验收；QA 不自行选稿 | — | E0 | | REQUIREMENT_CONFLICT（待裁决，不阻塞核心验收） |

## M6 · Finding Lifecycle（I2）

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-BR-058 | BR | 可信 human 直接提交的 user_confirmed Finding 创建即 CONFIRMED | S7 §3.4（R2 裁决）；I2-023 | **A1** | **P0** | INT | HumanDirect 创建即 CONFIRMED 且审计齐 | `finding.rs:111-113`；`tests/discovery.rs::i2_023` 全绿（G-23） | E4 | | TRACEABLE |
| S01-BR-059 | BR | Agent 转录的 Finding 必须 PENDING，即使标注 user_confirmed | S7 §3.4.3；I2-024 | **A1** | **P0** | INT | AgentRelayed 创建恒 PENDING | `finding.rs:112`；`tests/discovery.rs::i2_024` 全绿（G-23） | E4 | | TRACEABLE |
| S01-BR-060 | BR | 外部调用方不得自行指定 verification_status | S7 §11.1（S7 裁决）；I2-026 | A3 | **P0** | API | 契约无该字段；服务端定态 | G-23（契约删除字段）；`tests/discovery.rs::i2_026` 全绿 | E4 | | TRACEABLE |
| S01-FR-061 | FR | 人工决定统一入口：确认/拒绝/要求补充，单事务完成 | S7 §4；G-24；I2-014 | A3 | P1 | API | 三类决定可执行且状态正确 | `record_finding_verification` 统一决定服务；`tests/discovery.rs` 19 例（I2-014/019/020 场景） | E4 | | TRACEABLE |
| S01-SEC-062 | SEC | 可信 human 判定=服务端验证 `principal.kind=='human'`，不信任客户端声明 | S7 §3.4.1；S3 §3.2 | A3 | **P0** | SEC | Agent 伪造 human 被拒且零审计 | 决定服务真人门（I2-025 场景） | E4 | | TRACEABLE |
| S01-SEC-063 | SEC | G9/G10：确认与创建操作的跨项目越权 P0 修复 | S7 §6.3/§6.5；I2-D 铁律 | A3 | **P0** | SEC | 跨项目确认/创建被拒且零副作用 | G-24（先校验后写）；I2-D 双入口同批改造 | E4 | | TRACEABLE |
| S01-DATA-064 | DATA | operation_id 幂等：同一决定重放零副作用 | S7 §10.5；I2-025/029 | A3 | P1 | INT | 重复提交仅一次生效 | G-24（operation_id 幂等，N-4 先例） | E4 | | TRACEABLE |
| S01-DATA-065 | DATA | 「待补充」展示态从最近一次有效人工决定审计推导 | S7 §8.2（R4 裁决）；I2-033 | **A1** | P1 | API | REVISION_REQUESTED 仅由审计推导 | G-25（`discovery_finding_revision_requested` 审计 + display_state） | E4 | | TRACEABLE |
| S01-DATA-066 | DATA | 确认动作与审计记录同事务；审计失败则业务回滚 | S7 §9.5/§3.4.2；I2-030 | A3 | **P0** | DB | 审计缺失=业务回滚 | G-24（单事务统一决定服务） | E4 | | TRACEABLE |
| S01-SEC-067 | SEC | REST 与 MCP 同一领域规则；不得一个入口绕过 | S7 §13；S2 §5.2 | A2 | **P0** | SEC | 双入口行为一致（含错误码） | 双入口同批改造（I2-D）；I2-E 一致性场景 | E4 | | TRACEABLE |
| S01-DATA-068 | DATA | 并发确认/拒绝竞争：条件更新保证唯一终态 | S7 §10.2-10.4；§15.3 | A3 | P1 | INT | 并发下终态唯一且审计正确 | G-24（条件 UPDATE）；I2-E 6 例 | E4 | | TRACEABLE |

## M7 · Project Archive（I1）

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-FR-069 | FR | 归档为终态转换 active→archived；不新增 archiving 中间态 | S6 §2.1/§2.2；S1 Q4 | A3 | P1 | API+DB | 状态机仅此转换 | `archive.rs`；`tests/project_archive.rs::archive_guards_all_entries_and_cancels_queued` / `double_archive_conflicts` | E4 | | TRACEABLE |
| S01-BR-070 | BR | 归档后禁止新活动（清单）；历史读取允许 | S6 §2.3；S1 Q4 | A3 | **P0** | INT+SEC | 禁止清单逐项拒绝；读正常 | `project_gate` 三入口；`project_archive.rs::archive_guards_all_entries_and_cancels_queued` + `command_center_acceptance.rs::archived_project_keeps_history_but_accepts_no_new_activity` | E4 | | TRACEABLE |
| S01-FR-071 | FR | 归档事务：状态+审计+后续任务同一事务；失败整体回滚 | S6 §6.2；I1-004/005 | A3 | **P0** | DB | 注入失败→回滚 | `archive.rs` 单事务（结构保证）；I1-004/005 专项注入用例待 Phase 3 复核 | E3 | | TRACEABLE |
| S01-FR-072 | FR | queued Run 归档时条件取消，与 claim 先到先得 | S6 §7.2；P1；I1-015 | A3 | P1 | INT | queued→cancelled 且不产生 failure 行 | `archive.rs` 条件取消；`tests/project_archive.rs::archive_guards_all_entries_and_cancels_queued` | E4 | | TRACEABLE |
| S01-BR-073 | BR | claimed/running Run：先撤销凭据写终态，再终止进程 | S6 §7.3；T-032 D3；I1-016/017 | A3 | **P0** | INT | 凭据撤销后 MCP 调用被拒；终态先落库 | `record_termination` 条件 UPDATE；`tests/project_archive.rs`（外部取消场景）——专项用例名未逐一核定 | E3 | | TRACEABLE |
| S01-BR-074 | BR | 进程终止失败不回滚 Run 终态 | S6 §9.1；I1-030 | A3 | P1 | INT | terminate 失败终态保持 | `tests/run_executor_handshake.rs::failed_terminate_keeps_terminal_state_and_retries` | E4 | | TRACEABLE |
| S01-FR-075 | FR | 持久化清理账本：pending→cleaning→completed；退避 8 次→needs_human；无时间窗 | 0035；S6 §9.2；I1-031/033/040 | A3 | P1 | INT+DB | 超窗残留可发现；崩溃可恢复；阈值转人工 | `tests/run_executor_handshake.rs` **5 例全绿**：`archive_between_spawn_and_confirmation_blocks_start` / `start_confirmation_rejects_archived_project_and_terminates` / `failed_terminate_keeps_terminal_state_and_retries` / `cleanup_exhaustion_lands_in_needs_human_and_recovers` + 无时间窗清理 | E4 | | TRACEABLE |
| S01-BR-076 | BR | Outbox 归档合法跳过并 ACK；死信重放被门禁拦截 | S6 §10；I1-035/036/037 | A3 | P1 | INT | 归档后消费/重放不建 Run | `tests/outbox_handlers.rs::archived_project_suppresses_task_ready_run_creation` / `run_created_before_archive_gets_cancelled_by_archive`（11 例含新增 2 例） | E4 | | TRACEABLE |
| S01-FR-077 | FR | 归档及取消操作留审计 | S6 §11；I1 §11.3 | A3 | P1 | DB | 审计行含 actor/前后状态 | `archive.rs`（审计 `project.archived`） | E3 | | TRACEABLE |
| S01-SEC-078 | SEC | 归档须由有权限真人调用；Agent 不得自行归档；MCP 不得绕过 | S6 §13.1-13.3；S1 Q4 | A3 | **P0** | SEC | Agent 归档被拒 | `project_lifecycle.rs`（管理面 owner/admin）；专项负向用例待 Phase 3 复核 | E3 | | TRACEABLE |
| S01-BR-079 | BR | 重复归档幂等/409，不重复产生副作用 | S6 §12.1；I1-002 | A3 | P1 | API | 二次归档稳定结果 | `tests/project_archive.rs::double_archive_conflicts` | E4 | | TRACEABLE |
| S01-BR-080 | BR | 归档项目历史全部可查 | S6 §2.3；I1-006；I5-019/036 | A3 | P1 | API | 历史读取 200 | `command_center_acceptance.rs::archived_project_keeps_history_but_accepts_no_new_activity` | E4 | | TRACEABLE |
| S01-BR-081 | BR | 归档时撤销相关执行凭据（lease 三件套清理） | S6 §7.3；任务书 §3.D | A3 | **P0** | DB | lease 三列清空 | `record_termination` lease 清理——专项断言用例名未核定 | E3 | | TRACEABLE |
| S01-BR-082 | BR | 归档项目不可经其他 API/MCP/Outbox 绕过限制 | S6 §2.3/§10.4；任务书 §3.D | A3 | **P0** | SEC | 全入口收口 | `project_gate` 三入口接入；`outbox_handlers.rs::archived_project_suppresses_task_ready_run_creation` + `project_archive.rs::claim_skips_archived_project` | E4 | | TRACEABLE |
| S01-BR-083 | BR | 外部取消入口：POST /task-runs/{id}/cancel（reason 协议化；queued/终态 409） | S6 §18.1 P2 | A3 | P1 | API | 200/409 语义正确 | `run_executor.rs`（reason 参数）；`tests/project_archive.rs::external_run_cancel_entry` | E4 | | TRACEABLE |
| S01-BR-084 | BR | 归档终态化在飞 Import 并释放初始化名额 | S6 §18.1 P4/N-2；G-12 | A3 | P1 | INT | Import 终态化+名额释放 | `tests/project_archive.rs::archive_finalizes_inflight_import_and_releases_slot` | E4 | | TRACEABLE |

## M8 · S01-06 / S01-07

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-FR-085 | FR | S01-06 Codex/MCP Bootstrap——**正式需求文档缺失**；实质需求由 Codex Bootstrap V1.3（C138~C172）与 T-088/T-090 承载。**代码存在不代表需求已正式定义** | S22 :58；S27 | A3 | P1 | INT | DOCUMENT_MISSING；Phase 2/3 以 S27 验收矩阵临时代理；不得判定 S01-06 需求完整 | **R2.1 核定**：自动化证据=`tests/onboarding.rs::health_is_public_and_me_is_not`（/health）+ `tests/pm_devices.rs`（`register_reuse_and_server_generated_id` / `detail_visibility_and_404` / `bind_checks_revoke_flow` / `c184_only_tools_call_triggers_device_layer` 等设备层用例）；**C166 跨机实测为人工测试，不计入自动化证据**（T-090 项）；`/install` 端点无对应自动化用例确认。存在部分自动化测试但未与 S01-06 需求逐条映射 | **E2** | | DOCUMENT_MISSING |
| S01-FR-086 | FR | S01-07 初始化完成与交接——**无任何正式需求文档，不允许自行推断功能要求** | S22 :59；S14 附 A-2 | — | P2 | UI | DOCUMENT_MISSING；不得因既有代码行为判定需求完整 | 既有行为（S14 提及）仅旁证 | E0 | | DOCUMENT_MISSING |

## M9 · 横切需求（安全 / 数据 / 架构 / 非功能）

| ID | Type | 需求描述 | 来源 | 权威 | 优先级 | 验证 | 验收条件 | 实现证据 | EV | TC | 状态 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-NFR-087 | NFR | 五级验收 L1~L5；不能以 L1 通过代替 TASK 完成 | S2 §8；S3 §0 | A2 | P1 | INT | 各 TASK 五级标注齐备 | S3 §0 映射表（流程性需求） | E0 | | TRACEABLE |
| S01-DATA-088 | DATA | 审计四列统一 | S4 8.4；T-088 交付 | A2 | P1 | DB | 13 处生产写入点统一 | `pm_write/audit.rs:40-68`；C174/C180 清单驱动闸门 | E3 | | TRACEABLE |
| S01-SEC-089 | SEC | 多租户隔离：全表 tenant_id 谓词；跨租户 404 不泄露存在性 | 0019；S2 §5.1 | A2 | **P0** | SEC | 跨租户访问全拒 | `tests/tenant_isolation.rs::xt1_xt2_cross_tenant_project_is_invisible_and_unwritable`；组合 FK 体系 | E4 | | TRACEABLE |
| S01-ARCH-090 | ARCH | AI-PM Platform 是项目管理与 AI Orchestration Control Plane，不重复建设外部 AI Agent 的工程执行 Runtime；内部 AI 管理能力（认知、分析、风险识别、架构一致性检查、知识协调）仍属 Platform；统一 Task / Run / Delivery 协议协作；Agent 业务角色、调度机制与执行 Runtime 三概念不得混同；新 Task 生命周期不得与 InitItem 六态混用 | S28（V2.0 裁决）；审查决定精确化 | **A1** | P1 | DB+ARCH | 无六态列被 Task 域复用；无 Platform 内部工程执行 Runtime；三层概念分离 | 0036/0037 与 delivery.task 状态机独立；MCP 为通信接口——**E2=部分架构实现证据，非完整架构验收** | E2 | | TRACEABLE |
| S01-SEC-091 | SEC | 撤权即时生效：撤权后立即拒绝新领取；快照不得使已撤销操作继续 | S25（S01-W15）；S3 §3.7 | A2 | **P0** | SEC | 撤权后领取被拒 | `tests/project_members.rs::revoke_takes_effect_immediately` / `tenant_removal_cascades_project_memberships` | E4 | | TRACEABLE |
| S01-DATA-092 | DATA | 关键状态迁移具备合法条件；失败操作事务一致；重新执行不覆盖历史 | S2 §5.1/§5.4 | A2 | P1 | DB+INT | 各状态机非法迁移被拒 | `apply_transition` 唯一原语；各域条件 UPDATE | E3 | | TRACEABLE |

---

## 统计摘要（R2.1 — 全部由明细逐条汇总）

| 维度 | 数量 |
|---|---|
| 需求总数 | 92 |
| 模块分布 | M1=12 / M2=16 / M3=12 / M4=11 / M5=6 / M6=11 / M7=16 / M8=2 / M9=6 |
| 类型分布 | FR=22 / BR=38 / SEC=11 / DATA=9 / UX=6 / ARCH=3 / ADR=2 / NFR=1 |
| **P0** | **29**（M1×3=007/010/011；M2×2=013/014；M3×4=030/035/036/037；M4×4=041/044/047/050；M5×1=054；M6×7=058/059/060/062/063/066/067；M7×6=070/071/073/078/081/082；M9×2=089/091） |
| P1 / P2 / P3 | 53 / 9 / 1（P2：FR-008/UX-012/FR-018/BR-026/UX-028/BR-040/UX-056/FR-086/BR-003；P3：UX-057） |

### 状态分布（R2.1）

| Traceability Status | 数量 | 需求 ID |
|---|---|---|
| TRACEABLE | **87** | 其余 87 条 |
| IMPLEMENTATION_NOT_FOUND | **1** | S01-BR-003（⚑ DECISION REQUIRED：产品规则待裁决 + 受控修改/审计实现缺口，两项独立） |
| IMPLEMENTATION_GAP | **1** | S01-DATA-020（读取路径实现证据存在（E2）；刷新/重登一致性完整行为未验证，Phase 3 独立实测） |
| REQUIREMENT_CONFLICT | 1 | S01-UX-057（P3 待裁决功能，不阻塞核心验收） |
| DOCUMENT_MISSING | 2 | S01-FR-085 / S01-FR-086 |
| **合计校验** | **92** ✅ | 87+1+1+1+2=92 |

### Evidence Level 分布（R2.1，明细汇总）

| 等级 | 数量 | 需求 ID |
|---|---|---|
| E0 | **3** | UX-057 / FR-086 / NFR-087 |
| E1 | **4** | BR-009 / BR-022 / ARCH-036 / BR-040 |
| E2 | **31** | M1×9（FR-001/002、BR-003、SEC-005、FR-006/007、FR-008、BR-011、UX-012）＋ M2×8（ARCH-013/BR-014/BR-016/BR-019/DATA-020/BR-024/ADR-023/BR-026）＋ M3×5（FR-029/031、BR-033/034/039）＋ M4×3（ADR-041/SEC-042/BR-044）＋ M5×4（UX-052/UX-027/SEC-054/BR-055）＋ M8×1（FR-085）＋ M9×1（ARCH-090） |
| E3 | **13** | FR-004、FR-015、BR-032、BR-038（M3）；DATA-049、BR-051（M4）；FR-071、FR-077、SEC-078、BR-073、BR-081（M7）；DATA-088、DATA-092（M9） |
| E4 | **41** | 全清单见 `04-evidence-level-audit.md`（含 R2.1 新降 2 条后的 41 条核定明细） |
| E5 | **0** | Phase 3 产出 |
| **合计校验** | **92** ✅ | 3+4+31+13+41+0=92 |

> **R2.1 相对 R2 的 EV 变化**：DATA-020 E0→E2、FR-085 E3→E2、UX-027 E4→E2、SEC-054 E4→E2；BR-017 经断言行号核定（`:146`/`:163`）**确证 E4**。
> **E4 是开发交付记录，不等于独立 QA 验收结论**；人工实测（C166）不计入自动化证据等级。