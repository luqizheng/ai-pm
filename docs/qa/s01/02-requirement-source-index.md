# S01 需求来源索引（02-requirement-source-index）

> Phase 1 交付 · 2026-11-06 · QA Test Architect
> 职责：登记 RTM 全部需求来源文件、版本、权威等级、有效性判定与关联关系。
> 权威等级定义见 `03-requirement-conflict-report.md` §1。

## 1. 权威等级

| 等级 | 名称 | 判定规则 |
|---|---|---|
| A1 | 用户正式裁决 | 用户明示选择的方案/原话（含收编进设计稿的 Q/R 编号裁决） |
| A2 | 正式实施基线 | `docs/s01-implementation-baseline-v1.md` 及六份 Registry 体系 |
| A3 | 已过稿/已收编裁决的专项设计稿 | I1~I6 设计稿、最终基线 V1.2、已收编裁决的 UX 稿 |
| A4 | 已记录待冻结的专家供稿/修订记录 | `docs/ui/s01/` 设计修订五份（2026-11-05 记录，待基线确认） |
| A5 | 裁决记录（只记录未评审） | R-S01-01-01 等 |
| A6 | 代码/迁移事实 | 只作 Implementation Evidence，不自动成为产品需求 |
| H | 历史方案（已被取代） | 保留作决策留痕，不得作为当前需求 |

## 2. 需求来源文件清单

