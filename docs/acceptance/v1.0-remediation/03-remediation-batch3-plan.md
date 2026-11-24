# 03 — 第三批修复计划（R6）：执行派发面 + 导航一致性

- 日期：2026-11-24
- 触发：阶段 6 Runner 接入运行态复验发现的四处产品缺口（Runner 协议链路已验证 ✅，产品入口断链 ❌）

## 缺陷清单与修复方案

### D1（P2）执行授权无授予/拒绝入口
- 现状：`/project/execution/auth` 页明示「授权是一次性人工命令，授予/拒绝入口尚未开放；本页呈现等待授权的事实」（`ExecutionAuthorizationView` 只读）
- 后端：授权命令 API 待查（grep `auth.*decision|grant` in apps/pm-api/src/pm_devices.rs / execution 相关模块；若后端也缺，需新增 POST 授权端点 + E5 同款「AI 主体 403、真人唯一」闸门）
- 修复：列表项加「授予/拒绝」按钮 → 真实命令 → 刷新对齐服务端

### D2（P2）人工任务无完成通路
- 现状：状态机无 `Ready→Completed` 边（`pm-domain/src/task_transition.rs:49-51` 只有 Ready→Cancelled/Claimed/Superseded）；「无可用执行者」人工任务（如 f244b03c）成为永久 DAG 阻塞头
- 方案（保域不变量）：新增 `Ready→Completed` 边但**仅限人工任务**（task kind/无 executor 匹配），且要求 `reason` 必填留痕（对齐 FR-16 回退留痕纪律）；在 `task_transition.rs` 的 `TransitionInput` 加 `manual_completion: bool` + 校验；任务详情页给「人工完成（需填理由）」按钮
- 测试：域层非法边负例 + 合法人工完成正例（无理由拒绝）

### D3（P2）设备就绪判定
- 现状：执行中心「设备就绪 0/1」——设备在册、runner claim 正常，但就绪计数为 0；`last_heartbeat_at=null`（claim 前无心跳）
- 方案：查 `is_device_ready` 判定源（grep `device_ready|heartbeat` in pm-application）；预期=runner 心跳/或 claim 时写 `last_heartbeat_at` + 就绪=心跳新鲜度窗口；补齐 runner_loop 心跳上报或放宽判定

### D4（P3）任务页无导航入口
- 三层菜单 PLANNED 占位未接线；`/project/tasks*` 仅可 URL/卡片进入
- 方案：`navigation.ts` 开发组追加「任务管理」（mainNavGroups 机制，参考 memory「3层主菜单改造」）

### D5（P2）三视图 project_id 只读 query 不回退 store
- `CommandCenterView.vue:55` / `ReviewCenterPage.vue:47` / `TechnicalDesignDetailView.vue:266` 的 `currentProjectId()` 仅读 `route.query.project_id`；顶栏已选项目时左导航点击仍报「请先在顶栏选择一个项目」
- 方案：改为 query 优先、`useDesignProject()` store 回退（对齐 DiscoveryView 模式）+ 一致性测试

## 已就绪的验证资产
- Runner：设备 win-10-1-1-49（e110d8d8）绑定 TaskFlow；投放包 `C:\ai-pm-runner-1049\`（start.bat 带重试循环）；.49 已连通（claim 到达平台）
- 部署：`AI_PM_MCP_ALLOWED_HOSTS=192.168.2.103,192.168.2.103:8090` 已配；防火墙 8090 已放行
- 解锁路径：D2 修复后 f244b03c 人工完成 → 8 任务解锁 → runner 认领 → 阶段 6/7 走通（ZCode 在 .49 执行）
- 可选：`AI_PM_RUN_DISPATCH=on` + 排除列表（无人值守派发，部署指引）

## 验收口径
- D1：授权列表出现可操作入口，授予后任务进入可认领队列
- D2：人工任务带理由完成后，下游任务 dependency 解锁，runner 10s 内 claim
- D5：左导航点「项目总览」在有选中项目时直接渲染（不再要求选择）
