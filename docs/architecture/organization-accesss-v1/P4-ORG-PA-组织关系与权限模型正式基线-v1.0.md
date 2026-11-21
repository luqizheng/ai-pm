# smark-pm 组织关系与权限模型正式基线 V1.0（重建稿）

- 日期：2026-11-21
- 状态：**重建基线稿 / 待代码与决策记录逐项核验**
- 适用范围：Tenant、Team、Product、Project、Agent Registry、Product/Project 生命周期及人类访问权限
- 直接实施输入：`smark-pm 组织关系与权限模型改造计划 V0.3`（2026-11-20）
- 决策输入：`smark-pm-pa-product-access-decisions-2026-11-19.md`、PA-03/04/05/06 已归档决策资料
- 历史参考：P4-09 成员权限与敏感区材料、P4-01/P4-02 冻结材料、P4-00 架构审计

> **权威与冲突处理声明**：本文以 2026-11-20 的 V0.3 及 2026-11-19 PA Product Access 基线为当前组织/权限模型的最新输入。早期 P4-01/P4-02 冻结模型与 2026-11-19 P4-09 草案中“无 Product Membership / Project 成员继承 Product / Project 角色推导”等旧方向，凡与 V0.3 或 PA 基线冲突者均不得继续作为当前实施依据。本文不把历史草案中的 PROPOSED 内容升级为已确认决策。正式 DR-ORG / DR-PA 文件仍需补齐后与本文逐条对照。

## 1. 目标与设计原则

### 1.1 领域职责

1. **Tenant**：公司/租户边界。Tenant Admin 的管理权仅在租户级，不因该身份自动获得 Product Membership 或 Project 业务访问权。
2. **Team**：长期组织单元，表达组织归属。Team Member 不自动成为 Team 下所有 Product 的成员。Team Owner 管理 Team 及其 Products。
3. **Product**：人类业务协作、信息分发、需求归属和业务决策责任边界。Product 独立维护 Owner/Member 与生命周期。
4. **Project**：AI 员工研发执行上下文，承载仓库、任务、设计、技术知识和执行记录；不设置人类 Project Owner/Admin/Member 角色体系。
5. **Agent Registry / Capability**：承载 AI 员工职能、能力与承接匹配，不得借用人类角色字段或获得人类正式验收权。

### 1.2 V1 规模原则

面向小团队、内部研发使用和较低并发量。优先采用可理解、可审计、可维护的直接模型；不建设企业级多层审批、跨租户成员体系或复杂角色矩阵。未被明确批准的能力保持 OPEN。

### 1.3 权限三事分立

权限服务必须把以下三件事分开表达，不得以单一 `can_access` 或角色判断混合：

- **访问资格（Eligibility）**：主体是否有资格进入/读取该对象或信息面。
- **操作权限（Operation Permission）**：主体对对象可执行哪些动作。
- **对象状态限制（Object State Gate）**：对象当前生命周期/状态是否允许该动作。

统一权限推导服务负责组合三者；业务服务仍须在唯一后端强制点执行检查，前端展示不是安全边界。

## 2. 组织关系模型

```text
Tenant
└── Team（长期组织单元）
    ├── Team Owner(s) / Team Member(s)       core.team_member
    └── Product(s)（每个 Product 唯一归属一个 Team）
        ├── Product Owner(s) / Product Member(s)  core.product_member
        └── Product ↔ Project（多对多）          core.product_project_link

Project（研发执行上下文；无 Project 人类角色）
└── Agent Registry / Capability（AI 职能与能力）
```

### 2.1 Tenant 与 Team

