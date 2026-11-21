# smark-pm 组织关系与权限模型：数据库、权限服务与 API 实施契约 V0.1

- 日期：2026-11-21
- 状态：实施契约草案（以 2026-11-20 V0.3 为当前输入；需代码核验）
- 上位基线：`P4-ORG-PA-组织关系与权限模型正式基线-v1.0.md`
- 目标：把已确定的组织关系和权限原则转成可执行的数据库、事务、服务/API、审计和测试要求。

> 本文只写 V0.3 与可确认决策明确支持的契约。未确认字段、状态、授权范围不得由实现者自行补齐。下列 SQL 为逻辑结构示意，不替代实际迁移文件；字段类型、FK、索引名和约束名须以 0038/0040/0041 实际迁移核验。

## 1. 数据模型契约

### 1.1 `core.team`

职责：租户内长期组织单元。

要求：
- Team 必须归属一个 Tenant。
- Team 是否提供 dissolve 生命周期入口存在 V0.3 内部冲突（模型总图写“不可解散”，Phase 1 交付写“dissolve 全链删除”）；在 DR-ORG/0038 核验前，不新增或扩大 dissolve 行为。
- Team Owner/Member 关系由 `core.team_member` 维护。
- Team Owner 可管理 Team 下 Product，但不因此自动成为 Product Member/Owner。

### 1.2 `core.team_member`

逻辑字段：

| 字段 | 逻辑含义 | 约束方向 |
|---|---|---|
| `team_id` | Team 标识 | 必填，引用同租户 Team |
| `principal_id` | 人类主体 | 必填 |
| `tenant_id` | Tenant 边界 | 必填，与 Team 一致 |
| `role` | Team 身份 | `owner` / `member` |

要求：
- 唯一性为 Team 内同一 principal 一条有效成员关系。
- 支持多 Owner。
- 末位 Owner 不可被直接移除；变更须继任者先行、原子提交。
- leave 与成员管理必须走统一服务管线并写审计。
- dissolve 相关机制必须作为阻断核验项：确认其是否仍是正式业务能力、是否仅用于历史清理，以及与 Team 长期不可解散表述如何统一。

### 1.3 `core.product`

逻辑字段：

| 字段 | 逻辑含义 | 约束方向 |
|---|---|---|
| `id` | Product 主键 | 唯一标识 |
| `tenant_id` | Tenant 边界 | 必填 |
| `team_id` | 唯一所属 Team | 必填，Team 与 Product 租户一致 |
| `product_key` | Product 稳定业务键 | 租户内唯一、创建后不可变（需与 DR-ORG 当前版核验）|
| `status` | 生命周期 | `active` / `abandoned`，默认 `active`，CHECK |
| creator/audit fields | 创建审计 | 按现有迁移实际字段 |

要求：
- Product 不可跨 Team 转移。
- Product 创建只允许所属 Team Owner。
- Product 创建请求必须明确首位 Product Owner。
- Product 行与首位 Owner membership 同事务写入。

### 1.4 `core.product_member`

逻辑字段：

| 字段 | 逻辑含义 | 约束方向 |
|---|---|---|
| `product_id` | Product 标识 | 必填 |
| `principal_id` | 人类主体 | 必填 |
| `tenant_id` | Tenant 边界 | 必填，与 Product 一致 |
| `role` | Product 身份 | CHECK：`owner` / `member` |

约束：
- PK `(product_id, principal_id)`。
- 同一 Product/principal 只能有一条当前成员记录。
- 第一轮只允许同 Tenant principal 加入；跨 Tenant 仍 OPEN，不实现。
- 不得出现 `admin`、`reporter`、`developer`、`tester` 等 Product 人类角色值。
- Owner 与 Member 是身份，不应通过 Team Owner 身份自动推导。

### 1.5 `core.product_project_link`

职责：表达 Product 与 Project 的多对多业务关联。

