# P4-ORG 组织权限模型修订决策记录 V1.0

**版本：** V1.0（已签发生效）
**日期：** 2026-11-19
**裁决来源：** 人工裁决（同日）。影响审查报告 §六 D-1~D-5 全部裁决生效，本记录按裁决签发 DR-ORG-01~09。
**基线：** [P4-ORG-组织关系与权限模型基线-v0.1](P4-ORG-组织关系与权限模型基线-v0.1.md)（附录 A = 裁决记录）
**执行计划：** [P4-ORG-改造计划-v0.2](P4-ORG-改造计划-v0.2.md)
**冲突依据：** [P4-组织权限模型基线V0.1-影响审查报告-v0.1](../../audit/P4-组织权限模型基线V0.1-影响审查报告-v0.1.md)

**纪律声明：** 本记录不改动任何被修订条款的历史原文；所有被修订对象在「修订关系」栏登记，原文保留于原文件，**效力以本记录为准**（基线 §12 修订纪律）。架构基线冻结（2026-11-17）后的本轮修订经人工裁决生效。

---

## 一、裁决记录（D-1~D-5 → DR 落点）

| # | 裁决 | 落点 |
|---|---|---|
| D-1 | Tenant Admin 仅负责 Tenant 级管理，不拥有 Project 侧兜底权限；Tenant Owner/Admin 不自动穿透获得 Project 管理权限 | DR-ORG-02④、DR-ORG-04③、DR-ORG-06② |
| D-2 | Agent 职能（ai_pm/architect/developer/tester）迁出 project_member.role，建独立 Agent Registry / Capability 模型 | DR-ORG-05③ |
| D-3 | P4-09 DR-01/02/03 撤回，按新模型重写；不保留两套矛盾授权规则 | DR-ORG-03、DR-ORG-04；P4-09 两文件已加撤回公告 |
| D-4 | 人类 Member 直接修改正式需求和项目知识，不经审批；K4 等审阅链仅约束 AI 生成/回写内容 | DR-ORG-04④ |
| D-5 | 基线两文归档入库为引用基线 | 本目录三文件 |

---

## 二、修订决策记录

### DR-ORG-01 Team 不可解散

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R1（直接冲突）。
- **修订关系（原文保留）：**
  - P4-01 DR-01 Consequences（dissolve_team 清扫、`dissolved` 系统终态）→ 效力废止；
  - P4-02 DR-06 / 设计 §9（team_dissolved 统一归因清扫）→ 效力废止；
  - P4-03 §7.9 Team dissolved 行 → 删除；
  - docs/product/00-index.md L44 产品裁决（Owner 独占解散）→ 被本 DR 取代；
  - team-onboarding §3.1（Team status DELETED + deleted_at 软删）→ 删除该终态。
- **决定：** 任何角色（含 Tenant Admin、Team Owner）均无权解散 Team；不提供解散 API 与入口；后端拒绝。Team 生命周期与 Tenant 一致，无独立解散终态。
- **实现要点：** 删除 `POST /v1/pm/tenants/{tenant_id}/dissolve` 与 `dissolve_team`（apps/pm-api/src/team_onboarding.rs:59-63、crates/pm-application/src/team_onboarding.rs:1232）及其下游清扫链（项目软删、project_member 清除、邀请/链接清除、tenant_member 清除、租户休眠+slug 释放、`team.dissolved` 审计）。
- **后果：** 组织单元不可被事件链批量终结；Product/Project 不存在因解散产生的终态；P4-02 DR-06「清扫扩展」登记序列随触发源消失而关闭。

### DR-ORG-02 Team 角色收敛与多 Owner（含 D-1）

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R2（部分冲突）；裁决 D-1。
- **修订关系：**
  - menu-roles v1 §3.2（Team 三角色 Owner/Admin/Member、单 Owner 转让制）→ 修订为 Owner/Member 双角色、多 Owner 并存、任命/降级制；
  - team-onboarding §3.2（最后一名 Owner 保护）→ 保护原则保留，扩大到任命/降级/移除/退出全部路径；
  - `TenantRole{Owner,Admin,Member}`（crates/pm-domain/src/tenant.rs:19）→ 语义收敛（见 ④）。
