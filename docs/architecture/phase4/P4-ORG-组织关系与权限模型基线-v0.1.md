# smark-pm 组织关系与权限模型调整说明

**版本：V0.1**  
**日期：2026-11-19**  
**状态：已裁决入库（2026-11-19）——人工确认基线；D-1~D-5 裁决同日生效（附录 A）；修订记录见《P4-ORG-修订决策记录-v1.0》，执行计划见《P4-ORG-改造计划-v0.2》**

---

## 1. 本次调整目标

smark-pm 是面向软件开发公司的内部管理软件，不是面向互联网公众的通用项目协作平台。

系统需要围绕软件开发公司的实际组织关系建立权限体系：

- Team 是公司内部长期存在的组织管理单元。
- Team 管理多个 Product。
- Product 与 Project 是多对多关系。
- 人类成员通过 Team 获得 Product 和 Project 的访问权。
- Project 内通过 Member、Admin、Owner 管理具体操作权限。
- AI Agent 承担主要研发执行工作，人类成员承担综合协作、需求分析、技术支持和结果验证。

本次调整的核心目标是：

**减少权限层级，取消不必要的独立成员体系，避免重复授权和复杂的权限继承。**

## 2. 核心组织关系

### 2.1 组织结构

```text
Tenant（公司组织边界）
  │
  ├── Team A
  │     ├── Team Owner
  │     ├── Team Member
  │     ├── Product A
  │     └── Product B
  │
  ├── Team B
  │     ├── Team Owner
  │     ├── Team Member
  │     └── Product C
  │
  └── Team C
        ├── Team Owner
        └── Product D
```

Product 与 Project 的关系：

```text
Team A
  └── Product A ─────┐
                     │
Team B               ├── Project X
  └── Product C ─────┘

Team A
  └── Product B ─────── Project Y

Team C
  └── Product D ─────── Project Y
```

### 2.2 关系约束

| 关系 | 规则 |
|---|---|
| Tenant → Team | 一个 Tenant 可以拥有多个 Team |
| User ↔ Team | 多对多，一个人可以加入同一 Tenant 下多个 Team |
| Team → Product | 一对多，一个 Product 只归属一个 Team |
| Product ↔ Project | 多对多 |
| Team → Project | 通过 Product-Project 关联间接建立访问关系 |

注意：

1. Product 只能归属于一个 Team。
2. Product 可以关联多个 Project。
3. Project 可以关联多个 Product。
4. 同一个 Project 可以同时被不同 Team 管理的 Products 关联。
5. 不允许通过 Project 反向获得其他 Product 的访问权。

## 3. Team 成员管理

### 3.1 Team 成员加入

用户必须通过 Team 邀请加入。

规则：

1. 用户不能自行加入 Team。
2. Team Owner 发出邀请。
3. 用户接受邀请后成为 Team Member。
4. 用户可以同时属于同一个 Tenant 下的多个 Team。
5. 每个 Team 的成员关系独立管理。
6. 加入一个 Team 不会自动加入其他 Team。

### 3.2 Team Owner

一个 Team 可以有多个 Team Owner。

规则：

- 创建 Team 的人自动成为第一位 Team Owner。
- 现有 Team Owner 可以任命其他 Team Member 为 Owner。
- Team Owner 可以邀请成员。
- Team Owner 可以移除 Team Member。
- Team Owner 可以任命其他 Owner。
- Team Owner 可以将其他 Owner 降级为普通 Member。
- Team Owner 可以管理 Team 下的所有 Products。
- Team Owner 可以退出 Team，但必须保证至少保留一位 Owner。
- 不允许降级或移除最后一位 Owner。

Team 必须始终至少有一位 Owner。

### 3.3 Team 生命周期

**Team 不允许解散。**

这是公司内部管理软件的组织设计原则，不是普通互联网协作平台的临时团队模型。

任何角色均无权解散 Team，包括 Tenant Admin。

Team 是长期存在的组织管理单元。

## 4. Product 管理与访问规则

### 4.1 Product 管理

Product 归属于一个 Team。

不再单独维护 Product 成员名单。

权限规则：

| 身份 | Product 权限 |
|---|---|
| Team Owner | 自动拥有该 Team 下全部 Products 的管理权限 |
| Team Member | 自动获得该 Team 下全部 Products 的访问权限 |
| 其他 Team 成员 | 不自动获得访问权 |

Product 管理权由所属 Team 的 Team Owner 负责。

