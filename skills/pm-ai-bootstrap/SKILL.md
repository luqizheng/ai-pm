---
name: pm-ai-bootstrap
description: PM-AI 开发环境自动初始化——读取平台安装说明，完成技能安装、Remote MCP 配置、设备登记与自检，直到 Ready。
---

# PM-AI Bootstrap（T-090 占位骨架）

> **状态**：占位骨架（T-089 打包与分发先行）。完整七步 lifecycle 指令内容由
> T-090 实施（读 `/install/{tool}` → 识别环境 → 写 MCP 配置 → 验 `/health` +
> `/v1/pm/me` → `POST /v1/pm/devices` 登记 → 绑定项目与声明仓库 → 自检上报
> → Ready）。
>
> **本骨架已可验证的事实**（不要编造其他行为）：
> - 平台安装说明入口：`GET /install/{tool}`（Markdown 给人 / JSON 给 agent）。
> - MCP 端点：`{base_url}/mcp`（Streamable HTTP，Bearer = 真人 API Key）。
> - 健康探针：`GET /health`（公开）。
> - 设备登记/绑定/自检端点：`POST /v1/pm/devices`、
>   `POST /v1/pm/devices/{id}/bindings`、`POST /v1/pm/devices/{id}/checks`、
>   `POST /v1/pm/devices/{id}/revoke`。
> - API Key 只能由用户当面提供；**不进入**安装指令、URL、日志或本包文件。
>
> **红线**：不绕过执行端自身权限审批；不把 Key 写进任何文件除本工具 MCP
> 配置外；Skill 不绕过 MCP 工具直写平台。

执行 `scripts/verify-install`（见包内 scripts/）可对当前环境做连通性自检。