- Team 是长期组织单元。**生命周期存在 V0.3 内部冲突**：模型总图注明“不可解散”，但 Phase 1 交付又写有“dissolve 全链删除”。在核对 DR-ORG 与 0038 实际实现前，不把 Team 是否可解散写成最终规则，也不得新增/扩大 dissolve 行为。
- Team Membership 表示组织归属，不等于 Product 参与资格。
- Team Owner 可以管理其 Team 下的 Products，并负责 Product 创建入口。
- Team Owner 多人并存；末位 Owner 保护、继任变更与恢复机制沿 Phase 2 已实施规则执行。
- Team 成员离开、Owner 离开及团队成员变更不得绕过既有 `core.team_member` 管线和审计。Team dissolve 是否仍存在、是否仅历史迁移能力、以及与“Team 不可解散”冲突的裁决，列为阻断核验项。

### 2.2 Product 归属

- 一个 Product 唯一归属一个 Team。
- Product 创建由所属 Team Owner 独占。
- 创建请求必须指定首位 Product Owner；创建者身份本身不自动授予 Product Owner。
- Product 创建与首位 `core.product_member` Owner 记录必须在同一事务完成。
- 不允许跨 Team 转移 Product；V1.0 具体约束须与当前迁移及 DR-ORG 记录核验后冻结。

### 2.3 Product 与 Project

- Product 与 Project 为多对多关系，由 `core.product_project_link` 表达。
- 一个 Project 可服务多个 Product；一个 Product 可关联多个 Project。
- 关联本身不转移对象所有权，也不自动改变双方生命周期。
- 业务上下文归 Product；研发执行与共享技术资产归 Project。
- 需求可归属一个或多个 Product；Task 归属 Project，可通过需求关联识别服务的 Product；允许无需求关联的 Project 公共技术任务。
- 同一共享实现只维护一份 Task/Design/技术知识，不因 Product 数量复制。

## 3. Team Membership 与 Product Membership

### 3.1 Team Membership

- 存储：`core.team_member`。
- 身份：Team Owner、Team Member。
- 语义：组织归属及 Team 管理关系。
- Team Member 不自动成为所属 Team 下任何 Product 的成员。

### 3.2 Product Membership

- 存储：`core.product_member`。
- 主体：Tenant 内 principal（V0.3 第一轮范围；跨 Tenant 成员保持 OPEN，不得实现）。
- 唯一身份：`owner`、`member`；不设 Product Admin，也不设 Reporter/Developer/Tester 等人类细分角色。
- 主键：`(product_id, principal_id)`；记录须带 `tenant_id` 并满足租户一致性约束。
- Product Membership 由 Product 内部管理；不从 Team Membership 或 Project Membership 自动继承。

### 3.3 Product Owner

Product Owner 的核心特殊职责是 Product 生命周期决策：

- 执行 Product abandon / restart。
- 可有多位 Owner。
- 任意有效 Product Owner 可单独执行生命周期动作（PA 基线中“任意有效 Owner”方向；若正式 DR-PA-02 对此有不同裁决，以正式 DR 为准）。
- 不允许直接移除最后一位 Owner。
- 更换最后一位 Owner 时，先指定继任者，再原子完成变更。
- Owner 离开 Team 不自动撤销其 Product Owner 身份。
- 最后一位 Owner 离开前必须指定继任者。
- Product 暂时无 Owner 时，由所属 Team Owner 进行恢复性任命；这是明确的恢复通道，不等于 Team Owner 自动成为 Product Owner。

### 3.4 Product Member

- Product Member 之间日常管理与决策权限平等。
- 可以添加、移除普通 Product Member。
- 可以参与需求、项目协调、技术方案、设计、架构决策、知识管理、Agent 监督、Issue 处理和日常协作。
- Member 任命/降级 Owner 的权限仍是 OPEN，不得从“可管理普通成员”推导为可管理 Owner。

## 4. Project 与 Agent 身份

### 4.1 Project 不设人类角色

- 不建立 Project Owner / Project Admin / Project Member 人类角色体系。
- 不通过 `core.project_member.role` 承载人类业务权限。
- 不因 Project 创建者身份授予项目 Owner 权限。
- 不保留 Project-only 人类授权入口。
- Project 的人类操作权限由统一权限服务依据关联 Product 的有效成员资格与动作规则推导；读取资格规则仍受 PA OPEN 项约束，不能臆造具体公式。

