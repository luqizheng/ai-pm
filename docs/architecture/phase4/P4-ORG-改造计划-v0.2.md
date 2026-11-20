# smark-pm 组织关系与权限模型改造计划 V0.2

**日期：** 2026-11-19
**用途：** 开发实施计划（人工裁决签发，取代 V0.1）
**基线：** [P4-ORG-组织关系与权限模型基线-v0.1](P4-ORG-组织关系与权限模型基线-v0.1.md)（附录 A = D-1~D-5 裁决）
**修订记录：** [P4-ORG-修订决策记录-v1.0](P4-ORG-修订决策记录-v1.0.md)（DR-ORG-01~09 已签发）

## 〇、相对 V0.1 的修正（人工裁决，2026-11-19）

V0.1 原文不入库；其全部有效内容已并入本版，修正差异登记如下：

1. **删除全部数据迁移工作**：项目无存量数据（AGENTS.md 规则 1）。不做历史数据迁移；不做旧角色兼容；不保留旧权限体系作为过渡方案；直接重建或修改数据表、枚举、约束和测试；不需要迁移验证报告。
2. **Agent 职能与人类角色彻底分离**（§三.1）。
3. **统一权限服务先于业务模块改造**（§三.2）。
4. 执行顺序改为 Phase 0-7（§四）；**Project-only 授权入口暂不纳入第一轮实现**，先完成 Team → Product → Project 这一条明确访问路径。
5. 验收目标：**不是让旧权限模型继续兼容，而是让数据库、权限服务、API、Agent 和 UI 最终统一使用新模型。**

---

## 一、开发目标

统一 Tenant、Team、Product、Project 的组织关系和权限计算方式，消除重复成员体系与不必要的角色划分。系统定位为软件开发公司的内部研发管理软件。

核心原则：

1. Team 是公司内部长期组织管理单元，**不允许解散**。
2. Product 是业务管理对象，归属于一个 Team，不独立维护成员名单。
3. Project 是研发项目，可以关联多个 Product（多对多）。
4. 用户通过 Team → Product → Project 关系获得访问资格。
5. Project 内只保留 Member、Admin、Owner 三种角色。
6. Project Owner 由关联 Product 所属 Team 的 Team Owner 自动推导，不人工任命。
7. AI Agent 是主要研发执行主体，人类 Member 负责综合协作和验证。
8. Project Closed 状态下，所有写入行为必须被后端禁止。
9. 不做任何数据迁移/兼容层，直接改契约、重建表。
10. Agent 职能（ai_pm/architect/developer/tester）不是人类访问角色，独立建模。
11. 统一权限推导服务先行，先于业务模块改造。
12. Tenant Admin 仅负责 Tenant 级管理，不穿透 Project（D-1）。

## 二、目标组织关系

```text
Tenant
  ├── Team A
  │     ├── Team Owner(s)
  │     ├── Team Member(s)
  │     ├── Product A
  │     └── Product B
  │
  └── Team B
        ├── Team Owner(s)
        ├── Team Member(s)
        └── Product C

Product A ─────┐
Product B ─────┼── Project X
Product C ─────┘
```

约束：

- 一个 Tenant 可以有多个 Team；Tenant 是公司组织边界。
- 一个用户可以加入同一 Tenant 下多个 Team。
- 一个 Product 只能归属于一个 Team；一个 Team 可以管理多个 Product。
- 一个 Product 可以关联多个 Project；一个 Project 可以关联多个 Product。
- Team 不允许解散；Product 不再拥有独立成员名单。
- 不允许通过 Product 成员关系建立第二套权限体系。
- 不允许从 Project 反向获得 Product / Team / 其他 Product 的访问权。
- Tenant Owner/Admin 不自动穿透获得 Project 管理权限（D-1）。

## 三、模型分离原则（新增，人工裁决）

### 3.1 人类访问角色与 Agent 职能分离

目标结构：

```text
Project Membership
├── Human Access Role
│   ├── Member
│   ├── Admin
│   └── Owner
│
└── Agent Registry / Capability
    ├── AI PM
    ├── Architect
    ├── Developer
    └── Tester
```

- Agent 职能仅用于任务分配、能力匹配和执行策略。
- Agent 职能不得成为人类用户的 Project Role。
- Agent 职能不得因此获得人类 Member 的验收权限。

### 3.2 统一权限推导服务（先于业务模块改造）

