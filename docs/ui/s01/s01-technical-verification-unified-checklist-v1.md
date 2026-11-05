# S01 技术核验统一清单 V1（四稿合并）

> **V1.1（2026-11-05 P0 批次核验已完成，结果见下方「核验进度」节）**
> **实现仓定位更正**：实现仓=**`D:\kudi-project\templates\ai-pm`**（apps/crates/migrations/ai-frontend 全在本仓，master@b51ba00）；记忆中的旧路径 `D:\kudi-project\ai-pm` 已不存在。

## 核验进度 V1.1（2026-11-05 P0 批次，只读核验，evidence=路径:行）

| 统一编号 | 结论 | 证据 |
| --- | --- | --- |
| **U-B01** | **存在（G-08 过期，矛盾落定）** | `migrations/0037_s01_baseline.sql`：`ai.baseline_proposal`（:7，status ∈ pending/confirmed/superseded :15，单 pending 唯一索引 `uq_baseline_proposal_pending` :26）、`ai.baseline_item`（:29，state ∈ proposed/confirmed/rejected/investigation/deferred :43）、`ai.project_baseline`（:55）；路由 `apps/pm-api/src/routes.rs:205` merge `pm_baseline::router()`；处理器 `apps/pm-api/src/pm_baseline.rs`（测试 `apps/pm-api/tests/pm_baseline.rs`）。**待读稿细项**：确认动作权限校验是否满足 R-BL-06=B；`core.project.current_baseline_id` 是否存在 |
| **U-C01** | **存在** | `migrations/0036_s01_cognition.sql`：`ai.cognition_run`（:7）、`ai.cognition_item`（:32）、`ai.project_knowledge`（:66）；路由 `routes.rs:203`；`apps/pm-api/src/pm_cognition.rs` + 测试 |
| **U-R02** | **不存在（G-03 仍开）** | 全仓 grep `is_primary`/`primary_repository_id`（rs+sql）零命中 |
| **U-R04** | **不存在（G-04 仍开）** | 无 git_provider/oauth 表与路由；唯一命中=`auth.rs:752` 无关注释 |
| **U-R08** | **存在** | `0009_repo_import.sql:16-17`：`project_repository.status ∈ pending/verified/unreachable/archived`（软解绑承载在） |
| **U-R09** | **存在（词表差维持）** | `0008_repositories.sql:24`：`role IN ('main','docs','service','infra')` |
| **U-R10** | **存在** | `0009:18`（project_repository.head_commit_sha）、`0031:68`（repo_import_source.head_commit_sha，注释 write-once N-1） |
| **U-R12** | **不存在（G-05 仍开）** | 仓库域 grep `confirm` 零命中（仅 `documents.rs` confirm_redline，无关）；HEAD 变化人工确认仍无承载 |
| **U-R13** | **存在** | `0031:89/:141`：evidence/attempt 表含 `commit_sha` |
| **U-R15** | **部分存在** | `0031:82-101`：`repo_import_source_attempt`（attempt_no + `current_attempt_no` 轮指针）；单 Source 独立重试**行为**待读码确认 |
| **U-C02** | **部分存在** | 本分支两套：discovery_proposal（0025）、document_proposal（0014）；S02D 确认实体不在（见 U-B02）；**G-07 缓释**：读侧已防御性排除——`command_center.rs:759` 注释「`quality_gates_passed`/`confirmed` 现无写入路径，不计」，写入路径仍缺 → 挂账继续 |
| **U-C04** | **存在** | `apps/pm-api/src/auth.rs:133` `auth_mode()`：aud/client_id 推断（user_login→interactive）；A* 基础在 |
| **U-C08** | **已修（G-06 缓释，全量复读待）** | `command_center.rs:79` `WAITING_FOR_HUMAN` 常量、:95 判定纪律「不得用存在 PENDING 记录直接推出」、:110 在用（G-01 七值方案实施的一部分）；completed 分支全量复读待做 |
| **U-B02** | **不存在** | 本分支迁移止于 0037，无 S02 需求域迁移（S02 自 0038 起未实施）→ R-BL-01=B（引用 Requirement Revision）暂无实现基础 |
| **U-B05** | **部分存在** | baseline/cognition 路由已挂（routes.rs:203-205）；确认动作的服务端权限校验细节待读 `pm_baseline.rs` |
| G-16 余项 | **仍缺** | `ai-frontend/src/api/mock/` 仅 auth/command-center/fixtures/initialization/personal；discovery/devices/imports 夹具仍缺 |

---

> **建立日期**：2026-11-05（「开始修正」执行件）。**性质**：把四份稿各自的技术核验清单**归并为一张清单**，只做映射与去重，**不新增裁决、不下核验结论**。
> **合并来源**：
> ① S01-01 初始化总览稿 §14.1 缺口总表 **G-01~G-23**；
> ② S01-03 V1.2 §10 十七项（+ §10.1 历史审计风险线索五条）；
> ③ S01-04 V1.2 §15 十项（+「勿复制三套提案状态机」注意）；
> ④ S01-05 §八 八项 + 内嵌 V1.1 §11 十项 + §14 冻结条件四项。
> **使用规则**：
> 1. 核验一律以**当前分支代码**为准。各稿事实快照时间不同（2026-11-04~05），**已出现已知时间快照差**（见 U-B01 警示），不得跨稿直接引用旧结论。
> 2. 每项核验完成后**回写各来源稿**与本清单状态列；来源稿原文不因清单而改写。
> 3. 「已实施/已修复」标记均转引自总览稿 §14.1 原文，仍须随代码演进复核。

