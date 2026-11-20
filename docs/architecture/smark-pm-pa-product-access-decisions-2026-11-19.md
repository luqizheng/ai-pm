# smark-pm PA / Product 权限与协作模型阶段性基线

- 日期：2026-11-19
- 状态：阶段性决策基线；PA 整体尚未冻结
- 用途：供开发人员理解已确认的产品与权限方向，并作为后续架构设计依据
- 约定：本文明确区分用户已确认事项、架构建议和待确认事项。不得将待确认事项当作已决策需求实现。

## 1. 核心定位

1. **Product 是人类业务协作、信息分发与决策责任边界。**
2. **Project 是 AI 员工执行研发工作的上下文。**
3. Project 不设置人类 Owner、Admin、Member 等角色，也不建立独立的人类 Project 权限体系。
4. Product 与 Project 保持多对多关系，二者生命周期彼此独立。
5. V1 面向小型研发团队，通常不超过 30 人。避免提前建设复杂企业级权限、跨组织授权和多层级角色体系。

## 2. Team 与 Product Membership

### 已确认

- Team Membership 表示组织归属。
- Product Membership 表示参与某个 Product 的业务协作与决策。
- Team Member 不自动成为所属 Team 下所有 Product 的成员。
- Product 成员由 Product 内部管理。
- Team Owner 仍负责管理其 Team 下的所有 Products。
- 创建 Product 时，由所属 Team Owner 指定首位 Product Owner。
- Product 创建者不自动成为 Product Owner；Team Owner 也不因组织身份自动成为 Product Owner。

### 设计含义

同一个人可以属于一个或多个 Team，也可以只参与其中部分 Products。Product Membership 的主要用途是确定信息关注范围、决策参与者、通知接收者和责任归属，而不是作为普通 Project 知识库的隔离边界。

## 3. Product 人类角色

Product 仅设置两种人类身份：

### Product Owner

Product Owner 的核心职责是 Product 生命周期决策：
- 决定 Product 放弃。
- 决定已放弃的 Product 是否重新启动。

Product Owner 不垄断日常产品管理、项目管理、需求决策、技术决策或 AI 员工监督。

### Product Member

所有 Product Member 拥有相同的日常管理与决策权限，包括：
- 产品管理和需求管理；
- 项目管理与研发协调；
- 技术方案、设计和架构决策；
- Product 知识管理；
- AI Agent 工作监督、结果验证与验收；
- Issue 处理、人工决策和日常协作；
- 添加和移除其他普通 Product Member。

不设置 Product Admin，也不设置 Reporter、Developer、Tester 等细分人类角色。

## 4. Product Owner 与成员管理保护

### 已确认

- Product 可以有多位 Product Owner。
- 任意有效 Product Owner 都可以执行 Product 生命周期决策。
- 所有 Product Member 都可以添加和移除其他普通成员。
- 不允许直接移除最后一位 Product Owner。
- 更换最后一位 Product Owner 时，必须先指定继任者，再完成变更。
- Product Owner 离开所属 Team 时，不自动撤销其 Product Owner 身份。
- 最后一位 Product Owner 离开前必须指定继任者。
- 若特殊情况导致 Product 暂时没有 Owner，由所属 Team Owner 进行恢复性任命。

### 尚未完全明确

用户离开所属 Team 后，是否继续保留普通 Product Membership 的完整规则仍待确认。

此外，所有成员是否都可以任命、降级其他 Product Owner，尚未得到用户逐项明确确认，不应直接实现为已确认规则。

## 5. Product Lifecycle

V1 采用两个持久状态：
- `active`
- `abandoned`

重新启动是 `abandoned -> active` 的状态转换，不另设持久化的 `reactivated` 状态。

### Abandoned 行为（已确认）

Product Owner 决定放弃 Product 后：
- 不再接收新的 Product 需求；
- 不再产生新的 Product 业务决策任务；
- AI 不再主动开展该 Product 的业务工作；
- 保留历史资料和历史读取能力；
- Product Owner 可以决定重新启动。

### Product 与 Project 生命周期独立（已确认）

- Product 放弃不自动关闭其关联 Projects。
- Product 放弃不自动停止关联 Projects 中的 AI Agent。
- 共享 Project 可以继续服务其他仍处于 Active 状态的 Products。
- 仅服务于已放弃 Product 的 Project，后续由人类独立决定如何处理。
- Product 重新启动后，恢复该 Product 的正常协作与决策活动。
- Product Abandoned 不改变 Project 自身状态；Project Closed 与 Product Abandoned 是不同生命周期概念。

## 6. 知识库访问与信息路由

### 已确认原则

- Product Membership 决定人类需要关注哪些信息、参与哪些决策以及承担哪些责任。
- Product Membership 不作为普通 Project 知识库的隔离边界。
- 人类必须能够读取作出决策所需的充分研发知识。
- 对同一个 Project，具有有效业务访问资格的相关人类可以读取该 Project 的完整普通研发知识，包括需求、设计、技术认知、任务、决策记录及历史资料。
- 知识读取能力与正式决策权分离：能够读取知识，不代表自动获得某项正式审批权。
- 敏感内容、未发布内容、撤回版本和非正式版本仍遵循各自的状态与读取规则，不能因为普通知识开放原则而绕过这些约束。

### 架构方向

AI 员工产生需求分析、设计建议、技术决策请求、风险、阻塞和交付信息时，应根据 Product Membership 与事项归属进行信息路由，找到需要关注或决策的人，而不是把所有信息无差别发送给所有人。

## 7. 已废止或不再适用的旧设计

- 不再按原方案设计 Project Owner / Project Admin / Project Member。
- 不再建立 Project-only Member 授权机制。
- 原 PA-01“实时推导 Project 人类访问角色”的方案，不再适用于当前 Product / Project 职责模型。
- 不应将 Team Member 自动传播为所有关联 Product 的 Product Member。
- 不应将 Product Membership 当作 Project 普通知识库的访问隔离边界。
- 不应把 Product Owner 设计成传统意义上拥有全部管理权限的产品负责人。

## 8. 待确认事项

以下事项尚未被用户明确裁决：

1. Product Owner 离开 Team 后，是否继续保留普通 Product Membership。
2. 多位 Product Owner 之间的生命周期决策规则：任意一位可单独决定，还是需要共同确认。
3. 所有 Product Member 是否都可以任命、降级其他 Product Owner。
4. Product Membership 可以添加的对象范围，例如是否允许添加 Tenant 内任意 Team 的用户。
5. Product Abandoned 时，已经在途的决策请求、通知和 Agent Run 如何收尾。
6. 普通知识读取的精确业务资格边界，尤其是共享 Project 同时关联多个 Products 时，哪些人类主体属于有效业务访问者。
7. Product 成员变更、Owner 变更的审计事件和通知要求。

## 9. 开发实施约束

1. 不要重新引入 Project 人类角色表或 Project 独立成员权限体系。
2. 不要把 Product Membership 直接实现成知识库 ACL。
3. 不要将 Product Owner 扩展为所有日常管理操作的唯一审批人。
4. 不要因 Product Abandoned 自动关闭 Project 或停止其他 Active Product 仍在使用的共享 Project。
5. 所有尚未确认事项必须标记为待产品决策，不得擅自补全成冻结规则。
6. 实现时应分别建模 Team Membership、Product Membership、Product Owner 身份和 Product Lifecycle，避免混用组织归属、业务参与、决策权和生命周期责任。
