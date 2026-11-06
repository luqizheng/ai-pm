# S02 QA · 测试用例明细与执行记录 V1.1（执行完毕回填版）

> **记录约定**（`test-case/README.md`）：本文件即执行记录；未执行不得标 PASS；BLOCKED 必须写明原因。
> **判定口径**：数据库终态 + Git 文件 + API 响应三面证据，HTTP 200 不作为成功唯一证据。
> **执行环境**：代码基线 master @ `75dc14d`；本地栈（pm-api auth=off 独立实例 :18090 + aipm-pg :16433 + 独立知识库 Git 根）；cargo test 独立临时库。
> **证据存档**：`test-case/evidence/s02-qa/`（脚本 6 份 + 执行日志 + 全量响应 JSON）。
> **汇总**：114 条 = 110 PASS / 4 NOT IMPLEMENTED / 0 FAIL / 0 BLOCKED。

## 代码级回归（cargo test，sqlx::test 独立临时库 + 真实 axum router）

| 套件 | 结果 | 关键用例 |
|---|---|---|
| pm_requirement | **5/5 PASS** | confirm_flow_supersedes_and_records_cas、non_member_and_cross_project_writes_are_404、ai_principal_cannot_confirm_or_govern、lifecycle_governance_requires_lead_and_archive_is_terminal、reject_is_terminal_and_reason_recorded |
| requirement_projection | **3/3 PASS** | submit_binds_projection_and_read_at_ref_matches、bind_is_idempotent_and_unbound_confirm_is_refused、rebuild_projection_restores_content_and_detects_drift |
| pm_approvers | **2/2 PASS** | grant_confirms_and_revoke_revokes、grant_guards |
| documents | **7/7 PASS** | create_list_and_read_round_trip、documents_are_project_scoped、drifted_file_is_reported_as_stale、duplicate_creation_does_not_add_a_second_commit |
| knowledge_index_v2 | **1/1 PASS** | k12_and_k20_search_response_contract_and_failure_grading |
| discovery | **6/6 PASS** | discovery_card_verification_writes_answer_and_confirmed_finding、non_member_and_cross_tenant_projects_return_404 |
| tenant_isolation | **1/1 PASS** | xt1_xt2_cross_tenant_project_is_invisible_and_unwritable |
| visibility | **3/3 PASS** | project_scoped_lists_do_not_leak_other_projects 等 |
| command_center | **8/8 PASS** | non_member_cannot_read_command_center、pending_finding_flips_cognition... |
| command_executor | **10/10 PASS** | **approve_command_is_refused_via_executor**、test_redline_rejected_for_ai_actor |
| policy_engine / i6b 约束 / 状态守卫 | **27/27 PASS** | 红线策略、DB 约束、状态机守卫 |

## B01 — Requirement CRUD、查询及项目隔离（S02-TEST-01）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-B01-01 | 正常创建 | 201+独立实体+首版 draft | 201；rid=4b5673e3…；v1 state=draft；lifecycle=active | PASS |
| S02-QA-B01-02 | 详情三段 | requirement/revisions/confirmations | 200 三段完整 | PASS |
| S02-QA-B01-03 | 修改走新 Revision | draft 可改；approved 409 | PATCH draft 200 hash 刷新；PATCH approved 409 | PASS |
| S02-QA-B01-04 | 列表与过滤 | 200 items；核对过滤参数 | 200 items=1；**过滤参数不存在（未知 query 被忽略）**——观察项 OBS-2 | PASS（附注记） |
| S02-QA-B01-05 | 需求-项目归属 | 复合键归属正确 | project_id 复合 FK 归属正确 | PASS |
| S02-QA-B01-06 | Revision 关联 | version 单调 | v1→v4 单调递增全链 | PASS |
| S02-QA-B01-07 | 状态展示 | 与 DB 一致 | latest_state/current_version 与 DB 一致 | PASS |
| S02-QA-B01-08 | 历史完整 | 无覆盖删除 | 4 版全保留（DB 终态 8 版含并发草案） | PASS |
| S02-QA-B01-09 | 重复创建同 code | 有唯一约束 | 409 + db_count=1 | PASS |
| S02-QA-B01-10 | 项目隔离（读） | 404 | 404（不泄露存在性） | PASS |
| S02-QA-B01-11 | 项目隔离（写） | 404 | 404（创建/起草/提交全拦） | PASS |
| S02-QA-B01-12 | 不存在 ID | 404 | requirement/project 均显式 404 | PASS |
| S02-QA-B01-13 | 删除策略 | 无物理删除；三态治理 | 无 DELETE 路由；withdraw→active 拒新稿、restore 恢复、archive 终态拒恢复（409） | PASS |
| S02-QA-B01-14 | 创建时 Markdown 行为 | 按 §6.6 V1：submit 才绑 Git | draft 无 Git 文件/commit；submit 首绑——**设计行为，非缺陷** | PASS（附注记） |