### 4.2 Agent Registry / Capability

- `ai_pm`、`architect`、`developer`、`tester` 等职能属于 Agent Registry / Capability，不属于人类角色。
- Agent 承接任务时按 Registry 能力匹配解析，不再通过 `core.project_member.role` 或旧 `ROLE_EXECUTOR_MAP` 解析人类角色。
- Agent 职能不得赋予人类验收权、正式业务决策权或人类专属确认权。
- Agent 在已确认目标和授权范围内可自主规划、执行、验证和交付；不得改变正式业务目标或重大业务取舍。

## 5. 生命周期模型

### 5.1 Product Lifecycle

V0.3 明确 Product 状态：

- `active`
- `abandoned`

默认 `active`，由数据库 CHECK 约束状态域。

`abandoned` 的确定行为：

- 阻止新的 Product 需求、新业务决策任务和 AI 主动业务入口；
- 历史资料可读；
- 不自动关闭关联 Project；
- 不自动停止关联 Project 的在途 Run；
- 不因 Product abandoned 触碰共享 Project 中其他 Active Product 的执行。

`restart` 将 `abandoned` 转回 `active`。历史任务是否恢复执行不能仅因 Product restart 自动推断；按 PA-04 已确认规则，恢复历史任务须重新评估。

### 5.2 Project Lifecycle

V0.3 目标状态：

- `active`
- `closed`

但 `close/reopen API` 不开放，直到关闭守卫主体及业务裁决完成。当前先实现全局写入拦截机制（`project_gate`）并 fail-closed；不得开放无授权主体的 close/reopen 操作。

Project Closed 与 Product Abandoned 独立。Project Closed 的具体守卫主体、关闭语义和 reopen 规则仍受 P4-PA-05② OPEN 阻塞。

### 5.3 生命周期无传播

- Product abandoned 不自动改变 Project 状态。
- Product restart 不自动恢复 Task/Run。
- Product 与 Project 生命周期互不自动传播。
- 在途 Run 的处理遵循 V0.3 明确的“零触碰”要求；不自动取消、终止或改写 Run。

## 6. 业务归属、研发执行与信息路由

### 6.1 归属原则

- 需求归属一个或多个 Product。
- Task 属于 Project，可通过需求关系服务一个或多个 Product。
- 技术设计、架构决策和技术知识以 Project 为主要归属。
- Product 维护业务目标、需求和业务规则；Project 维护共享技术实现与技术知识。
- Product 引用 Project 技术设计，不复制技术正文。

### 6.2 冲突和验收

- Agent 识别多 Product 需求冲突，分析影响并提出方案。
- 涉及业务目标、优先级或业务取舍时必须交由人类决策。
- Agent 不得自行选择优先满足哪个 Product。
- 共享 Task 完成须满足所有关联 Active Products 的有效需求；Abandoned Product 的需求不再构成当前验收约束。
- 纯技术实现问题若不改变已确认业务要求，可由 Agent 自主处理。

### 6.3 Product Abandoned 对任务

- 只服务该 Product 的未完成任务：停止主动推进，保留任务和历史，等待人类重新评估；不自动关闭或取消。
- 同时服务 Active Product 的任务：可以继续执行，但须重新评估目标和验收标准，不再以 Abandoned Product 的目标作为当前约束。
- Project 公共技术任务：不因单个 Product 放弃而直接受影响。
- Product restart 不自动恢复历史任务。

### 6.4 信息路由

- Product Membership 用于确定业务事项的关注者、决策参与者、通知接收者和责任归属。
- 通知资格不等于知识读取资格；被通知不等于必须批准；已读不等于已处理。
- PA-03 的决策路由与通知策略继续有效：单 Product 事项路由至该 Product Members；多 Product 事项路由至相关 Product Members；Project 公共事项路由至关联 Active Products 的成员，合并去重。
- 日常决策由一位有资格的 Product Member 完成即可，完成后通知其他相关成员，不要求全体逐一批准。