| # | 文件 | 版本/日期 | 权威等级 | 有效性 | 覆盖模块 | 备注 |
|---|---|---|---|---|---|---|
| S1 | `docs/ui/smark-pm-s01-final-baseline-v1-2.md` | V1.2 / 2026-11-03 | **A3（S01 场景最终设计基线）** | 有效 | M1~M4、M7 | Q1~Q6 六项最终技术裁决；取代 V1 场景稿冲突的状态机设计 |
| S2 | `docs/s01-implementation-baseline-v1.md` | V1.0 / 2026-11-03 | A2（正式实施总基线） | 有效 | 全模块 | 基线优先级序、D-1a/D-2a 裁决、五级验收 |
| S3 | `docs/s01-acceptance-matrix-v1.md` | V1.1 / 2026-11-03 | A2 | 有效 | 全模块 | I1-001~040 / I2 / I3 / I4 / I5 / S01-Axxx 验收台账 |
| S4 | `docs/s01-code-facts-ledger-v1.md` | V1.0 / 2026-11-03 | A2 + A6 | 有效 | 全模块 | G-01~G-29 差异台账、D-C-01~06 差异裁决 |
| S5 | `docs/s01-task-registry-v1.md` / `s01-migration-registry-v1.md` / `s01-interface-matrix-v1.md` | V1.0 / 2026-11-03 | A2 | 有效 | 全模块 | TASK / 迁移（0001~0031）/ 97 REST + 19 MCP 接口登记 |
| S6 | `docs/ui/smark-pm-s01-i1-archive-lifecycle-v1.md` | V1.2（V1.0→V1.1→V1.2 回填）/ 2026-11-04 | A3 | 有效 | M7 | I1-001~040；§17 十四条裁决；§18/§19 实施回填 |
| S7 | `docs/ui/smark-pm-s01-i2-finding-human-confirmation-v1-1.md` | V1.1 / 2026-11-03 | A3 | 有效 | M6 | R1~R4、S1~S7 裁决；I2-001~038；G9/G10 P0 修复 |
| S8 | `docs/ui/smark-pm-s01-i3-git-permission-and-repo-write-authorization-v1.md` | V1.1 / 2026-11-04 | A3（§0.0 最高优先级） | 有效 | M3 | V1.1 简化裁决；§0.0.6 验收映射；V1.0 六能力模型=H |
| S9 | `docs/ui/smark-pm-s01-i4-project-goal-knowledge-v1.md` | V1.0 + §15 实施落地 / 2026-11-04 | A3 | 有效 | M4 | I4-D01~D10；I4-001~024；Q1/Q2 待裁项已由 G-26 落地 |
| S10 | `docs/ui/smark-pm-s01-i5-project-command-center-v1-1.md` | V1.1 / 2026-11-04 | A3 | 有效 | M2 | I5-001~036；读模型六块 |
| S11 | `docs/s01-i5-read-model-contract.md` | 定稿 / 2026-11-04 | A3 | 有效 | M2 | 七事项判定规则、C-1~C-8 通用约束、D-C-05/06 口径 |
| S12 | `docs/ui/smark-pm-s01-i6-b-v1-1.md` | V1.1 / 2026-11-03 | A3 | 有效 | M3、M6 | D-1a/D-2a 落地；I6-B-036~051（本稿自定 16 条） |
| S13 | `docs/ui/s01/s01-02-project-creation-ux-v1-1.md` | V1.1 | A3（Q1~Q3 用户裁决收编稿） | 有效 | M1 | §13 裁决记录表 |
| S14 | `docs/ui/s01/s01-02-project-creation-design-revision-v1-1.md` | V1.1 / 2026-11-05 | A4 | 有效（待基线确认） | M1 | Q3 落地修订=创建后进 Command Center |
| S15 | `docs/ui/s01/s01-03-repository-management-ux-v1-1.md` + `-design-revision-v1-2.md` | V1.1 / V1.2 | A3 + A4 | 有效 | M3 | R-03-01~04；AC-01~15；G-03/G-04/G-05 实施前提 |
| S16 | `docs/ui/s01/s01-04-ai-pm-cognition-ux-v1-1.md` + `-design-revision-v1-2.md` | V1.1 / V1.2 | A3 + A4 | 有效 | M4 | 五裁决 Q1~Q5；六认知领域 |
| S17 | `docs/ui/s01/s01-05-personal-workspace-ux-v1-1.md` + `s01-05-personal-home-high-fidelity-ui-v1.md` | V1.1 / V1.0 | A3 + A3（已实施） | 有效（命名冲突已裁决） | M5 | Personal Home 定名（2026-11-04）；§13 实施记录 |
| S18 | `docs/ui/s01/s01-command-center-design-revision-v1-1.md` | V1.1 / 2026-11-05 | A4 | 有效（待基线确认） | M1、M2 | 五区结构；§4 InitItem 六态规则；§8 验收 14 条 |
| S19 | `docs/ui/s01/s01-05-project-baseline-design-revision-v1-1.md` | V1.1（内嵌收编稿） | A4 + A1（R-BL-01~06 用户裁决） | 有效（冻结待四项核验） | M2 | R-BL-01=B / 02=A / 03=A / 04=A / 05=A / 06=B |
| S20 | `docs/ui/s01/s01-01-native-ai-pm-independence-ruling-v1.md` | V1.0 / 2026-11-05 | A5（专家裁决·方案 A，只记录） | 有效 | M4、M1 | R-S01-01-01 Native AI-PM Independence |
| S21 | `docs/ui/s01/s01-00-global-navigation-information-architecture-v1.md` | V1.0 | A1（用户裁决「以这份为主」） | 有效 | M5、M2 | 导航口径唯一权威；不引入 Workspace 业务概念 |
| S22 | `docs/ui/s01/s01-01-project-initialization-overview-ux-v1.md` | V1.0 | A3（§0.3 编号冲突待裁决） | 有效（编号挂账） | M2 | G-01~G-23 核验清单 |
| S23 | `docs/ui/s01/s01-technical-verification-unified-checklist-v1.md` | V1.0 / 2026-11-05 | A3 | 有效 | 全模块 | 统一技术核验清单（U-B01 时间快照差警示） |
| S24 | `docs/ui/s01/s01-08-knowledge-repository-git-backup-v1.md` + review | V1.0 / 2026-11-05 | A4 + 评审 | **部分无效**：原文截断待补发；评审 4 P0 未裁 | M4 | Q-08-01~06 待用户裁决；编号冲突（S01-08=Personal Home vs Git Backup） |
| S25 | `docs/ai-pm-s01-project-member-authorization-v1.md` | V1.0 / 2026-11-03 | A3 | 有效 | M1 | S01-W13；S01-A001~A016 |
| S26 | `docs/ai-pm-project-discovery-v1.md` | V1.1 | A3 | 有效 | M6、M4 | Discovery Session/Finding/Proposal 基础模型 |
| S27 | `docs/ai-pm-codex-bootstrap-v1.md` | V1.3（已过稿 2026-11-02） | A3 | 有效 | M8（S01-06 实质承载） | C138~C172；设备绑定不产生项目权限 |
| S28 | `docs/project-ai-team-v2/ai-pm-v2-architecture-baseline-v1.md` | V1 / 2026-11-06 | A1（V2.0 三题裁决） | 有效 | 横切 | Native AI-PM 无外部 Client 可用；新 Task 生命周期不得与 InitItem 六态混用 |
| S29 | `docs/ai-pm-adr-0001/0002` | Accepted | A2（ADR） | 有效 | M7、M6 | 身份边界；凭据分离 |
| S30 | 迁移 `0030`~`0038`（S01 批次） | — | A6 | 已实施 | 各模块 | 0030 成员角色 / 0031 Discovery Import V2 / 0032-0033 Git 能力建撤 / 0034 CC 索引 / 0035 Run 清理 / 0036 Cognition / 0037 Baseline / 0038 Repo Baseline Confirm |
| S31 | `apps/pm-api/tests/`（44 个测试文件）+ `crates/pm-application/tests/i6b_db_constraints.rs` 等 | — | A6 | — | 各模块 | 实现证据来源 |

