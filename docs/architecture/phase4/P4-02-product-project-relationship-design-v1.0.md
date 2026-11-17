# P4-02 — Product–Project Relationship 设计 V1.0

> **阶段：** Phase 4 · **切片：** P4-02 · **性质：** 只读研究 + 方案比较 + 决策记录，**非实施任务**
> **状态：** **V1.0 FROZEN（2026-11-17，最终架构冻结评审 PASS，12/12 项核查通过）** — DR-01~06 ACCEPTED；**冻结 ≠ 代码/Migration/数据库/部署实施授权**（评审记录：[P4-02-freeze-review-v1.0.md](P4-02-freeze-review-v1.0.md)）
> **冻结输入：** [P4-01 冻结评审记录](P4-01-freeze-review-v1.0.md)（V1.0 FROZEN @ `002c825c`，DR-01~06 ACCEPTED）
> **上游：** [P4-00 审计](../../audit/phase4-2026-11-16/P4-00_Current_Architecture_Ownership_Audit_V1.0.md) · [Phase 4 基线](Phase4-Product资产归属演进规划基线-V1.0.md) §5 P4-02
> **写操作声明：** 仅创建本设计文档与 DR 草案。零代码 / 零 Migration / 零 bootstrap 改动。

---

## 1. Executive Summary

**推荐方案（已获裁决 ACCEPTED，2026-11-17 分组人工裁决）：Product–Project 采用 N:M 关联，关联本身是独立聚合实体 `core.product_project_link`（id PK + 双侧组合 FK + `status: active | ended`，ended 为终态、行永久保留）——关联生命周期自带状态，解除=置 ended 而非物理删除；权限=单一执行者同时持有双侧授权（Product Owner∪tenant admin ∧ Project owner∪tenant admin，同 Team 域+实时验证）；建项目不强制选产品，独立 Project 合法；创建后链接（不改 bootstrap 事务）；读写权限分离；锚点终态清扫置 ended（team_dissolved）。**

四个设计立场：

1. **关联 = 独立聚合实体，非纯连接行**：仓库 10 张关联表全是无状态连接行（物理删/禁删），唯一带状态的"关系"先例是 `integration.project_repository`（独立聚合：status CHECK + 事件族）。P4-01 已裁决「Association 自身生命周期与审计独立」→ 引入自身生命周期即需状态承载，聚合实体是仓库原生模式（DR-02）。
2. **ended = 终态 + 行保留**：解除不留物理空洞，重建=新行——同时满足「历史可追溯」（基线 P4-DP-05、P4-00 GAP-09 教训）与「restore 不复活已解除关联」（P4-01 无传播原则，构造性保证）。
3. **交集原则落地为「单一执行者双侧授权」**：tenant admin 双侧兜底（两侧均有先例）；双侧确认工作流登记为扩展路径（approval_request 不可复用有实证）。
4. **不触碰 PC 归属**：P4-03 独立裁决；本设计对 PC 零引用零假设。

---

## 2. Design Goals & Non-goals

**Goals**
- G1：定义 Product–Project 的基数、主次立场、关联实体与生命周期、解除与历史机制。
- G2：把 P4-01 锁定的「两侧权限交集」具体化为可执行的角色判定与 API 守卫。
- G3：定义锚点终态（product dissolved / project deleted）与 archived 的关联语义。
- G4：给出 API 边界与审计设计（不实现）。

**Non-goals**
- 不裁决 PC 认知归属、不假设「一 Product 多 Project 认知」的任何形态（P4-03）。
- 不设计 Product 与 KB/需求/设计等资产的关联（P4-04）。
- 不改 bootstrap 事务、不改项目创建/归档现有行为；不实现任何代码。
- 不引入产品目录/组合管理 UX（P4-07）。

---

## 3. Inherited Frozen Inputs（P4-01 锁定输入，本切片不可变约束）