要求：
- Product 和 Project 必须同 Tenant。
- Link 不改变 Product/Project 的所有权与生命周期。
- 关联/解除关联必须记录操作者、时间及状态历史（精确字段/解除语义以 DR-ORG/PA 当前版核验）。
- 不允许以 link 创建或结束为由自动复制对象、迁移资产或传播生命周期。
- Product abandoned 后不自动删除 link；历史读取和关联历史保留。

### 1.6 `ai.project_agent_member`

职责：承载 Project 内 Agent 职能/成员关系。

要求：
- `ai_pm`、`architect`、`developer`、`tester` 等是 Agent Registry/Capability 语义，不是人类角色。
- 解析任务承接人时通过 Registry 能力匹配。
- 不允许从 Agent Registry 推导人类访问资格、业务决策权或验收权。

## 2. 事务边界

### 2.1 创建 Product

一个事务内完成：
1. 从认证上下文取得 Tenant/principal，不信任请求体自报 tenant。
2. 校验调用者是目标 Team 的有效 Team Owner。
3. 校验指定首位 Owner 是允许范围内的 principal（第一轮同 Tenant）。
4. 插入 `core.product`，状态默认 `active`。
5. 插入 `core.product_member` 首位 Owner 记录。
6. 写入创建审计事件。
7. 一次提交；任一步失败全部回滚。

### 2.2 添加/移除普通 Product Member

- 调用者必须是该 Product 的有效 Product Member。
- 只能通过本 API 添加/移除普通 `member`。
- 禁止此管线直接创建、降级或移除 Owner。
- 添加/移除与审计记录同事务。
- 同一成员重复添加应幂等或返回明确冲突，不产生重复行。

### 2.3 Owner 继任

- 禁止直接移除最后一位 Owner。
- 继任者必须先被明确指定并满足成员资格规则。
- 在同一事务内完成继任者 Owner 身份写入与原 Owner 身份变更。
- 并发请求必须串行化/加锁或通过条件更新防止出现零 Owner 状态。
- 恢复性任命只允许所属 Team Owner 通过明确恢复入口执行；该入口必须记录原因和审计。

### 2.4 Product abandon/restart

- 调用者必须是有效 Product Owner。
- 状态转换必须检查当前状态并以原子条件更新避免竞态。
- abandon 后 `product_gate` 在所有新业务写入口执行 fail-closed。
- 历史读取不被 `product_gate` 阻断。
- 不修改关联 Project 状态，不修改/终止 Project Run，不批量改写任务状态。
- restart 只恢复 Product 本身的 Active 状态；不自动恢复历史任务或 Run。

## 3. 权限服务契约

建议统一采用三个独立判断结果，不允许调用方绕开：

```text
AuthorizationDecision
  eligibility: eligible | ineligible | unresolved
  operation: allowed | denied | unresolved
  state_gate: open | blocked | unresolved
  reason_codes: [...]
```

以上是接口语义示意，不是当前代码结构声明。

### 3.1 Eligibility

判定主体是否具备目标信息/对象的访问资格。

- Team 面：按 `core.team_member` 的有效关系判断。
- Product 面：按 `core.product_member` 判断；Tenant Admin 不因管理身份自动成为 Product Member。
- Project 普通知识读取：V0.3 明确要求权限服务读取资格实现暂留 OPEN；在 OPEN-6 未关闭前，不得擅自写入“所有关联 Product Member 全量可读”或“仅某一 Product Member 可读”等规则。应 fail-closed 或由已批准的现有 Project 资格规则承载。
- Agent principal 与 human principal 必须区分；Agent 不可冒充人类主体。

### 3.2 Operation Permission

操作权限根据对象域与动作分别判定。

- Product Owner：Product abandon/restart；特殊 Owner 继任/恢复入口按专门规则。
- Product Member：日常 Product 管理/业务决策；添加/移除普通 Product Member。
- Product Member 不自动具有 Owner 任命/降级权。
- Project 不存在 Project 人类角色；项目写操作由关联 Product 的已确认日常权限映射与 Project 状态守卫共同决定。未明确的映射保持 OPEN。
- Agent 可执行授权范围内的 Task/Issue/Risk/Blocker 与研发任务；不得执行人类专属正式确认。

