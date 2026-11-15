# D03 代码访问链索引（V-1 / V-2 专项审计）

> 代码快照：master 05ce33d8 · 2026-11-16 · 证据标记：【已验证】=人工亲验；【代码推断】=代理报告+路径自洽未亲验
> 配套：V-1 结论=映射任务书 §9；V-2 结论=专项任务书附二

## 1. V-1 确认链（Project Cognition）

| # | 链 | 入口 | 身份来源 | 权限校验 | Service/Repo | 最终写入 | 判定 |
|---|---|---|---|---|---|---|---|
| C1 | 人工决策链 | POST /v1/pm/projects/{pid}/cognition/items/{iid}/decision（pm_cognition.rs:344-356）【已验证】 | JWT→AuthenticatedIdentity→AuthedPrincipal（auth.rs:310-355；service token :327 拒、跨租户 :333 拒）【已验证】 | require_project_visible（pm_cognition.rs:285-291→support.rs:178-196，JOIN project_member+tenant）【已验证】 | repo decide_item（repo_cognition.rs:218-261，FOR UPDATE+仅 proposed 可决+重复 409）【已验证】 | UPDATE ai.cognition_item + INSERT ai.project_knowledge 回写 source_item_id（repo_cognition.rs:272-283）【已验证】 | 主链可达；**无显式 kind/human 校验**（SEC-P1-1） |
| C2 | AI 提案链 | POST …/cognition/runs（start_run）→ 确定性派生（repo_cognition.rs:161-209）【已验证】 | 同上 | 同守卫【代码推断】 | INSERT cognition_run + items **state 恒 proposed**【已验证】 | 只写提案，无确认路径 | PASS（AI 无确认入口成立） |
| C3 | MCP 面 | /mcp（rmcp） | HumanActor 仅鉴权层生产（caller.rs:36-56）【代码推断】 | require_project_member（authz.rs:33-60，双 tenant 绑定）【代码推断】 | tools.rs 无 cognition 工具【已验证】 | 无 | PASS |
| C4 | cpk 凭据链 | 任意业务端点 | **路径门：非 /mcp 一律 401 IAM_AUTH_1007（auth.rs:225-227）**【已验证】 | 四项链每请求直查：key_hash/status=active/owner 同租户且 active/credential_revoked_at NULL（device_credential.rs:273-308）【已验证】 | — | 不可达业务端点 | PASS（R1 证伪） |
| C5 | 后台/事件旁路 | worker/outbox handlers | tenant 由 core.project 反查（outbox_handlers.rs:371）【代码推断】 | claim JOIN ai.project_ai_config（job_queue.rs:34-73）【代码推断】 | cognition_item 状态变更唯一入口=C1【已验证：全仓写者仅 repo_cognition/pm_cognition/viz 只读】 | 无确认写入 | PASS |
| C6 | 审批借用 | governance.approval_request | — | 仅导入链使用（project_import/advance.rs:99）【已验证】 | 与 cognition_item 无代码关联【已验证】 | 无 | PASS（V1-04 语义隔离成立，待负测确认） |

## 2. V-2 访问链

### 2.1 被击穿链（P0×5，均【已验证】关键环节）

| # | 入口 | 身份 | 校验缺口 | 最终读写 |
|---|---|---|---|---|
| C7 | POST /api/v1/events（routes.rs:232→handler :532） | bearer JWT（任意租户） | handler 无 Extension<AuthedPrincipal>；submit_event（ingestion.rs:70-95）仅查 project 存在 | INSERT inbound_event + ai_analysis job（受害项目 AI 分析，actor 客户端声明） |
| C8 | POST /api/v1/supervision/approvals/{id}/decision（:235→:615） | 同上 | decide_approval（supervision/approval.rs:24-35）仅 approval_id 定位 | **跨租户写**：命令在受害项目真实执行（executor） |
| C9 | POST /api/v1/supervision/veto（:233→:574） | 同上 | veto.rs:20-26 仅 WHERE id+project_id；AuditActor::principal(req.actor_id) 主体取自 body | 跨租户回滚已执行命令（compensate） |
| C10 | POST /api/v1/supervision/freeze\|unfreeze（:238-239→:668/:691） | 同上 | project_id/actor_id 全由 body | 跨租户翻转 automation_mode |
| C11 | GET /api/v1/projects/{id}/notifications（:241→:712） | 同上 | feed.rs:13-18 WHERE project_id=$1 无 tenant | 跨租户读 notification payload |

### 2.2 防御成立链（PASS 锚点）

| 链 | 锚点 |
|---|---|
| 新栈业务面 | require_project_visible（support.rs:199）/ kb_memory_api.rs:73-89 / knowledge_services.rs:141-147【已验证】 |
| MCP 面 | caller.rs:36-56（只认鉴权层 HumanActor）+ authz.rs:33（成员+双 tenant JOIN）+ cpk 仅 /mcp（auth.rs:224-227）【已验证/代码推断】 |
| 后台任务 | job_queue.rs:34-73 claim JOIN project_ai_config；tenant 一律 core.project 反查（outbox_handlers.rs:371）【代码推断】 |
| 旧栈受守卫 SQL | knowledge.rs:358/658、knowledge_proposal/veto.rs:189/232、pm_write/task.rs:415/440 的 project_id 均来自已过 project_visible 的 handler【代码推断】 |
| 执行器 fencing | pm_submit_result 仅 run_id+lease_token（tools.rs:643-645），无 project_id 伪造面；lease fencing tools.rs:1533/1565【代码推断】 |

### 2.3 跨域引用链（PARTIAL）

| 引用 | 校验现状 | 结论 |
|---|---|---|
| kb.memory_ref（KB source） | source_linked_tx 强制关联行（memory.rs:229-246+source.rs:328-340）【代码推断】 | PASS |
| kb.memory_ref（非 KB 目标） | 仅结构校验不验目标存在/归属（memory.rs:175-227）【代码推断】 | P2 悬空指针 |
| pc.source.target_* | 仅域通道状态校验（evidence.rs:17-63）；mark_analyzed 全局 LIMIT 1（analysis_report.rs:228-250）【代码推断】 | P2 |
| pc.evidence | 主体+document_section 已校验（evidence.rs:97-166）；其余 evidence_type target_* 不校验【代码推断】 | P2 |
