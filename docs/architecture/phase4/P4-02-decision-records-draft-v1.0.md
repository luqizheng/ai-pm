# P4-02 Decision Records 草案 V1.0

> **状态纪律：DR-01 ~ DR-06 = ACCEPTED（2026-11-17 分组人工裁决，决议记录见各 DR 正文，为其裁决状态唯一真源）。文件整体 = V1.0 FROZEN（2026-11-17，冻结评审 12/12 PASS，记录=P4-02-freeze-review-v1.0.md）；冻结不构成实施授权。Recommendation 不因文档存在而生效。**
> **冻结输入：** P4-01 V1.0 FROZEN（@ `002c825c`）——本文件任何修订不得与 P4-01 已 ACCEPTED 决议冲突；一致性修订（指令 F-01~F-10，2026-11-17）不将任何已 ACCEPTED DR 退回 PROPOSED。
> **上游：** [P4-02 设计文档](P4-02-product-project-relationship-design-v1.0.md)

---

## DR-01 — 基数与主次

**Status: ACCEPTED（2026-11-17，人工裁决·第一组）**

- **Context**：基线 P4-DP-03 禁止 Product/Project 一对一固定关系；P4-02 待研究项明列「一 Project 是否可服务多 Product」「主次关系是否需要区分」「独立 Project 是否允许」。
- **Problem**：关联基数（1:N vs N:M）、是否设 primary 标记、独立 Project 合法性。
- **Options**：(a) N:M 关联表 (b) project.product_id 单列 1:N (c) N:M + is_primary（部分唯一）。
- **Trade-offs**：(b) 放弃多产品项目场景且日后 N:M 需迁移；(c) 的 primary 在无展示场景时是无消费者的列（AGENTS.md 规则 1）。(a) 与 (c) 表结构相同，差异仅一列——先 (a) 后按需加列成本极低（部分唯一索引先例成排）。
- **Recommendation**：**(a) N:M + V1 无 is_primary + 独立 Project 合法（不强制选产品）**。扩展路径登记：出现「每项目主产品」展示需求时加 `is_primary bool` + 部分唯一索引 `(tenant_id,project_id) WHERE is_primary`。
- **Consequences**：项目页/产品页的关联展示需处理多值（P4-07 输入）；**评审补充**：P4-07 必须明确「一 Project 关联多 Product 展示为多值关系，前端不得自行推断主产品」。
- **Dependencies**：P4-01 F-1（锚点）；被 DR-02 表结构依赖。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**（第一组）——N:M、V1 无 is_primary、独立 Project 合法；P4-07 多值展示约束随裁决生效。
- **Status: ACCEPTED**

---

## DR-02 — 关联形态与历史（纯连接行 vs 独立聚合实体）

**Status: ACCEPTED（2026-11-17，人工裁决·第一组，含评审补充 A~E）**

- **Context**：仓库 10 张关联表全部为无状态纯连接行（物理删或禁删 + `X.linked/.unlinked` 审计）；唯一带状态的关系先例是 `integration.project_repository`——独立聚合实体（id + status CHECK + revision + 事件族）。P4-01 已裁决「Association 自身生命周期与审计独立」（补充 B）与「Product 恢复不恢复已解除关联」（无传播③）；P4-00 GAP-09 登记「5/6 关联表物理 DELETE，历史能力不齐」。
- **Problem**：关联解除的历史如何保留；关联生命周期用什么承载。
- **Options**：
  - (a) 纯连接行 `core.product_project`（无状态）：解除=物理 DELETE + 审计事件（requirement_source_input 先例）。
  - (b) **独立聚合实体 `core.product_project_link`**：id PK + `status(active|ended)` + ended_by/at/reason + 行永久保留；同对部分唯一 active；重建=新行。
- **Trade-offs**：
  - (a) 最贴合仓库多数先例、实现最薄；但「已解除」只存在于审计事件流中，行级历史=0（GAP-09 同款缺口），且「Association 自身生命周期」没有行级承载——与 P4-01 已裁决的独立性存在语义落差。
  - (b) 行级历史完整（ended 行即记录：何时/谁/为何解除）；「restore 不复活已解除关联」获得**构造性保证**（ended 行在，restore 无路径触碰）；解除-重建序列可查询（relink 次数=行数-1）。代价：表略重、需部分唯一索引 + ended 语义守卫——均为现成模式（partial unique：uq_pc_approval_pending / memory CURRENT；聚合实体：project_repository）。
