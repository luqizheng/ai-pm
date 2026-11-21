# P4-ORG / P4-PA 决策效力与修订关系总表 V1.0

**日期：** 2026-11-21  
**状态：** 基于已归档签发文件整理；不新增业务决策  
**用途：** 作为组织关系与权限模型资料包的决策索引，供基线、实施契约和验收台账引用。

## 1. 权威顺序

1. **P4-ORG 修订决策记录 V1.0（2026-11-19，DR-ORG-01~09）**：原始签发决策；保留未被后续修订的效力。
2. **PA Product Access 阶段性基线（2026-11-19）**：已确认的 Product/Project 方向与 §8 OPEN；整体未冻结。
3. **P4-PA 修订决策记录 V1.0（2026-11-20，DR-PA-01~06）**：对指定 DR-ORG 条款作后续修订；不改写旧 DR 原文，效力以 P4-PA 修订记录为准。
4. **P4-ORG 改造计划 V0.3（2026-11-20）**：实施计划和状态快照，不得覆盖正式 DR 或 OPEN。
5. 本资料包中的重建基线、实施契约、台账：解释和落实以上来源，不产生新授权。
6. P4-09 历史敏感区补充设计及 Decision Records Draft：按各自状态使用；PROPOSED 不得视为 ACCEPTED。

## 2. DR-ORG → DR-PA 修订矩阵

| P4-PA 决策 | 修订对象 | 被废止/修改的旧条款 | 保留部分 / 当前结果 |
|---|---|---|---|
| DR-PA-01 Product Membership 恢复 | DR-ORG-03 | DR-ORG-03①不建 Product 成员表、②Team Member 自动获得所属 Team 全部 Product 访问权废止 | Product 内部维护 Owner/Member；Team Member 不自动成为 Product Member；DR-ORG-03③“不跨 Team 自动获得”保留 |
| DR-PA-02 Product Owner 与成员治理边界 | DR-ORG-03 及相关 Owner 管理条款 | 以 PA 原文逐项限定 Owner 与 Member 的治理权限；不得将 OPEN 推成权限 | Owner 负责 abandon/restart；Member 平等承担日常管理与决策；Owner 任命/降级规则仍按 OPEN 限制 |
| DR-PA-03 Product 生命周期 | 新增，不替代 Project 生命周期 | 不将 Product 生命周期与 Project Closed 合并 | Product `{active, abandoned}`；abandoned 不自动关闭 Project、不停止关联 Project 的在途 Run；PA §8-5 收尾事项 OPEN |
| DR-PA-04 Project 人类角色整体废止 | DR-ORG-05①②及原 Project Owner/Admin 推导 | Project 人类 Owner/Admin/Member、Project-only Member 授权、Owner 推导方案废止 | DR-ORG-05③ Agent Registry 分离保留并成为剩余主体；Project 不设人类角色；业务操作方向指向 Product Member；共享 Project 精确访问资格为 OPEN-6 |
| DR-PA-05 Project Closed 与知识开放 | DR-ORG-06、DR-ORG-04④ | 按 PA 修订 Project Closed 与知识读取边界；不得把旧 Project 角色授权模型复活 | Project Closed 写入限制与知识读取规则按 DR-PA-05；Project Gate 守卫归属仍 OPEN（以原 DR 正文为准） |
| DR-PA-06 Issue 验收主体 | DR-ORG-07 | 将旧“Project Member 及以上”验收主体与新模型衔接 | 验收主体必须是人类 principal 且具备有效 Product Member 资格；Agent principal 排除；资格推导受 OPEN-6 约束 |

> 注：DR-PA-02/05 的详细规则及其状态以正式 `P4-PA-修订决策记录-v1.0.md` 原文为准。本矩阵只提供导航，不替代原始 DR。

## 3. 不受 P4-PA 修订影响、继续有效的 P4-ORG 决策

P4-PA 修订记录纪律声明明确：DR-ORG-01、02、08、09 不受影响，继续有效。