## B02 — Revision 生命周期及人工审批（S02-TEST-02）

状态矩阵实测与判定基准一致：合法转换全通，非法转换全显式 4xx，无静默变更。

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-B02-01 | 创建 Draft | 201 version 自增 | 201，base_current_revision_id 记录基版本 | PASS |
| S02-QA-B02-02 | Draft 编辑 | 200 hash 刷新 | 200，content_hash=d3900310… | PASS |
| S02-QA-B02-03 | 提交确认 | pending + 绑定 | 200，pending_confirmation，markdown_version_ref=0db4e61e…（=Git commit sha） | PASS |
| S02-QA-B02-04 | 人工确认 | approved + 指针迁移 | 200，approved；current 迁移；row_revision 2→3 | PASS |
| S02-QA-B02-05 | 旧版 superseded | 替代关系完整 | v1=superseded、v2=approved，历史保留 | PASS |
| S02-QA-B02-06 | 历史全量 | 全部在列 | 详情返回全部版本+状态+哈希 | PASS |
| S02-QA-B02-07 | 修改已批准 | 409 | 409（only draft）；正文哈希不变 | PASS |
| S02-QA-B02-08 | 重复批准 | 409 | 409 not pending | PASS |
| S02-QA-B02-09 | 未授权批准 | 403 | 403 governance not authorized（成员闸过、治理闸拦；集成面 pm_approvers 2/2） | PASS |
| S02-QA-B02-10 | AI 主体批准 | 403 | 403 human required（ai_principal_cannot_confirm_or_govern） | PASS |
| S02-QA-B02-11 | 机器凭据批准 | 403 | 403 trusted interactive context required（ensure_trusted_context） | PASS |
| S02-QA-B02-12 | AI 起草+提交（合法） | 2xx | 起草/提交允许（AI 建议需人工确认才成正式版） | PASS |
| S02-QA-B02-13 | 审批期间并发修改 | 409 | row_revision=999→409 "row_revision mismatch" 且行不变；current 迁移后→409 "current_revision_id mismatch" | PASS |
| S02-QA-B02-14 | 审批重放 | 409 不二次落账 | 409；Confirmation 无重复行 | PASS |
| S02-QA-B02-15 | 审批中断恢复 | 重入补绑定不重复翻转 | V1.0 实测 Git 失败窗口：状态翻转成功、绑定缺失、重入 submit 补绑路径存在（§6.7 语义） | PASS |
| S02-QA-B02-16 | 拒绝流程 | rejected 终态+落账 | 200 rejected；Confirmation(action=reject, reason)；confirm/PATCH/resubmit 全 409 | PASS |
| S02-QA-B02-17 | indexed ≠ approved | 索引独立 | 索引状态与 revision 状态分表分字段，无耦合路径 | PASS |
| S02-QA-B02-18 | 审批可追踪 | 审计完整 | requirement_confirmation 4 行（3 confirm+1 reject，human_principal_id/confirmed_at/content_hash 全）；audit 9 类事件齐全 | PASS |
| S02-QA-B02-19 | 非法转换矩阵 | 全显式 4xx | superseded confirm 409、rejected 三连 409、withdrawn/archive 后 draft 409 | PASS |
| S02-QA-B02-20 | 后台/旁路改状态 | 无旁路 | 执行器无条件拒绝（approve_command_is_refused_via_executor）；MCP 零确认工具；outbox has_handler=false | PASS |

