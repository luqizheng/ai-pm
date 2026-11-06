# S02 QA · 测试用例明细与执行记录 V1.0

> **记录约定**（`test-case/README.md`）：本文件即执行记录——每条用例执行后回填「实际结果 / 判定」；未执行不得标 PASS；BLOCKED 必须写明原因。
> **判定口径**：以数据库终态 + Git 文件 + API 响应三面证据为准，HTTP 200 不作为成功唯一证据。
> **用例编号**：`S02-QA-{批次}-{序号}`；缺陷挂 `DEF-xx`（历史）或 `S02QA-xx`（本轮新登记）。
> **身份说明**：集成测试面走 auth-off `dev-off`（human、租户 owner、测试项目 owner 成员）；REST 网络面走 jwks JWT。AI/服务主体用 `core.principal.kind` 区分（S02D-02 #2）。

---

## B01 — Requirement CRUD、查询及项目隔离（对应 S02-TEST-01）

| ID | 场景 | 步骤 | 预期 | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-B01-01 | 正常创建需求 | POST `/v1/pm/projects/{pid}/requirements`（code/title/type/priority/markdown） | 201；返回独立实体 id/UUID；生成首版 revision(version=1, state=draft)；lifecycle_status=active | 待执行 | 待定 |
| S02-QA-B01-02 | 详情三段结构 | GET `/v1/pm/requirements/{id}` | 200；requirement + revisions + confirmations 三段， revisions 含全部历史 | 待执行 | 待定 |
| S02-QA-B01-03 | 正文修改走新 Revision | PATCH `.../revisions/{rid}` 改 markdown；另对 approved 版本尝试 PATCH | draft 可改且 content_hash 刷新；approved 409（only draft） | 待执行 | 待定 |
| S02-QA-B01-04 | 列表与过滤 | GET `/v1/pm/projects/{pid}/requirements` | 200 items；核对是否存在过滤参数（无则如实记录现状） | 待执行 | 待定 |
| S02-QA-B01-05 | 需求-项目归属 | 校验创建后 project_id 归属与复合键 (project_id,id) | 归属正确 | 待执行 | 待定 |
| S02-QA-B01-06 | 需求-Revision 关联 | 建 3 个 revision，查详情 | version 单调递增 1→2→3，全链保留 | 待执行 | 待定 |
| S02-QA-B01-07 | 状态展示 | 列表项 latest_state/current_version 字段 | 与 DB 一致 | 待执行 | 待定 |
| S02-QA-B01-08 | 历史记录完整 | 连续起草 3 版后查详情 | 3 条 revision 均在，无一被删除/覆盖 | 待执行 | 待定 |
| S02-QA-B01-09 | 重复创建同 code | 同 project 两次 POST 同 code | 记录实际行为（有/无唯一约束），若静默重复 → 按严重度评级 | 待执行 | 待定 |
| S02-QA-B01-10 | 项目隔离（读） | 项目 A 成员 GET 项目 B 需求详情/列表 | 404（不泄露存在性） | 待执行 | 待定 |
| S02-QA-B01-11 | 项目隔离（写） | 项目 A 成员在项目 B 上下文 POST 创建/起草/提交 | 404 | 待执行 | 待定 |
| S02-QA-B01-12 | 不存在的 ID | GET/POST 不存在 requirement_id / revision_id / project_id | 404 | 待执行 | 待定 |
| S02-QA-B01-13 | 删除策略核查 | 检索 DELETE 路由与 withdraw/restore/archive 语义 | 无物理删除；withdraw/restore/archive 按 0039 三态执行；archived 不可恢复（I1-D05） | 待执行 | 待定 |
| S02-QA-B01-14 | 创建时 Markdown 文件行为 | 创建后检查知识库 Git | 实测记录：draft 阶段是否已有 Git 文件（按 S02-I2 设计 submit 才绑定；若创建即写文件则核对协议一致性） | 待执行 | 待定 |

## B02 — Revision 生命周期及人工审批（对应 S02-TEST-02）

生命周期矩阵（判定基准）：

