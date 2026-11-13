# S01 Evidence Level 审计（04-evidence-level-audit）

> Phase 1 R2.1 交付 · 2026-11-06 · QA Test Architect
> 职责：记录 R2.1 对全部 E4 条目的重新核定结果（Test File / Test Case·Scenario ID / Development Test Result / 断言覆盖判断）及 FR-085 的 E3 重核。
> 核定规则：每条 E4 必须同时具备①测试文件、②用例名或验收矩阵登记的场景名、③开发测试通过记录；三者缺一降级。**人工实测不计入自动化证据等级**。**开发测试通过 ≠ 独立 QA 验收通过**。

## 1. 重点核查 4 条的核定结论（审查指令 §3）

| ID | R2 等级 | 核查发现 | R2.1 等级 | 依据 |
|---|---|---|---|---|
| S01-UX-027 | E4 | `CommandCenterView.test.ts` 仅 2 用例（`注册路由后渲染六大区块与设计稿样本数据`、`页头「项目设置」发出 settings 事件`），**无「Needs Attention 优先于统计卡片与初始化进度」的顺序断言**；「首屏 5 例」无对应测试文件 | **E2** | 组件渲染证据存在；顺序断言列 Phase 3 UI 实证 |
| S01-SEC-054 | E4 | `PersonalHomeView.test.ts` 7 用例无「不可见项目不出现」断言；`project_members.rs::outsider_cannot_see_project_members` 验证的是成员 API 而非 Personal Home 列表过滤 | **E2** | 后端过滤代码 + 间接用例（`command_center.rs::non_member_cannot_read_command_center` 为补充）；直接断言列 Phase 3 实证 |
| S01-FR-053 | E4 | `PersonalHomeView.test.ts::「新建项目」进入 S01-02 项目创建页`——断言 `push("/project/create")`（:219-231 实测确认） | **E4 维持** | 用例名 + 断言明确 + 通过记录 |
| S01-FR-021 | E4 | `pm_baseline.rs::proposal_decision_and_confirm_flow`——覆盖提案→逐项决策→确认流程（与需求描述一致） | **E4 维持** | 用例名 + 场景覆盖 + 通过记录 |

## 2. FR-085 的 E3 重核（审查指令 §4）

| 项 | 结论 |
|---|---|
| C166 跨机实测性质 | **人工跨机实测**（T-090 两机实测项，`todo.md` §10.7 登记「待安排环境」的实测已由 Codex 执行线完成）——**不属于自动化测试，不得作为 E3 依据** |
| 存在的自动化测试 | `tests/onboarding.rs::health_is_public_and_me_is_not`（/health 公开性）；`tests/pm_devices.rs`（`register_reuse_and_server_generated_id` / `detail_visibility_and_404` / `bind_checks_revoke_flow` / `c184_only_tools_call_triggers_device_layer` / `c176_http_side_ignored_variants_and_valid_claim`——设备层 C157~C184 区间） |
| `/install` 端点 | 无对应自动化用例确认 |
| 核定结论 | **E3 → E2**：存在部分自动化测试但未与 S01-06 需求逐条映射；DOCUMENT_MISSING 维持（**实现存在不等于正式需求已完整**） |

## 3. DATA-020 的 EV 重核（审查指令 §1）

| 项 | 结论 |
|---|---|
| 读取路径证据 | `command_center.rs` 每请求实时查询领域表（无会话态、无缓存层）——**业务代码证据成立** → EV E0→**E2** |
| 一致性验证 | 无任何刷新/退出登录/重新登录/重新打开项目后的 InitItem 一致性测试记录 → 完整行为未验证 |
| 状态 | IMPLEMENTATION_NOT_FOUND → **IMPLEMENTATION_GAP**（部分实现依据存在，完整行为尚未验证） |
| Phase 3 义务 | 必须独立执行六步验收（刷新、退出登录、重新登录、重新打开项目后的 InitItem 状态/required/source_state/expanded 逐项对比） |
| 红线 | 不得因为缺少 E5 就认定实现不存在；也不得因读取路径存在就认定一致性已验证 |

## 4. E4 全量核定清单（41 条）

> 41 条 = M1×1 + M2×5 + M3×3 + M4×6 + M5×2 + M6×11 + M7×11 + M9×2。

### M1（1 条）

| ID | Test File | Test Case / Scenario ID | Dev Test Result | 断言覆盖判断 |
|---|---|---|---|---|
| S01-SEC-010 | `apps/pm-api/tests/project_members.rs` | `grant_makes_member_able_to_see_project` / `outsider_cannot_see_project_members` / `cross_tenant_project_is_not_found` / `grant_requires_tenant_membership` / `grant_rejects_agent_principal` / `plain_project_member_cannot_grant` / `duplicate_grant_keeps_single_row` / `revoke_takes_effect_immediately` / `cannot_revoke_project_owner` / `owner_transfer_moves_ownership` / `tenant_removal_cascades_project_memberships` 等 15 例 | 全绿（S01-A001~A016 验收记录） | ✅ 直接覆盖可见性/404/不泄露存在性 |

### M2（5 条）