- **Recommendation**：**(b) 独立聚合实体**。schema=core，重 FK（双侧组合 FK），不进零 PK 新域惯例（P4-01 DR-02 同论证）。ended 状态二元 + end_reason 承载原因（不设独立 dissolved 状态值）。
- **评审补充（2026-11-17 第一轮审查，采纳后随本 DR 裁决生效）**：
  - **A. 状态一致性约束（C-03）**：`ck_pp_link_state`——active ⇒ ended_by/ended_at/end_reason 全空；ended ⇒ 三者全非空；`ck_pp_link_reason` 枚举 CHECK。
  - **B. 重建身份语义**：每次重建=新 id、不复用历史 link_id；旧 ended 行的结束字段永不被覆盖（守卫禁改+无更新通道）；「当前关联」查询只返回 active，「历史」查询显式含 ended。
  - **C. 不可变与不可复活（C-04）**：`trg_pp_link_guard` 四禁——①禁改身份列（id/tenant_id/product_id/project_id/linked_by/linked_at）②禁 DELETE（历史行永久保留）③禁 ended→active ④**OLD.status='ended' 时拒绝任何 UPDATE（ended 行整体冻结：ended_by/ended_at/end_reason 及其余历史字段不可覆盖；历史纠错必须另设显式审计纠错机制，不得直接改写原始记录）**——④为一致性修订指令 F-03 补强。
  - **D. 系统清扫身份（C-05）**：ended_by 人工=caller（kind='human'）、清扫=**DISPATCH_SERVICE_PRINCIPAL**（core.principal.kind='service'，0001:1228；在役惯例 pc/publish.rs:303 等）——保持 NOT NULL 语义（条件 CHECK），不引入可空执行者歧义。
  - **E. C-01 前置**：core.project 现仅 UNIQUE(id,customer_id)（0001:4030-4034），**迁移须先补 UNIQUE(tenant_id,id)**，双侧组合 FK 方可成立。
- **Consequences**：审计与行级历史双轨（事件流 + ended 行）；解除 API 语义=POST end 而非 DELETE；迁移含 1 项 core.project 增量唯一约束（C-01）。
- **Dependencies**：DR-01；被 DR-05/06 依赖。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**（第一组，含评审补充 A~E）——独立聚合实体 + 状态一致性 CHECK + 重建身份语义 + 守卫（裁决时为三禁；F-03 一致性补强后为四禁，见补充 C）+ 清扫身份 DISPATCH_SERVICE_PRINCIPAL + C-01 迁移前置（core.project 补 UNIQUE(tenant_id,id)，证据 0001:4030-4034）。
- **Status: ACCEPTED**

---

## DR-03 — 权限交集具体化

**Status: ACCEPTED（2026-11-17，人工裁决·第二组，含评审补充 A~C）**

- **Context**：P4-01 DR-06 补充 B 冻结「两侧权限取交集、任一侧不穿透另一侧」，并明令具体角色/审批由 P4-02 裁决。两侧授权集合均有现役先例：Product 侧=Owner∪tenant admin（P4-01 冻结）；Project 侧=owner∪tenant admin（`is_project_manager` 守卫，project_lifecycle.rs:45-66）。
- **Problem**：「交集」的执行形态：单一执行者同时持双侧授权，还是双侧分别确认（两个 actor）。
- **Options**：
  - (a) **单一执行者双侧授权**：`(PO∪TA) ∧ (ProjOwner∪TA)`；tenant admin 天然满足双侧（两侧 break-glass 先例一致）。
  - (b) 双侧确认工作流：一侧发起、另一侧裁决（两个 actor）。