```text
Tenant
  ↓
Team Membership
  ↓
Team Owner / Member
  ↓
Team Products
  ↓
Product-Project Links
  ↓
Project Access Qualification
  ↓
Project Role
  ↓
Operation Authorization
```

- 所有 API、MCP、Agent 工具、异步任务都必须调用统一授权逻辑。
- 必须分清三件事，不得混成一个 `is_project_member` 判断：
  1. **访问资格**：用户是否能进入 Project。
  2. **操作权限**：进入 Project 后可以做什么。
  3. **对象状态限制**：Project 当前是否允许写入。

---

## 四、开发阶段

### 4.0 顺序总表

| 顺序 | 阶段 | 工作内容 | 是否阻塞后续 |
|---|---|---|---|
| Phase 0 | 文档基线 | D-1~D-5 落实：基线入库、DR-ORG-01~09 签发、P4-09 甲部分撤回（**本批已完成**） | 是（已完成） |
| Phase 1 | 组织关系数据模型 | Team / Product / Link 建表；删解散通道；角色语义收敛 | 是 |
| Phase 2 | Team 成员与 Owner 管理 | 邀请/任命/降级/退出/最后 Owner 保护 | 是 |
| Phase 3 | Product 管理及权限简化 | Team 推导访问；确认无第二套授权 | 是 |
| Phase 4 | Project 访问资格、三角色、Owner 自动推导 | 统一权限服务落地；枚举直改 | 是 |
| Phase 5 | Active/Closed 生命周期与后端写入门禁 | **可与 Phase 3、4 部分并行** | 否 |
| Phase 6 | 人工处理中心与 Issue 验收权限 | 统一入口；验收守卫收紧 | 依赖 Phase 4 |
| Phase 7 | 旧逻辑清理和完整测试 | 最后 | — |

不建议先改 UI 再补后端权限；先数据关系和权限计算服务，再 API、Agent 工具、UI。

### Phase 1：组织关系数据模型

任务：

1. 新建 `core.team`：归属 tenant_id；长期组织单元；无解散终态（DR-ORG-01）。
2. 新建 `core.team_member`：`(team_id, principal_id, role ∈ {owner, member})`；多 Owner 并存；最后一位 Owner 保护的约束校验。
3. 新建 `core.product`：`team_id NOT NULL`；归属唯一 Team。
4. 新建 `core.product_project_link`：Product↔Project 多对多。
5. 删除解散通道：dissolve API、`dissolve_team` 服务及全部下游清扫链与审计事件（DR-ORG-01）。
6. Tenant 角色语义收敛：Tenant Admin 仅租户级管理；tenant_member 不再充当 Team 成员表（DR-ORG-02④）。
7. `core.project` 保留 tenant_id 作为公司边界；与 Team 的关系仅经 Product 关联推导。
8. 组织关系约束与数据库测试。

重要约束：

- 不将 Team 改造成临时 Workspace；不提供解散入口（前端+后端）。
- 不为 Product 新建独立成员名单。
- **直接重建表，不搬运数据**（DDL 变更编号顺延）。

交付：DDL 变更与领域类型；组织关系约束测试。

### Phase 2：Team 成员与 Owner 管理

规则（DR-ORG-02）：

- 用户不能自行加入 Team；必须 Team 邀请并接受；同一用户可加入多个 Team；各 Team 成员关系独立管理。
- 创建 Team 者自动成为首位 Team Owner。
- Team Owner：邀请成员、移除成员、任命 Member 为 Owner、将其他 Owner 降级为 Member、管理 Team 下全部 Products。
- Owner 可退出，但必须保证至少剩余一位 Owner；最后一位 Owner 不得退出/降级/被移除。
- Team 不允许解散；后端禁止任何解散操作。

交付：Team 成员管理 API 与 UI；Owner 数量保护；成员变更审计记录。

### Phase 3：Product 权限模型简化

规则（DR-ORG-03）：

- Team Owner 自动拥有该 Team 全部 Products 的管理权限；Team Member 自动获得全部 Products 访问权。
- Product 不维护独立成员名单，不通过手动添加成员扩大访问范围。
- 不存在（也不新建）Product Owner / Product Member / 成员邀请 / 成员移除 / Product 独立权限判断等任何第二套授权代码路径。
- 边界：Product Cognition 等特殊知识内容的发布与读取规则仍遵循其领域自身状态和授权约束；Team Member 的自动访问资格不自动扩大到未发布内容。