- **决定：**
  1. Team 角色仅 Team Owner / Team Member；一个 Team 可多 Owner 并存。
  2. 创建 Team 者自动成为首位 Team Owner；Owner 可任命 Member 为 Owner、将其他 Owner 降级为 Member、邀请/移除成员、管理 Team 全部 Products；Owner 可退出但必须剩余至少一位 Owner。
  3. 最后一位 Owner 不可退出/降级/移除（现状仅项目侧有 OWNER_LAST 校验，project_member.rs:358-363；Team 级补齐）。
  4. **D-1：Tenant Admin 仅负责 Tenant 级管理**（租户设置、租户级名册与审计等），不因 Tenant 身份自动获得任何 Project 的 Owner/Admin/写入权限；Tenant Owner/Admin 不自动穿透 Project。删除 `is_project_manager` 中租户 owner/admin 兜底（project_member.rs:181-199）。
- **实现要点：** 新建 `core.team`（归属 tenant）与 `core.team_member(role ∈ {owner, member})`；invite→accept 加入链保持（用户不能自行加入）；tenant_member 不再充当 Team 成员表（语义收敛为租户级管理记录，细节随 Phase 1 定稿）。

### DR-ORG-03 Product 归属 Team、无独立成员名单

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R3（部分冲突）；裁决 D-3。
- **修订关系：**
  - P4-01 DR-06 ③④⑤⑦（Product Owner 单列人工移交、详情仅 Owner∪admin 其余 403、Owner 离开 Team 移除检查的人工移交语义）→ 由 Team 推导访问取代；
  - P4-09 DR-01（引入 Product 成员资格 S1~S3）→ **撤回（D-3）**；
  - P4-01 DR-01（归属 Team）→ 保留一致。
- **决定：**
  1. Product 归属唯一 Team（`team_id` NOT NULL）；不建立 Product 成员表，不建立任何第二套 Product 授权体系。
  2. Team Owner 自动拥有该 Team 全部 Products 的管理权限；Team Member 自动获得该 Team 全部 Products 的访问权（基本信息与业务内容读取）。
  3. 其他 Team 成员不自动获得访问权；无跨 Team 自动授权。
- **边界：** Product Cognition 等特殊知识内容的发布与读取规则仍遵循其领域自身状态与授权约束（基线改造计划 Phase 3 注意条款）；Team Member 的自动访问资格不自动扩大到未发布内容。

### DR-ORG-04 Product↔Project 多对多、正向传播、统一权限服务（含 D-1/D-4）

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R4/R5/R12/R13；裁决 D-1、D-3、D-4。
- **修订关系：**
  - P4-01 DR-06 ⑥ / 补充 B、P4-02 §7.2「双向隔离零穿透」→ 修订为「正向传播 + 反向禁止」；
  - P4-02 DR-01（N:M）→ 保留一致；
  - P4-02 DR-03 交集公式 `(PO∪TA)∧(ProjOwner∪TA)` → 重写：TA 项删除（D-1），ProjOwner 项改为 DR-ORG-05 推导集合；
  - P4-09 DR-02（来源撤销）/DR-03（单层传播）→ **撤回（D-3）**，其中「多来源并集、来源级撤销、实时重查、即时生效」机制思想并入本 DR；
  - P4-09 DR-04（显式可见性例外框架）→ 保留，作为自动全访问的例外机制，具体策略仍 OPEN，需补 Closed 态交互。
- **决定：**
  1. `product_project_link` 多对多；同一 Project 可被多 Team 管理的 Products 关联；关联各 Team 成员均获该 Project 访问资格，无需跨 Team 授权。
  2. 传播唯一路径：User → Team Membership → Team Product → Link → Project 访问资格。**禁止 Project → Product 反向推导**；Project-only 访问（来源二）为独立来源，不外溢为 Product 权限，其授权入口暂不纳入第一轮实现（基线 §13-4 后置裁决）。
  3. **统一权限推导服务先于业务模块改造**，推导链：Tenant → Team Membership → Team Owner/Member → Team Products → Product-Project Links → Project Access Qualification → Project Role → Operation Authorization。全部 API、MCP、Agent 工具、异步任务、内部服务统一调用，禁止各业务模块重复实现关系判断。
  4. **三事分立，不得混成单一 `is_project_member` 判断**（现状 authz.rs:33 即混用，需拆）：访问资格（能否进入 Project）/ 操作权限（进入后能做什么）/ 对象状态限制（Project 当前 Active/Closed 是否允许写入）。
  5. 多访问来源取有效来源并集；任一来源失效只撤销该来源；撤销即时生效、实时重查不读旧缓存；权限来源可解释可追溯（审计/排障链）。
  6. 关联变化自动生效：Team 成员进出、Owner 任命/降级、Link 建立或解除，均实时反映到有效权限，不靠人工同步多份名单。
  7. **D-4：人类 Member 直接修改正式需求和项目知识，不经审批**。K4 人工审阅链（KB-03 kb.memory 修订链）与 PC 审批闸的约束对象 = **AI 生成/AI 回写内容**；P4-03 §7.8/H8 与 §2.3 铁律 2 的措辞按此限定（在途 V1.1 修订随重定向落实，本批不改其文件）。

