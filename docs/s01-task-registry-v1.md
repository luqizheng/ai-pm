# S01 TASK Registry V1.0

> 文档性质：S01 正式交付体系（`docs/s01-implementation-baseline-v1.md` §9 第二份）
> 上位：**`docs/s01-implementation-baseline-v1.md`（正式实施基线 V1.0，唯一总基线）**；`docs/s01-implementation-plan-v1.md`（阶段出口与门禁）、`docs/ui/smark-pm-s01-overall-implementation-plan-v1.md`（总体计划 V1.0）
> 代码基线：`feat/s01-implementation`（master 2933db8 + S01 批次）　版本：V1.1　日期：2026-11-03
> 字段定义：正式基线 §7（13 项必备字段）+ 总体计划 §2.2（拆分为 `stage` 与 `work_package` 两字段，见实施计划 §8 E-6）

---

## 1. 字段与状态口径

| 字段 | 说明 | 基线依据 |
|---|---|---|
| `task_key` | 原始 TASK 编号（不改名，总体计划 §2.1） | §7 TASK ID |
| `stage` | S01 阶段：`S01-0`~`S01-6` | 总体计划 §1 |
| `work_package` | 总体计划 §3.1/§4/§5/§6/§8/§9 的 `S01-Wxx`（**非平台 TASK ID**） | 总体计划 §2.1 |
| `title` | 任务名称 | §7 |
| `objective` | 目标 | §7 |
| `source_design` | 来源设计稿及版本（**设计依据**） | §7 设计依据 |
| `code_facts` | 现有实现与核验结论（引用 `docs/s01-code-facts-ledger-v1.md` 的条目号） | §7 代码事实 |
| `dependencies` | 前置 TASK | §7 依赖 |
| `migration_scope` | 涉及的 Migration | §7 Migration |
| `rest_scope` | REST 改动 | §7 API |
| `mcp_scope` | MCP 改动 | §7 API |
| `worker_scope` | Worker 改动 | §7 Worker |
| `code_scope` | 代码模块 | §7 实施范围 |
| `ui_scope` | 关联页面与组件（无前端改动填「无」） | §7 UI |
| `test_scope` | 单元 / 集成 / 并发测试 | §7 测试 |
| `acceptance` | 验收标准 + **验收级别 L1～L5**（详见 `docs/s01-acceptance-matrix-v1.md`） | §7 验收、§8 |
| `parallelism` | 串行 / 接口依赖 / 可并行 | 总体计划 §12 |
| `freeze_gate` | 冻结门禁（见实施计划 §6） | 总体计划 §6.5 |
| `evidence` | **证据**：提交号 + 测试报告 + 运行结果（完成状态必须有） | §7 证据 |
| `status` | `已实施` / `仅设计` / `未开工` / `待裁` / `延后` | — |

状态机（总体计划 §2.3）：TASK 状态、Run 状态、Delivery 状态**独立管理**；Agent 执行成功 ≠ TASK 完成；Commit 成功 ≠ 交付验收通过。

**实施纪律（基线 §7）**：一个 TASK 有明确业务边界；不跳过前置依赖；不未经裁决改变已冻结设计；发现代码事实与基线冲突先提交差异；发现新的 P0/P1 先暂停受影响范围；所有完成状态必须有测试或运行证据。

---

## 2. 任务总表

### 2.1 S01-0 统一实施基线

| task_key | stage | W | title | source_design | dependencies | migration | rest | mcp | worker | code | acceptance | parallelism | gate | status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-W01 | S01-0 | W01 | 汇总 I1–I6 设计稿（设计索引） | 总体计划 §3.1 | — | — | — | — | — | `docs/ui/*` | 实施计划 §1 | 可并行 | — | 已完成 |
| S01-W02 | S01-0 | W02 | 设计稿与代码事实核对（代码事实矩阵） | 总体计划 §3.1；I6-A §15 | W01 | — | — | — | — | 实施计划 §2 | 可并行 | — | 已完成 |
| S01-W03 | S01-0 | W03 | 数据库 Migration 盘点 | 总体计划 §10 | W02 | 0001–0028 全量 | — | — | — | `migrations/` | Migration Registry §2 | 可并行 | — | 已完成 |
| S01-W04 | S01-0 | W04 | REST/MCP/Worker 接口梳理 | 总体计划 §11 | W02 | — | 全量 | 全量 | 全量 | `apps/pm-api`、`crates/pm-mcp` | Interface Matrix | 可并行 | — | 已完成 |
| S01-W05 | S01-0 | W05 | 状态机和权限边界核对 | 总体计划 §3.1 | W02 | — | — | — | — | 实施计划 §4 缺陷台账 | 可并行 | — | 已完成 |
| S01-W06 | S01-0 | W06 | TASK 依赖关系梳理 | 总体计划 §3.1 | W01–W05 | — | — | — | — | 本文 | Acceptance Matrix | 串行（W01–W05 后） | — | 已完成 |
| S01-W07 | S01-0 | W07 | 统一验收规范 | 总体计划 §13 | W06 | — | — | — | — | `docs/s01-acceptance-matrix-v1.md` | Acceptance Matrix §1 | 串行（W06 后） | — | 已完成 |