## 3. 关键裁决索引（任务书 §7 十三项历史裁决的权威落点）

| # | 历史裁决摘要 | 最新正式依据（文件 § 章节） | 判定 |
|---|---|---|---|
| 1 | AI-PM 项目创建后即可独立工作，不依赖 MCP/Codex/Repository | R-S01-01-01（S20 §一/§二）；V2.0 基线 S28 红线；I5 V1.1 §4.7 | 与正式文件一致 |
| 2 | Project Goal 用 Goal 实体及 doc_key=project-goal | S1 Q3 裁决（:236-303）；I4 §0.2 I4-D02；`project_goal.rs:22` | 一致 |
| 3 | InitItem 六态属 Command Center Read Model | S18 §4.1；S11 §4.2「status 是展示层状态」；0036/0037 无六态列 | 一致 |
| 4 | 人工提交 Finding 可直接 CONFIRMED | S7 §3.4（R2）；`FindingCreator::HumanDirect`（G-23） | 一致 |
| 5 | Agent 转录 Finding 必须 PENDING | S7 §3.4.3；I2-024；`AgentRelayed` | 一致 |
| 6 | 多 Repository 允许；初始无 Primary Repository 允许 | S1 Q1（项目级关联）；S11 §4.2 repository_access required=false | 一致 |
| 7 | Repository 变更检测后需用户确认 | S30 迁移 0038 注释（Observed HEAD vs Confirmed Commit）；S15 修订稿 R-03-04 | 一致 |
| 8 | Archive 禁止新活动 | S6 §2.3；S1 Q4 归档语义表 | 一致 |
| 9 | Archive 先撤 Credential 写终态再进程清理 | S6 §7.3/§9.1（T-032 D3）；0035 清理账本 | 一致 |
| 10 | 项目创建后立即进入初始化流程 | S13 Q3=A；S14 §2.3（修订：进 Command Center+初始化内嵌） | **存在表述演进**，见冲突报告 C-03 |
| 11 | Dashboard 优先待人工处理/当前执行/风险；初始化完成后折叠 | S10 §3（首屏优先级）；S18 §3.2/§3.5；I5-026/027 | 一致 |
| 12 | 不得引入未经裁决的通用 Workspace 概念 | S21（S01-00 冻结范围）；S17 命名裁决（Personal Home） | 一致 |
| 13 | 不得把 Git 六操作拆分为产品权限层级 | S8 §0.0.1（用户裁决原文）；D-C-03 | 一致 |

## 4. S01-06 / S01-07 文档状态

| 工作包 | 正式需求文档 | 状态 |
|---|---|---|
| S01-06 Codex / MCP Bootstrap | **未建独立 UX/需求稿**（S22 :58 登记「未建」）；实现依据=`docs/ai-pm-codex-bootstrap-v1.md` V1.3（T-087 过稿）+ T-088/T-090 | DOCUMENT_MISSING（S01-06 编号下）；实质需求由 S27 承载 |
| S01-07 初始化完成与交接 | **未建稿**（S22 :59）；S14 附 A-2 提及「S01-07 并入 S01-01 的实施记录」 | DOCUMENT_MISSING |