| # | 锁定输入 | 来源 | 本设计遵守方式 |
|---|---|---|---|
| F-1 | `core.product` 组合键锚点（id PK + `UNIQUE(tenant_id,id)`，key 不可变） | P4-01 DR-02/03 | 关联表双侧组合 FK 直接引用 |
| F-2 | archived Product 禁止新增关联（既有关联静止保留） | P4-01 DR-04/§6.2 | 建链守卫：product lifecycle 必须 = 'active' |
| F-3 | 两侧权限取交集，任一侧不穿透另一侧 | P4-01 DR-06 补充 B | §7 具体化（DR-03） |
| F-4 | 无传播原则双向：Product 恢复不恢复已解除关联；Project 归档不归档 Product | P4-01 DR-04③ | §6/§9（ended 行构造性不可复活；project archived 不动链接） |
| F-5 | Association 生命周期与审计独立于两侧主体 | P4-01 DR-06 补充 B | 关联自带 status（§6） |
| F-6 | dissolved 仅由 dissolve_team 写入 | P4-01 DR-01 补充 | §9 锚点终态联动（清扫扩展，实施期登记） |
| F-7 | Team 解散终结其名下产品；载体复活隔离 | P4-01 DR-01 | dissolved 产品的链接随清扫终态化 |

---

## 4. Relationship Model（关联实体）

**领域定义：Product–Project Link 是一条有治理含义的长期关系记录——「该产品由该项目承建/演进」。它不是标签、不是外键快照，是独立于两侧主体的第三实体，拥有自己的生命周期与审计。**

**实体结构（已裁决 ACCEPTED，非迁移）：**

```
core.product_project_link
  id              uuid PK
  tenant_id       uuid NOT NULL FK → core.tenant(id)
  product_id      uuid NOT NULL   -- 组合 FK (tenant_id,product_id) → core.product(tenant_id,id)
  project_id      uuid NOT NULL   -- 组合 FK (tenant_id,project_id)  → core.project(tenant_id,id)
                  -- ⚠️ C-01 前置：core.project 现仅 UNIQUE(id,customer_id)（0001:4030-4034），
                  --    迁移须先 ADD CONSTRAINT UNIQUE(tenant_id,id)（增量安全：id 为 PK 恒满足）
  status          text NOT NULL DEFAULT 'active' CHECK (active | ended)
  linked_by       uuid NOT NULL FK → core.principal(id)   -- 建立后不可变
  linked_at       timestamptz NOT NULL DEFAULT now()      -- 建立后不可变
  ended_by        uuid NULL FK → core.principal(id)
                  -- 人工解除=caller（kind='human'）；系统清扫=DISPATCH_SERVICE_PRINCIPAL（kind='service'，0001:1228）
  ended_at        timestamptz NULL
  end_reason      text NULL       -- CHECK: manual | team_dissolved
  created_at / updated_at timestamptz
  UNIQUE(tenant_id, id)
  部分唯一索引：uq_pp_link_active (tenant_id, product_id, project_id) WHERE status='active'
                -- 同一对 (product,project) 同时最多一条 active；ended 行不限（重建=新行=历史）
```

**状态一致性约束（评审补充 A / C-03，实施期随迁移建立，语义在此锁定）：**

```
ck_pp_link_state:  (status='active'  AND ended_by IS NULL     AND ended_at IS NULL     AND end_reason IS NULL)
                OR (status='ended'   AND ended_by IS NOT NULL AND ended_at IS NOT NULL AND end_reason IS NOT NULL)
ck_pp_link_reason: end_reason IN ('manual', 'team_dissolved')
```

**不可变与不可复活守卫（评审补充 / C-04 + 一致性修订指令 F-03 补强，参照 pc_proposal_anchors / trg_kb_source_immutable 惯例）：**

```
trg_pp_link_guard：
  1) 禁改身份列（id / tenant_id / product_id / project_id / linked_by / linked_at）
  2) 禁 DELETE（历史行永久保留，无任何删除路径）
  3) 禁 ended → active（终态不可复活）
  4) OLD.status='ended' 时拒绝任何 UPDATE（ended 行整体冻结：
     ended_by / ended_at / end_reason 及其余历史字段一律不可覆盖；
     未来如需纠正历史错误，必须另设显式审计纠错机制，不得直接改写原始记录）
```