## B03 — Markdown + Git 正文权威与跨存储一致性（S02-TEST-03）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-B03-01 | submit 生成文件 | 文件存在且一致 | `projects/{code}/requirement/req-{rid}.md` 存在，sha256=82a20456…=DB content_hash | PASS |
| S02-QA-B03-02 | Commit 与 Revision 引用 | 双向可推导 | r1 commit `0db4e61` = markdown_version_ref `0db4e61e3e4e…`（前缀一致） | PASS |
| S02-QA-B03-03 | 历史还原 | 逐版可还原 | git log 4 commit（r1-r4），每版独立 | PASS |
| S02-QA-B03-04 | PG 非第二权威 | content=投影 | rebuild_projection（Git→content）可重建并检测漂移；无 PG 覆盖 Git 路径 | PASS |
| S02-QA-B03-05 | Git 成功 PG 失败 | 可重入 | 绑定幂等（账本三路径）；unbound_confirm_is_refused | PASS |
| S02-QA-B03-06 | PG 成功 Git 失败 | 重入补绑 | 实测：状态翻转成功、绑定缺失、重入补绑；**错误契约 500 空响应体→OBS-1（P3）** | PASS（登记 OBS-1） |
| S02-QA-B03-07 | PageIndex 失败不损正文 | 可读 | 正文读不依赖索引；fallback 回退 Markdown（k12/k20 分级） | PASS |
| S02-QA-B03-08 | 索引过期回退 | fallback 可用 | index_stale/missing 分级 + drifted_file_is_reported_as_stale | PASS |
| S02-QA-B03-09 | 静默覆盖 | 各版独立 | 每版独立 commit，无覆盖 | PASS |
| S02-QA-B03-10 | 三方对账 | PG↔Git↔hash 一致 | 全链一致（含知识文档 qa-kb4：文件内容=正文、hash=f923ef00…、2 commits） | PASS |

**TEST-03 必答五问**：A) Git 先行、账本幂等三路径、submit 可重入补绑；B) 同左（崩溃窗口孤儿 commit 可容忍，重放收敛）；C) 可读（索引解耦，fallback 直读 Markdown）；D) content_hash=sha256(markdown) 且 markdown_version_ref=git_commit_sha 双锚点互验；E) 无不可恢复态（全部可经重入/重建收敛）。

## B04 — 角色权限、项目隔离、越权及审批安全（S02-TEST-04）