### 2.2 S01-1 身份、租户、Project 权限基础

| task_key | stage | W | title | source_design | dependencies | migration | rest | mcp | worker | code | acceptance | parallelism | gate | status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-W10 | S01-1 | W10 | 身份上下文与调用人识别 | ADR-0001/0002；T-088 已实施 | — | 0027/0028 已有 | 已有 `/v1/pm/me` | caller 上下文已有 | — | `apps/pm-api/src/auth.rs`、`crates/pm-mcp/src/caller.rs` | 已有测试覆盖 | 可并行 | — | 已实施（需扩展：多 Project 身份） |
| S01-W11 | S01-1 | W11 | Tenant 成员管理 | `tenant_members.rs` | W10 | 0019 已有 | 已有 6 端点 | — | — | `apps/pm-api/src/tenant_members.rs` | 已有 | 可并行 | — | 已实施 |
| S01-W12 | S01-1 | W12 | 角色与权限管理 | 总体计划 §4；T-055/T-058 待裁 | W11 | — | **缺 Policy 管理端点（延后 C-1）** | 权限码表 | — | `delivery.policy_scope` | 待裁 | 可并行 | G-USER(B-4/T-058) | 部分实施 |
| S01-W13 | S01-1 | W13 | **Project 成员授权** | 总体计划 §4.1 W13；缺陷 D-6 | W11 | **0030**（role CHECK + 索引，无新表无回填） | **新增 4 端点**（members 列表/授予/撤销 + owner-transfer） | 成员门已复用 | — | `core.project_member`（写侧已补） | Acceptance Matrix §3-A（S01-A001~A016） | 可并行 | — | **已实施（2026-11-03）** |
| S01-W14 | S01-1 | W14 | Agent 执行授权（Agent Principal 校验） | ADR-0001 γ+δ；T-051/T-057 | W10、W13 | 0020/0027 已有 | — | `resolve_project_executor` 已有 | — | `crates/pm-application/src/authz.rs` | 已有 | 可并行 | G-USER(T-051/T-057) | 部分实施 |
| S01-W15 | S01-1 | W15 | 权限撤销即时生效 | 总体计划 §4.2/§4.3 | W13、W14 | — | — | — | — | 撤权路径 | Acceptance Matrix §3-A 并发/幂等 | 串行（W13 后） | — | 未开工 |

### 2.3 S01-2 Project / Repository 管理

| task_key | stage | W | title | source_design | dependencies | migration | rest | mcp | worker | code | acceptance | parallelism | gate | status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-W20 | S01-2 | W20 | Project 生命周期（含归档） | I1 | W13 | 0001 已有 | 列表/创建已有；**缺单 Project GET 与归档端点** | — | — | `core.project` | Acceptance Matrix §3-B | 可并行 | — | 部分实施 |
| S01-W21 | S01-2 | W21 | Repository CRUD | 总体计划 §5 | W20 | 0008/0009 | 已有 6 端点（**缺单仓 GET**） | — | — | `pm_repositories.rs` | 已有 | 可并行 | — | 大部分已实施 |
| S01-W22 | S01-2 | W22 | Project–Repository 多仓关联 | 总体计划 §5.2 | W21 | `project_repository` 已有 | 已有列表/创建 | — | — | `0008_repositories.sql` | Acceptance Matrix §3-B | 可并行 | — | 已实施 |
| S01-W23 | S01-2 | W23 | Repository 独立验证 | 总体计划 §5.1 | W21 | 0009 已有 | 已有 `/verify` | — | — | `verify_repository` | 已有 | 可并行 | — | 已实施 |
| S01-W24 | S01-2 | W24 | 仓库级授权 | I3-C；缺陷 D-7 | W21、I3-B | 需新增 | 需新增 | — | — | 授权判定层 | Acceptance Matrix §3-B | 串行（I3-B 后） | G-USER(B-5/B-6) | 未开工 |
| S01-W25 | S01-2 | W25 | Git 基准管理（Commit 基准） | I6-B-03（N-1 write-once SHA） | W22、I6-B-02 | 0031（source 唯一键） | — | — | — | `repo_import_source` | Acceptance Matrix §3-B 数据类 | 串行（I6-B-02 后） | G-DDL | 未开工 |

### 2.4 S01-3 Discovery / Import

