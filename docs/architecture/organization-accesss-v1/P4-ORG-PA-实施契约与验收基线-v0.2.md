# P4-ORG / P4-PA 实施契约与验收基线 V0.2

**日期：** 2026-11-21  
**状态：** 决策对齐版实施核验基线；非代码审计报告、非架构冻结令  
**依据：** P4-ORG DR-ORG-01~09、PA 阶段性基线、P4-PA DR-PA-01~06、P4-ORG 改造计划 V0.3。

## 1. 目标模型

- Tenant 是公司边界；Tenant Admin 仅执行租户级管理，不自动获得 Product Membership 或 Project 业务访问权。
- Team 是长期组织单元，不能解散；Team Membership 表达组织归属，角色仅 Owner/Member，多 Owner 并存，至少保留一位 Owner。
- Product 唯一归属一个 Team；Product Membership 独立维护 Owner/Member，Team Member 不自动成为 Product Member；Team Owner 管理所属 Team 下 Products。
- Product 与 Project 多对多关联；关联不等于资产所有权转移，不触发自动复制或生命周期传播。
- Project 是 AI 员工研发执行上下文，不设人类 Project Owner/Admin/Member 角色体系。
- Agent Registry/Capability 只表达 Agent 职能和能力，不产生人类访问权、业务决策权或验收权。

## 2. 权限判断必须三事分立

统一权限推导服务至少分开处理：

1. **Eligibility（访问资格）：** 主体能否进入/读取目标对象。
2. **Operation（操作权限）：** 在具备资格后，能执行何种操作。
3. **State Gate（对象状态限制）：** Product/Project 当前状态是否允许该操作。

不得用单一 `is_project_member` 或 UI 可见性替代三类判断。API、MCP、Agent 工具、异步 Worker、内部服务、搜索、导出和下载等数据获取/写入路径均须执行相应后端检查。

### 2.1 Project 资格的明确限制

- Project 不存在人类角色表/角色推导。
- PA §8 OPEN-6 未关闭前，不实现“所有关联 Product Member 自动获得 Project 全量读取资格”的规则。
- Product Member 的日常权限不能被解释为对 Project 全部数据的无条件访问。
- 对尚无已批准资格来源的读取请求，按 fail-closed 或已批准的既有资格规则处理；不得以猜测填补 OPEN。
- Agent principal 与 human principal 必须明确区分。

## 3. 数据模型与事务契约（逻辑要求，字段细节以迁移核验为准）

### 3.1 Team

- `core.team` 归属 Tenant；Team 不设置 dissolved/deleted 业务终态。
- `core.team_member` 表达 Team Owner/Member；邀请接受后加入；末位 Owner 不得退出、降级或被移除。
- 不存在可用的 Team dissolve API/服务/入口；旧级联清扫链应被删除。

### 3.2 Product 与 Product Membership

- `core.product` 归属唯一 Team。
- `core.product_member` 仅 `{owner, member}`，同一 Product/principal 唯一；多 Owner 可并存。
- Product 创建由所属 Team Owner 发起，并指定首位 Product Owner；Product 行、Owner membership 与审计应在一个事务内完成。
- Product Member 可添加/移除普通 Member；不得由该普通成员管线直接任命、降级或移除 Owner。
- Owner 继任、末位保护、无 Owner 时 Team Owner 恢复性任命必须有原子性/并发保护和审计。
- Product 不可跨 Team 转移；Product–Project Link 必须满足同 Tenant 等已签发约束，具体数据库约束以迁移核验。

### 3.3 Product 生命周期

- `product.status ∈ {active, abandoned}`；restart 是 abandoned→active，不另设持久化 reactivated 状态。
- abandon/restart 由有效 Product Owner 发起；多 Owner 的单人决策范围受 OPEN-2 限制，不额外扩大。
- abandoned 后 product_gate 对新 Product 需求、新业务决策任务及 AI 主动业务入口 fail-closed；历史读取不由该 Gate 阻断。
- abandon 不自动关闭关联 Project、不自动停止共享 Project 中其他 Active Product 所需的 Run；OPEN-5 的其他在途通知/决策收尾不得自行补全。

### 3.4 Project 与 Agent Registry

- Project 不维护人类角色；不得从 `core.project_member.role` 推导人类权限。
- `ai.project_agent_member` / Agent Registry 表达 Agent 职能；Agent 不得获得人类正式验收权。
- Project Closed 的状态写入限制由统一 project_gate 强制；close/reopen API 是否开放按正式 DR-PA-05 的守卫归属状态执行，不得自行开放。
- Project Closed 对在飞 Run 的处理遵守正式 DR 与 OPEN；无论采用何种策略均不得绕过 Closed 写入限制。