### 4.2 Product 访问范围

Team Member 加入 Team 后，自动获得：

- 该 Team 管理的全部 Products 的访问权。
- 这些 Products 的基本信息和业务内容读取权。
- 这些 Products 关联的全部 Projects 的访问资格。

不需要再进行 Product 成员添加。

### 4.3 Product 与 Project 的访问传播

Product 与 Project 是多对多关系。

访问规则：

只要用户所属 Team 管理的任意一个 Product 关联了某个 Project，该用户就获得该 Project 的访问资格。

例如：

- Team A 管理 Product A。
- Team B 管理 Product B。
- Product A 和 Product B 都关联 Project X。

那么 Team A 和 Team B 的成员均可以访问 Project X。

不需要额外的跨 Team 授权。

## 5. Project 访问与角色模型

### 5.1 Project 访问来源

Project 访问资格有以下来源：

**来源一：Team 关联访问**

用户通过：

```text
User
  → Team Membership
  → Team Product
  → Product-Project Link
  → Project
```

获得 Project 访问资格。

**来源二：Project 直接访问**

如果用户只在某个 Project 内部，不属于关联 Product 所属 Team，则只允许访问该 Project 内部内容。

不能因为拥有 Project 访问权而反向获得 Product 访问权。

直接 Project-only 访问的具体邀请入口和授权管理方式需要在实现设计中保留明确边界，不得自动扩展为 Product 权限。

### 5.2 Project Role

取消 Reporter、Developer、Tester 等传统细分角色。

Project 只保留三个角色：

| 角色 | 定位 |
|---|---|
| Member | 综合项目参与者 |
| Admin | 项目经理 |
| Owner | 项目最高管理者 |

### 5.3 Member 职责

Member 是综合项目参与者，不再按照传统开发公司岗位拆分项目权限。

Member 负责：

1. 录入客户需求。
2. 修改正式需求。
3. 协助 AI 分析需求细节。
4. 修改项目知识。
5. 响应 AI Agent 的疑问。
6. 处理 AI Agent Coding 过程中的技术问题。
7. 协助解决执行阻塞。
8. 参与项目讨论。
9. 验证 AI Agent 的测试和交付结果。
10. 提交 Issue。
11. 确认 Issue 是否已解决或未解决。
12. 向 AI 提问和提供项目相关信息。

Member 可以直接修改正式需求和项目知识，不需要每次修改都经过 Admin 或 Owner 审批。

### 5.4 Admin 职责

Admin 是项目经理。

拥有 Member 的全部权限，另外负责：

- 项目管理。
- 项目内管理性裁定。
- 项目决策。
- 项目日常管理工作。

### 5.5 Owner 职责

Owner 与 Admin 拥有相同的项目管理权限。

Owner 唯一额外的权限是：

**关闭项目和重新开启项目。**

不额外建立一套复杂的 Owner 专属管理权限。

## 6. Project Owner 的产生方式

这是本次调整的重要规则。

Project 不再单独维护一套需要人工任命的 Owner 成员关系。

Project Owner 由关联 Product 所属 Team 的 Team Owner 自动获得。

规则：

1. 当 Product 关联 Project 时，该 Product 所属 Team 的所有 Team Owner 自动获得该 Project 的 Owner 权限。
2. 一个 Project 如果关联多个 Team 管理的 Products，那么所有相关 Team 的 Team Owner 都自动成为该 Project 的 Owner。
3. Team Owner 身份发生变化时，Project Owner 权限自动同步。
4. Team Owner 不再担任某 Team 的 Owner 后，也同步失去通过该 Team 获得的 Project Owner 权限。
5. 如果该用户仍通过其他关联 Team 具有 Owner 身份，则保留对应权限。
6. 不需要 Project 创建者自动成为第一位 Owner。
7. 不需要单独任命 Project Owner。
8. 不需要单独维护 Project Owner 的继任机制。

Project Owner 权限必须根据有效的 Team Owner 身份及 Product-Project 关联实时推导。

## 7. Project Role 的默认规则

通过 Team → Product → Project 关联获得访问资格的普通成员：

默认成为 Project Member。

Project Admin 由 Project Owner 任命。

Project Owner 不需要人工任命，由关联 Team Owner 自动产生。

Project 内角色由以下规则决定：