## 7. 知识访问与权限边界

### 7.1 普通 Project 知识

- Product Membership 不是普通 Project 知识库 ACL。
- 具有有效业务访问资格的相关人类可读取作出决策所需的普通研发知识。
- 读取知识不自动赋予正式审批权、修改权或执行权。
- 具体哪些主体构成共享 Project 的有效业务访问者，属于 PA §8 OPEN，必须先完成裁决再落地访问公式。

### 7.2 敏感区

历史 P4-09 材料提出 Product/Project 独立敏感区、独立存储、访问时授权、普通 Agent 不读取敏感正文等方向；这些材料在旧版 DR 中多为 PROPOSED/OPEN。除非 V0.3/正式 DR 明确吸收，不得把旧草案整体当成已冻结权限契约。现阶段至少不得因普通 Product Membership 或 Project 访问资格自动开放敏感正文。

### 7.3 权限强制点

- 统一权限服务承载资格、操作、状态三类判断。
- API、搜索、索引、导出、下载等所有可获取数据的后端入口必须一致执行权限过滤。
- UI 隐藏仅是展示行为，不能替代服务层授权。
- 来源资格撤销后，授权应按最新有效关系判断；缓存不得使已撤销权限继续生效。具体缓存策略需与 PA OPEN/权限服务实现核对。

## 8. 数据与服务基线

### 8.1 已知表/迁移

| 对象 | 当前已知实现 | 说明 |
|---|---|---|
| Team | `core.team` / `core.team_member` | Migration 0038；Phase 2 已实施并验证收尾中 |
| Product | `core.product` | Migration 0038 创建；Product 状态在 0040 增补 |
| Product Membership | `core.product_member` | Migration 0040 |
| Product–Project Link | `core.product_project_link` | Migration 0038 |
| Agent Registry membership | `ai.project_agent_member` | Migration 0041 |

### 8.2 统一服务与守卫

- `product_gate`：Product abandoned 状态拦截新业务写入/新决策/AI 主动业务入口；历史读取不拦。
- `project_gate`：Project closed 写入拦截的统一后端守卫；先行 fail-closed，close/reopen API 保持关闭。
- 权限推导服务：分别判定访问资格、操作权限、对象状态限制。
- Product Member 管理管线沿 Team 面模式，成员变更与生命周期变化必须有审计事件。

### 8.3 API 范围（实施目标，不代表所有端点已交付）

- Product 创建：Team Owner 独占，指定首位 Product Owner，事务内创建 Product 与 Owner membership。
- Product Member 管理：Product Member 可添加/移除普通成员；第一轮仅 Tenant 内 principal。
- Owner 管理：继任制、末位保护、Team Owner 恢复性任命。
- Product 生命周期：Owner 执行 abandon/restart。
- Project 生命周期：close/reopen 暂不开放。

## 9. 已废止设计与禁止回归

1. Product 不设 Admin；Product 人类角色只有 Owner/Member。
2. Team Member 不自动获得 Product Membership。
3. 不恢复“Project 三个人类角色 + Owner 推导”模型。
4. 不通过 `core.project_member.role` 表达 Agent 职能。
5. 不允许 Tenant Admin 自动穿透成为 Product Member 或 Project 业务访问者。
6. 不把 Product Membership 直接当作普通 Project KB ACL。
7. 不因 Product abandoned 自动关闭 Project、取消/停止关联 Project 的在途 Run。
8. 不因 Product–Project Link 自动复制业务知识或技术资产。
9. 不实现 PA §8 OPEN 中尚未裁决的事项。

## 10. OPEN 台账（必须保持 OPEN）

以下为 V0.3 明确列出的 PA §8 七项；不得因重建文档而关闭：