**principal.kind 校验职责（F-06）**：本表 CHECK 只负责状态字段一致性与 end_reason 枚举——PostgreSQL CHECK 不具备跨表校验 `core.principal.kind` 的能力。职责切分：①人工 caller 的 human 身份由**服务层**判定（沿用 `require_human` 真人判定惯例）；②DISPATCH_SERVICE_PRINCIPAL 的 service 身份由**清扫代码路径**保证（内部常量，crates/pm-application/src/run_dispatch，非任何 API 输入）；③**API 调用者不存在可指定系统清扫身份的参数**。

- 落点=**core schema**（双侧锚点均在 core；沿用 0001 重 FK 惯例，不复制 GAP-04 零 PK 债务——P4-01 DR-02 同款论证）。
- end_reason 仅两值：`manual`（人工解除）与 `team_dissolved`（清扫）。**不设 product_dissolved/project_dissolved**——dissolved 仅由 dissolve_team 写入（P4-01 冻结），清扫时两侧主体必然同时终态化，Team 级事件统一归因（评审补充，详见 §9）。

---

## 5. Cardinality & Primary（基数与主次，DR-01）

| 问 | 裁决结论（ACCEPTED） | 依据 |
|---|---|---|
| 一 Product 多 Project？ | ✅ 是（link 表天然支持） | 基线 P4-DP-03；产品多项目演进常态 |
| 一 Project 服务多 Product？ | ✅ 是（N:M） | 平台/基础设施项目同时服务多产品是真实场景；link 表成本与 1:N 相同，先开放后收紧只需加约束，反向则需迁移 |
| 主次关系（primary）？ | **V1 不设** is_primary；登记扩展路径=部分唯一索引 `(tenant_id,project_id) WHERE is_primary`（每项目至多一主产品） | 无明确场景不设（AGENTS.md 规则 1）；扩展为先例模式（partial unique 现成） |
| 独立 Project（不关联任何 Product）合法？ | ✅ 合法，建项目不强制选产品 | 基线 P4-02 待研究项明示；F-2 不构成强制 |

---

## 6. Association Lifecycle & History（关联生命周期与历史，DR-02）

### 6.1 状态机

```
        link(交集权限, product=active ∧ project 未 archived/deleted)
   [无] ────────────────────────────────────────────► [active]
                                                          │
                    end(交集权限 | 清扫: 锚点终态)         ▼
                                                       [ended]  终态·不可复活
                                                          （重建 = 新 active 行）
```

- **active**：唯一有效态；同一 (product,project) 对至多一条 active（部分唯一索引保证）。
- **ended**：终态。`end_reason` ∈ manual / team_dissolved（CHECK 枚举，§4）。**行永久保留**（无 DELETE 路径、无触发器删行）；重建关系 = 插入新 active 行（旧行即历史）。
- **构造性保证（落实 P4-01 无传播原则③）**：Product restore 只翻 `core.product.lifecycle_status`，不触碰 link 行——「已解除的关联」以 ended 行存在，restore 语义上无从复活、实现上无代码路径。
- **重建身份语义（评审补充 B）**：每次重建 = 插入新 id，**不复用历史 link_id**；旧 ended 行的 ended_by/ended_at/end_reason 永不被后续任何操作覆盖（守卫禁改 + 应用层无更新通道）；「当前关联」查询只返回 active，「关联历史」查询显式包含 ended——关联身份、关联生命周期、关联历史三者分离。
- **仓库模式对齐**：关联聚合实体 = `integration.project_repository` 同款（独立 id + status CHECK + 事件族）；**不给连接行加状态列**（仓库 10 张关联表无此先例，见 P4-00 取证）。

### 6.2 与两侧主体的语义边界

| 事件 | 关联行为 | 依据 |
|---|---|---|
| Product archived（业务归档） | 既有关联**静止保留 active**；禁止**新增** | F-2（P4-01 §6.2：既有关联不解除） |
| Product restored | 关联**无任何动作** | F-4 无传播 |
| Product dissolved | 其 active 链接清扫置 `ended`（reason=**team_dissolved**，统一归因见 §9） | §9（关联自身生命周期对锚点终态的响应，非对 Product 状态的修改） |
| Project archived | 关联**不动**（归档≠终态，F-4：不传播） | §9 |
| Project deleted（仅 dissolve 场景） | 同上（同一清扫事务统一处置，reason=team_dissolved） | §9 |