| 从 \ 至 | draft | pending_confirmation | approved | rejected | superseded |
|---|---|---|---|---|---|
| draft | 编辑(PATCH) | submit | ✗（无直跳） | ✗ | ✗ |
| pending_confirmation | ✗（冻结） | — | confirm（六要素） | reject | ✗ |
| approved | ✗ | ✗ | ✗ | ✗ | 仅被新 confirm 替代 |
| rejected | ✗ | ✗ | ✗ | ✗（终态不可复活） | ✗ |
| superseded | ✗ | ✗ | ✗ | ✗ | ✗（终态） |

| ID | 场景 | 步骤 | 预期 | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-B02-01 | 创建 Draft | POST `.../revisions` | 201，version 自增，state=draft，base_current_revision_id 记录基版本 | 待执行 | 待定 |
| S02-QA-B02-02 | Draft 编辑 | PATCH draft revision | 200，content_hash 更新 | 待执行 | 待定 |
| S02-QA-B02-03 | 提交确认 | POST `.../submit` | 200，state=pending_confirmation，submitted_at 冻结，markdown_version_ref 绑定 | 待执行 | 待定 |
| S02-QA-B02-04 | 人工正式确认 | POST `.../confirm`（Lead，携 expected_content_hash） | 200，state=approved；current_revision_id 迁移；row_revision+1 | 待执行 | 待定 |
| S02-QA-B02-05 | 旧版本 superseded | 确认第二版后查旧版 | 旧 approved → superseded，历史保留，替代关系完整 | 待执行 | 待定 |
| S02-QA-B02-06 | 历史全量查看 | GET 详情 revisions | 全部版本 + 状态 + 哈希在列 | 待执行 | 待定 |
| S02-QA-B02-07 | 修改已批准版本 | PATCH approved revision | 409（only draft） | 待执行 | 待定 |
| S02-QA-B02-08 | 重复批准 | 对已 approved revision 再 confirm | 409（not pending confirmation） | 待执行 | 待定 |
| S02-QA-B02-09 | 未授权用户批准 | 非 Lead 非 Approver 的普通成员 confirm | 403（governance not authorized） | 待执行 | 待定 |
| S02-QA-B02-10 | AI 主体批准 | kind=ai/service principal confirm | 403（human required，DEF-02 服务面） | 待执行 | 待定 |
| S02-QA-B02-11 | 机器凭据批准 | api_key 鉴权上下文 confirm | 403（trusted interactive context required） | 待执行 | 待定 |
| S02-QA-B02-12 | AI 起草+提交（合法） | AI 主体 POST revisions / submit | 200/201（AI 允许起草与提交待确认） | 待执行 | 待定 |
| S02-QA-B02-13 | 审批期间并发修改 | confirm 携过期 expected_row_revision | 409（row_revision mismatch） | 待执行 | 待定 |
| S02-QA-B02-14 | 审批请求重放 | 同请求重发 confirm | 409（已 approved 或 hash/令牌失配），不产生第二条 Confirmation | 待执行 | 待定 |
| S02-QA-B02-15 | 审批中断恢复 | 模拟 submit 后绑定中断：重入 submit | pending 未绑定 → 重 submit 只补绑定不重复翻转/审计；已绑定 → 409 | 待执行 | 待定 |
| S02-QA-B02-16 | 拒绝流程 | reject pending revision（带 reason） | 200，state=rejected 终态；Confirmation(action=reject) 落账；current_revision_id 不动；reject 后不可复活 | 待执行 | 待定 |
| S02-QA-B02-17 | indexed ≠ approved | 构造索引已建但未确认的 revision | state 仍 pending_confirmation；索引状态独立 | 待执行 | 待定 |
| S02-QA-B02-18 | 审批可追踪 | 查 requirement_confirmation + governance.audit_log | human_principal_id + identity_id + confirmed_at + content_hash + audit(requirement.confirmed) 完整 | 待执行 | 待定 |
| S02-QA-B02-19 | 非法转换矩阵复核 | 对每种非法转换发请求 | 全部显式 4xx，无静默变更 | 待执行 | 待定 |
| S02-QA-B02-20 | 后台/旁路改状态 | 核查是否存在任何非 REST 路径写 approved（执行器/MCP/后台任务） | 无；执行器 ApproveRequirementRevision 无条件拒绝 | 待执行 | 待定 |