| ID | Test File | Test Case / Scenario ID | Dev Test Result | 断言覆盖判断 |
|---|---|---|---|---|
| S01-BR-017 | `apps/pm-api/tests/command_center.rs` | `new_project_renders_all_blocks_and_zero_repository_is_legal`（内含断言 `:146` completion=not_determinable、`:163` `assert!(!raw.contains("percent"))`） | 全绿 | ✅ 逐字断言确认 |
| S01-FR-018 | `apps/pm-api/tests/command_center_acceptance.rs` | `initialization_progress_collapses_and_expands_again` | 全绿（I5-027/028 映射） | ✅ 直接覆盖 expanded 翻转 |
| S01-FR-021 | `apps/pm-api/tests/pm_baseline.rs` | `proposal_decision_and_confirm_flow` | 全绿 | ✅ 覆盖提案→决策→确认流程 |
| S01-BR-025 | `apps/pm-api/tests/command_center.rs` | `new_project_renders_all_blocks_and_zero_repository_is_legal`（I5-023：code_analysis=not_applicable 且 required=false） | 全绿 | ✅ 直接覆盖不适用不阻塞 |
| S01-UX-028 | `apps/pm-api/tests/command_center_acceptance.rs` | `initialization_progress_collapses_and_expands_again` | 全绿 | ✅ 收缩/再展开断言 |

### M3（3 条）

| ID | Test File | Test Case / Scenario ID | Dev Test Result | 断言覆盖判断 |
|---|---|---|---|---|
| S01-BR-030 | `apps/pm-api/tests/command_center.rs` | `new_project_renders_all_blocks_and_zero_repository_is_legal` | 全绿 | ✅ 零仓库合法性直接断言 |
| S01-SEC-035 | `apps/pm-api/tests/repo_access_enforcement.rs`（8 例）+ `tests/run_scope_snapshot.rs`（4 例） | `out_of_scope_repository_is_denied` / `repository_no_longer_verified_blocks_execution` / `archived_project_blocks_execution` / `missing_snapshot_denies_everything` / `in_scope_verified_repository_is_allowed` / `snapshot_of_another_repository_is_rejected` / `claim_writes_access_snapshot_and_hash` / `snapshot_is_immutable_after_claim` | 全绿 | ✅ 五级判定链逐项覆盖 |
| S01-DATA-037 | `crates/pm-application/tests/i6b_db_constraints.rs`（13 例） | `negative_1_evidence_cannot_cross_projects` / `negative_2_evidence_cannot_use_import_without_source` / `negative_3_evidence_cannot_use_missing_attempt_round` / `negative_4_finding_project_must_match_session_project` / `negative_5_finding_cannot_attach_other_project_repository` / `negative_6_code_evidence_requires_commit_linkage` / `negative_7_non_code_evidence_must_not_carry_commit` / `positive_code_evidence_with_full_linkage_is_accepted` 等 | 全绿 | ✅ 直接打库断言 23503/23505/23514 |

### M4（6 条）

| ID | Test File | Test Case / Scenario ID | Dev Test Result | 断言覆盖判断 |
|---|---|---|---|---|
| S01-FR-043 | `apps/pm-api/tests/pm_cognition.rs` | `start_run_derives_items_and_confirm_produces_knowledge` / `reject_keeps_history_and_edit_confirm_updates_content` | 全绿 | ✅ 逐项确认/拒绝覆盖 |
| S01-FR-045 | `apps/pm-api/tests/project_goal.rs` | `goal_is_created_with_project_and_is_unique` | 全绿（3 例之一） | ✅ doc_key/创建路径覆盖 |
| S01-BR-046 | `apps/pm-api/tests/project_goal.rs` | `goal_is_created_with_project_and_is_unique` | 全绿 | ✅ 唯一性断言 |
| S01-BR-047 | `apps/pm-api/tests/project_goal.rs` | `goal_modification_requires_proposal_and_human_confirmation` | 全绿 | ✅ 提案红线+人类确认覆盖 |
| S01-BR-048 | `apps/pm-api/tests/project_goal_archive.rs` | 5 例：归档拒提案 / 归档拒确认 / 归档后历史可读 / 并发三轮无新 published / 被拒写入零发布审计（I4-F 回填登记） | 全绿 | ✅ 四入口门禁覆盖 |
| S01-BR-050 | `apps/pm-api/tests/discovery.rs` | I2-025 场景（非真人决定被拒且零审计） | 全绿（19 例之一） | ✅ 真人门覆盖 |

### M5（2 条）

| ID | Test File | Test Case / Scenario ID | Dev Test Result | 断言覆盖判断 |
|---|---|---|---|---|
| S01-FR-053 | `ai-frontend/src/views/personal/PersonalHomeView.test.ts` | `「新建项目」进入 S01-02 项目创建页`（断言 `push("/project/create")`） | 通过 | ✅ 快捷入口行为直接断言 |
| S01-UX-056 | `ai-frontend/src/views/personal/PersonalHomeView.test.ts` | `整页失败时给出可重试的错误态` / `没有任何项目时显示首次登录引导，不展示概览` / `待办清空时给出平静空态而不是告警` / `Inbox 失败只塌陷 Inbox 区块，项目区块仍可用` | 通过 | ✅ 三态断言齐备 |

