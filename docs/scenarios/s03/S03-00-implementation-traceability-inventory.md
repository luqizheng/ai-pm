# S03-00 · 实施追溯清单（Implementation Traceability Inventory）V1.1

> V1.1：收编 R6（Gate 与审批决策结构路线留待 S03-05 论证的边界备注）——修订记录见 [S03-00-v1.1-revision-log.md](S03-00-v1.1-revision-log.md)。

覆盖任务书第六部分 VF-15~VF-19（Implementation Baseline 与 S04 交接）。基线 @ `3ccb42f`。

## VF-15 Task 创建及依据

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | Task 创建入口 | 2 个：REST POST /v1/pm/tasks（**不支持需求关联**）+ 命令通道 create_tasks（唯一支持依据绑定的写点） | pm_write/task.rs:6-51；executor.rs:535-617 |
| 2 | Task↔Requirement | ✅ task_requirement(task_id,revision_id) 复合 FK，按 code 解析 current_revision_id；解析失败记 findings 不阻断 | 0002:68-80；executor.rs:535-546,603-617 |
| 3 | 引用 Design Revision | ❌ 无列无表（设计实体不存在） | migrations 零命中 |
| 4 | 引用 ADR Revision | ❌ 同上 | 同上 |
| 5 | 实施范围 | 🟡 =delivery.task_repository（0011:86）：Task↔仓库集合+write_access 标记交付对象；无"范围文档/边界"语义字段 | 0011:86；0022 视图 contract CTE |
| 6 | 验收标准 | ✅ task_acceptance_criterion（0015:25）+裁决表 acceptance_criterion_verdict（0026，append-only）；E1/E2 交付证据门守卫状态机 | 0015:25；0026；task_transition.rs:92-107 |
| 7 | 实施基线 ID | ❌ 无列（基线实体不存在） | — |
| 其他 | 附属 | task_knowledge_ref（0015:42）/task_dependency（0021:9）/task_waiting（0010:90 单活跃）/task_supersession（0010:73）/task_policy_override（0016）/乐观锁 revision BIGINT（0002:60 区段） | — |

## VF-16 Run 与代码执行

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | Run 记录 Task ID | ✅ task_run.task_id 组合 FK+attempt_no UNIQUE(task_id,attempt_no) | 0010:18-41 |
| 2 | Run 记录设计依据 | ❌ 无（无设计实体） | — |
| 3 | Run 记录 Requirement Revision | 🟡 间接：Run 不直接持修订指针；需求依据锁定在 task_requirement（Task 级）；RunContract（run_context.rs:200-281）含 Goal 快照/仓库范围/租约/Task/Deliverable/验收标准/知识引用——**无 Requirement Revision 视图**；AI 侧经 MCP pm_get_task_context 读最新锁定修订（read.rs:41-53） | run_context.rs 结构清单（本人复核） |
| 4 | Run 记录 Repository Commit | ✅ 两级：领取时基线 task_run_repository(base_ref/base_commit/workspace_path)；交付时 task_repository_commit(branch/commit_sha/author/authored_at/changed_files/test_evidence) | 0011:108-121；0022:51-81 |
| 5 | 重新执行保留原始依据 | ✅ 回收=新 Run+supersedes_run_id 链；旧 Run 行/旧基线快照/旧 commit 全保留（无 DELETE；task_repository_commit FK RESTRICT） | lease.rs:140-167；0022:51-81 |
| 6 | 设计版本变化影响已有 Run | 不适用（无设计版本）；近似事实：需求修订变化不影响既有 Run（锁定机制+无重校验逻辑）；policy_snapshot_hash 每 Run 不可变 | read.rs:46-51；0032:15；run_scope_snapshot 测试 |

Run 生命周期要点：execution_mode 'human'(真人 MCP 领取)/'spawn'(Worker 无人值守)（0028:15-17；claim.rs:640-644）；领取=条件 UPDATE→claimed+租约+lease_generation+1（claim.rs:646-673）；回收/隔离/lost 恢复（lease.rs:61-90,140-167）；统一取消协议=先落终态+撤凭据后杀进程（run_executor.rs:330-381）。

## VF-17 Delivery Manifest 追溯

视图 v_task_delivery_manifest（0022:105-181；0026:88-159 重建，latest_run 只看 delivery run）列：task_id/project_id/repository_id/repository_name/write_access(delivery_required)/latest_run_id/latest_attempt_no/integration_status/latest_submitted_run_id/base_commit/commit_sha/changed_files/test_evidence/branch/commit_author/commit_authored_at/expected_delivery_count/actual_delivery_count/delivery_state（四态判定 Q1：expected=0→delivered；actual=expected→delivered；0<actual<expected→partial；actual=0→never_submitted）。

| # | 追溯问 | 结论 |
|---|---|---|
| 1 | 需求追溯 | 🟡 间接：视图无需求列；可经 task_id Join delivery.task_requirement 拿锁定 revision_id——语义上成立但非视图能力 |
| 2 | 设计追溯 | ❌ 无设计对象可追 |
| 3 | 设计 Revision 追溯 | ❌ |
| 4 | ADR 追溯 | ❌ |
| 5 | 验收依据追溯 | 🟡 两级：commit 级 test_evidence 列（0022:157）+任务级 acceptance_criterion_verdict（0026，append-only，经 /tasks/{id}/criterion-verdicts 端点读）；均不在视图内 |
| 6 | 属于哪个实施基线 | ❌ 无基线实体；视图内 base_commit 是 **Run 级**基线（领取时刻快照），非 S03 语义的实施基线 |