## 4. 人工处理中心与 Issue 验收

- Agent 遇到需求不明确、需人类决策、阻塞、缺少项目背景或需验证交付时，统一进入人工处理中心，不按 Developer 等岗位定向派发。
- 人工回复/验收必须是 human principal。
- Issue 最终验收必须排除 Agent principal；在新模型下验收人类资格须结合有效 Product Member 资格，但 Project 资格映射受 OPEN-6 限制。

## 5. 审计

- 至少核验 Product Member added/removed/role_changed 与 Product lifecycle_changed 等 V0.3 已明确事件是否实际落地。
- Team 成员邀请、接受、退出、Owner 变更及末位保护相关审计按 DR-ORG-02 与 Team 面既有实现核验。
- 不因 OPEN-7 自行增加通知、审批或额外事件要求。

## 6. 实施阶段与证据纪律

V0.3 的 Phase 状态是截至 2026-11-20 的实施方报告，不等于独立审计结论。下表用于复核：

| 阶段 | V0.3 报告状态 | 必须取得的核验证据 |
|---|---|---|
| Phase 0 | 文档基线完成 | 正式基线、DR、撤回公告、版本/签发记录 |
| Phase 1 | 0038 与组织模型完成 | migration 全文、schema 实际结构、Team dissolve 路由/服务/清扫链全仓搜索、约束测试 |
| Phase 2 | 实施完成、验证收尾 | Team API/服务/UI、invite/accept/leave、Owner 保护并发测试、审计 |
| Phase 3 | 后端完成、UI 下一批 | 0040、Product API/服务、同事务创建、成员治理、Owner 继任、Gate 全入口、测试 |
| Phase 4a | Agent Registry 迁移完成 | 0041、Agent 解析全链、旧 role 映射全仓搜索、验收主体测试 |
| Phase 5 | 拆分 | Product Gate、Project Gate 全写入点、状态转换与并发测试；close/reopen API 状态 |
| Phase 6 | 待实施 | 人工处理中心聚合、权限路由、human-only 回复、Issue 验收守卫 |
| Phase 7 | 待实施 | 全量回归、旧逻辑清理、OPEN 闸门、文档/代码一致性 |

没有迁移/源码/测试证据的项目只能标记为“计划声称完成/待核验”，不得标记为“已审计通过”。

## 7. 最低验收清单

- [ ] Team 不可解散：无公开 API、无服务入口、无 UI 入口、无级联清扫路径。
- [ ] Team Owner/Member 双角色、多 Owner、末位 Owner 保护覆盖所有变更路径。
- [ ] Tenant Admin 不自动穿透 Product/Project 权限。
- [ ] Team Member 不自动成为 Product Member。
- [ ] Product 仅 Owner/Member；无 Product Admin/细分岗位角色。
- [ ] Product 创建与首位 Owner 写入同事务。
- [ ] 普通成员管线不能任命/降级 Owner；OPEN-3 未实现。
- [ ] Owner 继任、末位保护、恢复任命具备并发安全和审计。
- [ ] Product abandoned Gate 覆盖所有新业务写入口；历史读取不误拦。
- [ ] Product abandoned 不自动关闭 Project 或终止共享 Project Run。
- [ ] Project 不存在人类角色授权；Agent Registry 与人类权限隔离。
- [ ] OPEN-6 未关闭前不实现 Project 全量读取继承。
- [ ] Project Closed 写入限制覆盖 API、MCP、Agent、Worker、内部服务。
- [ ] Issue 验收排除 Agent principal，并核验 Product Member 资格路径。
- [ ] 搜索、索引、导出、下载等数据面权限一致。
- [ ] PA §8 七项 OPEN 全部保持未决，除非有后续正式裁决。

## 8. 不属于本轮实施契约的内容

- P4-09 敏感区具体策略，除已被正式 DR 明确吸收的安全底线外，须逐项核对签发状态。
- Project 共享知识读取资格的细节（OPEN-6）。
- Product abandoned 时在途通知/决策请求的完整收尾策略（OPEN-5）。
- 跨 Tenant Product Member（OPEN-4）。
- Product Member 任命/降级 Owner（OPEN-3）。
- 迁移、双写、兼容分支和过渡层（DR-ORG-09 禁止）。