---

## 7. Permission Intersection Model（权限交集具体化，DR-03）

### 7.1 两侧授权集合（均有现役先例）

| 侧 | 授权集合 | 先例 |
|---|---|---|
| Product 侧 | **Product Owner ∪ tenant admin** | P4-01 DR-06（冻结） |
| Project 侧 | **Project owner ∪ tenant admin** | `is_project_manager` 守卫=项目 owner 或租户 admin（project_lifecycle.rs:45-66，archive 端点现役） |

### 7.2 交集落地：两案比较

| 方案 | 语义 | 代价 | 评估 |
|---|---|---|---|
| (a) **单一执行者双侧授权**（推荐） | 执行者须同时满足 `(PO∪TA) ∧ (ProjOwner∪TA)`；tenant admin 天然满足双侧（break-glass，双侧先例一致） | 零新机制；守卫=两个判定函数 | ✅ V1 推荐。常见情形（产品负责人=项目负责人）直接可操作；跨负责人且无 tenant admin 时**有意阻断（403）**——同一执行者必须满足两侧授权，不允许权限降级或跨 Team admin 兜底（F-10） |
| (b) 双侧确认工作流 | 一侧发起请求、另一侧裁决（两个 actor） | 需新建专表（`governance.approval_request` **不可复用**：1:1 硬绑 change_proposal + UNIQUE(proposal_id) 挡多请求；仓库先例=按域建专表 design_approval/pc.approval） | 登记为扩展路径；出现「产品负责人与项目负责人必须互认」的治理场景时另立 DR |

- **零穿透重申**：Product 权限不计数为 Project 侧授权，反之亦然——判定函数独立，仅在同一条执行路径上求交。
- **审计留痕**：执行者身份（四身份列）随事件入账，天然回答「谁满足了两侈权限」。

### 7.3 执行域与实时性（评审补充，DR-03）

- **同 Team 域约束**：执行者必须在双侧资源**共同所属的同一 Team/Tenant** 内满足授权——tenant admin 兜底 = 该 Team 的 admin（`caller.tenant_id` = 链接租户）；不存在跨 Team 的 admin 算入交集。
- **实时有效授权**：每次建立/解除都重新判定当前状态——当前 Team 成员身份与 admin 角色（`caller.is_admin` 每请求查库惯例，pm-domain/tenant.rs:44-57）、当前 Product Owner、当前 Project owner、product lifecycle='active'、project.status 未终态。**不读缓存、不认历史创建者或历史 Owner**。
- **无暗中降级**：产品负责人与项目负责人不同且无 tenant admin 时，操作**有意阻断**（403）——这是 V1 治理约束，不以放宽单侧授权解决；双侧确认工作流保留为扩展路径（§7.2 方案 b）。

---

## 8. Project Creation Flow Rules（创建链路，DR-04）

- **建项目不强制选产品**；独立 Project 合法（DR-01）。
- **推荐：create-then-link**——项目创建后经关联端点建立链接；**不改 `bootstrap_inner` 事务**。依据：
  1. bootstrap 是唯一生产写 project 的单事务入口（bootstrap.rs:181-347），动它=扩大爆炸半径（P4-01 R-04）；
  2. 「建后补写非核心关系」有现役先例（description 在 commit 后另调 set_project_description，pm_lists.rs:168-174）；
  3. create 成功+link 失败无孤儿风险（项目存在、无链接=独立项目，合法态），重试即可。
- 创建时选产品若 P4-07 需要，实现形态=前端串联两调用（创建→立即链接），**API 层不做合并事务**（登记为 P4-07 输入，不预支）。

---

## 9. Anchor Terminal-State Linkage（锚点终态联动，DR-06）