### DR-ORG-05 Project 三角色与 Owner 自动推导（含 D-2）

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R6/R7/R8；裁决 D-2。
- **修订关系：**
  - 0030 迁移六值 CHECK（project_member role：owner|member|ai_pm|architect|developer|tester，0001_baseline.sql:1292）→ 直改三值（AGENTS.md 规则 1，无兼容）；
  - `PROJECT_MEMBER_ROLES` / `PROJECT_MEMBER_AGENT_ROLES`（crates/pm-domain/src/project_member.rs:47-57）→ 重定义；
  - s01-02 §3.3（建项目 Owner 默认当前用户、人工可改）→ 废止；
  - menu-roles L705（Project Owner 必须是有效真人项目成员——单列任命制）→ 修订为推导集合（真人 Team Owner）；
  - R-LLM-07/Q-2（「Project Admin 角色不存在」悬置裁决项）→ 关闭：Project Admin 角色现在存在，由 Project Owner 任命。
- **决定：**
  1. 人类 Project Role 仅 member / admin / owner：
     - **Member**：综合项目参与者；录入/修改正式需求（免审批，D-4）、修改项目知识、协助 AI 分析需求、响应 Agent 疑问、解决执行阻塞、参与讨论、验证交付、提交并确认 Issue；
     - **Admin**：Member 全部权限 + 项目管理、管理性裁定、项目决策；由 Project Owner 任命/撤销；
     - **Owner**：Admin 全部权限 + 关闭/重新开启 Project（唯一差异）。
  2. Project Owner 不人工任命、不转移、无继任机制：**Owner = 关联该 Project 的全部 Product 所属 Team 的全部 Team Owner**，实时推导、多来源并集、Team Owner 身份变化自动同步；用户仍经任一有效关联 Team 保持 Owner 身份即保留权限。删除 `transfer_owner`（project_member.rs:392）与 bootstrap 建项目写调用者为 owner 行（bootstrap.rs:119-135）；不需要 Project 创建者成为 Owner。
  3. **D-2：Agent 职能与人类角色彻底分离**：
     - project_member.role 仅承载人类三角色；ai_pm / architect / developer / tester 迁出该列，落**独立 Agent Registry / Capability 模型**（现 bootstrap.rs:440-460 role_kind 能力注册表方向）；
     - Agent 职能仅用于任务分配、能力匹配、执行策略；不得成为人类用户的 Project Role，不得因此获得人类 Member 的验收权限；
     - bootstrap 播种的三个 dev_agent 行（bootstrap.rs:386，architect/developer/tester）迁移至新模型。
- **实现要点：** Phase 4 落地；`require_project_member`（authz.rs:33，无角色区分）按 DR-ORG-04 三事分立重构。

### DR-ORG-06 Project 生命周期 Active/Closed（含 D-1 收口）

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R9（直接冲突）；裁决 D-1。
- **修订关系：**
  - P4-01 §6.3（lifecycle = planning/initializing/archived/deleted，无 CHECK）→ 重定义为 {active, closed} + CHECK；
  - P4-02 DR-05 F-07（建链允许集 {planning, initializing}）→ 改为 {active}；
  - P4-02 DR-06（deleted 仅发生于 dissolve）→ 随 DR-ORG-01 废止；
  - P4-02 §7.1/§10.1（关闭权限 = owner∪TA、状态守卫矩阵）→ 守卫改为仅 Project Owner（推导集合），TA 项删除（D-1）；
  - P4-03 §7.9 Project archived 行（消费授权保留只读）→ 改为 closed 语义。
- **决定：**
  1. status ∈ {active, closed}；Active 正常读写；Closed 全局只读。
  2. 关闭与重新开启**仅 Project Owner**（DR-ORG-05 推导集合）；Admin 不可。删除 `is_project_manager` 归档守卫（project_lifecycle.rs:52），新建 reopen API（现状无）。
  3. Closed：所有人类用户只读；AI Agent 不修改项目数据、不继续 Coding 写入；AI 可读取/检索/引用历史知识、回答历史问题。写入拦截复用 `project_gate` 统一门禁（已覆盖 KB 写、Run 执行、claim、Outbox、MCP 间接路径，project_gate.rs:46/64/88），改状态语义与守卫角色，并对全部写入口（API、Agent 工具、MCP、异步任务、内部服务）做覆盖审计。
  4. 在飞 Agent Run 的终止/处理策略按基线 §13-8 后置裁决，但**任何策略不得绕过 Closed 写入限制**（fail-closed）。
  5. 重开恢复 Active、原有 Project Role 与写入；关闭/重开全程审计。
