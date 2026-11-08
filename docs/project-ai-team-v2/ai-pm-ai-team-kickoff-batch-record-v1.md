# AI Team V2 本体开工批 · 实施记录 V1.0

> 状态：已落地并 commit · 2026-11-08
> 裁决依据：`ai-pm-ai-team-kickoff-adjudication-pack-v1.md` §〇（D-1~D-4 全部 = A，2026-11-08）
> 记录人：AI Agent（用户全权委托模式）；实施：三路子代理 + 主会话协调
> 代码基线：master，含并行会话同树高频 commit/fmt 波（全程 pathspec 纪律，零卷入）

---

## 一、提交清单

| Commit | 内容 | 规模 |
| --- | --- | --- |
| `3a7a4b3` | docs(ai-team)：V1.0 评审稿 6 分篇 + 代码综合比对报告 v1 + 开工前置裁决包 | 8 文件 |
| `0153ca5` | feat(ai-team)：Task 17 主分析链接真模型（`AI_PM_ANALYSIS_MODEL=auto\|fake` + llm_call_record 全量落账） | 5 文件 |
| `5393e70` | feat(ai-team)：D-1~D-4 合并批（0057~0060 四迁移 + Role/调度/会话三层全链） | 25 文件 3334 行 |
| `c8f508a` | docs(ai-team)：本记录 V1.0 | 1 文件 |
| `b71946b` | feat(ai-team)：Batch E——V2.0-D Capability Registry V1 渐进（0061 能力表+派生播种+registry-first/legacy 回退+成员角色解耦）+ task 读面补 `ai_role_version_id` | 9 文件 758 行 |
| `2329bba` | feat(ai-frontend)：会话三件套接真实后端——`/project/assistant` 会话页（首发自动建会话、诚实空态、失败重发）+ api 契约层 + TopBar 按钮点亮 + fixtures 解耦过 release-guard | 21 文件 897 行 |

**未 push**：本批 3 commits + 并行会话 Checkpoint-2 未 push 池（其 R4 真机未完，有意压池）；push 时机由用户定。

## 二、行为规格（已落地）

### D-1 Role 模型（0058）
- `ai.ai_role_template`（全局 5 预置：project_manager/requirement_analyst/architect/developer/tester，六组 default_definition JSONB）→ `ai.project_ai_role`（项目启用行，0019 组合 FK）→ `ai.ai_role_version`（不可变版本，append-only+RESTRICT）。
- `delivery.task.ai_role_version_id` 可空外键 = 不变量③（创建时版本锚）由库保证。
- 启停：enable 懒生效（建行+派生 v1.0），`project_manager` 不可禁用（409 `AI_ROLE_NOT_DISABLEABLE`，AT-D12/稿 §3.3）；启停落项目级审计，不进 outbox。
- REST：`GET/PUT /v1/pm/projects/{id}/ai-roles[...]`（列表合并视图 / 启停 admin-or-owner / 版本历史）；bootstrap `ensure_project_ai_roles` 每项目播种 5 角色。
- D-3（0057）：`project_ai_model_policy` UNIQUE(tenant_id,provider_id) 已 DROP，多项目可共享同一 Team Provider（回归用例在 team_llm 测试）。

### D-2 调度自动分配（0059）
- `task_waiting.reason` 扩第五值 `no_executor_available`（**非状态化**，守 §12.2 裁决）。
- `resolve_role_assignee`：role_version→enabled 项目角色→role_key→`ROLE_EXECUTOR_MAP` 过渡映射（developer/architect/tester/project_manager→ai_pm；requirement_analyst 无映射）→ kind='dev_agent' 且 active 的项目成员；空闲优先（活跃归因以 `task.assignee_id` 为过渡口径）。
- `TaskReadyHandler`：无 assignee 且有版本锚 → 解析成功写 assignee（审计 `task_ready_assignee_resolved`）继续建 Run；解析失败写等待行（questions=恢复条件 JSON，审计 `task_ready_skipped`，`auto_retry:"none_v1"`）跳过不进死信；无版本锚维持 `no_assignee` skip。
- 恢复语义：活跃等待行使 claimability 门③持续拦截；恢复=enable 角色/新增合格成员后重触发 TaskReady 或人工指派；**V1 无自动重试器**。
- 写入口：`POST /v1/pm/tasks` 接受可选 `ai_role_version_id`（校验同项目归属；跨项目/未知版本 400）；PATCH **有意不支持改锚**（§17.2 语义）。

