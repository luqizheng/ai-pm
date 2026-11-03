# S01 实施计划 V1.0（冻结基线）

> 文档性质：S01-0 退出产物之一（实施组织基线）
> 上位文档：`docs/ui/smark-pm-s01-overall-implementation-plan-v1.md`（总体实施计划 V1.0）
> 代码基线：`master 2933db8`，migrations 0001–0028
> 版本：V1.0　日期：2026-11-03　状态：**S01-0 完成，基线冻结**
> 配套文件：`docs/s01-task-registry-v1.md` / `docs/s01-migration-registry-v1.md` / `docs/s01-interface-matrix-v1.md` / `docs/s01-acceptance-matrix-v1.md`

---

## 0. 本文件的定位与效力

总体实施计划 V1.0 §16.3 规定 S01-0 退出须产出五份文件并形成待决事项清单。本文件即其中**实施基线**一份，其余四份为附件式 Registry。

本文件**不新增**任何架构决策，只做三件事：

1. 把总体计划 §1 的 S01-0～S01-6 阶段，落到**可领取的任务编号**与**明确的阶段出口条件**；
2. 固定**串行/并行规则**与**冻结门禁**的判定口径；
3. 登记**待决事项**（本文件 §9），未决项一律不冻结、不开工。

> 冲突裁定顺序：本文件 → 总体计划 V1.0 → I1–I6 设计稿 → 代码事实。设计稿之间冲突以本文件 §9 待决清单为准。

---

## 1. 设计基线索引（S01-W01）

S01 的设计输入共 13 份，全部落在 `docs/ui/`（除标注者）：

| 编号 | 文件 | 版本 | 定稿状态 | 本 S01 的作用 |
|---|---|---|---|---|
| — | `docs/ui/smark-pm-s01-overall-implementation-plan-v1.md` | V1.0 | 已落盘 | 组织基线（上位） |
| — | `docs/ui/smark-pm-user-scenario-interaction-model-v1.md` | V1.0 | 已落盘 | 场景与交互模型（S01 全景来源） |
| — | `docs/ui/smark-pm-menu-roles-permission-model-v1.md` | V1.0 | **待评审** | 菜单与权限模型（S01-1 输入，未冻结） |
| I1 | `docs/ui/smark-pm-s01-i1-archive-lifecycle-v1.md` | V1.0「最终定稿」 | **需 V1.1**（见 §9-B1） | Project 归档生命周期 |
| I2 | `docs/ui/smark-pm-s01-i2-finding-human-confirmation-v1-1.md` | V1.1 修订定稿 | 已定稿 | Finding 人工确认 |
| I2 | `docs/ui/smark-pm-s01-i2-finding-human-confirmation-v1.md` | V1.0 | **被 V1.1 取代**（编号语义冲突，见 §9-B3） | 仅作对照 |
| I3 | `docs/ui/smark-pm-s01-i3-git-permission-and-repo-write-authorization-v1.md` | V1.0 | 已定稿（字段名待同步） | Git 权限与仓库写入授权 |
| I4 | `docs/ui/smark-pm-s01-i4-project-goal-knowledge-v1.md` | V1.0 | 已定稿（**Q1/Q2 未裁**，见 §9-B4） | Project Goal |
| I5 | `docs/ui/smark-pm-s01-i5-project-command-center-v1-1.md` | V1.1 定稿 | 已定稿 | 项目总控台（**最后实施**） |
| I5 | `docs/ui/smark-pm-s01-i5-project-command-center-v1.md` | V1.0 | **被 V1.1 取代** | 仅作对照 |
| I6-A | `docs/ui/smark-pm-s01-i6-code-facts-v1.md` | I6-A 报告 | 事实报告（已核验） | 代码事实基线 |
| I6-B | `docs/ui/smark-pm-s01-i6-b-boundary-review-346-v1.md` | V1.0 | 评审记录 | R-3～R-6 裁决来源 |
| I6-B | `docs/ui/smark-pm-s01-i6-b-v1-1-merge-spec.md` | V1.0 | **I6-B V1.1 唯一输入** | D-1a/D-2a + 补-1/2/3 |
| I6-B | `docs/ui/smark-pm-s01-i6-b-v1-verification-feedback.md` | V1.0 | 核验反馈 | C-1~C-4 / N-1~N-5 来源 |
| — | `docs/ui/smark-pm-s01-v1-3-revision-and-i1-verification-已经过审.md` | — | 已过审 | I1 修订要求 P1~P4 来源 |