交付：Product 权限 API；权限测试（含「无第二套授权」检查）。

### Phase 4：Project 访问资格与角色重构

#### 4.1 访问资格（DR-ORG-04）

- 来源一（本轮实现）：用户属于某 Team，该 Team 管理的 Product 关联此 Project → 自动获得访问资格；多 Team 关联时并集。
- 来源二（Project-only 授权）：**暂不纳入第一轮实现**；入口与授权管理按基线 §13-4 后置裁决后补充。
- 访问资格与操作权限分离；不允许从 Project 反向获得 Product 权限。

#### 4.2 Project 角色（DR-ORG-05）

**Member**：录入和修改正式需求（直接修改，不经 Admin/Owner 逐次审批，D-4）；修改项目知识；协助 AI 分析需求；响应 Agent 疑问；解决执行阻塞；参与讨论；验证 Agent 测试与交付结果；提交并确认 Issue。

**Admin**：Member 全部权限 + 项目管理、管理性裁定与项目决策；由 Project Owner 任命/撤销。

**Owner**：Admin 全部权限 + 关闭/重新开启 Project；与 Admin 的唯一差异是生命周期管理权。

#### 4.3 Owner 自动推导

```text
Project Owner = 关联 Project 的全部 Product 所属 Team 的全部 Team Owner
```

- 实时推导、多来源并集；Team Owner 变化自动同步；失去某 Team Owner 身份即失去对应来源，仍经其他有效关联 Team 保持 Owner 身份则保留。
- 删除人工 Owner 转移 API 与建项目 owner 播种；不需要创建者成为 Owner、不需要继任机制。

#### 4.4 角色落地（DR-ORG-05 + D-2）

- project_member.role CHECK 六值直改三值 `{member, admin, owner}`；领域常量重定义。
- ai_pm/architect/developer/tester 迁出该列，落独立 Agent Registry / Capability 模型；bootstrap 播种的 dev_agent 行迁移。
- 统一权限推导服务按 §三.2 落地（三事分立）。
- 验收守卫收紧：「人类 principal + member 及以上」，显式排除 Agent principal（DR-ORG-07③）。

交付：资格计算服务、角色权限服务、Owner 推导、Admin 任命与撤销、Agent Registry 迁移、权限回归测试。

### Phase 5：Project 生命周期控制（可与 Phase 3/4 部分并行）

规则（DR-ORG-06）：

- status ∈ {active, closed}；Active 正常运行；Closed 全局只读。
- 仅 Project Owner（推导集合）可关闭/重开；Admin 不可。
- Closed：所有人类用户只读；AI Agent 不修改项目数据、不继续 Coding 写入；AI 可读取、检索、引用历史知识并回答历史问题。Closed 不是删除，也不禁止 AI 使用历史知识。
- 写入拦截必须由后端统一执行（复用 project_gate 门禁，改状态语义与守卫角色），覆盖全部写入口：API、Agent 工具、MCP、异步任务、内部服务；对写入口做覆盖审计。
- 在飞 Agent Run 的处理策略为待确认项，任何策略不得绕过 Closed 写入限制（fail-closed）。
- 重开恢复 Active、原有角色与写入；关闭/重开全程审计。

交付：生命周期 API（close/reopen）；Closed 全局写拦截；Agent 执行入口状态检查；审计；并发与异步写入测试。

### Phase 6：人工处理中心与 Issue 验收（依赖 Phase 4）

规则（DR-ORG-07/08）：

- Agent 遇需求不明确、需人类决策、阻塞、技术问题、缺背景、需验证交付时，统一进人工处理中心；不定向指派 Developer；下线职能定向派发映射。
- 人类 Member 按综合职责处理事项；Agent 按人类处理结果继续执行。
- Issue 流程：Member 提交 → Agent/执行方处理 → 提交解决结果 → Member 验证 → 已解决（Resolved）/ 未解决（Reopened）。
- Member 可提交 Issue；Agent 可处理并提交结果但**不得代替 Member 确认最终验收**；Admin/Owner 处理管理性争议。

交付：人工处理中心统一入口；Issue 状态流转调整；Agent 与人工确认边界（验收守卫收紧落地）；权限测试。

### Phase 7：旧权限体系清理与整体回归

重点检查：