## B03 — Markdown + Git 正文权威与跨存储一致性（对应 S02-TEST-03）

| ID | 场景 | 步骤 | 预期 | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-B03-01 | submit 生成 Markdown 文件 | submit 后按 markdown_version_ref 定位知识库文件 | 文件存在且内容=提交的 markdown | 待执行 | 待定 |
| S02-QA-B03-02 | Git Commit 与 Revision 引用 | 查文件所在 commit 与 PG revision.markdown_version_ref | 双方可互相推导，content_hash=sha256(markdown) | 待执行 | 待定 |
| S02-QA-B03-03 | 历史正文还原 | 对多版需求逐版 git show | 每版正文均可还原且与该 revision.content→markdown 一致 | 待执行 | 待定 |
| S02-QA-B03-04 | PG 非第二权威 | 代码核查 content 列角色 + 实测：PG content 与 Git 文件冲突时以何为准 | content 为投影可重建；权威=Git；无以 PG content 覆盖 Git 的路径 | 待执行 | 待定 |
| S02-QA-B03-05 | Git 成功 PG 失败恢复 | 静态+实测：bind_projection 顺序与失败路径 | 绑定失败 → revision 停留可重入状态（pending 未绑定），重 submit 补绑定（§6.7） | 待执行 | 待定 |
| S02-QA-B03-06 | PG 成功 Git 失败 | 同上反向窗口 | 状态翻转成功但绑定缺失 → 重入 submit 补绑，无重复审计 | 待执行 | 待定 |
| S02-QA-B03-07 | PageIndex 失败不损正文 | 索引失败/禁用后读需求正文 | 正文仍可从 Git/投影读取；索引状态不阻断 | 待执行 | 待定 |
| S02-QA-B03-08 | 索引过期回退 | mark_stale 后搜索/读取 | 直接回退 Markdown 来源，fallback 可用 | 待执行 | 待定 |
| S02-QA-B03-09 | 静默覆盖检查 | 多版提交后检查 Git 文件路径 | 各版本独立/版本化，无覆盖丢失 | 待执行 | 待定 |
| S02-QA-B03-10 | 一致性对账 | 全量比对 PG revision ↔ Git 文件 ↔ content_hash | 三方一致，无孤儿/缺失 | 待执行 | 待定 |

## B04 — 角色权限、项目隔离、越权及审批安全（对应 S02-TEST-04）

授权矩阵（O-6 裁决 + 0040）：confirm/reject/withdraw/restore/archive = 真人 + 项目成员 + (Lead OR approver_grant 生效行)；grant/revoke/list = Lead 专属。

