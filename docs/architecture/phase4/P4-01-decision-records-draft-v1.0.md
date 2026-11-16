# P4-01 Decision Records 草案 V1.0

> **状态纪律：未裁决记录为 PROPOSED；已裁决以对应 DR 的决议记录为准（含决定日期）。Recommendation 不因本文档存在而生效。**
> **上游：** [P4-01 设计文档](P4-01-product-domain-model-design-v1.0.md) · [P4-00 审计](../../audit/phase4-2026-11-16/P4-00_Current_Architecture_Ownership_Audit_V1.0.md)
> **证据基准：** master @ `b0d8973a`

---

## DR-01 — Product 的领域归属

**Status: ACCEPTED（2026-11-17，人工裁决）**

- **Context**：系统需要为 Product 建立归属主体。现行模型中 Team 无独立表、由 `core.tenant` 承载（provider+external_key 唯一，0001:1300）；Project 归属 tenant（0001:1251）；租户角色 owner/admin/member（0001:1399）。P4-00 审计确认「Team 语义映射 = core.tenant」，且任务书明令不得把该映射当作终局领域定义。
- **Problem**：Product 的归属主体在领域语义与物理承载两层分别是什么；Team 与 Tenant 的当前 1:1 映射是否要固化为架构约束。
- **Options**：
  - A. **归属 Team，物理承载 tenant_id**（与 customer/project 同构）。
  - B. 归属独立组织/客户主体（product 挂 core.customer 或新 org 实体）。
  - C. 独立产品组织域（product-org schema，不依赖 tenant）。
  - D. 平台级全局产品（无租户归属）。
- **Trade-offs**：
  - A：零新机制（隔离/权限/审计全复用）；代价是把 Team:Tenant 映射固化为物理耦合——接缝收敛为「归属列 + 权限判定入口」两处，未来解耦迁移面可控。
  - B：customer 是业务关系实体（code 可变 customers.rs:252、无所有权语义、parent 表集团层级），把长期主体锚挂在其上语义错位且引入客户生命周期对产品生命的传染；且无客户级所有权诉求证据。
  - C：域纯净，但需要新建隔离/权限/审计全套机制，且 AGENTS.md 规则 1 禁止无依据过渡层。
  - D：无跨租户共享场景；破坏全线租户隔离惯例。
- **Recommendation**：**A**。领域语义：「Product 属于 Team」；物理承载：`tenant_id`；强制规约：设计中凡写「Team」处均为领域语言，凡写 `tenant_id` 处均为隔离承载，禁止把二者混写为一个概念。成立条件：①产品不跨 Team 共享；②无客户级所有权诉求；③Phase 4 各切片均在 Team 语境。
- **Consequences（2026-11-17 修订，随裁决生效）**：
  1. 领域规则：Product 归属 Team；**Team 解散（领域事件）终结其名下 Product 内容**（与 project 同批进入解散清扫集）。这是关于 Team 的领域规则，不是「Tenant 生命周期驱动 Product」。
  2. 载体复活隔离：休眠 tenant 可被复用为新 Team（team_onboarding.rs:395 翻回 is_active + 档案重写）；旧 Team 的 Product 行仅作审计锚，**不得复活进新团队目录**。终态语义已裁决（2026-11-17 评审补充）：清扫置 `lifecycle_status='dissolved'`（系统终态，**不可恢复**，restore 守卫按状态拒绝；与业务归档 archived 的可恢复语义严格区分）——普通 restore 接口不可能复活已解散 Team 的 Product。
  3. V1 不变量：**Product 不能活得比它的 Team 更久；不存在跨 Team 产品转移**（出现场景 → 新开 DR）。
  4. 未来 Team:Tenant 解耦时唯一迁移点 = 归属列语义 + 权限判定入口（接缝收敛不变）。
  5. **实施集成点（登记，非本设计授权）**：`dissolve_team` 清扫语句集未来须纳入 `core.product` 处置。
- **Dependencies**：无（根裁决）；被 DR-02/06 依赖。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**——接受「归属 Team + tenant 物理承载」并采纳修订后 Consequences。评审质疑「Team=Tenant 生命周期耦合」经证据核验不成立：`dissolve_team` 已分离二者生命周期（内容终结 team_onboarding.rs:1219-1290 vs 载体休眠复用 :395）。
- **评审补充决议（2026-11-17）**：解散终态采用第三状态 `dissolved`（系统终态，区别于业务归档 archived），杜绝 restore 复活路径；该状态仅由 `dissolve_team` 清扫写入，非任何业务 API 可达；载体复活成新 Team 后 dissolved 行不进新目录（列表排除）。
- **Status: ACCEPTED**