| task_key | stage | W | title | source_design | dependencies | migration | rest | mcp | worker | code | acceptance | parallelism | gate | status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| S01-W30 | S01-3 | W30 | Discovery Session 生命周期 | 0025 已实现；I6-B-01 | — | 0031（operation 表） | 已有 8 端点 | `pm_discovery_*` 6 工具 | 无分析作业 | `discovery/session.rs` | Acceptance Matrix §3-C | 可并行 | G-DDL | 部分实施 |
| S01-W31 | S01-3 | W31 | Import 生命周期 | I6-B-01（C-2 显式终止） | W30 | 0031 | 已有 6 端点 | — | `handle_repo_job` 已有 | `project_import/lifecycle.rs` | Acceptance Matrix §3-C | 串行（0031 冻结后） | G-DDL | 仅设计 |
| S01-W32 | S01-3 | W32 | Source 与 Attempt（R-4-1 facts 迁 Attempt） | I6-B-02/03 | W31 | 0031（`repo_import_source_attempt`） | — | — | 需 Attempt 记录 | `repo_facts.rs`、`lifecycle.rs` | Acceptance Matrix §3-C 数据类 | 串行 | G-DDL | 仅设计 |
| S01-W33 | S01-3 | W33 | Finding 领域规则与人工确认 | I2-B/I2-C/I2-D | W30 | 0031（finding 加 project_id） | 已有 findings 端点 | `pm_discovery_submit_findings` | — | `discovery/finding.rs` | I2-001~033 | 可并行 | G-INTERFACE | 仅设计 |
| S01-W34 | S01-3 | W34 | Question / Answer | 0025 已实现 | W30 | 0025 已有 | 已有 3 端点 | `pm_discovery_submit_answers` | — | `discovery/question.rs` | 已有 | 可并行 | — | 已实施 |
| S01-W35 | S01-3 | W35 | **Evidence 实体**（缺陷 D-5/D-12） | I6-B-04（R-6-2 CHECK 判别器） | W32、W33 | 0031（`discovery_finding_evidence`） | 需新增 | 需新增回传路径 | — | 新模块 | Acceptance Matrix §3-C 数据类 | 串行 | G-DDL | 仅设计 |
| S01-W36 | S01-3 | W36 | Proposal（Discovery 提案） | I6-B-05 | W35 | 0031（换轨 FK） | 已有 | `pm_discovery_submit_proposal` | — | `discovery/proposal.rs` | Acceptance Matrix §3-C | 串行 | G-INTERFACE | 部分实施 |
| S01-W37 | S01-3 | W37 | Worker 恢复与幂等（N-5 / R-3-1） | I6-B-06/07 | W31、W32 | 0031 | — | — | `ai-pm-worker` | `run_dispatch/lease.rs` | Acceptance Matrix §3-C 恢复/幂等 | 串行 | G-TEST / G-DATA | 仅设计 |

#### I6-B 任务细分（合并规格 V1.0 §2 与总体计划 §6.4）

| task_key | stage | W | title | source_design | dependencies | migration | rest | mcp | worker | code | acceptance | parallelism | gate | status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| I6-B-01 | S01-3 | W30 | Project 初始化操作、占有与释放、generation、active 唯一性、取消及终态恢复 | merge-spec §1/§5；R-3-1~R-3-4；C-2 | — | 0031（`core.project_initialization_operation` + 重建 `repo_import_one_active_per_project`） | 已有 imports 端点 | — | — | `project_import/` | I6-B-036~051（内容待 B-2 补齐） | 串行 | G-DDL | 仅设计 |
| I6-B-02 | S01-3 | W32 | Source / Attempt 数据归属与重试语义 | boundary §2.2；R-4-1 | I6-B-01 | 0031（Attempt 表 + Source 删 facts 列） | — | — | Attempt 记录 | `project_import/` | 同上 | 串行 | G-DDL | 仅设计 |
| I6-B-03 | S01-3 | W32 | Source 集合固化与 Commit 基准 | 总体计划 §6.4；N-1 | I6-B-02 | 0031（write-once SHA 列 + 唯一键） | — | — | — | `project_import/lifecycle.rs` | 同上 | 串行 | G-DDL | 仅设计 |
| I6-B-04 | S01-3 | W35 | Finding / Evidence 模型与证据类型判别 | boundary §3.3；R-6-2 | I6-B-02 | 0031（`discovery_finding_evidence` + CHECK） | — | — | — | 新模块 | 同上 | 串行 | G-DDL | 仅设计 |
| I6-B-05 | S01-3 | W36 | Discovery 权限、提案与接口收敛 | 总体计划 §6.4；缺陷 D-10/D-11 | I6-B-04、I2-C、I2-D | 0031（换轨 FK） | 收敛 | MCP 权限码表补全 | — | `auth.rs`、`discovery/` | 同上 | 串行 | G-INTERFACE | 仅设计 |
| I6-B-06 | S01-3 | W37 | 审计与幂等 | N-4；总体计划 §6.4 | I6-B-05 | — | — | 幂等键透传 | — | `audit.rs` | 同上 | 串行 | G-TEST | 仅设计 |
| I6-B-07 | S01-3 | W37 | 历史数据与兼容性 | C-3；总体计划 §6.4 | I6-B-08 | 0031（回填脚本） | — | — | — | `migrations/` | 同上 | 串行 | G-DATA | 仅设计 |
| I6-B-08 | S01-3 | W35/W37 | 复合外键、跨 Project 约束与 Migration 集成 | merge-spec §2/§3/§4；R-6-1（扩展版） | 阻塞 I6-B-02/04/07 | 0031（全部 FK 换轨 + RESTRICT） | — | — | — | `migrations/0031_*` | Acceptance Matrix §3-C 数据类 | 串行 | **G-DDL（最终）** | 仅设计 |
| I6-B-09 | S01-3 | W37 | 空库初始化 + 0028 升级验证 + DB 级负向测试 1~5 | merge-spec §4/§6；C-3 | I6-B-08 | 0031 实测 | — | — | — | `migrations/` | Acceptance Matrix §3-C 数据类 | 串行 | G-DATA | 仅设计（**总体计划漏列，已勘误 E-1**） |