### 3.3 State Gate

- Product `abandoned`：阻断新 Product 需求、新业务决策任务、AI 主动业务入口；允许历史读取。
- Project `closed`：所有写入口由 `project_gate` 阻断；close/reopen API 未开放。
- 不能把状态门混入成员角色判断；即使主体具备资格和操作能力，对象状态仍可独立拒绝。

## 4. API 契约（逻辑级）

以下是接口职责，不强行规定 URL/HTTP method；具体路由命名以现有 Rust API 风格核验。

| API 能力 | 认证/授权 | 核心输入 | 原子行为 | 关键错误 |
|---|---|---|---|---|
| Create Product | Team Owner | Team、Product 基础信息、首位 Owner | Product + Owner membership + audit | TEAM_OWNER_REQUIRED、OWNER_INVALID |
| Add Product Member | Product Member | principal | membership + audit | PRODUCT_MEMBER_REQUIRED、CROSS_TENANT_NOT_ALLOWED |
| Remove Product Member | Product Member | principal | 校验非 Owner 后移除 + audit | OWNER_PROTECTED、MEMBER_NOT_FOUND |
| Transfer Product Owner | 专门继任规则 | successor、outgoing owner | 原子继任 + audit | OWNER_LAST、SUCCESSOR_REQUIRED |
| Recovery appoint Owner | 所属 Team Owner | successor、reason | 恢复性任命 + audit | TEAM_OWNER_REQUIRED、SUCCESSOR_INVALID |
| Abandon Product | Product Owner | reason/expected state | 状态更新 + audit | PRODUCT_OWNER_REQUIRED、INVALID_STATE |
| Restart Product | Product Owner | expected state | 状态更新 + audit | PRODUCT_OWNER_REQUIRED、INVALID_STATE |
| Close/Reopen Project | **不开放** | — | — | ENDPOINT_NOT_AVAILABLE |

### 4.1 错误响应

- 错误码应稳定、可测试，不通过含敏感对象信息的错误消息泄露资源存在性。
- 授权失败、状态阻断、输入校验失败应区分，便于审计和客户端提示。
- 不能把数据库 CHECK/FK 错误直接当作唯一业务错误反馈；服务层应先做明确校验，数据库约束作为最后防线。

## 5. Audit 契约

V0.3 指定至少以下事件：

- `product_member.added`
- `product_member.removed`
- `product_member.role_changed`
- `product.lifecycle_changed`

事件必须能识别：tenant、product、actor principal、目标 principal（如适用）、旧值/新值、时间、请求/关联上下文。实际 JSON payload 和事件表字段以现有 `tenant_audit_log` 管线核验。

Team 成员管理事件沿 Phase 2 已有审计管线，不在本契约另造第二套审计系统。

## 6. Product Gate 与 Project Gate

### 6.1 Product Gate

必须覆盖：
- 新建 Product 需求；
- 新建 Product 业务决策任务；
- AI 主动发起 Product 业务入口；
- 所有其他被定义为 Product 新业务写入的路径。

规则：
- `active`：继续进行权限判定。
- `abandoned`：拒绝新业务写入/主动入口，fail-closed。
- 历史读取不拦截。
- 不触碰 Project 与在飞 Run。

### 6.2 Project Gate

- 对 Project 所有写入入口集中拦截。
- `closed`：写操作拒绝。
- `active`：按权限服务继续判定。
- `close/reopen` API 暂不开放；当前守卫先行并 fail-closed。
- 守卫必须覆盖 REST、MCP、后台任务、Agent 工具、异步 Worker、批量导入和任何绕过 UI 的写入口。

## 7. Agent Registry 迁移契约