1. 无任何 Product 独立成员授权 / 第二套授权路径。
2. 无 Reporter、Developer、Tester 等人类角色残留（Agent 职能仅存在于 Agent Registry）。
3. 无手动维护 Project Owner 的逻辑（transfer_owner、bootstrap owner 播种已删）。
4. 无 Team Member 无法访问所属 Team Products 的情况。
5. 无 Team Member 无法访问关联 Project 的情况。
6. 多 Team 关联同一 Project 时正确合并访问资格。
7. Team Owner 变化后 Project Owner 正确同步（含降级/移除后的来源级撤销）。
8. 无 Team 解散入口或 API（含残留路由、服务、清扫链）。
9. Closed 状态无任何写入漏洞（全入口覆盖）。
10. Agent、MCP、API、异步任务遵循同一套权限规则。
11. Tenant Owner/Admin 无 Project 侧兜底权限残留（D-1）；Agent principal 无法触达人类验收入口（D-2 + DR-ORG-07）。

交付：旧权限代码清理报告；全量权限回归测试；架构文档与实际代码一致性检查。

---

## 五、技术原则

### 5.1 权限统一计算

统一权限判断入口，不在业务模块重复编写关系判断。至少提供：用户是否属于 Team；是否有权访问 Product；是否有权访问 Project；在 Project 中的有效角色；是否可管理 Project 生命周期；Project 当前是否允许写入。

### 5.2 权限来源可追溯

拥有 Project 访问权或 Owner 身份时，系统能解释来源链：

```text
User A → Team Owner of Team A → Team A manages Product A → Product A linked to Project X → User A is Project Owner
```

用于权限调试、审计和问题排查。

### 5.3 关联变化自动生效

Team 成员加入/退出、Member 晋升 Owner、Owner 降级/被移除、Link 建立或解除——实时反映到有效权限；不靠人工同步多份成员名单。

### 5.4 前后端权限一致

前端按权限展示入口；后端负责最终授权判断；绕过 UI 的调用不能突破权限边界。

### 5.5 审计

组织成员变化、角色变化、Project Owner 来源变化、Project 关闭与重开、Issue 验收等关键操作留审计记录。

---

## 六、暂不阻塞事项

以下不作为主干开发阻塞条件：

1. Tenant 是否严格等同公司，及 Tenant Admin 具体操作清单（方向已裁 D-1，操作边界待补）。
2. Project-only 用户的完整授权入口（第二轮）。
3. 多个 Project Owner 之间的协调与决策冲突。
4. Project Closed 时在飞 Agent Run 的具体终止策略（fail-closed 底线不动）。
5. Product 归档、删除及历史关联处理细节。

纪律：不擅自增加复杂权限体系；不创建与已确认模型冲突的新角色；不为未明确事项提前建设复杂抽象；需要决定的细节记录为待确认项；不允许未明确事项破坏已确认规则。

---

## 七、验收标准

1. Tenant 下可以有多个 Team。
2. 用户可以加入同一 Tenant 下多个 Team。
3. Team 不允许解散。
4. Team Owner 可以管理 Team 成员（多 Owner；最后 Owner 保护）。
5. Product 只归属于一个 Team。
6. Product 不再维护独立成员名单。
7. Team Member 自动访问所属 Team 的全部 Products。
8. Team Member 自动访问这些 Products 关联的 Projects。
9. Project 可以关联多个 Team 的 Products。
10. Project 访问资格与 Project Role 分离。
11. Project 只保留 Member、Admin、Owner。
12. Project Owner 根据关联 Team Owner 自动推导。
13. Project Closed 后所有写入均被后端禁止。
14. AI Agent 是主要执行主体，人类 Member 负责综合协作。
15. Agent 不能代替 Member 验收 Issue。
16. 旧角色和旧成员授权逻辑不再与新模型并存。
17. Agent、MCP、API、异步任务遵循同一套权限规则。
18. Tenant Owner/Admin 不自动穿透 Project 权限（D-1）。
19. project_member.role 仅三值；Agent 职能在独立 Agent Registry（D-2）。
20. 统一权限服务三事分立（资格/操作/状态），全部入口统一调用。
21. 人类 Member 直接修改正式需求与项目知识不经审批；审阅链仅约束 AI 回写（D-4）。

**最终要求：开发完成后，代码实现、数据库模型、API、Agent 工具、前端交互和架构文档必须使用同一套组织关系与权限模型；验收目标不是让旧权限模型继续兼容，而是全面统一到新模型。**