### 2.5 S01-4 Codex Bootstrap、执行引擎与 MCP

| task_key | stage | W | title | source_design | dependencies | migration | rest | mcp | worker | code | acceptance | parallelism | gate | status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| D1 | S01-4 | — | Run 调度与领取（原子领取、租约、调度） | slice-d §3.1 | — | 0010/0011/0023 已有 | REST 只读 | `pm_claim_task` | `claim_core` | `run_dispatch/claim.rs` | 已有 | 串行（D1→D5 顺序） | — | **已实施** |
| D2 | S01-4 | — | MCP 任务上下文接口 | slice-d §3.1 | D1 | 0015 已有 | — | `pm_get_task_context`/`pm_get_run_context` | — | `run_context.rs` | 已有 | 串行 | — | **已实施** |
| D3 | S01-4 | — | Codex 执行适配器 | slice-d §3.1 | D2 | — | — | — | `CodexExecutor` | `codex_executor.rs` | 已有 | 串行 | — | **已实施** |
| D3.1 | S01-4 | — | D3 可靠性与启动协议加固（载体编号 T-032） | `ai-pm-slice-d3-executor-hardening-v1.md` | D3、ADR-0002 | 0023/0028 | — | — | 已有 | `codex_executor.rs` | §8 十二条 | 串行 | — | **已实施**（总体计划漏列，已勘误 E-4） |
| D4 | S01-4 | — | 多仓库交付与结果回写 | `ai-pm-slice-d4-delivery-v1.md` | D3 | 0022 | — | `pm_submit_result` | 已有 | `delivery/mod.rs` | C70/C71~C97 | 串行 | — | **已实施** |
| D5 | S01-4 | — | Review、失败恢复与知识回写 | `ai-pm-slice-d5-review-v1.md` | D4 | 0026 | 已有 | — | `review/worker.rs` | `review/` | C98~C137 | 串行 | — | **已实施** |
| T-080 | S01-4 | — | 执行端 URL 引导式接入 | `ai-pm-t080-*` | T-087 | — | `/health`、`/install*` | — | — | `onboarding.rs` | 已有 | 可并行 | — | **已实施** |
| T-081 | S01-4 | — | `/mcp` 跨机暴露 | `ai-pm-t081-*` | — | — | Host 白名单 | — | — | `mcp_hosts.rs` | C166 七项 | 可并行 | — | **已实施** |
| T-082 | S01-4 | — | `pm_list_tasks` | todo §10.5 | T-088 | — | — | 已有 | — | `tools.rs` | 已有 | 可并行 | — | **已实施** |
| T-083 | S01-3 | W30 | DiscoverySession 数据模型 | todo §10.6 | 0019 | 0025 | — | — | — | `discovery/` | 已有 | 可并行 | — | **已实施** |
| T-084 | S01-3 | W34 | Web 项目访谈与发现卡片 | todo §10.6 | T-083 | — | 已有 | — | — | `discovery/web.rs` | 已有 | 可并行 | — | **已实施** |
| T-085 | S01-3 | W33 | MCP `pm_discovery_*` 六工具 | todo §10.6 | T-083 | — | — | 已有 6 个 | — | `tools.rs` | 已有 | 可并行 | — | **已实施** |
| T-086 | S01-3 | W34 | `/pm-learn` Skill | todo §10.6 | T-085、T-080/081 | — | — | — | — | `skills/` | 已有 | 可并行 | — | **已实施** |
| T-087 | S01-4 | — | Codex Bootstrap 设计稿定稿 V1.3 | `ai-pm-codex-bootstrap-v1.md` | — | — | — | — | — | `docs/` | 用户过稿 | — | — | **已实施** |
| T-088 | S01-4 | — | 服务端调用人上下文 + 成员校验 + devenv 域 | `ai-pm-t088-bootstrap-server-v1.md` | T-087、T-081 | 0027 | 已有 7 端点 | 已有 | — | `pm_devices.rs`、`device_claim.rs` | C173~C184 | 可并行 | — | **已实施** |
| T-089 | S01-4 | — | 中立技能包与分发 | 同上 | T-087 | — | `/install/packages/*` | — | — | `skills_package.rs` | C170 | 可并行 | — | **已实施** |
| T-090 | S01-4 | — | Bootstrap Skill + 自检闭环 | 同上 | T-088、T-089、T-082 | — | — | — | — | `skills/pm-ai-bootstrap` | C168/C169/C172 | 串行 | C172 需干净 Windows 环境 | **进行中 `[~]`** |
| T-091 | S01-6 | W61 | 前端两页（接入卡片 + 开发环境管理页） | 同上 | T-088 | — | — | — | — | `frontend/` | 已有 | 可并行 | — | **已实施** |
| T-092 | S01-4 | — | V2 无人值守 Runner | 同上 §十一 | V1 闭环稳定 | — | — | — | — | — | — | 延后 | — | **延后** |
| S01-W52 | S01-5 | W52 | Knowledge Coordinator（**复用已有**） | 总体计划 §8.3；T-060 | — | 0024 已有 | 已有 | 4 个文档工具 | 不参与 | `knowledge_write_coordinator.rs` | 已有 | 可并行 | — | **已实施**（无需重建） |
| S01-W53 | S01-5 | W53 | Document Proposal 稿件管理 | `ai.document_proposal` 8 态 | W52 | 0014/0024 | 已有 6 端点 | 已有 | — | `knowledge_proposal/` | 已有 | 可并行 | — | 已实施 |
| S01-W54 | S01-5 | W54 | 质量门禁 | 0014 | W53 | 0014 | 已有 | 已有 | — | `knowledge_proposal/` | 已有 | 可并行 | — | 已实施 |
| S01-W55 | S01-5 | W55 | 人工审核 | 0026 + knowledge | W53 | — | 已有 | — | — | `pm_reviews.rs` | 已有 | 可并行 | — | 已实施 |
| S01-W56 | S01-5 | W56 | 发布与回滚 | 总体计划 §8 | W54、W55 | 0014 | 已有 veto/reindex | `pm_veto_document_revision` | — | `knowledge.rs` | 已有 | 可并行 | — | 已实施 |