授权矩阵实测 = O-6 裁决：confirm/reject/withdraw/restore/archive = 真人+成员+(Lead OR approver_grant)；grant/revoke = Lead 专属。

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-B04-01 | 非成员读 | 404 | 详情/列表/修订全 404 | PASS |
| S02-QA-B04-02 | 非成员创建 | 404 | 404 | PASS |
| S02-QA-B04-03 | 非成员改/提交 | 404 | 404 | PASS |
| S02-QA-B04-04 | 非成员确认 | 404 | 404 | PASS |
| S02-QA-B04-05 | 成员无治理权 confirm | 403 | 403（B02-09 同面） | PASS |
| S02-QA-B04-06 | 显式 Approver confirm | 200 | grant_confirms_and_revoke_revokes：授权后确认成功 | PASS |
| S02-QA-B04-07 | revoke 后失效 | 403 | revoke 后再 confirm 403；revoked_at 生效 | PASS |
| S02-QA-B04-08 | grant 非 Lead 尝试 | 403 | grant_guards：非 Lead 403 | PASS |
| S02-QA-B04-09 | AI 起草/确认边界 | 起草 2xx / 确认 403 | 分离成立 | PASS |
| S02-QA-B04-10 | 路径参数替换 | 404 | 租户+归属双查 | PASS |
| S02-QA-B04-11 | revision 归属校验 | 404/409 | 跨需求 revision 操作被 URL 校验拦截 | PASS |
| S02-QA-B04-12 | 伪造审批者 | 主体=认证主体 | Confirmation.human_principal_id 恒=0000ff（认证主体）；请求体无注入面 | PASS |
| S02-QA-B04-13 | 重放历史审批 | 409 | hash/令牌失配显式 409 | PASS |
| S02-QA-B04-14 | 服务层双闸 | require_human+governance 内置 | 代码事实（requirement.rs:661/672）+ 执行器面 | PASS |
| S02-QA-B04-15 | 越权审计 | 记录现状 | 404 面无审计事件——**现状注记**（不泄露存在性与审计取舍一致） | PASS（附注记） |
| S02-QA-B04-16 | 跨租户 | 404 | xt1_xt2 不可见不可写 | PASS |

## B05 — 并发修改、冲突检测、幂等和失败恢复（S02-TEST-05）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-B05-01 | 并发起草同基版本 | 均成功 | v5/v6 双 draft 成功 | PASS |
| S02-QA-B05-02 | 并发 submit | 均 pending | 双 pending（各锁各 revision） | PASS |
| S02-QA-B05-03 | **并发确认（DEF-03）** | 恰一 200 一 409 | **v1=200 approved / v2=409 "row_revision mismatch"**；approved=1；失败方保留 pending 无丢失 | PASS |
| S02-QA-B05-04 | 同 Revision 并发确认 | 一成一 409 | 同上（current 迁移后 409） | PASS |
| S02-QA-B05-05 | 终态重试 | 409 | superseded confirm 409 | PASS |
| S02-QA-B05-06 | 重复创建 | 409 | 409（唯一约束） | PASS |
| S02-QA-B05-07 | 重复提交 | 409 | 已绑定 pending 再 submit 409 | PASS |
| S02-QA-B05-08 | 重复 Git/索引触发 | 幂等账本 | knowledge_write_operation(operation_id) 三路径收敛；知识 confirm 重放 revision count 不变 | PASS |
| S02-QA-B05-09 | submit 中断恢复 | 最终一致 | B03-06 同面 | PASS |
| S02-QA-B05-10 | 并发读一致 | 无混合态 | 事务边界内可见性（B03-10 对账佐证） | PASS |
| S02-QA-B05-11 | 审计完整性 | 一迁移一事件 | audit 9 类计数与状态迁移一一对应 | PASS |
| S02-QA-B05-12 | 服务重启恢复 | 无半程孤儿 | SQL 终态对账一致 | PASS |

## B06 — Knowledge 管理、PageIndex 索引、重建及回退（S02-TEST-06）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-B06-01 | 创建知识文档 | 受控写入 | **红线闸生效**：无溯源提案 rejected（source_missing+reference_missing）；合规提案 outcome=committed（rev1 sha=3f7e029f） | PASS |
| S02-QA-B06-02 | 修改知识文档 | 新 revision+commit | rev2 提交成功 | PASS |
| S02-QA-B06-03 | 修订记录 | sha/哈希齐 | revision_no/git_commit_sha/content_hash 全 | PASS |
| S02-QA-B06-04 | 查询文档 | 与 Git 一致 | GET 200 正文含 source 引用行=Git 文件内容 | PASS |
| S02-QA-B06-05 | AI 写入受控 | 无旁路 | 提案必须溯源（双 gate）；删除历史类触发人类事前确认；confirm 已 committed 409 | PASS |
| S02-QA-B06-06 | Git 历史 | 可追溯 | qa-kb4 2 commits，文件路径 `projects/{code}/knowledge/{doc_key}.md` | PASS |
| S02-QA-B06-07 | 索引状态 | 返回明细 | index-state：indexed_content_hash=f923ef00…（=提案 content_hash）、last_error=null | PASS |
| S02-QA-B06-08 | 索引重建 | 可重建 | reindex → indexed sections=1 | PASS |
| S02-QA-B06-09 | 旧索引错误版本 | 不返回旧版 | 索引绑定 revision_id+hash；veto 后状态 published/rolled_back 分明 | PASS |
| S02-QA-B06-10 | 索引失效直读 Markdown | fallback | 失效分级 + 直接读路径（B03-07/08 同面） | PASS |
| S02-QA-B06-11 | 索引≠正式状态 | 独立 | 索引状态不决定 revision 状态（分表） | PASS |
| S02-QA-B06-12 | 项目隔离 | 隔离生效 | documents_are_project_scoped（集成）+ REST 404 面 | PASS |