| 锚点事件 | 链接处置 | 执行者 |
|---|---|---|
| dissolve_team（Team 解散） | **统一终态处置**：该 Team 全部 active 链接置 `ended`，`end_reason='team_dissolved'`；**清扫集合=该 Team 下全部 active Link——无论 Product 或 Project 是否 archived，只要 Link 仍 active 即入集合（F-08）**；与主体终态处置**同一事务边界**；**幂等**（已 ended 行跳过，不重复执行结束操作） | dissolve 清扫（同事务扩展——**实施期登记项 #3**，非本阶段授权） |
| project deleted | 仅发生于 dissolve；含在上行统一处置内 | 同上 |
| project archived | **不动**（归档≠终态；F-4 不传播） | — |
| 载体复活成新 Team | ended 行不复活；新 Team 目录干净 | 已由 dissolved 行为保证（P4-01） |

- **end_reason 确定性（评审补充，C-07）**：解散时 Product 与 Project 在同一清扫事务内同时终态化，一条链接同时符合两侧终态——统一归因 `team_dissolved`（Team 级事件），**不依赖清扫 SQL 语句的执行顺序**；`product_dissolved/project_dissolved` 两值取消（清扫只会成对发生，单侧终态化不存在写入者）。
- **系统清扫身份（评审补充，C-05）**：`ended_by = DISPATCH_SERVICE_PRINCIPAL`（`core.principal.kind='service'`，0001:1228；跨 worker/系统写入既定惯例，pc/publish.rs:303 等十余处在役），与人工解除（caller，kind='human'）可区分、可追溯。
- **事务边界**：链接终态化必须与主体终态处置处于**同一事务**（dissolve 清扫事务内），不产生半终态窗口。
- **语义澄清**：清扫置 ended 是**关联自身生命周期对锚点终态的响应**，不构成「自动传播」——Product/Project 状态零改动（无传播原则约束的是两侧主体状态，不含第三实体的自身生命周期；此边界在 DR-06 显式裁决）。

---

## 10. API Boundary Proposal（仅边界，不实现；DR-05）

**读写权限分离（评审采纳，C-06——读权限≠写交集）：**

| 操作 | 权限 |
|---|---|
| 建立关联 | 双侧写权限交集（§7.2a） |
| 解除关联 | 双侧写权限交集 |
| 产品侧查询关联 | Product Owner ∪ Tenant Admin |
| 项目侧查询关联 | 项目可见性（`require_project_visible`） |
| 单条关联详情 | 所属侧读取权限（**任一侧满足即可**，非写交集；见 F-04 路由修正） |

| 端点 | 方法 | 守卫 | 语义 |
|---|---|---|---|
| `/v1/pm/products/{product_id}/projects` | POST | **交集守卫**（写）+ product lifecycle='active'（F-2）+ project.status ∈ **显式允许集** {planning, initializing}（F-07，未知状态默认拒绝） | 建立链接（201，active 行） |
| `/v1/pm/product-project-links/{link_id}/end` | POST | 交集守卫（写） | 解除=置 ended（**不用 DELETE**；reason=manual） |
| `/v1/pm/products/{product_id}/projects` | GET | **Product Owner ∪ tenant admin**（读） | 产品→项目列表（含 ended 历史，可过滤；**仅返回项目最小字段：code/name/status**——Product Owner 未必是项目成员，不暴露项目详情） |
| `/v1/pm/projects/{project_id}/products` | GET | `require_project_visible`（读） | 项目→产品列表（**最小字段集**：key/name/lifecycle——对齐 P4-01 列表层；不暴露 description 等受限聚合面） |
| `/v1/pm/product-project-links/{link_id}` | GET | 满足**任一侧**读取权限（产品侧：Product Owner∪TA；项目侧：链接所属 project 的 `require_project_visible`）——**F-04 路由修正**：原 `products/{id}/projects/{project_id}` 无法唯一定位历史 Link（同对可有多条 ended 行），改为按 link_id 寻址；权限判定语义不变（所属侧读取权限，非写交集） | 单条链接详情（active 或历史 ended 行；载荷=链接自身字段+双侧最小标识） |

- **数据最小化（评审补充）**：产品侧查询不因「可读 Product」而无条件暴露 Project 全部详情——只返回关联展示所必需的项目最小字段；反方向同理（项目成员见产品最小字段，见不到 description/受限聚合面）。
- 无 DELETE 端点（历史保留设计）。
- 归属字段不收请求体（tenant←caller 纪律）；product_id/project_id 服务层做同租户校验（P4-00 实证的跨资源引用惯例）。
- 新增关联守卫三元组：product active（F-2）∧ project ∈ 允许状态集 ∧ 交集权限——全部 409/403 语义化（对齐 ALREADY_ARCHIVED 惯例）。