### 2.6 S01-6 交付与审计

| task_key | stage | W | title | source_design | dependencies | migration | rest | mcp | worker | code | acceptance | parallelism | gate | status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| I1-A | S01-2/S01-4 | W20+W15 | Project 活动门禁基础设施 | I1 §16 | — | — | 归档 + Run 取消端点 | — | — | `project_gate.rs` + `archive.rs` | Acceptance Matrix §3-E（三入口守卫） | 串行 | G-USER(B-1) | **已实施（2026-11-04，B-1 默认并入 P1~P4，正文回填 V1.1）** |
| I1-B | S01-4 | — | Run 创建入口改造 | I1 §16 | I1-A、D5 | — | — | — | `outbox_handlers.rs` | `run_executor.rs` | I1-007~009、012~013、035~036 | 串行 | G-USER(B-1) | 仅设计 |
| I1-C | S01-4 | — | Worker claim 改造（两阶段候选 + Project 锁定） | I1 §16 | I1-A、D1 | — | — | `pm_claim_task` | `claim_core` | `run_dispatch/claim.rs` | I1-010~014 | 串行 | G-USER(B-1) | 仅设计 |
| I1-D | S01-6 | W62 | 归档事务与 Run 取消 | I1 §16 | I1-A、D3.1 | — | 已有（归档 + 外部取消） | — | — | `archive.rs` + `project_lifecycle.rs` | I1-015~024（部分） | 串行 | G-USER(B-1) | **已实施（2026-11-04）** |
| I1-E | S01-6 | W62 | Executor 启动握手与持久化清理 | I1 §16 | D3.1 | — | — | — | `codex_executor.rs` | `codex_executor.rs` | I1-025~034、040 | 串行 | G-USER(B-1) | 仅设计 |
| I1-F | S01-6 | W64 | 全链路回归 | I1 §16 | I1-A~I1-E | — | — | — | — | 集成测试 | I1-035~040 | 串行 | G-ACCEPT | 仅设计 |
| I2-A | S01-0→S01-3 | W02+W33 | 现状核验与安全边界确认 | I2 V1.1 §16 | — | — | — | — | — | `discovery/finding.rs` | 交付 `docs/s01-i2-code-facts.md` | 可并行 | — | 未开工 |
| I2-B | S01-3 | W33 | Finding 领域规则（F3 原子更新、可信 human 路径） | I2 V1.1 §16 | I2-A | 0031 | 创建契约删 verification_status（I2-026） | MCP 通道 AgentRelayed | — | `finding.rs` | I2-001~022（I2-023/024/026 已落地） | 串行 | G-INTERFACE | **部分已实施（2026-11-04）** |
| I2-C | S01-3 | W33+W34 | 人工决定入口与审计 | I2 V1.1 §16 | I2-B、I1-A | 0031 | 已有（verification 端点改统一决定服务，契约兼容） | 无 decide 工具（Agent 不可决定，符合 I2） | — | `discovery/web.rs` + `pm_write::audit` | I2-014/019/020/025/029/030/035/036/037/038 + I2-034~038 承接段 | 串行 | G-INTERFACE | **已实施（2026-11-04；I2-033 待补充推导验收）** |
| I2-D | S01-3 | W33 | Finding 创建入口与 **P0 安全修复** | I2 V1.1 §16 | I2-B、I2-C | 0031 | 已修（同项目仓库校验 + 定态收口） | 已修（AgentRelayed 通道） | — | `finding.rs` | I2-D 双入口同修 | 串行 | G-INTERFACE | **已实施（2026-11-04，双入口同批）** |
| I2-E | S01-3 | W37+W64 | 并发、审计推导与全量验收 | I2 V1.1 §16 | I2-C、I2-D | — | — | — | — | 集成测试 | I2-001~033 + §15.3 十五场景 | 串行 | G-ACCEPT | **已实施（2026-11-04；6 例并发/推导新绿；场景 10 由同事务结构性保证；归档竞争完整路径待 I1-D）** |
| I3-A | S01-0→S01-1 | W02+W05 | 现有权限链路核验 | I3 §12 | — | — | — | — | — | `git_access.rs` | 交付 Git 权限现状矩阵 | 可并行 | — | 已完成（本基线 §4-D7） |
| I3-B | S01-1 | W12+W24 | Git 访问模型（**V1.1 简化**：单一 `read_write` 档位） | I3 §0.0（V1.1） | I3-A | **0032**（policy_snapshot_hash）+ **0033**（DROP 授权表） | —（授权端点已撤下） | — | — | `pm-domain::repo_access` + `pm-application::repo_access` | I3-001~006（按 §0.0.6 改写） | 串行 | G-USER(B-5/B-6) | **已实施（2026-11-04；同日按 V1.1 简化裁决重做：撤下六能力/授权表/REST，判定层改单一档位）** |
| I3-C | S01-2 | W22+W24 | Task 仓库范围 | I3 §12 | I3-B、I6-B-03 | 0031（既有 `task_repository`） | — | — | — | `task_repository` + `repo_access::build_task_repo_access_tx` | I3-007~012 | 串行 | G-DDL | **已实施（2026-11-04）：范围 = Task 关联仓 ∩ verified** |
| I3-D | S01-4 | — | Run 授权快照（SHA-256 内容寻址） | I3 §12 | I3-B | 0032（`policy_snapshot_hash`） | — | — | — | `task_run.policy_snapshot` / `repository_policy_snapshots` | I3-013~017 | 串行 | G-USER(B-6) | **已实施（2026-11-04）：快照记「可访问仓库清单」** |
| I3-E | S01-4 | — | 执行期强校验（快照 ∩ 现状） | I3 §12 | I3-D、I1-A | — | — | `pm_*` 入口 | `CodexExecutor` | `repo_access::execution_allows` + 执行器物化前过滤 | I3-018~021 | 串行 | — | **已实施（2026-11-04）：归档/仓库失验即时阻断** |
| I3-F | S01-6 | W63 | 审计与回归 | I3 §12 | I3-B~I3-E、I1-D | — | — | — | — | 集成测试 | I3-022~024（按 §0.0.6 改写） | 串行 | G-ACCEPT | **部分实施（2026-11-04）：快照生成 + 归档/取消联动回归已过；直接 Git 绕过测试属 T-090 两机实测** |
| I4-A | S01-0→S01-5 | W02+W52 | Knowledge 现状核验 | I4 §13 | — | — | — | — | — | `knowledge.rs` | 交付 `docs/s01-i4-knowledge-facts.md` | 可并行 | — | 未开工 |
| I4-B | S01-5 | W52+W53 | Goal 领域规则 | I4 §13 | I4-A | 无需新迁移（复用 UNIQUE project_id+doc_key） | GET /projects/{id}/goal | — | — | `project_goal.rs` | I4-001~006 | 串行 | G-USER(B-4) | **已实施（2026-11-04，Q1/Q2 已裁）** |
| I4-C | S01-5 | W53 | Goal 修改流程（复用 DocumentProposal） | I4 §13 | I4-B、T-060 | — | 既有提案管线 | 既有 propose/confirm 工具 | — | `knowledge_proposal/submit.rs`（Goal 强制红线） | I4-007~012 | 串行 | G-USER(B-4) | **已实施（2026-11-04）** |
| I4-D | S01-5 | W52+W54 | 多 Agent Knowledge 协调 | I4 §13 | I4-C | — | — | — | — | `knowledge_write_coordinator.rs`（已覆盖） | I4-013~018 | 串行 | — | **部分已被 T-060 覆盖** |
| I4-E | S01-4/S01-5 | — | Agent 上下文读取（Goal 版本） | I4 §13 | I4-B、I4-C | — | — | `pm_get_run_context` 已带 goal 快照 | — | `run_context.rs` | I4-019~021 | 串行 | — | **已实施（2026-11-04）** |
| I4-F | S01-6 | W62+W63 | 归档与审计集成 | I4 §13（**验收口径勘误见 ledger D-C-04**：实为 **I4-019/020**） | I4-C、I1-A | — | — | — | — | `knowledge.rs` + `knowledge_proposal/` + `project_gate.rs` | I4-019/020（归档只读 + 归档竞争） | 串行 | — | **已实施（2026-11-04）：四个知识写入口加归档门禁（create/submit/confirm/finalize），与归档事务共享 project 行锁 → 提交与归档线性化；归档后正式 Goal/提案/修订历史仍可读** |
| I5-A | S01-0→S01-3 | W02+W04 | 代码事实核验（14 项） | I5 V1.1 §9.1 | I6 数据契约定稿 | — | — | — | — | — | 交付 `docs/s01-i5-code-facts.md` | 可并行 | — | 未开工 |
| I5-B | S01-3 | W04 | 读模型契约 | I5 V1.1 §9.2 | I5-A | — | — | — | — | — | 交付 `docs/s01-i5-read-model-contract.md` | 串行 | — | 未开工 |
| I5-C | S01-3 | W60 | 聚合查询实现 | I5 V1.1 §9.3 | I5-B、I6-B-05 | — | 需新增端点 | — | — | 新读模型 | I5-001~025 | 串行 | G-INTERFACE | 未开工 |
| I5-D | S01-6 | W61 | 前端总控台 | I5 V1.1 §9.4 | I5-C、I4-E | — | — | — | — | `frontend/` | I5-026~031 | 串行 | — | 未开工 |
| I5-E | S01-6 | W64 | 验收与性能 | I5 V1.1 §9.5 | I5-D | — | — | — | — | — | I5-032~036 | 串行 | G-ACCEPT | 未开工 |
| S01-W60 | S01-6 | W60 | 项目级 Delivery 聚合（缺陷 D-13） | 总体计划 §9.1 | I5-C | — | **已有**（`GET /v1/pm/projects/{id}/delivery`） | — | — | `delivery::list_project_delivery` | S01-A066~A075 | 可并行 | — | **已实施（2026-11-04）** |
| S01-W61 | S01-6 | W61 | 交付验收页面 | 总体计划 §9.2 | S01-W60 | — | — | — | — | `frontend/` | — | 串行 | — | 部分实施（任务级页面已有） |
| S01-W62 | S01-6 | W62 | 失败恢复 | 总体计划 §9.1 | I1-D、I1-E | — | — | — | 已有 | `run_dispatch/lease.rs` | Acceptance Matrix §3-E 恢复类 | 可并行 | — | 部分实施 |
| S01-W63 | S01-6 | W63 | 全链路审计 | 总体计划 §9.3 | I3-F、I4-F | — | — | — | — | `audit.rs` | Acceptance Matrix §3-E 审计类 | 串行 | G-ACCEPT | 部分实施 |
| S01-W64 | S01-6 | W64 | 端到端验收 | 总体计划 §9.1/§15.E | 全部前置 | — | — | — | — | E2E | Acceptance Matrix §4 | 串行 | G-ACCEPT | 未开工 |