---

## DR-02 — Product Schema 落点

**Status: ACCEPTED（2026-11-17，人工裁决）**

- **Context**：候选落点三向：core 域新表 / 独立 product schema / 其他。仓库存在两套建表惯例：0001 基线「重 FK + 组合键」与 0010/0011/0025 新域「零 PK/零 FK + 应用层校验」（P4-00 GAP-04 定级 P2：0010/0011/0025 全部表零 PRIMARY KEY，实体唯一性无库级保证，T-2 未兑现）。
- **Problem**：Product 作为**治理主体**（未来一切归属锚点）应落在哪套惯例下。
- **Options**：
  - A. `core.product`：UUID PK + `UNIQUE(tenant_id,id)` 组合键（0018:10 明文惯例）+ FK tenant/principal + CHECK 生命周期。
  - B. 独立 `product` schema：仿 projc（零 PK/FK，触发器部分守卫）。
  - C. customer 子类型 / PC 升格 / 平台全局（见 DR-01 D 项与设计文档 §11）。
- **Trade-offs**：
  - A：库级完整性最高；P4-02 关联表可直接组合 FK；权限/审计/路由零新建；实施面最小（1 表）。缺点：core 域承接新产品概念（但 core 本就是租户与主体层，语义成立）。
  - B：域纯净；代价是把已被定级为 P2 债务的「零 PK」模式复制给最重要的主体表；审计需新增写入点；跨域事务无保障。新域惯例的动机是「复用认知域机制、不复用业务表」（0025:9-12），Product 非认知域，语境不符。
  - C：分别违反语义（customer=关系实体）、任务书禁令（§4.2 PC 升格）、隔离惯例（全局）。
- **Recommendation**：**A（core.product）**。
- **Consequences**：avoid GAP-04 模式扩散；实施时迁移号全分支扫号（与 P3-11 的 0026 协调，C-9）。
- **Dependencies**：DR-01；影响 P4-02（FK 目标形态）。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**——采纳 core.product（决定性依据：0018:10 组合键官方惯例；GAP-04 零 PK 债务不向主权表扩散；审计/权限/路由零新建）。
- **Status: ACCEPTED**

---

## DR-03 — Product 身份与唯一性

**Status: ACCEPTED（2026-11-17，人工裁决）**

- **Context**：仓库业务键三种先例：调用方提供且不可变（requirement.code，UNIQUE(project,code)，全仓零 SET code）、系统生成且不可变（design/plan `{prefix}-NNN`，next_code pm_write/support.rs:134-150）、可变（customer.code 可 PATCH——反例）。名称唯一性先例：team_llm_provider UNIQUE(tenant,name)、device 部分唯一；customer/project name 均允许重名。
- **Problem**：Product ID 与 product_key 的唯一范围、可变性；名称策略；是否需要别名/版本；与 PC 对象身份的关系。
- **Options**：
  - Key 来源：(a) 调用方提供（格式 CHECK `^[a-z][a-z0-9._-]{1,62}$`）(b) 系统生成 `PRD-NNN` (c) 双轨（可传可不传）。
  - Key 唯一范围：(a) 租户内 (b) 全局。
  - Key 可变性：(a) 不可变 (b) 可变（customer 式）。
  - 名称：(a) 允许重名 (b) 租户内唯一。
- **Trade-offs**：
  - Key 是未来 PC 引用（P4-03）、资产关联（P4-04）、深链（P4-07）的稳定锚——可变键将使所有历史引用悬空（doc_key/requirement.code 不可变正是为此）；调用方提供保留业务命名权，系统生成省一次冲突处理但牺牲语义键。
  - 租户内唯一足够：跨租户同名产品是常态，全局唯一无业务收益。
  - 名称重名允许（customer/project 惯例）；唯一名会把命名成本转嫁给用户且无检索收益（列表展示 id+name）。
