# KP-3 KB 三层最小面 · 契约差异说明 V1.0

| 项 | 值 |
| --- | --- |
| 日期 | 2026-11-08 |
| 批次 | KP-3（V0.5 范围基线 V1.3 §3.3/§五，目标 4 主交付物） |
| 分支 | `feat/kp3-kb-minimal-surface` |
| 上位契约 | `kb-pc-independent-services-architecture-v1.1.md` §12（API 契约，错误码表沿 V1.0 §12.1）/ §13.3（事件目录）；`crates/kb-pc-contracts`（端口与事件的类型化承载）；`docs/ai-pm-slice-d2-mcp-context-v1.md` §4.2（`pm_get_run_context` 字段→来源映射） |
| 纪律 | 任务书「不改契约形状；确需改签名先落差异说明再动」——本文先于端口改动落库；凡涉 MCP 四工具契约者已逐项上报裁决 |

本批**未改动任何端口方法签名**（`KbServicePort` / `PcServicePort` 全部方法的参数与返回类型原样保留），未新增/修改事件目录条目，未新增 MCP 工具、未改任何工具入参。下列差异逐项登记：性质、依据、影响面、后续归宿。

## D-KP3-1 · `PortError` 补齐契约错误码表的两类结果（additive 枚举变体）

- **差异**：`kb_pc_contracts::ports::PortError` 新增 `Conflict(String)`（→ HTTP 409）与 `Internal(String)`（→ HTTP 500）。
- **依据**：V1.0 §12.1 错误码表（V1.1 §12.1「沿 V1.0」继承）明示 `409 CONFLICT（幂等键冲突但参数不一致、状态冲突）` 与 `500 INTERNAL`；§12.2 K4 幂等列 = **CAS**（`base_memory_revision_id`）——CAS 失配是契约内的合法结果，而现有枚举（`NotWired/NotFound/Forbidden/Invalid`）无法表达，只能误报为 400。这是**类型承载对既有契约的补齐**，不是契约变更。
- **影响面**：全仓 `PortError` 穷尽匹配点仅 `apps/pm-api/src/knowledge_services.rs` 的 `From<PortError> for ApiError`，同批更新；`PortNotWired` 不受影响；PC 端口（KP-1/KP-2 落点）可直接复用 `Conflict` 表达 P5 的 `expected_version_no` CAS。
- **归宿**：随 Phase 7 transport 评审确认错误体 wire 形态（业务错误码字段）。

## D-KP3-2 · K4 端口返回类型沿用 `KbAnalysisStatus`（不改签名，适配层映射）

- **差异**：§12.2 K4 响应为 `200 { memory_id, review_status }`；Phase 1-3 骨架的 trait 签名 `review_memory(..) -> Result<KbAnalysisStatus, PortError>` 与之不同构。
- **处置**：签名不动。实现侧映射：`status` = 审核结论（`APPROVED` / `REJECTED` / `REVISION_REQUESTED`）；`memory_draft_refs[0]` = 被审修订四元组 `{knowledge_base, memory_revision, <revision_id>, null}`；其余字段空。REST 层按 §12.2 形状输出 `{ memory_id, review_status, revision_ref }`。
- **归宿**：Phase 7 拆分前评审——建议换专用返回类型 `KbMemoryReviewResult`（届时为签名变更，须单独裁决）。

## D-KP3-3 · K4 调用方身份与项目作用域绑定在端口实例（不改签名）

- **差异**：trait 方法无 actor / project 参数；K4 需要审核人四身份列（审计 C174/C180、`reviewed_by`、真人校验）与项目边界。
- **处置**：`KbServiceLive::for_caller(pool, KbCaller { tenant_id, project_id, actor })` 每请求构造——进程内对 Phase 7「身份走请求头（双通道认证）+ `?project_id=` 作用域」的忠实类比；身份不进 request body，trait 不变。
- **归宿**：Phase 7 transport 以请求头承载同一信息，签名仍不变。

## D-KP3-4 · K5 `evidence_of` wire 形态定义