### M6（11 条，全部基于 `apps/pm-api/tests/discovery.rs` + G-23~25 验收记录）

| ID | Test Case / Scenario ID（登记于验收矩阵/设计稿回填） | Dev Test Result |
|---|---|---|
| S01-BR-058 | `i2_023`（HumanDirect 创建即 CONFIRMED） | 全绿 |
| S01-BR-059 | `i2_024`（AgentRelayed 恒 PENDING） | 全绿 |
| S01-BR-060 | `i2_026`（外部不可指定 verification_status） | 全绿 |
| S01-FR-061 | I2-014/019/020 场景（统一决定服务三类决定） | 全绿（19 例） |
| S01-SEC-062 | I2-025 场景（非真人被拒零审计） | 全绿 |
| S01-SEC-063 | I2-029/030 场景（G9/G10 先校验后写） | 全绿 |
| S01-DATA-064 | I2-029 场景（operation_id 重放零副作用） | 全绿 |
| S01-DATA-065 | I2-033 场景（REVISION_REQUESTED 审计推导） | 全绿 |
| S01-DATA-066 | I2-030 场景（审计同事务、before/after） | 全绿 |
| S01-SEC-067 | I2-E 双入口一致性场景 | 全绿 |
| S01-DATA-068 | I2-E 并发决定场景（6 例） | 全绿 |

### M7（11 条）

| ID | Test File | Test Case / Scenario ID | Dev Test Result |
|---|---|---|---|
| S01-FR-069 | `tests/project_archive.rs` | `archive_guards_all_entries_and_cancels_queued` / `double_archive_conflicts` | 全绿 |
| S01-BR-070 | `tests/project_archive.rs` + `command_center_acceptance.rs` | `archive_guards_all_entries_and_cancels_queued` / `archived_project_keeps_history_but_accepts_no_new_activity` | 全绿 |
| S01-FR-072 | `tests/project_archive.rs` | `archive_guards_all_entries_and_cancels_queued`（queued→cancelled 断言） | 全绿 |
| S01-BR-074 | `tests/run_executor_handshake.rs` | `failed_terminate_keeps_terminal_state_and_retries` | 全绿 |
| S01-FR-075 | `tests/run_executor_handshake.rs` | `archive_between_spawn_and_confirmation_blocks_start` / `start_confirmation_rejects_archived_project_and_terminates` / `failed_terminate_keeps_terminal_state_and_retries` / `cleanup_exhaustion_lands_in_needs_human_and_recovers` + 无时间窗清理（5 例） | 全绿 |
| S01-BR-076 | `crates/pm-infrastructure/tests/outbox_handlers.rs` | `archived_project_suppresses_task_ready_run_creation` / `run_created_before_archive_gets_cancelled_by_archive`（11 例含新增 2 例） | 全绿 |
| S01-BR-079 | `tests/project_archive.rs` | `double_archive_conflicts` | 全绿 |
| S01-BR-080 | `tests/command_center_acceptance.rs` | `archived_project_keeps_history_but_accepts_no_new_activity` | 全绿 |
| S01-BR-082 | `outbox_handlers.rs` + `tests/project_archive.rs` | `archived_project_suppresses_task_ready_run_creation` / `claim_skips_archived_project` | 全绿 |
| S01-BR-083 | `tests/project_archive.rs` | `external_run_cancel_entry`（200/409） | 全绿 |
| S01-BR-084 | `tests/project_archive.rs` | `archive_finalizes_inflight_import_and_releases_slot` | 全绿 |

### M9（2 条）

| ID | Test File | Test Case / Scenario ID | Dev Test Result |
|---|---|---|---|
| S01-SEC-089 | `apps/pm-api/tests/tenant_isolation.rs` | `xt1_xt2_cross_tenant_project_is_invisible_and_unwritable` | 全绿 |
| S01-SEC-091 | `apps/pm-api/tests/project_members.rs` | `revoke_takes_effect_immediately` / `tenant_removal_cascades_project_memberships` | 全绿 |

## 5. 核定统计

| 项 | 数 |
|---|---|
| E4 维持（R2 → R2.1） | 39 |
| E4 降级 E2 | 2（UX-027、SEC-054） |
| E4 新增（FR-021 于 R2 已升） | 0（本轮无新增） |
| **R2.1 E4 合计** | **41** |
| E3 变化 | FR-085 降 E2（C166 人工实测不计自动化）；其余 E3 维持 13 条 |
| E2 变化 | +DATA-020（E0→E2，读取路径证据）+FR-085+UX-027+SEC-054 = R2 基础上净增 |
| E0 | 3（UX-057 / FR-086 / NFR-087） |

> **最终 EV 分布（R2.1）**：E0=3 / E1=4 / E2=31 / E3=13 / E4=41 / E5=0，合计 92 ✅（由本清单与 RTM v2.1 明细逐条汇总）。
> 本清单所有「全绿/通过」均为**开发交付测试记录**，不构成独立 QA 验收结论；Phase 3 复跑后升级 E5。