| ID | 场景 | 步骤 | 预期 | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-B04-01 | 非成员读 | 非成员 GET 需求列表/详情/修订 | 404 | 待执行 | 待定 |
| S02-QA-B04-02 | 非成员创建 | 非成员 POST 需求 | 404 | 待执行 | 待定 |
| S02-QA-B04-03 | 非成员改/提交 | 非成员 PATCH draft / submit | 404 | 待执行 | 待定 |
| S02-QA-B04-04 | 非成员确认 | 非成员 confirm/reject | 404 | 待执行 | 待定 |
| S02-QA-B04-05 | 成员但无治理权 confirm | 普通成员（member 非 owner 非 Approver）confirm | 403（成员闸过、治理闸拦） | 待执行 | 待定 |
| S02-QA-B04-06 | 显式 Approver confirm | Lead grant → 被授人 confirm | 200；Confirmation 记录该真人 | 待执行 | 待定 |
| S02-QA-B04-07 | revoke 后失效 | Lead revoke → 同人再 confirm | 403；approver_grant.revoked_at 生效 | 待执行 | 待定 |
| S02-QA-B04-08 | grant/revoke 非 Lead 尝试 | 普通成员调用 grant API | 403（Lead 专属） | 待执行 | 待定 |
| S02-QA-B04-09 | AI 主体起草/确认边界 | AI 起草 POST revisions、submit；AI confirm | 起草/提交 2xx；确认 403 | 待执行 | 待定 |
| S02-QA-B04-10 | 路径参数替换 | 用他人项目的 requirement_id 拼 URL 但带本项目 token | 404（租户+归属双查） | 待执行 | 待定 |
| S02-QA-B04-11 | revision 归属校验 | 对 requirement A 的 URL 传 requirement B 的 revision_id | 404/409（不得跨需求操作） | 待执行 | 待定 |
| S02-QA-B04-12 | 伪造审批者 | 请求体夹带 approved_by/human_principal_id 字段 | 服务端忽略，确认主体恒=认证主体 | 待执行 | 待定 |
| S02-QA-B04-13 | 重放历史审批 | 用旧 expected_content_hash 确认已变更 revision | 409（hash mismatch） | 待执行 | 待定 |
| S02-QA-B04-14 | 直调应用服务层 | 检查 require_human + ensure_governance_authorized 是否服务层内置 | 是（非仅 REST 层）；MCP 无确认工具 | 待执行 | 待定 |
| S02-QA-B04-15 | 越权审计 | 复查越权尝试的审计行为 | 记录实际行为（404 面无审计则如实注记） | 待执行 | 待定 |
| S02-QA-B04-16 | 跨租户隔离 | 其他租户 principal 访问 | 404 | 待执行 | 待定 |

## B05 — 并发修改、冲突检测、幂等和失败恢复（对应 S02-TEST-05）

| ID | 场景 | 步骤 | 预期 | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-B05-01 | 并发起草同基版本 | 两用户各建 draft（同 base） | 均成功（合法并行草案），版本号不冲突 | 待执行 | 待定 |
| S02-QA-B05-02 | 并发 submit 两草案 | 并行 submit draft1/draft2 | 均 pending（各锁各的 revision） | 待执行 | 待定 |
| S02-QA-B05-03 | **并发确认不同 Revision（DEF-03 核心）** | 并行 confirm draft1 与 draft2 | 先至者 approved；后至者显式 409（FOR UPDATE 串行化）；仅一个 current_revision_id；无双 200 静默丢失 | 待执行 | 待定 |
| S02-QA-B05-04 | 并发确认同一 Revision | 并行双 confirm 同 rid | 一条 approved，另一条 409 | 待执行 | 待定 |
| S02-QA-B05-05 | 终态重试 | 对 superseded revision confirm | 409 | 待执行 | 待定 |
| S02-QA-B05-06 | 重复创建 | 同 code 并发/顺序重复 POST | 记录实际行为并评级 | 待执行 | 待定 |
| S02-QA-B05-07 | 重复提交 | 已绑定 pending 再 submit | 409（already submitted） | 待执行 | 待定 |
| S02-QA-B05-08 | 重复 Git/索引触发 | 投影绑定 operation_id 幂等账本 | 同 (project_id, operation_id) 不重复写 | 待执行 | 待定 |
| S02-QA-B05-09 | submit 中断恢复 | 见 B02-15 重入语义 | 状态与绑定最终一致 | 待执行 | 待定 |
| S02-QA-B05-10 | 并发读一致 | 起草/确认期间持续 GET | 不返回混合态（事务边界内可见性） | 待执行 | 待定 |
| S02-QA-B05-11 | 审计完整性 | 复核并发场景审计序列 | 每次实际状态迁移恰一条对应审计，无虚记 | 待执行 | 待定 |
| S02-QA-B05-12 | 服务重启恢复 | 重启后 pending/绑定状态核对 | 无半程孤儿（PG 与 Git 可对账） | 待执行 | 待定 |

## B06 — Knowledge 管理、PageIndex 索引、重建及回退（对应 S02-TEST-06）