- **实现要点：** archive.rs 收口逻辑迁为 close；补并发与异步写入测试；前端仅作入口展示，后端为唯一强制点。

### DR-ORG-07 Issue 验收权

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R10（文档未涉及，需成文）。
- **修订关系：** H8（P4-03 §7.8，AI 确认恒 403）先例延伸成文至 Issue/验收域；s04-ui-07（人工 Mark Resolved + waiver 留痕）与本 DR 对齐；docs/ 无独立 Issue 域生命周期成文，本 DR 补齐。
- **决定：**
  1. Member 提交 Issue；AI Agent 处理并提交解决结果（止步 Submitted）；最终 Resolved / Reopened 由人类 Member 验证确认；Admin/Owner 依管理权限处理争议裁定。
  2. 现有 13 态任务状态机与唯一受守卫迁移入口（task_transition.rs 合法边表、command_executor/transition.rs:36 行锁+revision+审计）保留。
  3. **守卫收紧**：验收决策入口现为 `require_project_visible`（apps/pm-api/src/pm_reviews.rs:79-100）——任何可见 principal 可验收；改为「人类 principal + member 及以上角色」，显式排除 Agent principal（Agent 行当前同为 project_member 行，可见性检查不排除它们；实现期核实 decide 服务内部是否已有人类校验，没有则补）。
- **后果：** Agent 不得代替 Member 验收；人工审批闸（Reviewing→Completed 需验收证据，review/decide.rs:506）语义不变。

### DR-ORG-08 人工处理中心统一入口

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R11（部分冲突）。
- **修订关系：** N-11（统一 Human Review 模块）→ 本 DR 落地；kickoff ROLE_EXECUTOR_MAP 按职能定向派发（L43-50）→ 废止。
- **决定：**
  1. Agent 遇需求不明确、需人类决策、执行阻塞、技术问题、缺项目背景、需验证交付时，统一提交人工处理中心；**不采用定向指派 Developer 机制**。
  2. 现有广播式求助链为机制底座：`governance.human_intervention` + task→awaiting_decision + human_wait 调度盾 + 仅 human principal 可答复（interaction.rs:102/969-989）+ 超时清扫/重试——聚合为统一入口模块（与 ReviewCenter/审阅中心、approval 合并形态在实施设计定）。
  3. 下线 legacy 职能定向派发映射 ROLE_EXECUTOR_MAP（ai_roles.rs:367-373）；任务承接经 DR-ORG-05 Agent Registry 能力解析。
  4. Member 按权限与综合职责处理事项；Agent 按人类处理结果继续执行；不为不同人类执行岗位建立复杂角色体系。
- **关联：** P3-D04-01 ④（discovery_writeback 产品断裂，caae372 require_human 闸）的在途产品裁决与本 DR 的统一入口设计衔接，不在本 DR 内替其裁决。

### DR-ORG-09 无迁移纪律清理（R14）

**Status: ACCEPTED（2026-11-19 人工裁决签发）**

- **冲突依据：** 审查报告 §二 R14（部分冲突，新发现——此前任何 audit 未登记）。
- **决定：** 本项目无存量数据（AGENTS.md 规则 1）：
  1. Phase4-Product资产归属演进规划基线中「兼容现有数据的演进和迁移方案」「分阶段迁移及回滚」「迁移策略完成」等条款（L44/289/292/344/413）→ 删除，改为「直改契约 / 重建表」。
  2. P4-ORG-改造计划-v0.2 已删除全部迁移条目（迁移脚本、旧角色数据迁移、迁移验证报告、历史重复成员处理）；本轮改造全程无迁移/双写/兼容分支/过渡层。
  3. kb-legacy-import-pilot-plan 重新定位：legacy 导入如保留，属外部数据接入功能，不构成本轮迁移义务；去留另行裁决，不阻塞 Phase 0-7。
- **后果：** 验收目标 = 数据库、权限服务、API、Agent、UI 统一新模型；旧权限模型不保留兼容路径。

---

## 三、签发后待办

1. P4-03 V1.1 在途修订按 DR-ORG-04④/05/06 重定向后随定向复审一并签发（在途文件本批不动）。
2. P4-09 DR-04~11（敏感区）授权主体称谓改写（Product Owner / Project Owner → Team Owner / 推导 Owner），随甲部分重写一并处理；DR-09 O9、DR-11 需补 Closed 态交互定义。
3. Phase 1 实施按 P4-ORG-改造计划-v0.2 §四启动。