### 10.1 状态守卫矩阵（F-05 / F-07，建立与解除分别描述）

| 操作 | Product 条件 | Project 条件 | Link 条件 | 权限 |
|---|---|---|---|---|
| 建立关联 | lifecycle='active'（F-2） | **显式允许集** `status ∈ {planning, initializing}`；未知状态默认拒绝（取证：默认值 planning，bootstrap.rs:270 区段；planning→initializing，lifecycle.rs:269；现役观测值集共 4 个） | 同对无 active 行（部分唯一索引） | 双侧写交集（DR-03） |
| 人工解除 | **不限**（archived 产品可解除其链接；dissolved 产品的链接已由清扫 ended，无可解除对象） | **不限**（archived 项目可解除其链接） | status='active' | 双侧写交集（DR-03） |
| Team dissolve 清扫 | 不限（无论 Product/Project 是否 archived） | 不限 | status='active' | 系统身份 DISPATCH_SERVICE_PRINCIPAL（无人工权限检查；§9） |

- **建立与解除的状态条件互不套用（F-05）**：建立是新增治理关系（从严——主体必须处于可承接治理的活跃态）；解除是收缩既有关系（仅要求链接仍 active，主体 archived 不阻碍收缩）。
- 该矩阵不改变 DR-03 已裁决的权限交集语义（两侧写权限对建立与解除同等适用），仅将**主体状态条件**按操作方向分离。

---

## 11. Audit Design

- **单点**：`pm_write::insert_tenant_audit`（租户级，事务内、失败回滚、四身份列、coverage 清单登记新写入点）。
- **事件集**：`product_project.linked`（after={status:'active',...}）/ `product_project.ended`（before/after + end_reason）。人工与清扫共用同一事件名，**以 actor 身份区分**：人工=caller（kind='human'）、清扫=DISPATCH_SERVICE_PRINCIPAL（kind='service'）——四身份列保证可区分、可追溯（C-05）。
- **不镜像项目级 audit_log**（V1）：链接是产品目录级事实；项目侧审计诉求登记为扩展（如需，加 project 级镜像事件，属实施期增量）。此取舍在 DR-05 内显式列出。
- target_type=`product_project_link`，target_id=链接 id；事件载荷含双侧 id（可双侧追溯）。

---

## 12. Entity Relationship Diagram

```mermaid
erDiagram
    CORE_TENANT ||--o{ CORE_PRODUCT_PROJECT_LINK : "tenant_id"
    CORE_PRODUCT ||--o{ CORE_PRODUCT_PROJECT_LINK : "product_id（组合FK）"
    CORE_PROJECT ||--o{ CORE_PRODUCT_PROJECT_LINK : "project_id（组合FK，N:M）"
    CORE_PRINCIPAL ||--o{ CORE_PRODUCT_PROJECT_LINK : "linked_by / ended_by"

    CORE_PRODUCT_PROJECT_LINK {
        uuid id PK
        uuid tenant_id FK
        uuid product_id FK "组合FK→core.product"
        uuid project_id FK "组合FK→core.project"
        string status "active | ended（ended 终态不可复活）"
        uuid linked_by FK
        datetime linked_at
        uuid ended_by FK "可空；人工=caller(human)，清扫=DISPATCH_SERVICE_PRINCIPAL(service)"
        datetime ended_at "可空；ended 时必填（ck_pp_link_state）"
        string end_reason "manual | team_dissolved（CHECK 枚举）"
        datetime created_at
        datetime updated_at
    }

    CORE_PRODUCT {
        uuid id PK "P4-01 冻结主体"
        string lifecycle_status "active | archived | dissolved"
    }

    CORE_PROJECT {
        uuid id PK "既有主体，本切片零改动"
    }

    PC_COGNITION_OBJECT ..> CORE_PRODUCT : "P4-03 未裁决（本切片零假设）"
```