- **Recommendation**：ID=uuid 全局唯一 + `UNIQUE(tenant_id,id)`；**product_key 调用方提供、租户内唯一、格式 CHECK、创建后不可变**；name 必填允许重名；description 可空；**不设**别名表/历史名表/主体级版本机制（无场景，AGENTS.md 规则 1）；**产品身份与 PC 认知对象身份严格分离**（core.product.id 是主体锚点，pc.cognition_object.id 是认知对象标识，P4-03 决定引用方式而非身份合并）。
- **Consequences**：改产品名≠改键；对外引用一律用 id 或 key（皆稳定）。
- **Dependencies**：DR-02；被 P4-03/P4-04/P4-07 依赖。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**——product_key 调用方提供（性质=组织级命名主体如租户 slug，非内部工件号；创建负担由前端预填/实时校验消解，P4-07 落实）；双轨方案因下游引用面（P4-03/04/07）永久双形态复杂化被否；customer.code 可变+可空为最弱身份先例，不效仿。
- **Status: ACCEPTED**

---

## DR-04 — Product 生命周期

**Status: ACCEPTED（2026-11-17，人工裁决）**

- **Context**：现行生命周期先例：project.status 无 CHECK（planning→initializing→archived/deleted，归档单向、无恢复端点）；requirement.lifecycle_status CHECK active/withdrawn/archived（restore 仅 withdrawn→active，requirement.rs:1006-1019）；design.adr 六态；审计不可硬删纪律（team_onboarding.rs:1222-1231「审计 FK 锚定+无级联→不可硬删」）。
- **Problem**：Product 最小状态集；是否 suspended/deleted/可恢复；archived 语义边界；与 Project/关联/PC 版本四个生命周期的分离。
- **Options**：
  - 状态集：(a) active/archived 两态 (b) draft/active/archived 三态 (c) 加 suspended。
  - 恢复：(a) archived 可 restore (b) archived 终态（对齐 requirement/project 惯例）。
  - 删除：(a) 无硬删 (b) 硬删。
- **Trade-offs**：
  - draft 需要定义晋升触发条件——现无「产品立项审批」场景，加了就是无触发器的死态（AGENTS.md 规则 1）；suspended 语义可被 archived+说明覆盖。
  - 可恢复 vs 终态：Product 是长期主体，误归档恢复成本低收益高；但与现行单向惯例不一致，评审可能偏好保守终态——两案并列，倾向可恢复。
  - 硬删被审计锚定纪律直接禁止。
  - archived 语义：可读、禁改、禁新增关联（服务层守卫）、既有关联不自动解除。
- **Recommendation**：**(a) active/archived + 可恢复 + 无硬删**；创建即 active；四生命周期（Product/Project/关联/PC 版本）四字段四表严格分离、禁止合并。
- **Consequences**：新增 restore 事件与守卫矩阵；archived 禁新关联语义成为 P4-02/04 的输入约束。
- **Dependencies**：DR-02（字段）/DR-06（谁可归档恢复）；影响 P4-02/04。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**——active/archived + 可恢复。复核确认两项语义：①restore=纯主体态翻转，不触发关联域任何自动化动作（既有关联静止保留原样可用，新增关联恢复开放）；②历史状态真相源=事件流（tenant_audit_log 单点，archived/updated/restored 带 before/after，归档时段守卫冻结=无事件=无变更，无审计盲区）。可恢复理由随 Phase 4 关联面出现而增强（误归档纠错成本=解除全部关联重建）；防振荡靠 Owner∪admin 门 + product.restored 事件可见。
- **评审补充决议（2026-11-17）**：
  - ③**无传播原则双向锁定**：Product 恢复**不**自动恢复已解除的 Project 关联；Project 归档**不**自动归档 Product。四个生命周期（Product / Project / Product–Project Association / PC Version）互不驱动，此约束对 P4-02/03/04 生效。
  - ④**解散终态**：`dissolved` 作为系统终态加入 lifecycle_status（由 DR-01 Team 解散规则引入），**不可恢复**——restore 守卫仅对业务归档 archived 生效；DR-04 的业务两态语义不变。
- **Status: ACCEPTED**

---

## DR-05 — Product 层级结构

**Status: ACCEPTED（2026-11-17，人工裁决）**