### D-4 会话存储（0060）
- `ai.conversation`（principal 属主）+ `ai.conversation_message`（user/assistant/system，REST 恒 user 且不外收 role/metadata）。
- 属主隔离：跨属主/跨项目/不存在统一 404；`list_messages` created_at ASC + `before` keyset 游标（limit clamp [1,500]）；append 同事务刷新 last_message_at。
- assistant 消息生产者接口已预留（内部链路直调 `append_message(role="assistant")`），前端三件套接线为独立后续批次（ai-frontend 并行占用中）。

### Task 17
- 主分析链每次 analyze 按 project 三级解析（项目指定严格不回退→Team 首个 enabled→None）；None/解析失败 → 结构化 WARN 回退 FakeModel（不静默）；真实调用逐次落 `ai.llm_call_record`（purpose=ai_analysis/repo_import，error_kind 七类映射，best-effort 不影响主流程）。

## 三、门禁证据

- pm-domain / pm-application / pm-infrastructure 全量 **0 failed**（ai_role 43、ai_role_dispatch 6、outbox_handlers 13、ai_conversations 5、waiting 5、audit_actor_coverage 闸门登记 C174/C180 等）。
- pm-api 全量 **333 passed / 2 failed**；2 红 = `team_onboarding_qa::qa_b05_02/b05_03`（DEF-01 基线挂账，归团队 QA 会话，非本批）。
- 迁移：60 个全部经 sqlx::test 实际应用；0057/0058 曾在干净 scratch 库按序实跑+introspect 逐项验证。
- Task 17：`cargo test -p pm-ai` 53/0（含 wiremock 真库落账断言）。

## 四、并行干扰与处置（存档）

1. D-3+D-1 实施中途并行会话 commit 0056 并重置 `team_llm.rs`（已写入测试被清）→ 在新 HEAD 重应用并复跑绿。
2. 全仓 fmt 波扫过我方文件 → 语义中性，随批入库。
3. 共享 lib.rs 三方（D-1/D-2/D-4）并发注册 → 按预分配迁移号+精确锚点增量 Edit 存活，最终以合并单 commit（5393e70）收口保证树自洽。
4. 双代理互相观察对方在途编译错误（E0382/E0252），按「不代改、等待自愈、复跑确认」纪律处理，零交叉污染。

## 五、有意挂账（不阻塞）

| # | 事项 | 归宿 |
| --- | --- | --- |
| 1 | `ROLE_EXECUTOR_MAP` 过渡映射 + busy 口径（task.assignee_id 归因） | **已由 Batch E registry-first 接管**（0061）；legacy CTE 保留至 Phase 3 删除（revoked×legacy 交互语义见 Batch E 报告 §2） |
| 2 | requirement_analyst 无执行体（P2-13） | Registry 手工/派生授权后解锁 |
| 3 | 前端 ai-interaction 三件套接线 + AI Team UI | **三件套接线已完成（`2329bba`，/project/assistant）**；遗留：消息 >500 条时新尾部不可达（后端需补 `after` 前向游标）；AI Team 管理 UI 待 Q4/Monitor 裁决 |
| 4 | Execution Monitor 数据面（事件表 vs audit/outbox） | UI 批次前用户裁决 |
| 5 | llm_call_record token 计量（usage 未解析） | 需要时改适配器 |
| 6 | `update_task` 不提供版本改锚 | 如需改锚须显式裁决 |
| 7 | push（含 Checkpoint-2 未 push 池 6 commits） | 用户定时刻（R4 真机完成后为宜） |

## 六、下一步建议序

V2.0-D Capability Registry V1（0061 执行体能力表+bootstrap 播种+registry-first 解析）→ 前端三件套接线 → AI Team 管理 UI（Q4 裁决口径）→ 真实栈冒烟（R4 时刻一并）。