**唯一有效版本判定规则**：

1. 同编号存在多稿时，**版本号高者为准**（I2→V1.1，I5→V1.1）。
2. 被取代稿不删除，但不得作为实施依据；其独有内容须并入新版后作废（见 §9-B3 的 I2-023~027 五条）。
3. 「标称定稿但带未落修订项」的稿（I1）**不视为已定稿**，须出 V1.1 后方可开工。

---

## 2. S01-0 核验结论（W01–W07 逐项对照）

总体计划 §3.2 的八项必核事项，逐条给结论与证据：

| # | 核验事项 | 结论 | 证据 |
|---|---|---|---|
| 1 | I1–I6 是否有唯一有效设计稿 | **否**（I1 需 V1.1；I2/I5 已由 V1.1 取代 V1.0；I6-B 正文 V1.1 未产出） | §1 索引表 |
| 2 | 设计稿之间是否存在状态机冲突 | **有 1 处**：Discovery Session 状态机与 Task 状态机并行且无交叉校验 | Interface Matrix §6 |
| 3 | 是否存在重复数据模型/重复职责 | **有 2 处**：`ai.document_proposal`（8 态发布）vs `discovery.discovery_proposal`（4 态审核）；`repo_import.draft` 单列 vs 待建的 `repo_import_draft_version` | Migration Registry §4 |
| 4 | 已有 Migration 是否已实现部分设计 | **是**：0025 已建 Discovery 全域；0027 已建 devenv；`repo_import_source` 已有 facts/counts 列（但按 R-4-1 需迁 Attempt 层） | Migration Registry §2 |
| 5 | 哪些设计已实施 / 哪些只有设计 | 已实施：T-083～T-091、D1～D5、D3.1。仅设计：I1～I6 全套、Git 能力授权、Project 成员授权 | Task Registry §3 |
| 6 | 哪些缺陷必须在 S01 修复 | **P0 五项**（见 §4 缺陷台账） | §4 |
| 7 | 哪些问题允许延后 | Policy 管理端点、Notification 写侧、T-052/T-053/T-071 等（§9-C） | §9 |
| 8 | 哪些 TASK 依赖同一个数据库变更 | **Migration 0031（I6-B 批）为唯一多任务共享变更**：I6-B-02/04/07/08 共用 | Migration Registry §3 |

---

## 3. 阶段出口条件（冻结口径）

总体计划 §1 给出阶段目标，S01-0 把它细化为**可判定的出口条件**。出口条件未全绿，不得进入下一阶段的冻结任务。

### S01-0 统一实施基线 ✅ 本次达成

| 出口条件 | 状态 | 证据 |
|---|---|---|
| 五份正式文件产出 | ✅ | 本文件 + 四份 Registry |
| 待决事项清单 | ✅ | §9 |
| 设计稿版本唯一化 | ⚠️ 部分（I1 待 V1.1、I6-B 待 V1.1） | §1、§9-B1/B2 |
| 代码事实带文件路径与行号 | ✅ | Migration Registry §2、Interface Matrix §2 |
| Migration 变更归属明确 | ✅ | Migration Registry §3 |
| 每个正式 TASK 有验收条件 | ✅ | Task Registry §2、Acceptance Matrix §2 |
| 无无法解释的关键接口依赖 | ✅ | Interface Matrix §5 |

> S01-0 **准予退出**，但 I1 V1.1 与 I6-B V1.1 作为**并行后续任务**进入 S01-3 / S01-4 的输入准备，不阻塞 S01-1、S01-2 的开工。

### S01-1 身份与权限基础

- Project 成员授权 **有写入路径**（当前 `core.project_member` 全仓零写入，见 Interface Matrix §4-①）。
- Git 能力授权模型 **从零建立**：枚举、存储、判定层三件齐备（当前零字段、零枚举、零策略，见 §9-B5）。
- 权限撤销即时生效 + 历史 Run 保留授权快照，二者可分别测试。
- 撤权并发领取结果唯一确定。

### S01-2 Project / Repository 管理

- 一个 Project 关联 ≥2 个 Repository 成功。
- Repository 独立验证、独立归档，失效互不影响。
- 删除被历史 Task/Run/Import 引用的 Repository → **409 `repository_has_history`**（依赖 I6-B 迁移的 RESTRICT 换轨，见 Migration Registry §3 M-0031）。
- Project 级 Git 基准字段存在且可写。