- **Context**：层级候选（Product/Product Line/Product Family/Sub-product/Variant）在仓库中零场景；core.customer 有现成自引用层级先例（parent_id 组合自引用 FK，0001:7786-7790）；AGENTS.md 规则 1 禁止投机结构。
- **Problem**：V1 是否支持层级、是否预留列。
- **Options**：(a) V1 扁平、不加列、登记扩展路径 (b) V1 扁平+预留可空 parent_product_id 列 (c) V1 有限层级（两级） (d) V1 完整产品族。
- **Trade-offs**：(b) 的「存在理由只有以后可能」，违反规则 1 判据；且预留列会诱发在无语义定义时提前使用（认知/资产/规划在层级下的继承语义全部未定义）。(c)(d) 无场景。(a) 的演进成本已被先例证明很低（照抄 customer.parent_id 即可）。
- **Recommendation**：**(a) V1 扁平，零预留**；扩展路径登记：未来加 `parent_product_id uuid NULL` + 自引用组合 FK + 「层级下的认知/资产归属语义」必须先过独立裁决。
- **Consequences**：列表即全量目录；未来引入层级为增量迁移。
- **Dependencies**：无；P4-04 资产关联设计不受影响（关联挂叶子或根均可后定）。
- **决议记录**：2026-11-17 人工裁决 **ACCEPTED**——V1 扁平零预留；扩展路径=customer.parent_id 同款自引用组合 FK（0001:7786-7790，仅登记）。
- **Status: ACCEPTED**

---

## DR-06 — Product 权限边界

**Status: ACCEPTED（2026-11-17，人工裁决，含补充 A/B）**

- **Context**：现行权限两套分离：租户角色 owner/admin/member（core.tenant_member，is_admin=Owner|Admin 每请求查库 pm-domain/tenant.rs:44-57）与项目角色（core.project_member）；治理闸 approver_grant（approver.rs:56-79）是项目级。租户级管理面先例=LLM Provider（guard_tenant + is_admin，team_llm.rs:219-224）；读面开放先例=/v1/pm/customers（无成员过滤，customers.rs:36-43）。owner 独占纪律：转让/解散（tenant.rs:49-51）。
- **Problem**：谁创建/改/归档/管成员/移交；Product 权限与 Project Member 的关系；可见性范围；是否需要成员表。
- **Options**：
  - 角色集：(a) Owner（单列 owner_principal_id）+ 租户角色兜底 (b) Owner+Maintainer+Member+Viewer 四级（product_member 表）。
  - 可见性：(a) 全租户成员只读 (b) 仅 owner/admin (c) owner/admin+显式 viewer。
  - 创建：(a) tenant admin (b) 任意租户成员。
- **Trade-offs**：
  - (b) 四级角色在无场景时是角色爆炸；member 表可后补（project_member 模式现成），单列起步最小且满足「单一问责」。
  - 可见性：产品目录是组织级信息（非项目敏感数据，与 GAP-08 的 docproc 争议性质不同），全租户只读对齐 customer 惯例且避免 P4-02 关联校验时被迫打开权限口；保守案 (c) 成本=新增 viewer 管理。
  - 创建门槛：产品是治理主体，admin 门槛对齐 LLM Provider 先例；任意成员创建会造成目录失控。
  - 双向穿透禁令：Product 权限不穿透 Project（成员概念都不引入）；Project 角色不穿透 Product（owner 判定只认 tenant 角色+owner 列）。唯一例外=tenant admin 兜底（先例一致）。
- **Recommendation（2026-11-17 按评审修订）**：**最小可见 + 详情受限**——①创建=tenant owner/admin；②变更/归档/恢复/移交=Product Owner ∪ tenant admin；owner 单列不可自我移除；③**可见性两层：列表（最小字段集 key/name/lifecycle/owner，不含 description）全租户成员；详情（全字段含 description 及未来聚合面）Owner∪admin，其余 403**；④无 member 表（V1，Viewer 授权可增量加、契约零变更）；⑤无任何自动穿透（Product↔Project 双向）。
  - 修订理由（评审采纳）：**不对称性——收紧接口破坏既有调用方，放宽纯增量**。Product 是 P4-03/04/05 敏感聚合枢纽（战略/商业信息），权限默认值必须落在将来想要的位置；原「全租户读」案撤回。
- **Consequences**：权限实现=2 个判定函数（is_admin / is_product_owner）+ 详情读守卫；P4-02 关联展示走列表最小字段集；P4-07 需为普通成员设计目录级页面。
- **Dependencies**：DR-01/02；影响 P4-02（关联管理授权）、P4-07（入口可见性）。

### 决议记录 — 2026-11-17

**人工裁决：ACCEPTED。** 接受修订后的「最小可见 + 详情受限」方案：