- **Trade-offs**：
  - (a) 零新机制；常见情形（同一人兼产品与项目负责人）直接可用；跨负责人时由 tenant admin 执行。弱点：互认治理缺失（双方未真正互相确认）。
  - (b) 治理更强，但需新建专表——`governance.approval_request` **实证不可复用**（1:1 硬绑 change_proposal + UNIQUE(proposal_id)；0010:21 注释明言按域建专表先例 design_approval/pc.approval）；为 V1 关联操作引入一套确认工作流，成本与场景不匹配。
- **Recommendation**：**(a) 单一执行者双侧授权**（V1）；(b) 登记为扩展路径——出现「产品负责人与项目负责人必须互认」治理场景时另立 DR（建专表，循 design_approval 先例）。
- **评审补充（2026-11-17 第一轮审查，采纳后随本 DR 裁决生效）**：
  - **A. 同 Team 域约束**：tenant admin 兜底=双侧资源共同所属同一 Team 的 admin（`caller.tenant_id`=链接租户）；不存在跨 Team admin 算入交集。
  - **B. 实时有效授权**：每次建/解链接重新判定——当前成员身份与 admin 角色（`caller.is_admin` 每请求查库惯例）、当前 Product Owner、当前 Project owner、product lifecycle、project status；不读缓存、不认历史创建者或历史 Owner。
  - **C. 无暗中降级**：跨负责人且无 admin=**有意阻断**（403），不以放宽单侧授权解决；双侧确认工作流保留扩展路径，V1 不引入。
- **Consequences**：守卫=两个独立判定函数在执行路径求交；执行者四身份列入审计，满足「谁授权」可追溯。
- **Dependencies**：P4-01 DR-06（不可弱化为单侧）；被 DR-05 守卫依赖。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**（第二组，含评审补充 A~C）——单一执行者双侧授权 + 同 Team 域约束 + 实时有效授权 + 无暗中降级（有意阻断 403）；双侧确认工作流=扩展路径。
- **Status: ACCEPTED**

---

## DR-04 — 创建链路规则

**Status: ACCEPTED（2026-11-17，人工裁决·第二组）**

- **Context**：项目创建唯一生产入口=`bootstrap_inner` 单事务（bootstrap.rs:181-347：customer 解析→project INSERT→project_member 5 行→AI 配置→PC 单例 bootstrap）；「建后补写非核心字段」有先例（description 在 commit 后另调，pm_lists.rs:168-174）；P4-01 R-04 提示创建时选产品将触及 bootstrap 事务。
- **Problem**：建项目是否强制/可选关联 Product；关联挂接点在哪。
- **Options**：(a) bootstrap 事务内加可选 product_id (b) create-then-link（创建后经关联端点链接，bootstrap 零改动）(c) 建项目强制选产品。
- **Trade-offs**：(c) 违反基线「独立 Project 合法」待研究项且强制依赖 Product 先于项目存在；(a) 改唯一写入口、扩大爆炸半径，且 bootstrap 失败回滚面变大；(b) 无孤儿风险（项目存在无链接=合法独立态）、零 bootstrap 改动、失败重试无害。
- **Recommendation**：**(b) create-then-link；不强制选产品**。P4-07 若要「创建时选产品」UX，形态=前端两调用串联，API 不做合并事务（登记为 P4-07 输入）。
- **Consequences**：bootstrap 保持冻结现状；关联建立永远是显式治理动作（审计天然清晰）。**评审补充**：项目创建成功但关联失败 = **合法的独立 Project 状态**，不是初始化失败、不触发项目回滚；前端若采用创建后自动链接，必须分别呈现项目创建结果与关联操作结果，不得包装成单一不可区分的成功状态（P4-07 输入）。
- **Dependencies**：DR-01（独立合法）；被 DR-05 端点设计依赖。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**（第二组）——create-then-link、不强制选产品、bootstrap 零改动；链接失败≠初始化失败、前端分别呈现（P4-07 输入）。
- **Status: ACCEPTED**

---

## DR-05 — API 与审计边界

**Status: ACCEPTED（2026-11-17，人工裁决·第三组，含读写分离修订）**