- **差异**：§12.2 只写 `?evidence_of=`，未定义取值形态；骨架曾臆设为「knowledge_base 文档 id」（无消费方，恒 501）。
- **处置**：定义为 `evidence_of=<memory|memory_revision>:<uuid>`（domain 隐含 `knowledge_base`），语义 = 该记忆当前版本（或指定修订）作为 evidence 引用的来源；其他取值 400。
- **归宿**：PC 回查（PK1）消费时如需更多维度，以 additive 取值扩展。

## D-KP3-5 · §12.2 未列的 V0.5 KB 管理面（KB 应用命令直调，不经端口）

- **差异**：最小面需要但 §12.2 K 表未列的入口——来源登记 `POST /v1/kb/sources`、来源详情 `GET /v1/kb/sources/{source_id}`、记忆提案 `POST /v1/kb/memories` 与 `POST /v1/kb/memories/{memory_id}/changes`、记忆列表/详情 `GET /v1/kb/memories[/{memory_id}]`。
- **处置**：挂 `/v1/kb/*` 同族路由（V1 同进程网关，鉴权链与项目可见性同 K 路由口径），由 `pm_application::kb_service` 的 KB 应用命令直接承接（同 K2/P3 进程内直连先例）；**端口形状不变**。
- **归宿**：Phase 7 拆分前评审——并入端口（新增方法 = 签名变更，届时裁决）或并入 K1 受理链（资料登记走 Intake → `kb.analysis.requested`）。

## D-KP3-6 · Platform 对象四元组沿用 `requirement` 命名空间

- **差异**：Discussion Memory 的 subject/evidence 需引用 Platform 对象（task / conversation_message / task_interaction），注册表无专职 domain 值。
- **处置**：沿 Phase 1-3 实施偏差⑧（V1.1 §0 ChangeLog #28：intake_ref/task_ref 暂取 `requirement` 作 Platform 域命名空间）——`target_domain = requirement`，以 `target_type` 区分；未改注册表。
- **归宿**：随 Phase 4 目录评审一并决定是否增设专职 domain 值（§7.3 domain 词表可扩展）。

## D-KP3-7 · `pm_get_run_context` 契约 additive 字段 `discussion_memory`（**已裁决**）

- **差异**：Discussion Memory 按 Task 注入 Agent Context 必然触碰 MCP 四工具之一 `pm_get_run_context`——D2 §4.2 把 `knowledge_refs[]` 钉为「`task_knowledge_ref` ⋈ KB 文档身份表」且禁拼装；现有 MCP 无工具可读知识正文（Agent 凭 `canonical_path` 读知识仓库文件），记忆无对应文件，复用 `knowledge_refs` 只能给 Agent 无法解引用的锚点。
- **裁决**：**用户 2026-11-08 裁决 = B**：`RunContract` 新增可选字段 `discussion_memory[]`（摘要级，不带 body），空集合时不序列化；开关 `AI_PM_DISCUSSION_MEMORY_INJECTION=off|on` **默认 off**（参照 `AI_PM_CONVERSATION_REPLY` 先例，拼写错误按 off）；开关 off 或无命中时输出与改动前逐字节一致；`CONTRACT_VERSION` 不跳（additive）；D2 §4.2 补映射行。
- **未选方案**：A（复用 `knowledge_refs` 同形元素，仍改 §4.2 映射且无实际价值）、C（仅注入原生 AI-PM AnalysisContext）。

## D-KP3-8 · `kb.memory.approved` 生产语义与消费端既有去重

- **生产**：K4 批准与业务同事务写 outbox；payload 严守目录三字段——`memory_ref`（钉被批准修订：`{knowledge_base, memory, <memory_id>, <revision_id>}`）、`reviewed_by`（审核人 principal id）、`approval_state`（批准后的版本状态 `CURRENT` / `WITHDRAWN`）；幂等键 `kb_memory_approved:{memory_revision_id}`（目录列 = memory_revision_id）；生产前过 `events::validate_payload_references`。驳回/退回不产事件（无批准）。
- **既有消费端瑕疵（只登记不改）**：Platform 消费者 `KbMemoryApprovedHandler`（`crates/pm-infrastructure/src/kb_pc_handlers.rs`）通知去重键 = `memory_id`，同一记忆的第二次批准不再产生通知行。该文件为同日并行会话热点，本批不动；建议后续改为 `memory_revision_id`。