1. Product 创建权限仅限 Tenant Owner/Admin。
2. Product 修改、归档、恢复和 Owner 移交权限属于 Product Owner ∪ Tenant Admin。
3. Product 列表对全体 Team 成员开放，但只返回最小字段集：product_key、name、lifecycle_status、Owner 展示信息。
4. Product 详情仅允许 Product Owner ∪ Tenant Admin 读取；普通 Team 成员访问详情接口时拒绝。
5. V1 不建立 product_member，不引入 Viewer 或其他 Product 级角色。
6. Product 权限与 Project 权限双向隔离，不自动穿透。
7. Product Owner 必须是当前 Team 的有效成员；Owner 离开 Team 前必须完成责任移交。
8. Product–Project Association 属于独立权限边界，其操作授权由 P4-02 裁决，必须同时满足 Product 侧与 Project 侧权限要求，不得由任一侧权限自动穿透。

**边界声明**：本裁决只确认 Product 自身的访问和治理权限，不预先裁决 Product–Project Association 的具体操作角色、授权 API 或审批流程。

### 补充 A — Owner 有效性（裁决第 7 条展开）

- `owner_principal_id` 必须属于 Product 所属 Team 的有效成员（创建时校验）。
- Owner 离开 Team 时，不允许留下无法履责的孤立 Owner 状态。
- Owner 转移必须由现任 Product Owner 或 Tenant Admin 执行。
- 不允许 Owner 通过修改自身成员状态绕过移交约束。
- **实施集成点（登记，非本设计授权）**：租户成员移除流程须增加 Product Owner 持有检查（有在管产品则要求先移交或拒绝移除）。

### 补充 B — Product–Project Association 权限边界（裁决第 8 条展开）

- 不能理解为「有 Product 权限即可操作关联的 Project」，也不能理解为「有 Project 权限即可修改 Product 关联」。
- P4-02 锁定原则：**两侧权限取交集**——Product 一侧满足 Product 关联管理权限、Project 一侧满足 Project 关联管理权限，两者齐备方可建立/解除；任一侧权限不穿透另一侧。
- Association 自身的生命周期与审计独立于 Product、Project（与 DR-04 无传播原则一致）。
- 具体角色、是否额外人工确认、API 形态 → P4-02 正式裁决；本条只锁边界。

**Status: ACCEPTED**

---

## DR-07 —（骨架）P4-03 Cognition Ownership Boundary

**Status: PROPOSED-DRAFT（独立裁决，不在 P4-01 内完成）**

- **Context**：P4-00 证实 PC product_profile/vision 为**逐项目私有单例**（bootstrap.rs:325；部分唯一索引 (tenant,project,object_type) 0001:6854），即产品认知权威的作用域=项目；Phase 4 基线把「认知权威归属」列为 Gate 2 必答项。任务书 §4.2 明令 P4-01 不得改 PC 归属/确认/版本结构、不得预支 P4-00 混合推荐为已确认。
- **Problem（登记）**：产品认知的正式权威是否从 Project 级上移至 Product 级；一 Product 对多 Project 认知的关联形态；产品认知与项目技术认知的冲突处理；实施发现→产品认知的反馈通道。
- **Options（占位，待 P4-03 切片展开）**：上移 / 维持项目私有+聚合引用 / 双层混合（P4-00 曾推荐，未确认）。
- **Constraints（已锁）**：C-3/C-4（PC 唯一权威、两域分立红线）；人工确认不可自动化；pc/projc 0025 在途实施（P3-11 W 批）时序约束。
- **Dependencies**：依赖 P4-01 产出=稳定主体锚点（core.product.id/product_key 不可变，DR-02/03）；依赖 P3-11 落地（projc 切换批不影响本裁决，但评审需吸收其终态）。
- **Consequences（若上移）**：触及 pc 已冻结模型 B01~B10 → 必须专项 Decision Record + 架构评审（P4-00 R-02 高风险）；逐项目单例骨架处置（数据近零，可直改契约）。
- **状态注记**：2026-11-17 评审确认本骨架定位——**继续留在 P4-03 独立裁决，不在 P4-01 内裁决认知权威归属**。
- **Status: PROPOSED-DRAFT — 定位已确认（2026-11-17），实体裁决待 P4-03 切片**

---

## 状态总览（P4-01 Freeze Candidate）

- DR-01 ~ DR-06：**ACCEPTED**（2026-11-17 人工逐项裁决；逐项状态与决议记录以各 DR 正文为**唯一真源**，本节不重复维护）
- DR-07：PROPOSED-DRAFT，定位已确认（留 P4-03 独立裁决）
- **冻结：V1.0 FROZEN（2026-11-17，架构评审 PASS，记录=P4-01-freeze-review-v1.0.md）；冻结不构成数据库迁移或代码实施授权**