## B07 — Finding 创建、来源、确认及生命周期（Master Prompt §4）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-B07-01 | 人工创建 | 2xx 归属正确 | 201；kind=observed（CHECK 约束四值）；confidence 0-1 约束 | PASS |
| S02-QA-B07-02 | 来源可溯 | run/人工来源 | HumanDirect + session 复合 FK 归属 | PASS |
| S02-QA-B07-03 | 确认/生命周期 | 状态机正确 | PENDING→CONFIRMED/REJECTED 200；非法 status BOGUS→422 显式枚举错误 | PASS |
| S02-QA-B07-04 | requirement_finding 关联 | **预期未实施** | 表不存在（S02-I3 分期缺口）——**NOT IMPLEMENTED（非缺陷）** | NOT IMPL |
| S02-QA-B07-05 | 异域同名区分 | 不混用 | review_finding 与 discovery_finding 两表独立 | PASS |

## B08 — S01 / S03 集成和端到端回归（S02-TEST-08）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-B08-01 | 项目→需求归属 | 正确 | 复合 FK + 租户边界 | PASS |
| S02-QA-B08-02 | S01 总控台视图 | current_revision 口径 | command_center 8/8（含 non_member_cannot_read、finding 口径） | PASS |
| S02-QA-B08-03 | AI 建议不自动成正式 | 停在草稿/待确认 | 提案/草稿管线全链需人工 confirm（B02-12 + B06-05） | PASS |
| S02-QA-B08-04 | 无外部 Agent 原生可用 | 可用 | 全程未接外部 Agent，REST 原生链路全通 | PASS |
| S02-QA-B08-05 | Task 绑定现状 | **预期未实施（I4）** | task_requirement 表有 revision_id 复合 FK（结构就绪），但 CreateTaskRequest 无绑定字段——入口缺失 | NOT IMPL |
| S02-QA-B08-06 | Task 默认单一执行依据 | 未实施 | 同上（S02-I4 分期缺口）——**NOT IMPLEMENTED（非缺陷）** | NOT IMPL |
| S02-QA-B08-07 | S02↔S03 Design 关联 | **预期未实施（S03 起步）** | design 表不存在——**NOT IMPLEMENTED（非缺陷）** | NOT IMPL |
| S02-QA-B08-08 | 端到端主链 | 可执行段全通 | 项目→需求→AI 起草→人工确认→新 Revision→批准→Git commit→索引→审计全链一致；S03 段与 Task 绑定段除外 | PASS |

## DF1 — 历史缺陷专项回归：跨项目权限越权（原 S02-02 V-4 FAIL）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-DF1-01 | 非成员修改 | 404 | 404（成员闸 require_project_visible 把门） | PASS |
| S02-QA-DF1-02 | 非成员提交 Revision | 404 | 404 | PASS |
| S02-QA-DF1-03 | 非成员批准/定稿 | 404 | 404 | PASS |
| S02-QA-DF1-04 | 绕过路径排查 | 无绕过 | MCP 零确认工具；执行器无条件拒绝；outbox 不生产 | PASS |
| S02-QA-DF1-05 | 服务层二次校验 | 成立 | 租户边界+成员闸双面（集成 non_member_and_cross_project_writes_are_404） | PASS |