- **Context**：守卫三元组需语义化错误（对齐 ALREADY_ARCHIVED 惯例）；审计单点 `insert_tenant_audit`（coverage 清单制）；P4-01 两层可见性（列表最小字段/详情受限）。
- **Problem**：端点族形态、解除的 HTTP 语义、事件落哪本账、读可见性。
- **Options**：
  - 端点：嵌套 `/v1/pm/products/{id}/projects`（建+列）+ 扁平 `/v1/pm/product-project-links/{id}/end`（解除）+ 项目侧只读 `/v1/pm/projects/{id}/products`。
  - 解除语义：(a) DELETE 物理删 (b) POST /end 置终态。
  - 审计：(a) 仅 tenant_audit_log (b) tenant + project 双账镜像。
  - 读可见性：产品侧链接列表=Owner∪admin（详情聚合面）/ 项目侧=项目成员（最小字段集）。
- **Trade-offs**：DELETE 与 (b) DR-02 行保留设计冲突（历史空洞）；双账镜像 V1 增加写入点与不一致面，项目侧诉求登记为扩展；读分层与 P4-01 冻结的可见性同构（关联属产品详情聚合=受限层）。
- **Recommendation**：嵌套建+列、扁平 end（**不用 DELETE**）、tenant_audit_log 单点（事件 `product_project.linked/.ended`）。
- **评审补充（2026-11-17 第一轮审查，采纳后随本 DR 裁决生效）——读写权限分离（C-06）**：

  | 操作 | 权限 |
  |---|---|
  | 建立关联 | 双侧写权限交集（DR-03） |
  | 解除关联 | 双侧写权限交集（DR-03） |
  | 产品侧查询关联 | Product Owner ∪ Tenant Admin |
  | 项目侧查询关联 | 项目可见性（require_project_visible） |
  | 单条关联详情 | 按调用入口所属侧的读取权限判定（**非写交集**） |

  - **数据最小化**：产品侧查询不因「可读 Product」而无条件暴露 Project 全部详情——仅返回关联展示必需的项目最小字段（code/name/status）；反方向同理（项目成员见产品最小字段，不见 description/受限聚合面）。
  - 读权限分离的正当性：读取关联记录 ≠ 修改关联关系；若单条详情也要求写交集，合法的 Product Owner 将无法查看自己产品的关联记录，Project Owner 同理——明显不合理。
- **Consequences**：新增 2 个审计事件进 coverage 清单；项目侧页面的产品信息只有最小字段（P4-07 输入）；项目侧审计镜像=扩展路径。
- **Dependencies**：DR-02/03/04。
- **一致性修订记录（2026-11-17 指令 F-04/F-05/F-07，不退回裁决、不改决议记录）**：
  - **F-04 路由修正**：单条详情改为 `GET /v1/pm/product-project-links/{link_id}`（原 `products/{id}/projects/{project_id}` 无法唯一定位历史 Link）；读权限判定语义不变（所属侧读取权限，任一侧满足即可）。
  - **F-05 状态守卫分离**：建立（product active + project 允许集）与人工解除（仅要求 link active，主体状态不限）分别描述，互不套用；见设计 §10.1 矩阵。
  - **F-07 指令修订（⚠️ 触及本 DR 已裁决守卫形态，依指令执行，待冻结评审确认）**：建立关联的 Project 状态条件由 deny-list `∉{archived,deleted}` 改为**显式允许集 `∈{planning,initializing}`，未知状态默认拒绝**（取证：现役观测值集共 4 个）。两形态在现值集上外延等价，allow-list 更严格。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**（第三组，含读写分离修订）——端点族 + end 非 DELETE + tenant 审计单点 + 读写权限分离 + 数据最小化。
- **Status: ACCEPTED**

---

## DR-06 — 锚点终态联动

**Status: ACCEPTED（2026-11-17，人工裁决·第三组，含 team_dissolved 修订）**

