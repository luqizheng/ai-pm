---
name: pm-knowledge
description: 用 PM-AI 的 MCP 文档工具读写 Git-backed 知识库——质量门、红线提案与撤销窗口的执行端侧规范。
---

# PM-AI Knowledge（T-090 占位骨架）

> **状态**：占位骨架（T-089 打包与分发先行）；正文由 T-090 补全。
>
> **工具面**（需 `PM_MCP_DOCUMENT_WRITE`）：`pm_submit_document`（直建，同
> doc_key 不允许两次）／`pm_propose_document`（走质量门：clean 自动提交，
> 红线类 `external_commitment` / `delete_history` / `rewrite_delivered` 挂起
> 待人确认）／`pm_confirm_document_proposal`（人类确认红线提案）／
> `pm_veto_document_revision`（撤销窗口内 Git revert + 回滚审计）。
>
> **硬约束**：
> - `base_revision_id` 乐观并发：基于已发布修订改写时必带；冲突 → 409。
> - 幂等键 `operation_id`：同 key 重放不产生第二条修订。
> - Markdown 正文 LF；提案需携带 `source_events:` 或文档链接（质量门要求）。
> - 知识最终写入权在平台质量门与人类确认，Skill 不直写。