**结论：DEF-01 已修复（修复面 S02-I1/I6-A），回归 PASS。**

## DF2 — 历史缺陷专项回归：AI 绕过人工确认（原 P0 FAIL）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-DF2-01 | AI 主体直接确认 | 403 | 403 human required（服务面 require_human） | PASS |
| S02-QA-DF2-02 | API Key 机器凭据确认 | 403 | 403 trusted interactive context required | PASS |
| S02-QA-DF2-03 | 执行器路径 | 双层拒绝 | AI 层 RedLineBlocked（test_redline_rejected_for_ai_actor）+ Human 层无条件拒绝（approve_command_is_refused_via_executor；orchestrator 预检仅做存在性校验，不执行写） | PASS |
| S02-QA-DF2-04 | MCP 工具面 | 零确认工具 | mcp_router 无确认类工具（H-15 裁决维持） | PASS |
| S02-QA-DF2-05 | 后台任务自动审批 | 无路径 | outbox CATALOG has_handler=false 不生产；无 worker confirm 调用 | PASS |
| S02-QA-DF2-06 | 伪造审批者 | 无效 | Confirmation 主体恒=认证主体（B04-12） | PASS |
| S02-QA-DF2-07 | 策略面回归 | 红线生效 | policy_engine 10/10（approve 类入红线，AutoApply 不放行） | PASS |

**结论：DEF-02 已修复（修复面 S02-I6-B 执行器退役 + 服务面双闸），回归 PASS。**

## DF3 — 历史缺陷专项回归：并发 Revision 静默丢失（原 S02-02 V-3 FAIL）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-DF3-01 | 双 Revision 并发确认 | 恰一成功一 409 | **REST 真并行**：v1=200 / v2=409 "row_revision mismatch"（race-probe.log） | PASS |
| S02-QA-DF3-02 | 终态唯一性 | 唯一 current | current_revision_id 单值；approved=1；row_revision 递增可解释 | PASS |
| S02-QA-DF3-03 | 历史链完整 | 四方一致 | revisions/confirmations/audit/Git 对账一致（B03-10） | PASS |
| S02-QA-DF3-04 | 同 Revision 并发确认 | 一成一 409 | 409（not pending / current mismatch） | PASS |
| S02-QA-DF3-05 | CAS 令牌漂移 | 409 且行不变 | row=999→409 "row_revision mismatch"，row 不变 | PASS |

**结论：DEF-03 已修复（修复面 CAS 双令牌 + FOR UPDATE 串行化），回归 PASS。**

## 观察项与缺陷登记

| ID | 级别 | 描述 | 阻断 |
|---|---|---|---|
| S02QA-OBS-1 | P3 | 知识库根未初始化/配置错误时 submit 返回 **500 空响应体**（根目录嵌套保护机制生效，日志有 ERROR，但 HTTP 错误契约非结构化）——生产部署前置项：先初始化独立知识库 Git 根 | 否 |
| OBS-2 | P3 | Requirement 列表 API 无过滤参数（未知 query 静默忽略） | 否 |
| OBS-3 | P3 | Finding 创建无审计事件、决定（decide）有审计——口径待产品确认 | 否 |
| OBS-4 | P3 | 搜索多词查询为 AND 语义，整短语不命中需单词/组合查询（定位器/anchor 正常返回） | 否 |
| NI-1 | 范围 | S02-I3 requirement_finding 关联未实施（B07-04） | 否（分期） |
| NI-2 | 范围 | S02-I4 Task 绑定 Revision 入口未实施（B08-05/06；表结构就绪） | 否（分期） |
| NI-3 | 范围 | S03 Design 关联未实施（B08-07） | 否（分期） |

**本轮无 P0 / P1 / P2 缺陷。**