| 来源 | Project Role |
|---|---|
| 关联 Team 的普通成员 | Member |
| 关联 Team 的 Team Owner | Owner |
| Project Owner 任命的 Admin | Admin |
| 其他具有 Project-only 访问资格的用户 | 默认 Member，具体授权入口待实现设计明确 |

注意：一个人可能属于多个 Team，也可能通过多个 Product 获得同一个 Project 的访问权。权限计算必须支持多个有效来源。

## 8. AI Agent 与人工处理中心

smark-pm 的研发模式不是传统的 Developer 直接执行代码、Reporter 负责测试。

AI Agent 是主要研发执行主体。

人类 Member 负责综合协作。

AI Agent 在执行过程中需要人类介入时：

- 统一提交到人工处理中心。
- 由人工处理中心接管问题、阻塞、澄清和决策请求。
- 不采用 Agent 直接指定某个 Developer 的机制。
- Member 根据权限和实际职责处理相关事项。
- AI Agent 根据人类处理结果继续执行。

人工处理中心是统一的人机协作入口，不应为不同人类执行岗位建立复杂的角色体系。

## 9. Issue 工作规则

Issue 主要用于记录问题、跟踪解决情况和验证结果。

基本流程：

```text
Member 提交 Issue
       │
       ▼
AI Agent / 研发执行处理
       │
       ▼
提交解决结果，等待验证
       │
       ▼
Member 验证
       │
       ├── 已解决 → Resolved
       │
       └── 未解决 → Reopened / 继续处理
```

规则：

1. Member 可以提交 Issue。
2. AI Agent 可以提交处理结果。
3. AI Agent 不可以直接确认 Issue 为 Resolved。
4. 最终是否已解决，由 Member 验证并确认。
5. Member 可以确认「已解决」或「未解决」。
6. Admin 和 Owner 可以依据项目管理权限处理争议及裁定事项。

## 10. Project 生命周期

Project 只定义两个主要生命周期状态：

- Active：正常运行。
- Closed：关闭，只读。

### 10.1 Active

正常状态下：

- 人类成员可以按角色权限读取和写入。
- AI Agent 可以正常执行项目任务。
- 可以进行需求、知识、Issue 和讨论等操作。

### 10.2 Closed

Owner 关闭项目后：

- 所有人类用户只能读取项目内容。
- 所有人类用户不能写入项目。
- AI Agent 不能修改项目数据。
- AI Agent 不能继续 Coding 并写入项目。
- AI Agent 可以读取、检索和引用项目知识。
- AI Agent 可以回答关于历史需求、设计、技术决策等问题。

Closed 不是删除项目，也不是禁止 AI 使用历史知识。

### 10.3 重新开启

只有 Owner 可以重新开启项目。

重新开启后：

- 恢复 Active 状态。
- 恢复原有 Project Role 权限。
- 恢复正常项目写入。
- 恢复 AI Agent 正常执行能力。

Admin 不具备关闭或重新开启项目的权限。

## 11. 需要开发遵循的权限计算原则

建议以统一的权限服务实现，不要在各业务模块中重复开发权限继承逻辑。

权限判断需要区分：

1. 用户身份是否有效。
2. 用户是否具有有效 Team Membership。
3. Team 是否管理相关 Product。
4. Product 是否关联目标 Project。
5. 用户是否具有 Project-only 访问资格。
6. 用户在目标 Project 中的有效角色。
7. Project 当前是否为 Active 或 Closed。
8. 本次操作是否属于允许的读取、写入或管理操作。

建议遵循以下原则：

- 访问资格和操作权限分开判断。
- 多个访问来源采用有效来源并集。
- 任一来源失效，只撤销该来源。
- 其他有效来源继续生效。
- 不允许从 Project 反向推导 Product 访问权。
- 不允许从 Project Member 自动获得其他关联 Team 或 Product 的权限。
- Closed 状态的写入限制必须由后端统一强制执行，不能只依赖前端隐藏按钮。
- Team Owner 产生的 Project Owner 权限必须支持多个来源。

## 12. 旧架构影响与修订要求

本次调整会影响已有 P4 架构，需要先做影响审查，再正式修订文件。

重点检查：