| 编号 | OPEN | 实施约束 |
|---|---|---|
| OPEN-1 | Product Owner 离开 Team 后，普通 Product Membership 是否保留 | 不推导、不自动清理普通 Member 关系 |
| OPEN-2 | Product Owner abandon/restart 决策是否可由任意 Owner 单独执行、是否需收紧 | 当前 V0.3 暂按有效 Owner 操作，但保留可收紧点，不扩大权限 |
| OPEN-3 | Member 是否可任命/降级 Owner | 不实现 |
| OPEN-4 | Product Member 添加对象范围 | 第一轮仅 Tenant 内 principal；跨 Tenant 不实现 |
| OPEN-5 | Product abandoned 时在途决策请求、通知和 Agent Run 如何收尾 | V0.3 已明确关联 Project/在飞 Run 零触碰；其他请求/通知收尾不擅自实现 |
| OPEN-6 | 普通知识读取的精确业务资格边界，尤其共享 Project 多 Product 场景 | 权限服务接口可预留，但读取资格实现留空/fail-closed |
| OPEN-7 | Product 成员/Owner 变更审计事件和通知的完整要求 | V0.3 已列基本审计事件；扩展通知策略不得自行补成冻结规则 |

> 注：编号按 V0.3 对应语义整理。应与 `P4-PA-修订决策记录-v1.0.md` 的原始编号和台账逐项核对；如原记录编号不同，以原始 DR 台账为准，不改写其编号。

## 11. 实施状态快照（2026-11-20）

| Phase | 状态 | 已知交付 |
|---|---|---|
| Phase 0 文档基线 | 完成 | 基线入库、DR-ORG-01～09 签发、P4-09 甲部分撤回 |
| Phase 1 组织数据模型 | 完成 | 0038：core.team/team_member/product/product_project_link；dissolve 全链删除与约束测试 |
| Phase 2 Team 成员管理 | 实施完成，验证收尾中 | Team 面切 core.team_member；leave；Owner 管理管线；前端切换 |
| Phase 3 Product 域 | 后端完成，UI 下一批 | 0040；product.rs/products.rs；product_domain 8 用例 |
| Phase 4 Project 角色域 | 4a 完成 | 0041 ai.project_agent_member；解析链换源；role CHECK 收敛；legacy 映射删除 |
| Phase 5 生命周期 | 拆分 | Product abandoned 纳入 Phase 3；Project Closed 受守卫 OPEN 阻塞 |
| Phase 6 人工处理中心 | 待实施 | 依赖 Phase 3；验收主体=Product Member；Product Membership 资格补齐受 OPEN-6 限制 |
| Phase 7 清理回归 | 待实施 | 全量权限回归、文档/代码一致性、OPEN 台账核对 |

## 12. 冻结前置核验

本文是基于可访问资料重建的正式基线候选，不宣称已完成代码审计或正式冻结。发布为 FROZEN 前必须：

1. 取得并逐项核对 DR-ORG-01～09 正式签发文本，优先解决 Team “不可解散”与 Phase 1 “dissolve 全链删除”的内部冲突。
2. 取得并逐项核对 DR-PA-01～06 正式修订记录及 PA §8 OPEN 原表。
3. 对照 0038/0040/0041 迁移全文及当前代码，验证表约束、事务、路由、守卫、审计和 UI 状态。
4. 复核 Product abandoned 全部写入口覆盖，确认 fail-closed 且历史读取不受影响。
5. 复核 Project closed 写入守卫已覆盖全部写入口，且 close/reopen API 未提前开放。
6. 对 Agent Registry 承接解析做全仓搜索，确认旧角色映射无残留。
7. 验证人工处理中心的验收主体排除 Agent principal。
8. 确认七项 OPEN 均保留、无擅自实现。

---

**文档维护规则**：后续修改必须同时更新领域模型、权限规则、OPEN 台账、实施状态和验收标准。若新 DR 改变本文规则，先修订 DR 与基线，再更新实现；禁止代码先行形成未记录的权限语义。