| ID | 场景 | 步骤 | 预期 | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-B06-01 | 创建知识文档 | documents API 创建 | 2xx；Git 文件 + PG revision 引用 | 待执行 | 待定 |
| S02-QA-B06-02 | 修改知识文档 | 更新内容 | 新 revision + 新 commit，历史保留 | 待执行 | 待定 |
| S02-QA-B06-03 | 知识修订记录 | 查 document_revision | git_commit_sha/内容哈希齐全 | 待执行 | 待定 |
| S02-QA-B06-04 | 查询知识文档 | 列表/详情 | 与 Git 一致 | 待执行 | 待定 |
| S02-QA-B06-05 | AI 写入受控 | AI 经统一协调器写入（knowledge_write_coordinator） | 受控流程（红线/审批面），无旁路直写 | 待执行 | 待定 |
| S02-QA-B06-06 | Git 历史核查 | git log 知识库 | 提交链完整可追溯 | 待执行 | 待定 |
| S02-QA-B06-07 | 索引状态查询 | index-state API | 返回 provider/status/indexed_content_hash | 待执行 | 待定 |
| S02-QA-B06-08 | 索引失效与重建 | 删索引/触发 rebuild | 可重建，rebuild 后内容正确 | 待执行 | 待定 |
| S02-QA-B06-09 | 旧索引错误版本风险 | 内容更新后查索引 | index_state 绑定 revision_id+hash，不返回旧版内容 | 待执行 | 待定 |
| S02-QA-B06-10 | 索引失效直接读 Markdown | 索引缺失/过期时搜索与读取 | fallback 直读 Markdown（index_state_missing/index_stale） | 待执行 | 待定 |
| S02-QA-B06-11 | 索引≠正式状态 | 核对索引状态与知识正式状态独立 | 索引状态不决定业务状态 | 待执行 | 待定 |
| S02-QA-B06-12 | 项目隔离 | 跨项目知识文档访问 | 404/隔离生效 | 待执行 | 待定 |

## B07 — Finding 创建、来源、确认及生命周期（Master Prompt §4；无用户任务书，按 Master 执行）

| ID | 场景 | 步骤 | 预期 | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-B07-01 | Finding 创建（人工） | discovery add_finding | 2xx；归属 project_id | 待执行 | 待定 |
| S02-QA-B07-02 | Finding 来源（发现任务产出） | discovery run 产出 finding | 来源可溯（run 关联） | 待执行 | 待定 |
| S02-QA-B07-03 | Finding 确认/验证 | verification API | PENDING→CONFIRMED 等状态机正确 | 待执行 | 待定 |
| S02-QA-B07-04 | requirement_finding 治理关联 | 检索该表/API | **预期 Not Implemented（S02-I3 未实施）**——记录为范围缺口，不判缺陷 | 待执行 | 待定 |
| S02-QA-B07-05 | 异域同名区分 | review_finding（0026）与 discovery_finding 不混用 | 代码与 API 无混用 | 待执行 | 待定 |

## B08 — S01 / S03 集成和端到端回归（对应 S02-TEST-08）

| ID | 场景 | 步骤 | 预期 | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-B08-01 | 项目→需求归属 | 项目下建需求，归属校验 | project_id/tenant_id 正确 | 待执行 | 待定 |
| S02-QA-B08-02 | S01 总控台需求视图 | command_center 需求统计 | 口径=current_revision_id 集合（Q-10） | 待执行 | 待定 |
| S02-QA-B08-03 | AI 建议不自动成正式 | AI 生成 draft/提案 | 停在 draft/pending，确认前无 approved | 待执行 | 待定 |
| S02-QA-B08-04 | 无外部 Agent 时原生可用 | 全程不接 Codex/Claude 等执行原生链路 | 可用 | 待执行 | 待定 |
| S02-QA-B08-05 | Task 绑定需求现状 | 检索 task_requirement 改造（S02-I4） | **预期未实施**——按 0002 旧表现状记录 | 待执行 | 待定 |
| S02-QA-B08-06 | Task 默认单一执行依据 | 验证绑定语义 | 未实施则记范围缺口 | 待执行 | 待定 |
| S02-QA-B08-07 | S02↔S03 Design 关联 | 检索 S03 实现面 | **预期未实施（S03 起步）**——BLOCKED/Not Implemented 记录 | 待执行 | 待定 |
| S02-QA-B08-08 | 端到端主链 | 项目→需求→AI 建议→人工确认→新 Revision→批准→Git→索引→Task 绑定→审计全查 | 可执行段全链一致；不可执行段单列 | 待执行 | 待定 |