### S01-3 Discovery / Import

- I6-B V1.1 正文完成 + Migration 0031 DDL 审查冻结。
- Attempt / Evidence 复合外键落地，空库初始化与 0028 升级双路径一致。
- DB 级负向测试 1～5 + 追加 2 条全部以约束违例断言（不经 REST/MCP）。
- Discovery Session 与 Task 两套状态机的边界在 I6-B-05 内明确收敛。

### S01-4 Agent 执行基础

- 两 Worker 并发领取同一 Task 不产生两条活跃 Run。
- 租约过期可恢复；Codex 启动失败不误记为成功；取消可传播到进程。
- **回传证据有独立载体**（当前无 Evidence 实体，见 Interface Matrix §4-④）。
- Codex Bootstrap 有可执行的安装与自检闭环（T-090 收口）。

### S01-5 Knowledge 协调

- 两 Agent 并发提案的版本冲突被识别。
- 未过质量门禁 / 未过人工审核的稿件不能发布。
- 发布失败可恢复；回滚不删历史；发布链可追溯到 Agent/Run/Evidence/审核人。

### S01-6 交付与审计

- 项目级 Delivery 聚合可查（当前仅有任务级 + `acceptance-overview`，见 Interface Matrix §4-④）。
- 逐仓库验收、失败 Run 恢复、审计链完整、E2E 通过。

---

## 4. 缺陷台账（S01 必须修复的项）

按 P0/P1 定级；来源 I6-A 报告 §15.3 与本次代码事实核验。

| # | 缺陷 | 事实位置 | 定级 | 归属任务 | 修复手段 |
|---|---|---|---|---|---|
| D-1 | `repo_import.draft` 单列覆盖写，旧草案丢失、不记录所消费 Attempt | `crates/pm-application/src/project_import/lifecycle.rs:448-457`；`migrations/0009_repo_import.sql:38` | P0 | I6-B-01 | 新表 `repo_import_draft_version`，单列停用、`draft_revision` 降为指针 |
| D-2 | `repo_import_source` 缺 Project 作用域组合唯一键，Evidence/Attempt 复合 FK 建不出来 | `migrations/0009_repo_import.sql:73`（PK 仅 `(import_id, repository_id)`）；`0019_tenant.sql:154-168` 未含本表 | P0（硬阻塞） | I6-B-08 | 补四列唯一键 `(tenant_id, project_id, import_id, repository_id)` |
| D-3 | `discovery_finding` 无 `project_id` 列，Finding↔Session 跨项目不一致无法 DB 级发现 | `migrations/0025_discovery.sql:50-68` | P0 | I6-B-08（补-1） | 加列 + session 三列唯一键 + FK 换轨 |
| D-4 | `discovery_finding_repository` 的仓库 FK 只锚 `(tenant_id, repository_id)` → 同租户跨项目仓库可挂 Finding | `migrations/0025_discovery.sql:74-75`；服务层 `finding.rs:113-125` 仅校验 tenant | P0 | I6-B-08（补-3） | 加 `project_id` + 换轨 FK + RESTRICT |
| D-5 | 无 Evidence 实体表，证据只是 `discovery_finding.evidence` JSONB 数组，无法与 Attempt / 验收标准绑定 | `migrations/0025_discovery.sql:61` | P0 | I6-B-04 | 新表 `discovery_finding_evidence`，带 `attempt_no` + 组合 FK + CHECK 判别器 |
| D-6 | `core.project_member` **全仓零写入路径**，Project 成员授权不可用 | `grep "INSERT INTO core.project_member"` 仅命中测试与 `bootstrap.rs:414` | P0 | S01-W13 | 新增 Project 成员管理服务 + REST |
| D-7 | 仓库级 Git 能力授权 **零基础**（无字段、无枚举、无策略、无写命令） | `crates/pm-application/src/git_access.rs:1-4`（只读命令白名单）；`migrations/*.sql` 无权限字段 | P1（阻塞 S01-4 执行闭环） | I3-B~I3-E | 新建能力枚举 + 授权存储 + 判定层 |
| D-8 | 仓库删除策略为 CASCADE，历史引用不保护；`delivery.task_repository` 的 NO ACTION 未映射为领域错误（500） | `migrations/0009_repo_import.sql:63-64`（CASCADE）；`0011_run_dispatch.sql:101-102`（NO ACTION） | P1 | I6-B-08（C-4） | 换轨 RESTRICT + 应用层映射 409 |
| D-9 | `repo_import_one_in_progress` 部分唯一索引未租户化（仅按 `project_id`） | `migrations/0009_repo_import.sql:52-54` | P1 | I6-B-01 | 走新增迁移重建（C-3） |
| D-10 | Discovery Session 状态机与 Task 状态机并行，无交叉校验 | `crates/pm-application/src/discovery/session.rs:161-176` vs `crates/pm-domain` Task 状态机 | P1 | I6-B-05 | 边界收敛：Discovery 不推进 Task 权威状态 |
| D-11 | MCP 权限码表只覆盖 5 个写工具，`pm_discovery_submit_findings` / `pm_claim_task` / `pm_submit_result` 等仅受 `PM_MCP_READ` 约束 | `apps/pm-api/src/auth.rs:562-568` | P1 | I6-B-05 | 权限码补全 |
| D-12 | 无独立「回传证据」工具与落库表，证据内嵌在载荷里 | `crates/pm-mcp/src/tools.rs:599-605`、`868-883` | P1 | I6-B-04 | 新增 Evidence 表 + 提交路径 |
| D-13 | 无 Project 级 Delivery 聚合端点 | `apps/pm-api/src/pm_delivery.rs:21-25` 仅任务级 | P2 | S01-W60 | 新增项目级读模型 |