---

## 域 A · Repository / 授权 / Primary / Branch（来源：S01-03 §10 十七项 + G-03/G-04/G-05/G-11）

| 统一编号 | 核验项 | 来源映射 | 已知事实与注记 |
| --- | --- | --- | --- |
| U-R01 | Project Repository 数据模型是否支持多仓库 | S01-03-1 | `project_repository` 多行模型存在（0008/0009），多仓库=多行；组合唯一约束以代码为准 |
| U-R02 | Repository 是否允许暂时没有 Primary | S01-03-2 | **G-03（P0）**：后端无 `is_primary`/`primary_repository_id`；前端已假定存在（`types/initialization.ts:69`、`initialization.ts:170-174`）；裁决=role='main' 派生 vs 新增列 |
| U-R03 | 当前 Provider 支持范围 | S01-03-3 | — |
| U-R04 | GitHub/GitLab OAuth 或 App 授权是否已实现 | S01-03-4 | **G-04（P0）**：无表无路由；前端 `listGitProviders` 等三调用悬空（`initialization.ts:200-222`）；R-03-03 落地前提 |
| U-R05 | 授权凭据的存储与撤销机制 | S01-03-5 | SSH Key 机制存在（`project_ssh_key`）；**G-11（P2）**：`ssh_key_id` 列从未被写入 |
| U-R06 | Repository URL 重复关联检测 | S01-03-6 | — |
| U-R07 | Repository 与 Project 的跨项目关系约束 | S01-03-7 | I6-B 有组合约束先例（`repo_import_source`），语义可参照 |
| U-R08 | Disconnect 是否已有软删除/归档语义 | S01-03-8 | 后端已有 `status='archived'`（0009）；与 S01-03 V1.2 §4.4「软解绑优先」同向 |
| U-R09 | Repository Role 是否已有持久化字段 | S01-03-9 | `role ∈ main/docs/service/infra`（CHECK）；**词表三方差**：前端 `primary\|supporting\|infrastructure\|test` vs S01-03 V1.2 建议 Backend/Frontend/… 待收敛 |
| U-R10 | Default Branch 与 HEAD 的读取方式 | S01-03-10 | `head_commit_sha`/`last_verified_at`/`last_error_code` 已有（0009） |
| U-R11 | HEAD 检测是否已有独立机制 | S01-03-11 | — |
| U-R12 | Confirmed Commit 的数据承载位置 | S01-03-12 | **G-05（P1）**：无确认记录表/端点；R-03-04 与 S01-05 §五 落地前提 |
| U-R13 | Baseline/Agent Run 是否保存不可变 Commit 引用 | S01-03-13 | evidence 表已含 commit_sha（0031:128，记忆来源以代码为准） |
| U-R14 | Discovery 是否可以独立重跑 | S01-03-14 | — |
| U-R15 | 多仓库 Discovery 部分失败与单仓重试 | S01-03-15 | §10.1 线索：Source 级独立重试曾被标记不存在；不可把「多仓库关联」误认为「部分成功能力已具备」 |
| U-R16 | Repository 全部变更是否有审计 | S01-03-16 | §10.1 线索：CRUD/verify 有审计；Repo Import create/cancel/apply 曾有缺口 |
| U-R17 | 页面统计数字是否有可靠数据接口 | S01-03-17 | command_center 聚合已有部分来源 |

## 域 B · Cognition / 提案机制（来源：S01-04 §15 十项 + G-06/G-07/G-10/G-12）

| 统一编号 | 核验项 | 来源映射 | 已知事实与注记 |
| --- | --- | --- | --- |
| U-C01 | 是否已有独立 Project Cognition 数据模型 | S01-04-1 | 已实施 V1=迁移 0036+`pm_cognition.rs`（记忆来源以代码为准）；与 V1.2 六领域范围差待演进裁决 |
| U-C02 | 是否已有 Cognition Proposal 或可复用提案机制 | S01-04-2 | **三套既有提案机制**：`discovery_proposal` 四态 / `ai.document_proposal` 八态 / S02D 确认实体；勿复制第四套 |
| U-C03 | 正式确认人/确认时间/版本历史 | S01-04-3 | **G-07（P1）**：`document_proposal.confirmed`/`quality_gates_passed` 从未被写入，判定须用 `committed` |
| U-C04 | AI Actor 与 Human Actor 可区分 | S01-04-4 | A* 治理线：auth_mode 三分类已存在（aud=interactive/client_id=api_key）；MCP 无正式确认入口 |
| U-C05 | Knowledge 是否已有独立生命周期 | S01-04-5 | 知识 v2 已有（0014）；发布治理独立 |
| U-C06 | 统一的来源与证据引用机制 | S01-04-6 | evidence 表先例（0031） |
| U-C07 | Context 组装是否支持默认注入+按需检索 | S01-04-7 | 当前无此机制（Q5=C 落地前提） |
| U-C08 | 未确认提案是否被拦在正式上下文外 | S01-04-8 | **G-06（P0）**：`project_cognition` completed 判定把分析完成当认知完成 |
| U-C09 | Repository/Discovery 结果能否进统一提案审核流 | S01-04-9 | **G-10（P1）**：`discovery_proposal` 决策仅 MCP 写入、无 UI 入口 |
| U-C10 | 变化/冲突检测与历史版本保留机制 | S01-04-10 | **G-12（P2）**：`session_open` 计数口径未明 |

