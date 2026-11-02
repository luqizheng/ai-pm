---
name: pm-task-execution
description: 用 PM-AI 的 MCP 工具领取任务、读取执行契约、上报进度并提交结果——执行端侧的工作规范。
---

# PM-AI Task Execution（T-090 占位骨架）

> **状态**：占位骨架（T-089 打包与分发先行）；工作规范正文由 T-090 补全。
>
> **可用的 MCP 工具面**（18 个，`tools/list` 为准）：
> - **发现**：`pm_list_tasks`（项目内 ready 任务 + 最新 queued run id）。
> - **领取与执行**：`pm_claim_task`（签发一次性 lease_token）→
>   `pm_get_run_context`（执行契约）→ `pm_report_progress`（续租 + 阶段推进）
>   → `pm_submit_result`（交付落库 + 业务驳回语义）。
> - **状态查询**：`pm_get_submission_status`、`pm_get_task_context`。
> - **知识/文档**：`pm_submit_document` / `pm_propose_document` /
>   `pm_confirm_document_proposal` / `pm_veto_document_revision`（需
>   `PM_MCP_DOCUMENT_WRITE`）。
> - **项目认知**：`pm_discovery_*` 六个（`pm-learn` skill 驱动）。
>
> **执行请求头**：`X-AI-PM-Device-Id: <device.json 中的设备 id>`——仅归因，
> 不构成权限；设备声明非法不影响授权。
>
> **关键语义**（正文由 T-090 展开，先给出硬约束）：
> - `lease_token` 明文只在 `pm_claim_task` 响应出现一次；服务端只存 sha256。
> - `lease_generation` 双验证；心跳 ≠ 进展（`last_agent_heartbeat_at` 与
>   `last_progress_at` 分立）。
> - `pm_submit_result` 的业务驳回（409 语义）**不是调用失败**：响应带
>   `outcome: "rejected"` 与机器可读错误码，调用方据此知道「没有交付」。
> - 所有状态变更由平台后端统一处理；执行端不得绕过 MCP 工具直写。