---

## 3. 状态统计与结论

| 状态 | 数量 | 说明 |
|---|---|---|
| 已实施 | 26 | D1–D5(含 D3.1)、T-080~T-089、T-091、T-083~T-086、S01-W10/W11/W20(部分)/W21~W23/W34/W52~W56 |
| 进行中 | 1 | T-090（两机实测待用户安排环境） |
| 仅设计 | 33 | I1-A~I1-F、I2-B~I2-E、I3-B~I3-F、I4-B~I4-F、I5-B~I5-E、I6-B-01~09、S01-W15/W24/W25/W31/W32/W33/W35/W36/W37/W60~W64 等 |
| 未开工 | 10 | I2-A/I4-A/I5-A/S01-W13/S01-W51 等核验与新建项 |
| 延后 | 1 | T-092 |

**结论**：S01 的工作量分布是「**执行引擎与知识域已建成（≈26 项已实施）**，**发现/接入域与授权域几乎全空（I1~I6 + Git 授权零实现）**」。真正的 S01 主体工作是 **S01-1 授权 + S01-3 Discovery/Import（I6-B）+ S01-2/I3 仓库授权** 三块，其中 I6-B 因 Migration 0031 为单一硬串行点。

---

## 4. 编号勘误与冲突登记

| # | 冲突 | 处理 |
|---|---|---|
| N-1 | **I6-B-09 在总体计划漏列**（总体计划 §2.1/§6.4 止于 08；merge-spec §4/§6 与 verification-feedback C-3 均引用 09） | 已回改总体计划（勘误 E-1），本表收录 09 |
| N-2 | **I2 编号语义冲突**：V1.0 §10 的 I2-023~027（既有 verification 端点兼容 / `set_finding_verification` 收口 / 事务原子性 / 404 跨租户 / kind-verification 一致性）在 V1.1 §15.1 中被占作新号 | **待用户裁决（B-3）**，建议 V1.1 追加 I2-034~038 承接，原编号不动 |
| N-3 | **I1 标称「最终定稿」但带 P1~P4 未落修订** | 阻塞 I1-A/D/E 开工，见 B-1 |
| N-4 | **I5 实施顺序**（V1.1 §11.2 `I1→I2→I3→I4→I6→I5`）与总体计划阶段表不一致 | 以 I5 V1.1 为准，已勘误总体计划（E-3） |
| N-5 | **Git 能力枚举两稿不一致** | 以 I3 §3.1 为准，已勘误总体计划（E-2）；I3 §3.5 字段名残留见 B-5 |
| N-6 | **C 编号段位相撞**：D4 稿写 C70→C97、D5 稿写续 C85–C97→C98、todo 写 C71–C97 | 待裁决（B-8），不影响 S01-0 退出 |
| N-7 | **D3.1 未收录于总体计划 §7.1** | 已勘误（E-4），载体编号 T-032 |
| N-8 | **Codex Bootstrap 稿双版本号**（文件头写 V1.2，元信息写 V1.3） | 统一取 **V1.3** |
| N-9 | **I6-A §15.3 的九个工作块**与 **I6-B-01~09 的九项**是两套切法，内容不对应 | 不可混用；任务归属以 I6-B V1.1 正文最终映射为准（总体计划 §6.4 末句） |
| N-10 | **I6-B-036~051 编号段位已占用但内容缺失**（来源「裁决 §11」文档不在本仓） | 待裁决（B-2） |
| N-11 | **T-051 状态冲突**（todo §11 标 `[ ]`，正文写「关闭于 T-088」） | 以 T-088 交付行为准，todo 待同步 |
| N-12 | **C138~C162 归属「Slice F」**，Slice F 不在 D1~D5 序列内 | 归属 T-087~T-091，不单列任务 |
| N-13 | **I6-B 任务表总表止于 08** | 见 N-1 |