- 将 `ai_pm/architect/developer/tester` 从旧 `core.project_member.role` 域迁至 Agent Registry/Capability。
- bootstrap 中创建旧 dev_agent 人类/角色行的逻辑必须删除或替换为 Registry seed。
- `resolve_role_assignee`、`ROLE_EXECUTOR_MAP` 等旧映射必须下线。
- Agent 承接任务由能力匹配解析；无匹配时显式返回 unresolved/unsupported，不回退到旧人类角色。
- 旧 Reporter/Developer/Tester 人类角色值全仓清理；V0.3 已确认 Reporter 全仓 0 命中，Developer/Tester 只允许出现在 Agent 职能域。

## 8. 必测用例

### 8.1 Tenant / Team

1. 跨 Tenant principal 不得加入 Team/Product（Product 第一轮限制已明确；Team 沿既有约束）。
2. Team Owner 可以创建所属 Team Product，普通 Team Member 不可以。
3. Team Owner 不因 Team 身份自动成为 Product Member/Owner。
4. Product 创建者不是指定首位 Owner 时，不自动取得 Owner 身份。
5. 多 Team Owner 并存有效；移除末位 Owner 被拒绝。

### 8.2 Product Membership

6. Team Member 不自动成为 Product Member。
7. Product Member 可以添加/移除普通 Member。
8. Product Member 不可通过普通成员管线任命/降级 Owner。
9. 末位 Product Owner 不可被移除。
10. Owner 继任必须 successor-first 且原子完成。
11. Owner 离开 Team 不自动撤销 Owner 身份。
12. Owner 离开前必须完成继任；无 Owner 时仅 Team Owner 恢复性任命。
13. 同 Tenant 边界校验；跨 Tenant 成员请求拒绝。

### 8.3 Lifecycle

14. Product abandoned 后新需求写入拒绝。
15. Product abandoned 后新业务决策任务拒绝。
16. Product abandoned 后 AI 主动业务入口拒绝。
17. Product abandoned 后历史读取仍允许。
18. Product abandoned 不关闭关联 Project。
19. Product abandoned 不终止/取消关联 Project 的在飞 Run。
20. Product restart 不自动恢复历史 Task/Run。
21. Project closed 写入被 `project_gate` 阻断。
22. close/reopen API 不可调用。

### 8.4 Agent / Human

23. Agent Registry 职能不进入人类角色字段。
24. Agent principal 不能通过人类验收守卫。
25. 任务承接解析只走 Registry/Capability，不回退旧角色映射。
26. Agent 不得代替人类完成正式业务决策。

### 8.5 三事分立与安全

27. 有访问资格但无操作权限的主体可读不可写。
28. 操作权限具备但对象状态关闭时写入仍拒绝。
29. 状态开放但主体资格 unresolved 时按 fail-closed 处理。
30. UI 隐藏操作不影响后端守卫测试；直接 API 调用仍被正确拒绝。
31. 搜索、列表、详情、导出、下载等数据出口的权限结果一致。
32. Product Membership 不被误用为 Project 普通知识 ACL。

## 9. 实施核验清单

- [ ] 读取 0038/0040/0041 迁移全文并与表契约逐字段比对。
- [ ] 检查所有 Product 写入 API 是否统一调用 Product Gate。
- [ ] 检查所有 Project 写入入口是否统一调用 Project Gate。
- [ ] 全仓搜索 `project_member.role`、`is_project_manager`、`resolve_role_assignee`、`ROLE_EXECUTOR_MAP`、Reporter/Developer/Tester。
- [ ] 检查 Product 创建、Owner 继任、Member 变更的事务与并发保护。
- [ ] 检查审计事件完整性。
- [ ] 检查 Agent principal 与 human principal 的类型隔离。
- [ ] 检查 PA §8 七项 OPEN 未被代码隐式实现。

---

**实现原则**：数据库约束负责不可绕过的完整性底线；统一服务负责可解释的权限决策；每个业务入口负责调用统一服务；审计记录负责还原关键权限与生命周期变化。V1 不引入第二套权限框架或复杂策略引擎。
