# P4-ORG / P4-PA 冲突与 OPEN 核验台账 V0.2

**日期：** 2026-11-21  
**状态：** 决策对齐后的工作台账；不新增裁决。

## 1. 已关闭的文档理解问题

| 原问题 | 当前结论 | 处理 |
|---|---|---|
| C-01 Team 不可解散 vs “dissolve 全链删除” | DR-ORG-01 明确 Team 不可解散；V0.2 原文要求删除 dissolve API/service/清扫链。V0.3 的“dissolve 全链删除”应解释为删除 dissolve 功能链，不是执行解散 | 文案统一改成“移除旧 Team dissolve API、服务和级联清扫链” |
| C-03 缺 DR-ORG 文件 | 已找到正式签发 V1.0，DR-ORG-01~09 ACCEPTED | 以原文件为准，建立 DR 交叉索引 |
| C-04 缺 DR-PA 文件 | 已找到正式签发 V1.0，DR-PA-01~06 ACCEPTED | 以原文件为准，建立修订矩阵 |

## 2. 仍需保留的架构 OPEN / 阻断项

| ID | 问题 | 当前处理 | 关闭条件 |
|---|---|---|---|
| C-02 / OPEN-6 | Project 人类角色废止后，共享 Project 有效业务访问资格及读取范围 | 不建 Project 人类角色；不把 Product Member 自动解释为全量 Project reader；未批准的资格 fail-closed | 用户正式裁决并更新 DR-PA/PA 基线；同时定义读取和操作映射 |
| C-04a | P4-09 DR-04~11 敏感区正式效力及后续主体称谓修订 | 旧 Decision Records Draft 声明全部 PROPOSED；“方向继续有效”不能代替 ACCEPTED/FROZEN 证据；保持安全边界，不扩大访问 | 找到后续逐项签发/修订记录，确认每项状态、授权主体和 Closed 态交互 |
| C-05 | Product abandoned 对在途决策、通知、Run/Task 的收尾 | DR-PA-03 已明确 Product/Project 生命周期独立及 Run 不自动终止；OPEN-5 其他收尾策略保持未决 | 正式裁决并补齐 Gate/调度语义；不得通过例外绕开 abandoned Gate |
| C-06 | Product Owner 多人时的生命周期决策权限范围 | 不自行增加会签或扩张权限；以 OPEN-2 为准 | 正式裁决是否任意一位 Owner 可单独决定及适用范围 |
| C-07 | Product Owner 离开 Team 后普通 Membership 的处理 | 不自动推导普通 Member 关系的保留/撤销 | OPEN-1 正式裁决 |
| C-08 | Product Member 是否可任命/降级 Owner | 禁止实现该能力 | OPEN-3 正式裁决 |
| C-09 | Product Member 添加对象范围 | 第一轮同 Tenant；跨 Tenant 不实现 | OPEN-4 正式裁决 |
| C-10 | 成员/Owner 变更审计与通知细节 | 保留 V0.3 明确最低审计事件，不额外扩张通知/审批 | OPEN-7 正式裁决 |

## 3. Phase 4 规划文件需要的修订

`Phase4-Product资产归属演进规划基线-V1.0.md` 仍为 Draft，且包含与 DR-ORG-09 不一致的迁移/兼容工作包。处理原则：

- 保留 Product 身份、Product–Project 关系、Cognition 权威边界、资产关联、Product Plan、UX、架构评审等研究范围。
- 删除本轮“兼容现有数据”“数据迁移”“分阶段迁移及回滚”“迁移策略完成”等实施要求。
- P4-06 不再是数据迁移切片；建议改为“Data Contract & Integrity Review（数据契约与完整性核验）”，仅检查目标模型约束，不迁移存量数据、不建立兼容路径。
- 将 Gate 4 中“迁移策略完成”替换为“目标 Schema/API 契约、数据完整性规则与无迁移实施约束完成核验”。
- 验收标准中“如何兼容当前系统、如何实施和回滚”须改为“如何按新契约直接重建/直改；如何验证一致性”，不得引入 DR-ORG-09 禁止的迁移与回滚机制。

## 4. 代码核验优先级

### P0 — 决策强制点

- [ ] 0038：Team/Product/Link 字段、FK、CHECK、唯一约束；Team dissolve API/service/清扫链删除证据。
- [ ] 0040：Product Membership、Product status、tenant 一致性约束。
- [ ] 0041：Agent Registry 与旧 `project_member.role` 收敛。
- [ ] Product 创建与首位 Owner 同事务。
- [ ] Product Member 添加/移除普通成员权限；Owner 任命/降级限制。
- [ ] Owner 继任、末位保护、恢复性任命并发测试。
- [ ] Product Gate 所有新业务写入口覆盖。
- [ ] Project Gate 所有写入口覆盖，尤其 MCP、Agent、Worker、内部服务。
- [ ] Issue 验收明确拒绝 Agent principal。

### P1 — 数据面与生命周期

- [ ] 搜索、索引、导出、下载与 API 采用一致权限过滤。
- [ ] Product abandoned 不误伤共享 Project 的 Active Product。
- [ ] Project Closed 不允许任何写入绕过。
- [ ] Link 建立/解除、成员变更后资格即时重算，不读过期缓存（遵守 DR-ORG-04 尚未被修订的部分；与 OPEN-6 不冲突）。
- [ ] 审计事件与操作者归因覆盖。

### P2 — UI 与回归

- [ ] Product Member 管理 UI 与 API 权限一致。
- [ ] Project UI 不展示已废止的人类角色管理入口。
- [ ] 人工处理中心按有效权限路由，不按人类岗位硬编码。
- [ ] 全量回归与旧角色/旧权限代码清理。

## 5. 证据状态纪律

每一项必须标注以下之一：

- **Decision-confirmed：** 有正式 ACCEPTED DR 支撑。
- **Plan-reported：** V0.3 报告已完成，但未独立验证。
- **Code-verified：** 已检查当前代码/迁移/测试并记录路径与证据。
- **OPEN：** 正式待裁决，不得实现。
- **Superseded：** 被后续正式 DR 明确取代。

不得将 Plan-reported 写成 Code-verified；不得将 PROPOSED 写成 ACCEPTED；不得把本台账中的解释升级为新业务决策。