---

## 5. 已定稿待实施的裁决清单（摘录）

来源：`docs/ui/smark-pm-s01-i6-b-v1-1-merge-spec.md` §0–§4、`docs/ui/smark-pm-s01-i6-b-v1-verification-feedback.md` §2/§3、`docs/ui/smark-pm-s01-i6-b-boundary-review-346-v1.md` §1–§3。

| 编号 | 裁决要点 |
|---|---|
| **D-1a** | `draft_ready` 可重试；重试成功后重生成草案（`draft_revision + 1`，旧版不覆盖）；进入 `proposed` 后禁止重试。落地为新表 `repo_import_draft_version`，`repo_import.draft` 单列停用、`draft_revision` 降为指针 |
| **D-2a** | 数据库级 Project 归属约束：`repo_import` / `repo_import_source` / `discovery_finding` / `discovery_finding_evidence` 全链携带 `project_id`，同 Project 一致性由 DB 保证 |
| **C-1** | Import 为一次性 bootstrap；`applied` 后再创建返回 **409 `project_not_empty`** |
| **C-2** | `proposed` 须有显式终止/放弃操作，**并入 I6-B-01 同批** |
| **C-3** | 索引变更必须走**新增迁移**（`DROP INDEX repo_import_one_in_progress` + 建 `repo_import_one_active_per_project`），不得原地改 0009 |
| **C-4** | 「有历史引用即拒绝删除仓库」以 `ON DELETE RESTRICT` 落地，DB 兜底映射 **409 `repository_has_history`**，并修 `delivery.task_repository` 的 NO ACTION→500 |
| **N-1** | `head_commit_sha` write-once 持久化；`collect` 按固定 SHA 读取 |
| **N-2** | 冻结/归档时终态化在飞 Import；`start_import` 创建时 fail-fast 校验 |
| **N-3** | `repository_ids` 为新 API 面：必填、空数组拒绝、REST 与 MCP 同契约 |
| **N-4** | Import 幂等透传客户端幂等键，复用 `integration.inbound_event` 既有唯一键 |
| **N-5** | 恢复以 lease 过期重认领为准；job 级失败路径一律 `Ok(())` |
| **R-3-1** | 初始化操作的占有与释放都必须检查 `rows_affected`，命中 0 行整体回滚 |
| **R-3-2** | 删除测试专用 `bootstrap_project_with_status(Some("initializing"))` 变体，seeding 走 `start_import` 真路径 |
| **R-3-3** | 「同 Project 至多一个 active 初始化操作」DB 化（部分唯一索引），与 `repo_import_one_active_per_project` **并存**，同事务成对建立/释放 |
| **R-3-4** | generation 权威 = 操作表 + Project 行锁，**不在 `core.project` 加列** |
| **R-4-1** | `facts` / `commit_count` / `doc_count` / `size_bytes` / `truncated` **从 Source 迁入 Attempt**；Source 只留身份/基准/当前态指针 |
| **R-6-1** | `repo_import_source` 补组合唯一键，**扩展为四列** `(tenant_id, project_id, import_id, repository_id)`；三列键无引用方默认省略 |
| **R-6-2** | Evidence 增加 CHECK 判别器：代码类证据必须 `import_id`/`repository_id`/`commit_sha`/`attempt_no` 全非空，非代码类全空；非代码来源用 `evidence_type` 表达，**不伪造 Commit** |
| **补-1** | `discovery_session` 补 `UNIQUE (tenant_id, project_id, id)`，finding→session FK 换轨为携带 project 的版本 |
| **补-2** | Evidence 不设 `source_attempt_id` UUID FK，改设 `attempt_no INT` 参与组合 FK 锚定 Attempt |
| **补-3** | `discovery_finding_repository` 同批升格（加 `project_id` + 两条换轨 FK），消除 D-09 复发路径 |
| **FK 策略** | source→import CASCADE；source→project_repository **RESTRICT**；attempt→source CASCADE；evidence→finding/source/attempt CASCADE；finding→session CASCADE；finding_repository→repository **RESTRICT**；project_repository→project CASCADE |

---

## 6. Registry 维护规则

1. **只追加不删除**：任务状态原地改；已作废任务移入 §4 勘误表并注明作废依据。
2. **编号不改名**：既有 `D1`~`D5`、`I1-A`~`I6-B-09`、`T-0xx` 一律沿用；新任务用 `S01-Wxx`，**待接入平台 Task 后回填真实 TASK ID**（总体计划 §2.1）。
3. **Migration 归属唯一**：一个数据库变更只有一个主责 TASK（实施计划 §12.3）。
4. **验收编号归属唯一**：同一验收编号不得归属两个 TASK；冲突时按 Acceptance Matrix §3 的裁决表执行。
5. **状态独立**：TASK / Run / Delivery 状态三者独立，不互相代替。