## VF-18 Repository Baseline

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 仓库身份 | ✅ integration.project_repository：UNIQUE(project_id,url)、role 四值、status pending/verified/unreachable/archived、ssh_key_id（一项目一把密文） | 0008:18-32；0009:14-21 |
| 2 | Branch | ✅ 两级：领取基线 task_run_repository.base_ref；交付 commit.branch（0022:158） | 0011:108-121 |
| 3 | Commit SHA | ✅ 三级：导入期 head_commit_sha（write-once 0031:68）；Run 基线 base_commit；交付 commit_sha | 0031:68；0011；0022 |
| 4 | Working Tree 状态 | ❌ 平台不管理执行端工作树；替代=交付校验 commit==执行端 HEAD 且为基线后代（D4 逐仓校验） | tools.rs pm_submit_result 校验；delivery.rs 测试 |
| 5 | 实施前基准校验 | ✅ claim 事务内逐仓快照+policy_snapshot_hash（SHA-256）；执行期判定=不可变快照上界（缺失 deny-all）∩现状（未归档+仍 verified） | lease.rs:4-57；repo_access.rs:61-92,110-170；claim.rs:701-726 |
| 6 | 执行期间仓库变更 | ✅ 交付时逐仓校验 commit 须为基线后代；快照上界 deny-all（如被移出范围/未 verified 拒绝） | pm_submit_result；repo_access.rs:140-170 |
| 7 | 交付时 Commit 记录 | ✅ task_repository_commit（UNIQUE(task_run_id,repository_id)，FK RESTRICT 不随 Run 删除丢史） | 0022:51-81 |
| 8 | 重新执行基准重校验 | ✅ 新 Run 重新 claim 重新快照（旧 Run+旧快照保留）；0033 简化后访问恒 read_write 单档（git_capability_grant 已撤 0033:26） | lease.rs:140-167；repo_access.rs:61-92 |

## VF-19 Implementation Readiness

| # | 检查 | 结论 | 现有最近似机制 |
|---|---|---|---|
| 1 | 需求完整性检查 | ❌ 无（需求实体无类型/无完整性元数据；无检查入口） | — |
| 2 | 设计审批检查 | ❌ 无对象 | — |
| 3 | ADR 决策检查 | ❌ 无对象 | — |
| 4 | 实施范围检查 | 🟡 task_repository 集合+write_access+task_dependency（依赖存在性）；无"范围齐备"判定 | executor.rs 建任务解析 |
| 5 | 验收标准检查 | 🟡 task_acceptance_criterion 存在+E1（交付已提交）/E2（验收证据）状态机守卫+审批证据门 block claim（run_dispatch 测试） | task_transition.rs:92-107；run_dispatch.rs |
| 6 | 依赖检查 | ✅ task_dependency+图锁并发测试（task_dependency_concurrency.rs 9 例）；域层 allowed_edges 守卫 | 0021:9；transition.rs |
| 7 | Repository 基准检查 | ✅ claim 快照+deny-all+D4 后代校验（见 VF-18） | repo_access.rs |

**组合可行性判断（任务书原问）**：现有三类门（依赖/仓库基线/验收证据）可作为 Readiness Gate 的**判定输入挂接点**；但「需求完整性/设计审批/ADR 决策」三类检查因对象不存在无从组合——**Readiness Gate 主体（五条件聚合判定+基线冻结+授权语义）需新建**；已有先例可复刻其形态：uq_review_approval_pending（同对象至多一个 pending）、pending 单向迁移触发器（0026:273-296）、policy_snapshot_hash 内容寻址快照（0032:15）、project_gate 行门闩与锁序（project_gate.rs:7-10,84-99）、I1 归档事务模式（archive.rs:52-174）。**边界（R6）：Gate 及 Implementation Baseline Freeze 的结构路线留待 S03-05 设计评审与批准切片论证，本清单仅陈述形态先例，不预设路线。**

## 附：生命周期与 Git 基准交叉事实（E-1/E-2 精要）

- **Task 状态机**：13 值（task.rs:15-29）、终态 4、30 边（transition.rs:32-66）+守卫（等待解除/交付已提交/验收证据/B4 失败条件/取代 intent）——域层纯函数无 I/O；legacy 5 值映射（task.rs:90-104）。
- **Run 状态**：7 值 CHECK（0011:26-28）；领取/续租 fencing=事务内持 Run 行锁重验 token/generation/状态/过期（run_context.rs:174-177）。
- **交付视图语义**：latest_run 不考虑是否提交——不得以旧 Run 交付顶替最新（0022:100-103 注释钉死）。
- **归档（I1）**：行门闩+统一锁序 project→run→task（project_gate.rs:7-10）；归档事务=锁行→archived→P1 条件取消 queued→N-2 导入终态化/名额释放→审计→提交；P2 逐 Run cancel_run("project_archived") 事务外补偿（archive.rs:52-174）；外部入口 POST /v1/pm/task-runs/{run_id}/cancel 与 /projects/{id}/archive（project_lifecycle.rs:29-36）。
- **知识空间 Git**：根=AI_PM_KNOWLEDGE_ROOT 或 `<cwd>/../ai-pm-knowledge`；路径 projects/<code>/<doc_type>/<key>.md；四写入口全部过行锁门禁（I4-F）；并发=advisory lock+operation_id 账本+base_revision 校验。