- **Context**：P4-01 已锁 dissolved 仅由 dissolve_team 写入（DR-01 补充）且已登记「清扫纳入 core.product」为实施集成点；项目 deleted 仅发生于 dissolve（archive.rs 无 deleted 写入）；project archived 单向但非终态；无传播原则③约束的是两侧**主体**状态。
- **Problem**：锚点进入终态时，active 链接如何处置；这算不算违反无传播原则。
- **Options**：(a) 清扫同步置链接 ended（reason=锚点终态）(b) 链接保持 active，读侧容错悬空 (c) 触发器自动级联。
- **Trade-offs**：(b) 制造悬空 active（列表脏数据 + 语义谎言：已解散团队的产品"仍在建设中"）；(c) 触发器级联与仓库「清扫显式化」惯例相悖（dissolve 现状=显式 SQL 清扫集）且把治理语义藏进库层；(a) 与 dissolve 现有显式清扫模式一致，语义诚实。
- **Recommendation**：**(a)**——dissolve 清扫在同事务内将该 Team 全部 active 链接置 ended，`end_reason='team_dissolved'`。**边界澄清（本 DR 核心裁决点）**：这是关联**自身生命周期**对锚点终态的响应，不修改 Product/Project 任何状态，不构成无传播原则禁止的「自动传播」（该原则约束主体状态）；project archived **不动**链接（归档≠终态）。清扫扩展登记为**实施期登记项 #3**（非本阶段授权）。
- **评审补充（2026-11-17 第一轮审查，采纳后随本 DR 裁决生效）**：
  - **同一事务边界**：链接终态化与主体终态处置处于 dissolve 清扫**同一事务**，不产生半终态窗口。
  - **幂等**：同一关联不得重复执行结束操作（已 ended 行跳过）。
  - **end_reason 确定性（C-07）**：解散时 Product 与 Project 同事务内同时终态化，一条链接同时符合两侧原因——统一归因 **`team_dissolved`**（Team 级事件），**不依赖清扫 SQL 执行顺序**；`product_dissolved/project_dissolved` 两值取消（清扫只会成对发生）。此为 P4-02 链接层原因细化，**不改变 P4-01 的 dissolved 定义**。
  - **清扫审计身份**：ended_by=DISPATCH_SERVICE_PRINCIPAL（kind='service'），与人工解除（kind='human'）可区分（C-05）；每条链接一行 `product_project.ended` 事件。
  - **F-08 清扫集合明确化（一致性修订指令）**：清扫集合=该 Team 下**全部 active Link——无论 Product 或 Project 是否 archived**，只要 Link 仍 active 即入集合；与主体终态处置同一事务、幂等、已 ended 跳过。
- **Consequences**：dissolve 事务范围 +1 表；ended 行天然成为「该产品在该项目的历史」终局记录。
- **Dependencies**：P4-01 DR-01（dissolved）/F-4（无传播语义边界）；被实施期登记承接。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**（第三组，含 team_dissolved 修订）——清扫置 ended：team_dissolved 统一归因 + 同事务 + 幂等 + 清扫身份可区分；archived 不动；清扫=关联自身生命周期非传播（不改 P4-01 dissolved 定义）。
- **Status: ACCEPTED**

---

## 裁决清单（按评审指定分组顺序）

**第一组：关系模型基础**
| DR | 一句话决议请求 |
|---|---|
| DR-01 | N:M + 无 primary + 独立 Project 合法（P4-07 多值展示约束已记） |
| DR-02 | 关联=独立聚合实体（含评审补充 A~E：状态 CHECK/重建身份/三禁守卫/清扫身份/C-01 前置） |

**第二组：治理与创建**
| DR | 一句话决议请求 |
|---|---|
| DR-03 | 交集=单一执行者双侧授权（含同 Team 域/实时验证/无降级补充）；双侧确认=扩展 |
| DR-04 | create-then-link + 链接失败≠初始化失败、前端分别呈现 |

**第三组：接口与终态**
| DR | 一句话决议请求 |
|---|---|
| DR-05 | 端点族 + end 非 DELETE + tenant 审计单点 + **读写权限分离** + 数据最小化 |
| DR-06 | 锚点终态清扫置 ended（team_dissolved/同事务/幂等/清扫身份）；archived 不动；无传播边界澄清 |

> 冻结：**V1.0 FROZEN（2026-11-17，最终架构冻结评审 12/12 PASS，记录=P4-02-freeze-review-v1.0.md）；冻结不构成代码/Migration/数据库/部署实施授权。**
