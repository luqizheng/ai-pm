# S02 QA Release Acceptance Report — Final

> **结论（§8）：PASS WITH CONDITIONS**——P0 全过、历史缺陷全部回归通过；附 3 项分期范围缺口（裁决内）与 1 项 P3 部署前置条件。用例级证据见 `s02-qa-test-cases-v1.md`（114 条逐条判定），原始证据存 `test-case/evidence/s02-qa/`。

## 1. Test Summary

- Project: PM-AI Platform
- Module: S02 Requirement & Knowledge Management
- Test Version: S02-I1 + I2 + I6-A + I6-B（需求核心统一服务 / Markdown+Git 投影协议 / Approver 显式授权 / 执行器定稿退役）
- Build / Commit: master @ `75dc14d`（测试期间产出 `69c0a5a` 文档提交，不含代码变更）；迁移 0001–0042 已应用；运行实例 `pm-api` 本地构建（HEAD 源码）
- Environment: ① 本地 REST 实例（auth=off dev-off，:18090，独立知识库 Git 根 `D:/kudi-project/templates/.s02qa-knowledge`）② aipm-pg（宿主 :16433）psql 直查 ③ cargo test / sqlx::test 每用例独立临时库 ④ Git 命令行直查知识库
- Test Period: 2026-11-06 08:00–08:40 (UTC)
- QA Lead: CodeArts（独立验收，未采信开发团队任何口头结论）

## 2. Execution Summary

| Metric | Result |
|---|---|
| Total Test Cases | 114 |
| Executed | 114 |
| Passed | 110 |
| Failed | 0 |
| Blocked | 0 |
| Not Executed（Not Implemented，裁决内分期缺口） | 4（B07-04、B08-05、B08-06、B08-07） |

代码级回归合计 73 套件用例全绿：pm_requirement 5/5、requirement_projection 3/3、pm_approvers 2/2、documents 7/7、knowledge_index_v2 1/1、discovery 6/6、tenant_isolation 1/1、visibility 3/3、command_center 8/8、command_executor 10/10、policy_engine 10/10、i6b 约束 13/13、state_transition_guard 4/4。

## 3. Batch Results

| Batch | Name | Result | Defects |
|---|---|---|---|
| 01 | Requirement Core | PASS（14/14） | OBS-2（P3 注记） |
| 02 | Revision Lifecycle | PASS（20/20） | — |
| 03 | Markdown + Git | PASS（10/10） | OBS-1（P3 登记） |
| 04 | Authorization | PASS（16/16） | OBS-3 注记 |
| 05 | Concurrency | PASS（12/12） | — |
| 06 | Knowledge + PageIndex | PASS（12/12） | OBS-4 注记 |
| 07 | Finding | PASS（4/5） | NI-1 |
| 08 | Integration | PASS（5/8） | NI-2、NI-3 |

## 4. Historical Defect Regression

| Defect | Description | Result | Evidence |
|---|---|---|---|
| DEF-01 | Cross-project unauthorized write | **PASS（已修复）** | 非成员六探针全 404（DF1-01~06）；服务面租户+成员双闸；集成面 non_member_and_cross_project_writes_are_404 / xt1_xt2 |
| DEF-02 | AI bypasses human approval | **PASS（已修复）** | AI 主体 confirm 403（require_human）；api_key 403（trusted context）；执行器双层拒绝（test_redline_rejected_for_ai_actor + approve_command_is_refused_via_executor）；MCP 零确认工具；outbox has_handler=false 不生产；伪造审批者无效（Confirmation 主体恒=认证主体）；策略红线 10/10 |
| DEF-03 | Concurrent Revision lost update | **PASS（已修复）** | 真并行双 confirm：恰一 200 / 一 409（race-probe.log）；FOR UPDATE 串行化 + CAS 双令牌；approved 唯一、失败方保留 pending、四存储对账一致 |

## 5. Data Authority Verification

- **Markdown Authority: PASS** — 正文权威=Git 知识库文件；Requirement 投影文档 `req-{requirement_id}.md`（O-3 1:1）、知识文档 `projects/{code}/knowledge/{doc_key}.md`；实测文件 sha256=82a20456… 与 DB content_hash 严格一致。
- **Git History: PASS** — 每次正式修订独立 commit（r1-r4 链）；知识文档 2 commits；commit message `docs(requirement): req-{id} r{n}` 可追溯。
- **PostgreSQL Business State: PASS** — content 列为投影（rebuild_projection 可从 Git 反向重建并检测漂移）；PG 承载 ID/状态/版本引用/审批/审计，无第二权威路径；markdown_version_ref = git_commit_sha（O-4）双锚点互验。
- **PageIndex Rebuild: PASS** — index-state 返回 indexed_content_hash（与 Git/PG 三方一致）；reindex → indexed sections=1；索引失效分级回退直读 Markdown（k12/k20）。
- **Cross-storage Consistency: PASS** — PG revision ↔ Git commit ↔ content_hash ↔ 索引四方对账一致，无孤儿/缺失。
- **Recovery Verification: PASS** — Git 先行 + 幂等账本三路径（完整提交返回 sha / 绑定缺失补齐 / 已绑直接返回）；submit 可重入；实测 Git 失败窗口后状态可收敛（OBS-1 仅错误契约问题）。