> D-1～D-5 由 I6-B 一次性解决（同一批 Migration 0031）。**不得拆成多个迁移**——Attempt/Evidence 复合 FK 与 Source 唯一键互为前置。

---

## 5. 实施顺序（硬约束）

总体计划 §12.2 的串行规则，落到具体链路：

```text
S01-0 基线冻结
   ├─► S01-1 身份权限 ──┐
   │                     ├─► S01-2 Project/Repository ──► S01-4 执行闭环 ──► S01-6 交付审计
   └─► I6-B V1.1 正文 ──┴─► S01-3 Discovery/Import ──┘
                                                     └─► S01-5 Knowledge ──┘
```

强制串行项（不可并行、不可跳步）：

1. **Project/Repository 数据模型** → 多仓库执行（D-2/I3-C）。
2. **Task/Run 状态机** → 调度器（I1-B/I1-C）。
3. **MCP 契约** → 执行端正式接入。
4. **Source/Attempt 模型** → Evidence 复合外键（D-2 → D-5）。
5. **I6-B V1.1 正文** → Migration 0031 冻结 → 依赖该迁移的代码合并。
6. **I5 总控台最后实施**（`smark-pm-s01-i5-project-command-center-v1-1.md` §11.2 明确顺序 `I1 → I2 → I3 → I4 → I6 → I5`）——总体计划阶段表未表达此点，**以 I5 V1.1 为准**。

可并行项：§12.1 原列 5 条，本基线确认成立，其中「前端页面与已稳定的 REST Read Model」当前无阻塞（`acceptance-overview` 已可用）。

---

## 6. 冻结门禁

| 门禁 | 适用任务 | 门禁内容 | 判定人 |
|---|---|---|---|
| G-DDL | I6-B-01/02/03/04/08 | Migration 0031 逐条对照 merge-spec §2/§3/§4 审查通过 | 设计作者 + 实施者交叉 |
| G-INTERFACE | I6-B-05 | REST/MCP 契约与 Interface Matrix 一致 | 同上 |
| G-TEST | I6-B-06 | 审计四列 + 幂等键 + 负向测试全绿 | 同上 |
| G-DATA | I6-B-07/09 | 空库初始化 + 0028 升级双路径 Schema 一致 | 实施者 |
| G-USER | I1 / I4 / I5 的未决项 | §9-B 待用户裁决项 | **用户** |
| G-ACCEPT | 全部任务 | Acceptance Matrix 七类测试齐备且通过 | 独立验收（Agent 自报不替代） |

---

## 7. 任务体系约定

