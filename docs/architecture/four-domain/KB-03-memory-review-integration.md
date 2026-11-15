# KB-03 · KB Memory 审核接入统一审阅中心审计（Q3 补审）

> commit `05ce33d8` · 2026-11-15 · 只读 · 回答专家定向问题 Q3
> 结论：**接入无硬冲突，工作量=后端聚合（小）+前端卡片（中）+审计下游（零新增）**；需决策的是动作集差异（revise 有/defer 无）与 K4 缺 trusted-context 门的一致性策略。

## Q3-1 待审核记录查询方式

- **无专门 "list pending revisions" 函数**。最接近的 `list_memories`（kb_service/memory.rs:1025-1094）是内存过滤（拉全部修订后按 review_status 推导，:1066-1069）——复用可行但低效。
- 可参考的 SQL 形态：review() 事务内 `SELECT ... WHERE memory_id=$1 AND review_status='PENDING_REVIEW'`（memory.rs:649-651）。
- **聚合所需字段齐备**：RevisionView（memory.rs:847-865：memory_type/title/summary/change_action/change_reason/proposed_by/created_at）；操作 link.params 必须带 `memory_id` + `revision_id`（=base_memory_revision_id，K4 CAS 基准 memory.rs:661-666、port.rs:77-79）。一条 `WHERE review_status='PENDING_REVIEW'` + tenant/project 过滤的 SQL 即可产出。

## Q3-2 command_center 装配模式与后端改动面

代表例 proposal_pending（action.rs:106-133）：一条过滤 SQL + 逐行 push ActionItem{kind,domain,id,title,detail,source_state,updated_at,action_hint,link}；计数在 pending_action_total（action.rs:9-50，合计 :49）。

新增 kind='memory_review' 后端改动（全在一个读模型文件）：
1. action.rs:9-50 加 count 段；
2. action.rs:52-206 加行查询+push 段；
3. action.rs:208-217 action_kind_rank 加 rank（缺省 `_=>9`）；
4. **mod.rs 结构体零改动**（HumanActionSummary/ActionItem/ActionLink 通用形状 mod.rs:409-436，domain 自由字符串）。

## Q3-3 统一 Review Queue 接口：**不存在**

唯一聚合真源=command_center 读模型 human_action_summary（action.rs:52-205；前端注释明言「不另造后端端点」reviewCenter.ts:6-9）。无 queue 表；governance.review_approval / review_finding 都是领域状态表非队列；integration.notification 与审阅队列无关。

## Q3-4 前端改动面

| 部位 | 文件:位置 | 改动 |
|---|---|---|
| 页签归并 | api/reviewCenter.ts:18-31 reviewTabOf | 加 kind→tab 分支 |
| Store | stores/reviewCenter.ts:45（ReviewTab 类型）、:92-96+loader 模式 :183-255、submit 模式 :275-381 | 新 ctx loader + submit（**K4 CAS 参数=base_memory_revision_id，不能复用 proposal 的 expected_version_no 分支 :337-343**；含 revise 动作与 409/403 处理 :259-272） |
| 页面 | ReviewCenterPage.vue :120 emptyKind、:135-145 KIND_PARAM_TO_TAB、:201-204 计算属性、:385-393 label/tone、:482-533 context 插槽、:535-起 decision 插槽 | 各加 memory_review 分支 |
| Card 组件 | 本身 kind 无关（ReviewQueueItem 只收 label/tone props :457-458） | **零改动**；新建 MemoryReviewContext.vue（参照 FindingReviewContext/ProposalReviewContext/DeliveryReviewContext） |
| i18n | locales/zh-CN/review.yaml:13-31 + en | tabs+kind 节 |

## Q3-5 审核结果下游：换入口即原样触发

approve 事务内（memory.rs:646→790）：
- 审计 kb.memory.change_approved（:715-730）；
- outbox `kb.memory.approved` 同事务（:731-739，幂等键 `kb_memory_approved:{revision_id}`）；
- notification：该路径无写入（唯一写入方 supervision/feed.rs:74）——无缺口。
- Reject/Revise 仅审计（:749-779）。
- **从审阅中心触发 = 前端改调 POST /v1/kb/memories/{memory_id}/review，事务与下游完全继承，零后端事务改动。**

## Q3-6 与 PC Proposal 审批的同列分析

| 维度 | K4（memory review） | P5（pc proposal decision） | 冲突？ |
|---|---|---|---|
| 动作集 | approve/reject/**revise**，无 defer | approve/reject（暂记可改期） | 动作组件须按 kind 分发，不能共用按钮组 |
| 决策终局性 | **终局**（DB 触发器禁改判 BL:259） | 暂记可互转（审阅流 v2） | 卡片须标注「可改/终局」差异 |
| CAS 基准 | base_memory_revision_id + base 仍 CURRENT 二次校验 | expected_version_no | 前端 CAS 分支不能复用 |
| 权限 | guard 项目可见 + require_human | ensure_trusted_context（API Key 403）+ 服务层真人 | **无权限冲突**（两者都真人-only）；宽严差一档：K4 放行非可信上下文的真人会话，P5 会 403 |
| 语义重叠 | 都是「AI 提案→人工批准」 | 同 | 同列展示需区分文案 |

## Q3-7 工作量分级（结论）

| 项 | 分级 | 说明 |
|---|---|---|
| 后端聚合 | **小** | action.rs 三段 + 一条 PENDING_REVIEW SQL；mod.rs 零改动；K4 事务零改动 |
| 前端卡片 | **中** | reviewTabOf + store loader/submit + MemoryReviewContext.vue + 页面 6 处分支 + 双语 i18n |
| 审计/下游 | **零新增** | outbox/审计已在 K4 事务内，换入口自动继承 |
| 冲突项 | **无硬冲突** | 需决策：① 动作集差异（revise 有/defer 无）是否补 defer；② K4 是否补 trusted-context 门统一宽严（策略决定） |

## 对裁决的输入

- 直接支持 **KB-07 / D-REVIEW / P2-2（工作流 A 第 3 项）**：接入路径清晰，不触碰 B08（操作端点仍指向 KB 域 K4 权威，聚合只读）。
- 建议实施顺序：后端聚合先行（一处文件）→ 前端卡片 → K4 宽严策略随 U15 一起裁（与 cognition 决策门统一考虑）。
- 若 cognition 确认（U15/U25）同期接入，二者可共用同一套「新 kind 接入模式」，一次前端改造覆盖两个待办类型。