> 虚线 = 未裁决关系。N:M 无 is_primary（V1，扩展路径已登记）。

---

## 13. Schema Placement Analysis（DR-02 附带）

| 选项 | 评估 |
|---|---|
| **core.product_project_link（推荐）** | 双侧锚点均 core 表，组合 FK 原生可达；重 FK 惯例与 P4-01 DR-02 同论证（主权治理关系不进零 PK 新域惯例） |
| delivery/design 等业务域 | 语义错位（跨产品域目录关系，非交付域内部） |
| integration | project_repository 先例在 integration，但其语境是外部资源绑定；产品-项目是内部治理关系，core 更准 |
| 独立 schema | GAP-04 债务模式，拒绝（P4-01 同论证） |

---

## 14. Dependencies（P4-03 / P4-04 / P4-07）

- **→ P4-03**：本切片零假设 PC。交付的稳定事实=「一 Product ↔ N Project 的正式关系存在且可查询」——P4-03 讨论「一 Product 多 Project 认知」时以此为关系底座，但**不得回改本切片**（冻结纪律）。
- **→ P4-04**：资产关联可复用本切片确立的「聚合实体链接 + ended 终态 + 交集/单侧守卫」模式取舍结论（不强制同构，按资产域特性裁决）。
- **→ P4-07**：项目页产品徽标走 `GET /projects/{id}/products`（最小字段）；产品详情页关联区走 `GET /products/{id}/projects`（Owner∪admin）；创建时选产品=两调用串联（§8）。

---

## 15. Risks & Open Questions

| # | 风险/开放问题 | 级别 | 处理 |
|---|---|---|---|
| R-01 | 交集守卫（方案 a）在「产品负责人≠项目负责人且无 admin」时不可操作 | 低（P4-01 已裁决交集，此为实现形态取舍） | DR-03 列 (b) 双侧确认扩展路径 |
| R-02 | 链接行随 ended 历史增长（反复建/解） | 低 | 事件频率天然低；无清理需求（审计锚定） |
| R-03 | project.status 无 CHECK（自由文本），存在未知状态风险 | 中 | **F-07 指令修订**：建立关联采用显式允许集 `{planning, initializing}`，未知状态默认拒绝；实施期回归测试覆盖；不改既有列 |
| R-04 | 产品侧读（Owner∪admin）与项目侧读（成员）不对称，跨侧可见性缝隙 | 低 | 与 P4-01 两层可见性同构；P4-07 统一导航体验 |
| OQ-1 | ended 行是否参与「产品下项目计数」等聚合展示 | 开放 | 默认仅计 active；P4-07 裁决展示口径 |
| OQ-2 | 双侧确认工作流（方案 b）触发场景 | 开放 | 出现跨负责人互认治理需求时另立 DR |

---

## 16. Decision Records Required

全部 DR 落盘 **[P4-02-decision-records-draft-v1.0.md](P4-02-decision-records-draft-v1.0.md)**（Status=ACCEPTED，决议记录见 DR 文件）：

| DR | 裁决点 | 推荐（PROPOSED） |
|---|---|---|
| DR-01 | 基数与主次 | ✅ **ACCEPTED（2026-11-17·第一组）**：N:M；V1 无 is_primary；独立 Project 合法 |
| DR-02 | 关联形态与历史 | ✅ **ACCEPTED（2026-11-17·第一组，含补充 A~E）**：独立聚合实体（active/ended 行保留）+ 状态 CHECK + 守卫（F-03 后为四禁）+ 清扫身份 + C-01 前置 |
| DR-03 | 交集具体化 | ✅ **ACCEPTED（2026-11-17·第二组，含补充 A~C）**：单一执行者双侧授权（同 Team 域/实时验证/无降级）；双侧确认=扩展 |
| DR-04 | 创建链路 | ✅ **ACCEPTED（2026-11-17·第二组）**：不强制选产品；create-then-link 不改 bootstrap；链接失败≠初始化失败 |
| DR-05 | API 与审计边界 | ✅ **ACCEPTED（2026-11-17·第三组，含读写分离修订）**：端点族 + ended 非 DELETE + tenant_audit_log 单点 + 读写权限分离 + 数据最小化 |
| DR-06 | 锚点终态联动 | ✅ **ACCEPTED（2026-11-17·第三组，含 team_dissolved 修订）**：清扫置 ended（同事务/幂等/team_dissolved）；archived 不动；无传播边界澄清 |