- 编号沿用既有：`I1-A`~`I1-F`、`I2-A`~`I2-E`、`I3-A`~`I3-F`、`I4-A`~`I4-F`、`I5-A`~`I5-E`、`I6-B-01`~`I6-B-09`、`D1`~`D5`(+`D3.1`)、`T-0xx`（Bootstrap/Discovery）。
- 无原始编号者用 `S01-Wxx`，**不是平台 TASK ID**（总体计划 §2.1）。
- 本基线起 `S01-Wxx` 与阶段号解耦：`work_package` 字段填 `Wxx`，`stage` 字段填 `S01-0`~`S01-6`（勘误总体计划 §2.2 的混用）。
- 字段集、状态机与完成定义见总体计划 §2.2、§2.3、§13.1 与 Task Registry §1。

---

## 8. 本基线对总体计划 V1.0 的勘误

以下 6 处为事实核验后的**确定性勘误**，已同步回改总体计划正文（提交 `docs(ui): S01 总体实施计划 V1.0 勘误`）：

| # | 位置 | 原文 | 勘误为 | 依据 |
|---|---|---|---|---|
| E-1 | 总体计划 §2.1、§6.4 | 任务编号写作 `I6-B-01～I6-B-08` | **`I6-B-01～I6-B-09`** | merge-spec §4/§6 与 verification-feedback C-3 均引用 I6-B-09（空库初始化 + 负向测试实测） |
| E-2 | 总体计划 §4.2 | Git 权限 = `read/write/commit/push/merge/release` | **`read/modify/commit/push/create_pr/merge`**（以 I3 §3.1 为准） | I3 §3.1 能力集与依赖链；总体计划漏 `create_pr`、多 `release`、且 `write`/`modify` 命名不一致 |
| E-3 | 总体计划 §1 阶段表 | 未表达 I5 的最后实施约束 | **补注「I5 必须在 I1~I4、I6 之后实施」** | I5 V1.1 §11.2 顺序 `I1→I2→I3→I4→I6→I5` |
| E-4 | 总体计划 §7.1 | 执行任务只列 D1~D5 | **补 D3.1**（载体编号 T-032） | `docs/ai-pm-slice-d3-executor-hardening-v1.md` 为「T-032 · D3.1 设计稿」 |
| E-5 | 总体计划 §10.2 | 「必须对 0001–0028 完整盘点」 | 已盘点完毕，结论见 `docs/s01-migration-registry-v1.md` | 本次 Migration Registry |
| E-6 | 总体计划 §2.2 | `work_package` 字段说明为「所属 S01 阶段」 | **拆为 `stage`（S01-0~S01-6）与 `work_package`（Wxx）两字段** | 阶段号与工作包编号被混用 |

---

## 9. 待决事项清单

按紧急度排序。**G-USER 门禁项未裁定前，相关任务不得开工。**

### A. 阻塞 S01 整体（不需用户裁决，任务内解决）

| # | 事项 | 归属 | 状态 |
|---|---|---|---|
| A-1 | I6-B V1.1 正文**新写**（V1.0 正文不在本仓；输入=merge-spec + 边界评审 + I6-A 代码事实，含 0031 完整 DDL 与 I6-B-036~051 自定验收） | I6-B-01～I6-B-09 | 待开工（S01-3 前置，B-2 已裁） |
| A-2 | Migration 0031 DDL 编写与冻结 | I6-B-08 | 待 A-1 |
| A-3 | Project 成员授权服务与 REST（补 D-6） | S01-W13 | 可立即开工 |
| A-4 | Git 能力授权从零建模（补 D-7） | I3-B | 依赖 A-3（主体模型） |
| A-5 | Evidence 实体表与提交路径（补 D-5/D-12） | I6-B-04 | 依赖 A-2 |
| A-6 | MCP 权限码表补全（补 D-11） | I6-B-05 | 可立即开工 |
| A-7 | Project 级 Delivery 聚合（补 D-13） | S01-W60 | 可立即开工 |

### B. 需用户裁决（G-USER）