- **DR-ORG-01：** Team 不可解散；任何角色（含 Tenant Admin、Team Owner）均无权解散；不提供解散 API/入口，后端拒绝；无独立 dissolved 终态。
- **DR-ORG-02：** Team 仅 Owner/Member，可多 Owner；末位 Owner 保护；Tenant Admin 仅租户级，不自动穿透 Project。
- **DR-ORG-08：** 人工处理中心统一入口，不按 Developer 等人类岗位定向派发；Agent 通过 Registry/Capability 承接。
- **DR-ORG-09：** 本轮无存量数据迁移；无迁移、双写、兼容分支或过渡层。Phase 4 规划中的迁移与回滚内容须按本 DR 删除或重新定位为非本轮范围。

DR-ORG-04、06、07 中与 PA 修订明确改变的部分，必须按 DR-PA 对应条款读取；未被修订部分仍以 DR-ORG 原文为准。DR-ORG-03、05 不得整体删除，须按修订矩阵保留未被修订部分。

## 4. PA §8 七项 OPEN：不得实现

| 编号 | OPEN 原意 | 实施闸门 |
|---|---|---|
| OPEN-1 | Product Owner 离开 Team 后，普通 Product Membership 是否保留 | 不自行确定普通 Member 关系的保留/撤销规则 |
| OPEN-2 | 多位 Product Owner 的生命周期决策是否任意一位单独执行，或需收紧 | 不增加会签；不扩张超出当前明确授权 |
| OPEN-3 | Product Member 是否可任命/降级 Product Owner | 不实现 Member 的 Owner 任命/降级权限 |
| OPEN-4 | Product Membership 可添加对象范围 | 第一轮按已明确边界限制同 Tenant；跨 Tenant 不实现 |
| OPEN-5 | Product abandoned 时在途决策请求、通知、Agent Run 收尾 | 不擅自设计自动撤回/关闭/重派；遵守已明确的 Run 零触碰边界 |
| OPEN-6 | 普通知识读取的精确业务资格边界，尤其共享 Project 多 Product | 不推导 Product Member 全量读取/写入；读取资格留空或 fail-closed |
| OPEN-7 | Product 成员/Owner 变更的审计事件与通知要求 | 保留 V0.3 已明确最低审计事件；不自行增加通知/审批要求 |

OPEN 编号以 PA 基线 §8 原编号为准。任何台账别名不得取代原编号。

## 5. Team dissolve 术语纠正

DR-ORG-01 的真实要求是**删除 Team dissolve 能力及其下游清扫链**，不是执行 Team dissolve。

V0.3 的“0038 + dissolve 全链删除”应统一改写为：

> 0038 建立 Team/Product/Link 关系模型；同时移除旧 Team dissolve API、`dissolve_team` 服务及其级联清扫链，确保 Team 不存在独立解散终态。

禁止将“dissolve 全链删除”理解为“执行 Team 解散并删除关联对象”。此处属于计划文案澄清，不新增决策。

## 6. P4-09 敏感区材料的状态纪律

- P4-09 Decision Records Draft V0.1 明确声明全部 DR-01~11 均为 PROPOSED；其中 DR-01/02/03 已撤回作废。
- 同一草案说明 DR-04~11 敏感区方向继续作为后续设计材料，但该描述本身不等于全部 DR 已 ACCEPTED/FROZEN。
- P4-ORG DR-ORG-03/04 的修订关系保留 DR-ORG-04 显式可见性限制例外框架，具体策略仍 OPEN；P4-ORG 签发后待办还要求改写 P4-09 授权主体称谓，并补 Closed 态交互定义。
- 因此：敏感区不得因旧草案的“继续有效”表述而直接视为完整冻结实施契约。需要逐项查明后续签发/修订记录；在此之前保持既有安全边界，不扩大访问。

## 7. 使用规则

- 本文件是决策导航表，不替代 DR 原文。
- 新 DR 只有在明确签发后才能改变本矩阵。
- 发现计划、代码、UI 与 DR 不一致时，先登记差异；不得用实施状态反向改写决策。
- OPEN 的关闭必须有明确人工裁决和正式 DR 更新。