## DF1 — 历史缺陷专项回归：跨项目权限越权（原 S02-02 V-4 FAIL）

| ID | 场景 | 步骤 | 预期（修复后） | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-DF1-01 | 非成员修改需求 | 复刻 V-4：项目 A 成员写项目 B 需求（改 draft） | 404（require_project_visible 把门） | 待执行 | 待定 |
| S02-QA-DF1-02 | 非成员提交 Revision | 同上下文 submit | 404 | 待执行 | 待定 |
| S02-QA-DF1-03 | 非成员批准/定稿 | 同上下文 confirm | 404 | 待执行 | 待定 |
| S02-QA-DF1-04 | 绕过路径排查 | MCP 工具/内部 API/执行器路径是否绕过成员校验 | 无绕过面；执行器拒绝一切定稿 | 待执行 | 待定 |
| S02-QA-DF1-05 | 服务层二次校验 | 直调 application service（模拟） | 租户边界+成员闸在服务面成立 | 待执行 | 待定 |

## DF2 — 历史缺陷专项回归：AI 绕过人工确认（原 P0 FAIL）

| ID | 场景 | 步骤 | 预期（修复后） | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-DF2-01 | AI 主体直接确认 | kind=ai principal 调 confirm API | 403（require_human） | 待执行 | 待定 |
| S02-QA-DF2-02 | API Key 机器凭据确认 | api_key 鉴权调 confirm | 403（trusted context） | 待执行 | 待定 |
| S02-QA-DF2-03 | 执行器路径 | ProjectCommand::ApproveRequirementRevision（AI 主体 / Human+RedlineApproval 双层） | 层1 RedLineBlocked；层2 无条件拒绝（refuse_requirement_confirmation），不改状态不审计 | 待执行 | 待定 |
| S02-QA-DF2-04 | MCP 工具面 | 枚举 MCP 工具清单 | 零确认类工具（H-15） | 待执行 | 待定 |
| S02-QA-DF2-05 | 后台任务自动审批 | 排查 worker/outbox/调度是否有 confirm 调用 | 无；outbox 需求事件 has_handler=false 不生产 | 待执行 | 待定 |
| S02-QA-DF2-06 | 伪造审批者 | 夹带审批者字段 + 检查 Confirmation 主体来源 | 主体恒=认证主体，伪造无效 | 待执行 | 待定 |
| S02-QA-DF2-07 | 策略面回归 | 策略测试重跑：approve 类红线补录生效 | AutoApply 不再放行 ApproveRequirementRevision | 待执行 | 待定 |

## DF3 — 历史缺陷专项回归：并发 Revision 静默丢失（原 S02-02 V-3 FAIL）

| ID | 场景 | 步骤 | 预期（修复后） | 实际 | 判定 |
|---|---|---|---|---|---|
| S02-QA-DF3-01 | 双 Revision 并发确认 | 复刻 V-3 场景1：并行 confirm v1/v2 | 恰一成功一显式 409；无双 200 | 待执行 | 待定 |
| S02-QA-DF3-02 | 终态唯一性 | 查 DB | current_revision_id 唯一指向；旧版恰一 superseded；row_revision 递增可解释 | 待执行 | 待定 |
| S02-QA-DF3-03 | 历史链完整 | 查 revisions/confirmations/audit/Git | 四方一致，无静默丢失 | 待执行 | 待定 |
| S02-QA-DF3-04 | 同 Revision 并发确认 | 并行双 confirm 同 rid | 一成一 409 | 待执行 | 待定 |
| S02-QA-DF3-05 | CAS 令牌漂移 | 携过期令牌 confirm | 409（row_revision / current_revision_id mismatch） | 待执行 | 待定 |