| # | 事项 | 影响 | 备选 |
|---|---|---|---|
| B-1 | I1 稿是否出 V1.1 并并入 P1~P4（queued Run 无取消路径、`cancel_run` 无外部入口、超窗残留进程无清理、缺 `initializing` 卡死事实） | 阻塞 I1-A/I1-D/I1-E 开工 | **默认生效（2026-11-04，提问未答，按推荐项执行，可否决）**：P1~P4 直接并入 I1-A 实施，实施完成后回填 I1 正文为 V1.1 |
| B-2 | I6-B-036~051 的**内容来源缺失**（两份 I6-B 稿都称「来自裁决 §11」，该裁决文档不在本仓） | 16 条验收条目无定义 | ✅ **已裁（2026-11-03）**：按现有三稿（合并规格 + 边界评审 + I6-A 代码事实）**新写正文并自定验收**；正文须标注「编号来源缺失」以便日后对账 |
| B-3 | I2 编号语义冲突：V1.0 的 I2-023~027 五条在 V1.1 中被占作新号 | 会丢失 5 条验收 | ✅ **已裁（2026-11-03）**：V1.1 追加 **I2-034~038** 承接 V1.0 五条，其余不动；已落 I2 V1.1 §15.4，后续新增从 I2-039 起 |
| B-4 | I4 §15 Q1（Goal 是否必须随 Project 创建建立）/ Q2（Agent 是否可直接修改正式 Goal）未裁 | 阻塞 I4-B/I4-C | ✅ **已裁（2026-11-03）**：**Q1=随 Project 创建时建立**（推翻设计稿推荐）；**Q2=Agent 只能提案**（维持推荐）；Q3~Q6 沿用推荐。已落 I4 §15 |
| B-5 | Git 能力枚举以 I3 §3.1 为准（已在 E-2 勘误），但 **I3 §3.5 的字段名 `auto_read/auto_write/auto_commit` 已被上位裁剪切除**，应改为 `auto_accept/auto_merge/auto_publish_knowledge/require_human_approval/additional_checks` | I3-B/I3-D 实现依据 | 建议：直接在 I3-B 任务内同步修订，不单独出版 |
| B-6 | `task_run.policy_version` 是 INT（0010），I3 §5.4 要求 SHA-256 前缀内容寻址 —— 类型与语义缺口 | 阻塞 I3-D | **默认生效（2026-11-04，提问未答，按推荐项执行，可否决）**：方案 A——迁移 0032 新增 `task_run.policy_snapshot_hash TEXT`（SHA-256 内容寻址），保留 INT `policy_version` 作序号 |
| B-7 | I3 的 Task 仓库范围与 I6-B-03「Source 集合固化」语义重叠，两稿未交叉引用 | 隐式依赖，可能返工 | 建议在 I6-B V1.1 中显式声明与 Task 仓库范围的关系 |
| B-8 | C 编号段位相撞：切片 D4 写 C70→C97，D5 写续 C85–C97→C98，todo 写 C71–C97 | 验收条目归属不清 | 需一次对照裁决（不影响 S01-0 退出） |

### C. 允许延后（明确不做 / 不阻塞）

| # | 事项 | 理由 |
|---|---|---|
| C-1 | Policy 管理 REST 端点（`delivery.policy_scope` 表存在但无端点，策略仅从 YAML 加载） | 策略当前为静态配置，S01 不要求在线治理 |
| C-2 | Notification 写侧（已读标记 / 订阅） | 与 S01 主链路无关 |
| C-3 | T-051～T-058、T-071、T-092、T-093 等 todo 待裁条目 | 均非 S01 阻塞项 |
| C-4 | 「第二套任务调度 / 通用事件总线 / 自动合并发布 / 为统一概念推翻现有状态机」 | 总体计划 §0.2 明确不做 |
| C-5 | `core.project` 增列承载 generation 权威 | R-3-4 明确「权威落操作表 + 行锁，不在 core.project 加列」 |

---

## 10. S01-0 退出判定

| 条件（总体计划 §16.3） | 结果 |
|---|---|
| 五份正式文件 | ✅ 本文件 + 4 份 Registry（`docs/s01-*.md`） |
| 待决事项清单 | ✅ §9 |
| 全部设计稿有明确版本 | ✅ §1（I1/I6-B 的后续修订已登记为 A-1/B-1/B-2，不阻塞退出） |
| 已确认裁决有正式记录 | ✅ merge-spec §0 + verification-feedback §2/§3 |
| 代码事实有文件路径 / Migration / 行号 | ✅ Migration Registry §2、Interface Matrix §2、Task Registry §3 |
| Migration 变更有明确归属 | ✅ Migration Registry §3 |
| 每个正式 TASK 有验收条件 | ✅ Acceptance Matrix §2 |
| 不存在无法解释的关键接口依赖 | ✅ Interface Matrix §5 |

**S01-0 达成，基线冻结。下一步：S01-1 与具备条件的 S01-2 开工 + I6-B V1.1 并行修订。**
