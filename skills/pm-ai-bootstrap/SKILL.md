---
name: pm-ai-bootstrap
description: PM-AI 开发环境自动初始化——读取平台安装说明，完成技能安装、Remote MCP 配置、设备登记与自检，直到 Ready。
---

# PM-AI Bootstrap

把当前开发环境初始化为 PM-AI 的已登记开发环境：技能包就位、Remote MCP 配置写入、设备登记、
项目绑定与自检上报，直到服务端判定 Ready。**全程不手工编辑配置文件**；API Key 只由用户当面提供。

## 硬约束（先读）

1. **Key 只能向用户当面索取**；不进入安装指令、URL、日志或本包任何文件——只写入本工具的
   MCP 配置。
2. 不绕过执行端自身的权限审批（写配置文件、执行脚本都要走本工具的确认机制）。
3. Skill 不绕过 MCP 工具直写平台；所有状态变更由平台后端统一处理。
4. 设备声明（`X-AI-PM-Device-Id`）只是归因，不构成权限；声明非法不影响授权结果。
5. **就绪（Ready）是服务端状态**，不是安装程序的最后一步成功信号。

## 七步生命周期

### 第 1 步：读安装说明

`GET {base_url}/install/{tool}`——本工具是 Codex 时路径为 `/install/codex`。
- agent 用 `?format=json`（含 `bootstrap` 段：技能包地址、release manifest、端点清单）。
- 用户可读同一 URL 的 Markdown 形态。
- 记下 `bootstrap.package_url` 与 `release_manifest.sha256`。

### 第 2 步：识别环境

确认：操作系统与版本（`windows` / `macos` / `linux`）、本工具的种类与版本、是否已初始化
（找本地 `device.json`，见第 5 步）。

### 第 3 步：装技能包 + 写 MCP 配置

1. 下载 `bootstrap.package_url`，**校验 SHA-256** 与 manifest 一致；不符即中止并重试一次，
   仍不符则报告用户（不要降级安装）。
2. 解包到本工具的技能目录（Codex：技能目录；trae / z-code：按 `/install/{tool}` 指引落盘）。
3. 按 `config_template` 写入 Remote MCP 配置：`{base_url}/mcp`，Bearer 认证。
4. **向用户索取 API Key**，写入配置；写前备份原配置，失败回滚（不破坏既有配置）。

### 第 4 步：验证连通

1. `GET {base_url}/health` → 200。
2. `GET {base_url}/v1/pm/me`（Bearer = 用户 key）→ 200；若 `is_active=false`，
   指引用户联系管理员激活后重试（不要继续后续步骤）。
3. 也可运行包内 `scripts/verify-install.{ps1,sh}`（参数与四项检查见脚本用法行）。

### 第 5 步：登记设备

1. 本地有 `device.json` 且其中 id 有效 → 用该 id 调 `POST /v1/pm/devices`（幂等：命中且
   owner 匹配则复用；**已撤销的设备不会通过相同 id 恢复**，服务端会发新 id）。
2. 没有 `device.json` → 不带 id 调用（服务端生成），把返回的 `id` 写入本地 `device.json`
   持久化；后续所有步骤与以后的每次运行都用同一个 id。
3. 请求体：`{name, platform, os_version, executor_kind, executor_version}`（自报值，
   仅展示用途）。

### 第 6 步：绑定项目与声明仓库

1. 问用户要绑定的 Project；`POST /v1/pm/devices/{id}/bindings`，载荷
   `{project_id, declared_repository_ids}`。
2. `declared_repository_ids` ⊆ 该项目的仓库（服务端会校验归属，越界返回 400）。
3. 幂等：重复绑定不产生重复行；**原有有效绑定不会被清除**；换声明 = 幂等更新（旧验证结果
   失效，需重新自检）。
4. 解绑是显式动作：`DELETE /v1/pm/devices/{id}/bindings/{binding_id}`——不要用「重新绑定空
   列表」代替解绑。

### 第 7 步：自检上报 → Ready

逐项检查后 `POST /v1/pm/devices/{id}/checks`：

```json
{
  "installed": true,
  "connected": true,
  "tool": true,
  "accessible_repositories": ["<本机实测 git 可访问的仓库 id>"],
  "evidence": { "...": "本地工作目录 / git 状态等证据，服务端只记录不判定" }
}
```

- 服务端**不采信客户端的 ready 声明**：就绪结论由服务端按可验证事实计算（设备有效、绑定
  存在、`verified_repositories` 非空、版本兼容），`verified_repositories` = 自报仓库 ∩ 项目
  归属校验通过的仓库。
- MCP 侧工具自检：`tools/list` 可列、`pm_list_tasks`（T-082）在绑定项目上可读。
- 响应的 `readiness` 就是当前快照：`ready` / `degraded`（部分仓库未验证）/ `failed`（安装
  未完成）/ 中间层（installed → connected → authenticated）。
- **Ready 之后**：认证过期、绑定撤销、MCP 不可用都会使快照退化——下次自检为准。日常任务
  执行见 `pm-task-execution` skill。

## 失败路径指引（给用户的话）

- `/v1/pm/me` 401：key 无效或未带 Bearer——检查配置。
- `/v1/pm/me` 200 且 `is_active=false`：账号未激活——联系租户管理员在「成员管理」激活。
- manifest SHA-256 不符：下载被劫持或不完整——重试一次，仍不符停止并报告。
- 登记 404：device_id 属于他人——删除本地 `device.json` 重新登记（不要复用他人 id）。
- 自检 `degraded`：部分声明仓库本机不可达——修 git 访问后重跑第 7 步即可，不必重装。