---

## 17. Implementation Readiness Assessment

| 维度 | 判定 |
|---|---|
| 模型完备性 | ✅ 字段级定义+约束+守卫三元组+事件集齐备 |
| 依赖阻塞 | ✅ 无——完全建立在 P4-01 冻结输入之上，不依赖 P4-03/P4-04 |
| 实施面估算 | 小：1 迁移（1 表+部分唯一索引）+ 1 服务模块 + 5 端点 + 2 审计事件 + dissolve 清扫扩展（登记项#3） |
| 实施前置 | ✅ DR-01~06 已全部 ACCEPTED（2026-11-17 分组裁决）；迁移含 C-01 前置（core.project 补 UNIQUE(tenant_id,id)）；迁移号全分支扫号 |
| 结论 | **V1.0 FROZEN（2026-11-17，评审 12/12 PASS）**——见 [P4-02-freeze-review-v1.0.md](P4-02-freeze-review-v1.0.md)；冻结≠实施授权 |

---

## 18. Quality Self-Check

| 自检项 | 结果 |
|---|---|
| 使用 P4-01 冻结输入且未回改 | ✅ §3 F-1~F-7 全表对照 |
| 未提前裁决 PC 权威归属 | ✅ §2 Non-goals + §12 ERD 虚线 + §14 |
| 交集原则未弱化为单侧授权 | ✅ §7 两案比较，(a) 仍是双侧求交 |
| 解除不丢历史 | ✅ §6.1 ended 行保留 + 构造性防复活 |
| 无传播原则未破坏 | ✅ §6.2/§9 显式边界澄清（清扫置 ended=关联自身生命周期） |
| 独立 Project 合法性明确 | ✅ §5/§8 |
| bootstrap 零改动 | ✅ §8 |
| 提供真正可选备选 | ✅ 每 DR ≥2 案+trade-off |

---

## 19. 模型一致性核查（C-01 ~ C-07，评审要求）

| # | 检查项 | 结果 | 证据 / 落点 |
|---|---|---|---|
| C-01 | Project 组合 FK 可行性 | ✅ **已核验（存在缺口 → 迁移前置）**：core.project 现仅 `UNIQUE(id, customer_id)`（0001:4030-4034），**无 (tenant_id,id) 组合唯一** → P4-02 迁移须先 `ADD CONSTRAINT UNIQUE(tenant_id, id)`（增量安全：id 为 PK 恒满足；组合唯一先例成排，如 project_ai_role 0001:3810）。未补此约束前，`(tenant_id,project_id)→core.project(tenant_id,id)` 组合 FK 不成立 | §4 C-01 注记 |
| C-02 | 双侧租户一致性 | ✅ DB 级三重保证：link.tenant_id FK + product 侧组合 FK（现成）+ project 侧组合 FK（依赖 C-01 补约束） | §4 |
| C-03 | Link 状态完整性 | ✅ `ck_pp_link_state` / `ck_pp_link_reason`（active⇒结束字段全空；ended⇒结束字段全非空） | §4 约束清单 |
| C-04 | Link 不可复活 | ✅ `trg_pp_link_guard` 三禁（禁改身份列 / 禁 DELETE / 禁 ended→active）+ 无 DELETE 端点 + 重建=新 id 不复用 | §4 / §6.1 / §10 |
| C-05 | 审计身份可区分、可追溯 | ✅ 人工=caller（kind='human'）vs 清扫=DISPATCH_SERVICE_PRINCIPAL（kind='service'，0001:1228；在役先例 pc/publish.rs:303 等）+ 审计四身份列 | §9 / §11 |
| C-06 | 写权限交集与读权限分离 | ✅ §10 读写分离表——读按调用侧可见性判定，不套用写交集 | §10 |
| C-07 | Team 解散时 end_reason 确定性 | ✅ `team_dissolved` 统一归因，与主体终态同事务、幂等、不依赖清扫 SQL 顺序 | §9 |