## 6. Security Verification

- **Project Isolation: PASS** — 非成员读/写/确认全 404（不泄露存在性）；跨租户不可见不可写；列表不泄漏。
- **Human Approval: PASS** — 六要素确认协议（真人+授权+CAS 双令牌+hash+绑定+状态）；Confirmation 行记录 human_principal_id/identity_id/confirmed_at/content_hash；审计 9 类事件完整。
- **AI Permission Boundary: PASS** — AI 可起草/提交待确认，confirm 类一律 403；执行器 ApproveRequirementRevision 无条件拒绝（双层）；MCP 零确认工具；后台任务无审批路径。
- **API Authorization: PASS** — 服务层 require_human + ensure_governance_authorized 内置（非仅 REST 层）；Lead OR approver_grant 授权模型实测（grant→confirm→revoke 闭环）。
- **Audit Completeness: PASS** — 每次实际状态迁移恰一条审计；404 越权面无审计为现状取舍（OBS-3 注记）。

## 7. Defect List

| Defect ID | Severity | Description | Release Blocking |
|---|---|---|---|
| S02QA-OBS-1 | P3 | 知识库根未初始化/配置错误时 submit 返回 500 空响应体（根目录嵌套保护机制正确生效，日志有 ERROR，但 HTTP 错误契约非结构化）。**部署前置条件：生产环境必须先初始化独立知识库 Git 根** | No |
| OBS-2 | P3 | Requirement 列表 API 无过滤参数（未知 query 静默忽略） | No |
| OBS-3 | P3 | Finding 创建无审计事件、决定有审计——口径待产品确认 | No |
| OBS-4 | P3 | 搜索多词查询为 AND 语义（单词/组合可命中，定位器正常） | No |
| NI-1 | 范围缺口 | S02-I3 requirement_finding 治理关联未实施 | No（裁决内分期） |
| NI-2 | 范围缺口 | S02-I4 Task 绑定 Revision 入口未实施（delivery.task_requirement 表结构就绪，CreateTaskRequest 无绑定字段） | No（裁决内分期） |
| NI-3 | 范围缺口 | S03 Design 关联未实施（design 表不存在） | No（裁决内分期） |

**无 P0 / P1 / P2 缺陷。**

## 8. Release Recommendation

**PASS WITH CONDITIONS**

条件（均为非阻断，经本次验收正式记录）：
1. **C1（部署前置）**：生产部署前必须初始化独立知识库 Git 根（不得嵌套于任何 Git 仓库内）；OBS-1 的 500 空响应体错误契约改进排入后续迭代。
2. **C2（功能暴露边界）**：S02-I3（Finding↔Requirement 关联）与 S02-I4（Task 绑定 Revision 入口）补齐前，相关能力不得对用户呈现为可用；Task 执行依据钉定语义（§3.4）以 I4 落地为准。
3. **C3（后续验证）**：S03 Design 关联（NI-3）随 S03 交付线另行验收。

## 9. QA Lead Conclusion

1. **S02 是否符合已批准架构**：符合。正文权威=Markdown+Git（实测三面对账）；PG 仅业务状态/版本引用/审批/审计；PageIndex 为可重建派生索引且不决定业务状态；统一服务单点实现九命令；执行器/MCP/后台无审批旁路——Master Prompt §3 四组约束逐条验证通过。
2. **P0 测试是否全过**：全过。DEF-01/DEF-02/DEF-03 三个历史 P0 级缺陷经真并行、真越权、真凭据复测，全部修复且无回归路径。
3. **历史缺陷是否解决**：是。三项缺陷的修复面（成员闸/服务双闸/CAS 双令牌/执行器退役/Approver 显式授权）均有代码级 + REST 网络面 + DB 直查三面证据。
4. **数据完整性风险是否残留**：无阻断级残留。跨存储一致性具备幂等账本与可重入恢复；仅 OBS-1 错误契约瑕疵（机制本身正确）。
5. **S02 能否安全进入生产**：能，但必须先满足 C1（知识库根初始化为部署前置项）；C2/C3 为功能暴露边界约束。
6. **发布前必须解决项**：仅 C1。其余为排期项。

**判定独立性声明**：本轮全部结论基于 `test-case/evidence/s02-qa/` 下 6 份可复现脚本、执行日志与响应存档、`_sqlx_migrations`/业务表 psql 直查输出、知识库 git log/文件哈希——无任何一项基于开发团队说明。