| 原有设计 | 本次调整方向 | 处理要求 |
|---|---|---|
| P4-01 Team 解散后 Product 终止 | Team 永远不允许解散 | 必须修订 P4-01 |
| Product 独立成员管理 | Product 不再维护独立成员名单 | 检查并修订相关模型 |
| Product Owner 独立管理权限 | 由 Team Owner 自动管理 Product | 检查 P4-01 |
| Product 成员手动授权 | Team 成员自动访问 Team 下 Products | 检查 P4-09 |
| Project Owner 独立任命 | 由关联 Team Owner 自动推导 | 修订 Project 权限设计 |
| Reporter / Developer 角色 | 统一为 Member | 修订 Project Role |
| Project 角色继承规则 | Team Owner 自动拥有 Project Owner | 检查 P4-09 |
| AI Agent 人工指派 | 人工处理中心统一接管 | 检查 Agent 协作设计 |
| Project 关闭规则 | Owner 关闭/重新开启，Closed 只读 | 修订 Project Lifecycle |

特别注意：

不能因为本文件中的人工确认，就直接修改已有冻结文档并宣称原有决议已经失效。

必须保留原始决策记录，并通过正式 Decision Record / Revision 流程修订受影响条款。

## 13. 当前尚需明确的事项

本文件是本轮确认的设计基线，不代表整个组织权限模型已经冻结。

后续还需要处理：

1. Tenant 是否严格等同软件开发公司。
2. Tenant Admin 的完整职责及权限边界。
3. Team Owner 对 Product 的具体管理操作。
4. Project-only 用户的邀请、授权和撤销入口。
5. Project Admin 的任命、撤销和具体管理操作边界。
6. 多个 Team 同时管理同一 Project 时的 Admin 任命与管理协调规则。
7. Team 成员退出或被移除后的权限实时撤销。
8. Project Closed 时正在执行的 AI Agent Run 如何终止或处理。
9. 已有 P4-01、P4-02、P4-03、P4-09 的正式文档修订与影响审查。

---

## 14. 给开发团队的执行要求

本次文档首先用于统一架构理解。

请开发团队暂时不要直接按照旧的 Reporter、Developer、Product 独立成员、Project 独立 Owner 模型继续扩展。

建议执行顺序：

1. 对照现有数据库和权限服务，盘点当前实现。
2. 对照本文件，列出模型冲突。
3. 列出需要修改的数据库关系、权限计算和 API。
4. 列出需要调整的 UI 和交互。
5. 评估数据迁移方案。
6. 完成架构修订和正式决议。
7. 再进入实现。

**最终架构原则：Team 管组织与 Product，Project 管项目协作，Project Role 管具体操作，AI Agent 承担主要研发执行，人类 Member 负责综合协作。**

不再为同一用户在组织、产品、项目之间建立多套重复的成员管理体系。

---

## 附录 A：D-1~D-5 裁决记录（2026-11-19，人工裁决生效）

> 正文 V0.1 为确认文本，存档不动；本附录记录入库时同步生效的裁决。正式修订记录见《P4-ORG-修订决策记录-v1.0》。

| 编号 | 裁决 | 理由 |
|---|---|---|
| D-1 | Tenant Admin 选 B：仅负责 Tenant 级管理，不拥有 Project 侧兜底权限；不因 Tenant 身份自动获得 Project Owner / Admin / 写入权限 | 保持组织管理与项目管理边界；Tenant Owner/Admin 不自动穿透获得 Project 管理权限 |
| D-2 | Agent 职能选 A：独立 Agent Registry / Capability 模型；ai_pm / architect / developer / tester 是 Agent 职能，不是人类访问角色，迁出 project_member.role | Agent 职能可用于任务分配、能力匹配和执行策略，但不能成为人类 Project Role，也不能获得人类 Member 的验收权限 |
| D-3 | P4-09 DR-01/02/03 撤回，按新模型重写 | 旧方案引入 Product 独立成员和 Project→Product 反向传播，与当前模型相反；不保留两套相互矛盾的授权规则 |
| D-4 | 人类 Member 直接修改正式需求和项目知识，不需要 AI 审批链 | K4 等审阅机制用于约束 AI 生成或回写的内容，不能反过来限制人类 Member 的正常编辑权限 |
| D-5 | 本调整说明与改造计划归档 docs，作为本次修订的引用基线 | 解决仅存会话粘贴件、缺少稳定文档引用的问题 |

配套执行安排：改造计划按人工裁决三修正（删除全部数据迁移工作、Agent 职能与人类角色彻底分离、统一权限服务先于业务模块改造）升版为 V0.2；执行顺序 Phase 0-7；Project-only 授权入口暂不纳入第一轮实现（先完成 Team → Product → Project 明确路径）。