## 域 C · Baseline（来源：S01-05 §八 八项 + §11 十项 + §14 冻结条件 + G-08）

| 统一编号 | 核验项 | 来源映射 | 已知事实与注记 |
| --- | --- | --- | --- |
| U-B01 | 是否已有项目级 Baseline 实体/读模型 | S01-05§八-1；§11-1；**G-08（P0）** | ✅ **已核验落定（V1.1）**：0037 三表 + `pm_baseline::router()` 在（见核验进度节）；G-08 的「无表无路由」为过期快照，已在总览稿 G-08 行就地勘误 |
| U-B02 | Requirement Revision 的版本化引用能力 | S01-05§八-2；§11-2 | R-BL-01=B（只引用 Revision）的实施前提 |
| U-B03 | Cognition/Knowledge 确认记录与来源证据可被引用 | S01-05§八-3；§11-3 | 与 U-C03/U-C06 交叉 |
| U-B04 | 多 Repository 逐仓 Branch/Commit 快照能力 | S01-05§八-4；§11-4 | 与 U-R10/U-R12/U-R13 交叉 |
| U-B05 | 正式确认协议、权限校验与审计机制 | S01-05§八-5；§11-5 | **G-19（P2）**：成员角色 6 种、无细粒度能力授予；R-BL-06=B（授权角色≠创建者）的实施前提 |
| U-B06 | Baseline Version 不可变与历史追溯 | S01-05§八-6；§11-6 | R-BL-05=A（完整逻辑快照）的实施前提 |
| U-B07 | Change Proposal 独立保存差异/证据/影响 | S01-05§八-7；§11-7 | — |
| U-B08 | S01 Project Baseline 与 S03 Implementation Baseline 边界 | S01-05§八-8；§11-8 | 设计稿已定义互不取代；代码侧尚无涉及 |
| U-B09 | 未决事项的保存/展示/后续处理能力 | S01-05§11-9 | R-BL-02=A（未决允许且标记、不阻断确认）的实施前提 |
| U-B10 | 历史引用对象被修改/删除/失效时的完整性保障 | S01-05§11-10 | — |
| U-B11 | 冻结条件四项（代码与领域模型/数据模型与版本引用/权限确认协议/S02·S03 边界） | S01-05§14 | 四项完成前 S01-05 V1.1=已裁决产品设计基线，不等于实施冻结 |

## 域 D · 总控台/初始化派生与展示（来源：G 表剩余项）

| 统一编号 | 核验项 | 来源映射 | 已知事实与注记 |
| --- | --- | --- | --- |
| U-D01 | `project_basics.source_state` 硬编码 `"planning"` | G-02（P1） | 改为透传真实 `core.project.status` |
| U-D02 | 两套 i18n 状态词表并存且均为孤儿 | G-15（P2） | `pm.commandCenter.init.*` vs `s01.initialization.state.*`；裁决主词表 |
| U-D03 | mock 夹具仍缺 discovery/devices/imports | G-16（P1，部分已实施） | command-center 夹具已接入；余三套仍缺 |
| U-D04 | `delivery.task` 待验收混入初始化待办 | G-17（P2） | 初始化总览应只取初始化相关子集 |
| U-D05 | `human_actions` 作为 required 的合理性 | G-18（P2） | 与 U-D04 一并裁决：拆 required 子集 |
| U-D06 | 归档门禁对读路径的覆盖 | G-20（P2） | `project_gate.rs` 写路径为主，读路径待核 |
| U-D07 | 活动词表 camelCase/snake_case 不一致 | G-21（P1，部分已实施） | 本页已按后端 snake_case 自建词表；全局统一待另立任务 |

**已在总览稿 §14.1 标记为已实施/已修复/已规避的项**（登记备查，随代码复核）：G-01（waiting_for_human 七值方案已前后端实施）、G-13（初始化视图已新建，InitStageCard）、G-14（已规避：卡片网格非 stepper）、G-22（AI 执行中改由阶段派生）、G-23（`ui/descriptions` columns 语义已修复）。

---

## 执行方式

1. 逐项核验按域推进，每项产出：结论（存在/不存在/部分存在）+ 代码证据（路径:行）+ 对应来源稿回写。
2. 核验结论不得回改专家修订记录文件（那些是供稿记录，保持当日原样）；修正落到**来源稿与实施**侧。
3. 本清单自身随核验推进更新状态列，版本号递增（V1→V1.1…）。
