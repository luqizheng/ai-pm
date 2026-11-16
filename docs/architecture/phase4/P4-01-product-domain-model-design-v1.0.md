# P4-01 — Product Domain Model 设计 V1.0

> **阶段：** Phase 4 · **切片：** P4-01 · **性质：** 领域模型设计（只读研究 + 方案比较），**非实施任务**
> **状态：** **FROZEN V1.0（2026-11-17，架构评审 PASS）** — DR-01~06 ACCEPTED，DR-07 留 P4-03；**冻结 ≠ 数据库迁移或代码实施授权**（评审记录：[P4-01-freeze-review-v1.0.md](P4-01-freeze-review-v1.0.md)）
> **上游：** [P4-00 审计报告](../../audit/phase4-2026-11-16/P4-00_Current_Architecture_Ownership_Audit_V1.0.md)（PASS WITH ISSUES）· [Phase 4 规划基线 V1.0](Phase4-Product资产归属演进规划基线-V1.0.md)
> **审计基准：** master @ `b0d8973a` · **Decision Record 草案：** [P4-01-decision-records-draft-v1.0.md](P4-01-decision-records-draft-v1.0.md)（DR-01~06 + DR-07/P4-03 骨架）
> **写操作声明：** 仅创建本设计文档与 DR 草案文件。零代码 / 零 Migration / 零 PC/KB 模型改动。

---

## 1. Executive Summary

**推荐方案（已获裁决 ACCEPTED，2026-11-17）：在 `core` 域新增 `core.product` 实体——Product 是 Team（租户）级的产品业务主体，物理承载沿用 `tenant_id`（当前 Team:Tenant = 1:1 语义映射），身份 = 全局 UUID + 租户内唯一 `product_key`（不可变，调用方提供），生命周期 = `active / archived`（可恢复）+ `dissolved` 系统终态（不可恢复），权限 = 显式 `owner_principal_id`（须为 Team 有效成员）+ tenant admin 兜底 + 两层可见性（列表最小字段/详情受限），V1 扁平无层级，与 Project/PC/KB 的关系全部留待 P4-02/P4-03 独立裁决。**

关键设计立场：

1. **Team ≠ Tenant 的语义分层贯穿全文**：Team 是领域概念（归属主体），Tenant 是当前数据隔离与认证上下文（`core.tenant`）。本设计把「Product 属于 Team」作为领域语义，把 `tenant_id` 列仅当作隔离承载——二者当前 1:1 映射，但接缝收敛为「归属列 + 权限维度」单点，未来若解耦不影响其余设计。
2. **Product 身份与 Product Cognition 对象身份严格分离**：`core.product.id` 是业务主体锚点；`pc.cognition_object`（product_profile/vision 单例）是认知对象，P4-00 已证实其逐项目私有。P4-01 不改 PC 任何模型，只保证未来存在可供 PC 引用的稳定锚点。
3. **不预支 P4-02/P4-03**：基数、Primary、认知上移全部保持开放；本设计只承诺「不阻碍」。

---

## 2. Design Goals & Non-goals

**Goals**

- G1：定义 Product 的领域归属、身份、生命周期、层级立场、权限边界（六个裁决点，各带 DR）。
- G2：与现有 core 域惯例完全对齐（组合键、审计单点、生命周期 CHECK、显式授权）。
- G3：为 P4-02（Product–Project）/P4-03（认知权威上移）提供稳定锚点与依赖清单，不越界预决。
- G4：满足 Phase 4 基线四原则中最相关的三条：P4-DP-03（Product/Project 独立）、P4-DP-05（历史可追溯）、P4-DP-07（显式授权/最小权限）。

**Non-goals**

- 不实施任何代码/迁移/API；不定义 Product–Project 关系（P4-02）；不裁决认知归属上移（P4-03）；不设计资产关联（P4-04）；不实现 Product Plan（P4-05）；不改导航/UI（P4-07）。
- 不引入产品族/产品线/变体结构（无业务场景）；不引入别名/历史名称表；不引入 Product 级版本修订机制。

---

## 3. Existing Architecture Constraints（继承约束清单）

| # | 约束 | 来源 | 本设计的遵守方式 |
|---|---|---|---|
| C-1 | P3-KB-01：KB 资产 Team 级所有权，Project 仅关联上下文（已裁未落地，GAP-02） | 台账 v1.0:145 | 不触碰；Product 与 KB 的关联属 P4-04，本设计零涉及 |
| C-2 | Q-PC-13：Product Plan 不属 PC 正式认知 | registry.rs:57 固化 | 不实现、不重定义 Product Plan |
| C-3 | PC 正式认知 = 唯一权威，AI 提案/人工确认 | P4-00 §7.2 | Product 身份 ≠ 认知对象；不预支上移裁决 |
| C-4 | PC/ProjC 分域红线（不共享身份/状态机/字典） | 0025:9-12 | Product 不进入任一认知域字典 |
| C-5 | AGENTS.md 规则 1：无依据不建兼容层/过渡层/投机结构 | 仓库根 AGENTS.md | 拒绝：层级列、别名表、org 间接层、member 表（V1）、suspended 态 |
| C-6 | core 域惯例：UUID PK + `UNIQUE(tenant_id,id)` 组合键支撑组合 FK（0018:10 明文） | 0018:10 | 方案 A 直接采用 |
| C-7 | 租户级事件审计单点：`pm_write::insert_tenant_audit`（事务内、失败回滚、四身份列、coverage 测试钉死） | pm_write/support.rs:96-131 | 生命周期/归属变更全部走该单点 |
| C-8 | 租户角色 = owner/admin/member（CHECK），`is_admin` 每请求查库 | 0001:1399；pm-domain/tenant.rs:44-57 | 权限模型基于该既有枚举扩展 |
| C-9 | 迁移号须全分支扫号（历史 0055/0064 占位教训）；P3-11 占用 0026 | P4-Q-12 | 实施排号注记，不影响设计 |

---

## 4. Product Domain Definition（Product 是什么）

**领域定义（一句话）：Product 是一个 Team 长期拥有并演进的产品业务主体，是产品认知、产品资产、产品规划与建设项目的未来归属锚点；它不是项目、不是认知对象、不是客户，也不是文档。**

| 判定 | 是/否 | 依据 |
|---|---|---|
| Product 是业务主体（可被归属、可被关联） | 是 | 基线 §2 目标 1-3 |
| Product 是资产所有者（V1 即拥有 KB/需求/设计） | **否（V1 不授予）** | 资产归属演进属 P4-04；本设计只建立主体 |
| Product 是认知权威 | 否 | PC 是认知权威（C-3）；Product 与 PC 关系=P4-03 |
| Product 是 Project 的父级/容器 | 否 | P4-DP-03 独立原则；基数=P4-02 |
| Product 是租户级目录条目 | 是 | 归属 Team（DR-01） |

**领域归属裁决（DR-01 摘要，全文见 DR 文件）：**

- **推荐（PROPOSED）：方案 A——Product 归属 Team。** 领域语义上 Product 是 Team 的资产主体；物理上以 `tenant_id` 承载（与 customer/project 同构）。
- **Team 与 Tenant 的语义分层（任务书强制要求）**：

| 概念 | 层 | 当前实现 | 本设计中的地位 |
|---|---|---|---|
| Team | 领域概念 | 无独立表；**语义映射到 core.tenant** | Product 的**归属主体**（设计语言用 Team） |
| Tenant | 数据隔离/认证上下文 | core.tenant（provider+external_key 唯一，is_active） | Product 行的**隔离承载列**（tenant_id） |

- **为何不把映射当终局**：当前 Team:Tenant=1:1 是实现巧合而非领域必然。本设计把耦合收敛到两处——①归属列 `tenant_id`、②权限判定入口（tenant 角色 + owner 显式授权）。若未来 Team 与 Tenant 解耦（如一 Team 跨租户协作），迁移面=这两处，其余模型不动。
- **成立条件**：①产品不跨 Team 共享（当前无此场景）；②不存在客户级产品所有权诉求（客户是业务关系实体，非所有权主体——见方案 C 拒绝理由）；③Phase 4 各切片均在 Team 语境下设计。
- 方案 B/C/D 的完整比较与拒绝理由 → §11 / DR-01。

---

## 5. Product Identity Model（身份模型，DR-03 摘要）

### 5.1 最小身份属性集（逐字段给依据，不设无据字段）

| 字段 | 类型/约束 | 依据与取舍 |
|---|---|---|
| `id` | uuid PK，`UNIQUE(tenant_id,id)` | 全局唯一 ID（uuid）；组合键惯例（0018:10）支撑未来组合 FK（P4-02 关联表） |
| `tenant_id` | uuid NOT NULL，FK core.tenant | 隔离承载（DR-01 接缝点①） |
| `product_key` | text NOT NULL，`UNIQUE(tenant_id, product_key)` + 格式 CHECK `^[a-z][a-z0-9._-]{1,62}$` | 业务键；**租户内唯一**（对齐 doc_key/requirement.code/design.code 的作用域惯例，全局唯一无业务依据）；**创建后不可变**（requirement.code/design/doc_key 全部不可变惯例；customer.code 可变是反例，不采——产品键是跨项目/跨认知的引用锚点，可变将破坏引用稳定性） |
| `name` | text NOT NULL | **允许重名**（customer.name/project.name 均无唯一约束；产品名是展示语义非身份键） |
| `description` | text NULL | 最小描述；无扩展属性表（无场景） |
| `owner_principal_id` | uuid NOT NULL，FK core.principal(id) | 产品负责人（显式授权锚点，DR-06）；单主责人；**必须属本 Team 有效成员**（创建校验；成员离开须先移交——补充 A，实施集成点=租户成员移除流程加持有检查） |
| `lifecycle_status` | text NOT NULL DEFAULT 'active'，CHECK (active, archived, dissolved) | DR-04 + DR-01 解散终态（dissolved 仅由 dissolve_team 写入，不可恢复） |
| `created_by` / `updated_by` | uuid，FK core.principal | 惯例（requirement.rs:469 同款） |
| `created_at` / `updated_at` | timestamptz | 惯例 |
| ~~row_revision~~ | **不加** | core.* 业务实体无此列（customer/tenant/tenant_member 均无）；Product 低频变更+审计行已可追溯；如需 CAS 可后补（PmWriteError::Conflict 模式现成） |
| ~~alias / 历史名表~~ | **不加** | 无业务场景（AGENTS.md 规则 1） |
| ~~版本/修订机制~~ | **不加** | Product 是主体非认知工件；版本化知识属 PC（P4-03）；主体变更走审计行 before/after |

### 5.2 任务书 §三.P4-01-Q2 六问直答

1. **Product ID 是否全局唯一？** 是——uuid 全局唯一，且带 `UNIQUE(tenant_id,id)` 组合键以支撑未来组合 FK（0018 惯例）。
2. **Product Key 唯一范围？** **租户内**（`(tenant_id, product_key)`）。全局唯一无业务依据；跨租户同名产品是常态（如各团队各自的"移动端 App"）。
3. **名称允许重复？** 允许（同 customer/project）。
4. **Key 创建后可改？** **不可变**。引用稳定性优先；改名的正确通道是改 `name`，不是改键。
5. **需要别名/历史名？** V1 不需要（无场景）。若未来需要，落点是独立别名表而非改键。
6. **产品身份与 PC 对象身份分离？** **是，严格分离**：`core.product.id` 是业务主体标识；`pc.cognition_object.id/object_key` 是认知对象标识（且当前逐项目私有）。P4-03 未来若裁决上移，引用方式是「PC 侧引用 Product 锚点」（DomainRef 机制现成），不是身份合并。P4-01 不改 PC 任何结构。

---

## 6. Product Lifecycle Model（生命周期，DR-04 摘要）

### 6.1 V1 状态机

```
            create(owner/admin)            archive(owner/admin)
   [active] ───────────────────► [active] ───────────────► [archived]
      ▲                                                        │
      └──────────────── restore(owner/admin) ─────────────────┘
   [active|archived] ── dissolve_team(Team 解散清扫，系统路径) ──► [dissolved 终态·不可恢复]
```

- **业务两态：`active / archived`**。创建即 active（customer 惯例 `status default 'active'`）。
- **系统终态：`dissolved`**（2026-11-17 评审补充，落实 DR-01）——仅由 `dissolve_team` 清扫写入，非任何业务 API 可达；**restore 守卫按状态拒绝，dissolved 永不可恢复**；载体复活成新 Team 后 dissolved 行不进新目录（列表排除）。
- **不含 draft**：draft 需要一个「晋升触发条件」，当前无业务场景（AGENTS.md 规则 1）。若未来出现「产品立项审批」，在 DR-04 增量补充 draft→active 门，不需要现在预留。
- **不含 suspended**：暂停语义可由 archived 表达 + 说明字段；独立态无消费方。
- **不含硬删除**：审计 FK 锚定先例明确不可硬删（team_onboarding.rs:1222-1231 注释：「审计 FK 锚定 + 数十张业务表无级联 → 不可硬删」）。
- **允许恢复**：archived→active 显式 restore（Owner/admin），事件审计。⚠️ 这与 requirement（仅 withdrawn→active，archived 不可恢复）和 project（归档单向）的现行惯例不同——刻意选择：requirement 的 archived 是文档级终态，而 Product 是长期主体，误归档恢复是低成本高价值能力。此为 DR-04 内的显式取舍点，保守替代案（archived 终态）同样列出。

### 6.2 Archived 语义

| 行为 | archived 态下 | 实现层 |
|---|---|---|
| 读取（详情/列表/历史资料） | ✅ 允许 | 读面不过滤 |
| 修改基本信息 | ❌ 拒绝（409） | 服务层守卫 |
| 新增任何关联（P4-02 项目关联、P4-04 资产关联） | ❌ 拒绝 | 服务层守卫（本设计登记语义，关联实现属 P4-02/04） |
| 既有关联是否自动解除 | ❌ 不解除 | 关联生命周期独立（见 6.3） |
| restore | ✅（Owner/admin） | 事件审计 |

**`dissolved`（系统终态）行为**：列表不可见、详情不可达、restore 拒绝、一切业务写拒绝；行仅作审计锚（`product.dissolved` 事件随清扫写入）。

### 6.3 四种生命周期严格分离（任务书强制要求）

| 生命周期 | 载体 | 状态集 | 本切片动作 |
|---|---|---|---|
| ① Product 生命周期 | `core.product.lifecycle_status`（新） | active/archived | 本设计定义 |
| ② Project 生命周期 | `core.project.status`（既有） | planning/initializing/archived/deleted（无 CHECK，观测值集） | **不触碰** |
| ③ Product–Project 关联生命周期 | P4-02 未来关联表自有状态列 | 未裁决 | **不定义、不合并**；仅登记约束「archived Product 禁新增关联」 |
| ④ PC 正式版本生命周期 | `pc.cognition_version.status`（既有） | published/archived | **不触碰** |

四个生命周期是四个独立字段/表，**禁止合并成一个状态字段**（任务书 §三.P4-01-Q3）。

**无传播原则（2026-11-17 评审锁定，双向）**：任何方向零自动传播——Product 恢复**不**自动恢复已解除的 Project 关联；Project 归档**不**自动归档 Product。此约束对 P4-02/03/04 生效（DR-04 决议记录 ③）。

---

## 7. Product Hierarchy Analysis（层级，DR-05 摘要）

| 选项 | 内容 | 评估 |
|---|---|---|
| V1 完整支持产品族/线/变体 | 多级结构+继承语义 | ❌ 拒绝：无业务场景；认知/资产/规划三类关联在层级下的语义全部未定义，过度设计 |
| V1 有限层级（parent 列预留） | 加可空 `parent_product_id` 自引用 | ❌ 拒绝（AGENTS.md 规则 1：「存在理由只有以后可能」的列不写） |
| V1 仅预留扩展能力（文档级） | 不加列，登记扩展路径 | ✅ **推荐** |
| V1 扁平 | 同上 | ✅ 与上一致——**V1 即扁平**，扩展路径写入 DR-05 |

**登记的扩展路径（零代码）**：当出现真实产品线场景时，加 `parent_product_id uuid NULL` + 自引用组合 FK `(tenant_id,parent_id)→core.product(tenant_id,id)`——该模式在 `core.customer.parent_id`（0001:7786-7790）已有完整先例，属增量演进不破坏现有引用。

---

## 8. Product Ownership Model（所有权语义）

**所有权三层声明（区分「归属主体 / 负责人 / 权限角色」）：**

| 层 | 含义 | 承载 |
|---|---|---|
| 归属主体（Ownership） | Product 属于哪个 Team | `tenant_id`（领域语义=Team，DR-01） |
| 负责人（Accountability） | 谁对产品负主责 | `owner_principal_id`（单人，显式指派，DR-06） |
| 操作权限（Authority） | 谁能执行哪类操作 | 租户角色（owner/admin）+ Product Owner 判定（DR-06） |

**明确否定项**：Product 不因被 Project 关联而获得/让渡任何所有权（P4-DP-02）；`owner_principal_id` 是职责锚点而非所有权转移——Product 永远归属 Team，负责人可换，归属不随人走。

---

## 9. Product Permission Model（权限模型，DR-06 摘要）

### 9.1 任务书要求的五角色比较与 V1 收敛

| 候选角色 | V1 采纳 | 理由 |
|---|---|---|
| Product Owner | ✅（= `owner_principal_id`） | 单一问责人；管理信息/归档/恢复/移交 |
| Product Maintainer/Manager | ❌ 并入 Owner | 多写角色无场景；出现时扩 product_member 表（模式现成） |
| Product Member | ❌ 不设 | 「成员可写」与最小权限冲突且无场景 |
| Product Viewer | ❌ 不设 | V1 可见性=租户成员全量只读（见 9.3），Viewer 冗余 |
| Team Administrator | ✅ 兜底 | tenant owner/admin 天然可管理全部产品（先例：team_llm `caller.is_admin` 守卫面） |

### 9.2 任务书 §三.P4-01-Q5 七问直答

1. **谁可以创建 Product？** tenant owner/admin（`caller.is_admin`）。产品是租户级目录主体，创建是治理动作（先例：LLM Provider 同门槛）。
2. **谁可以修改基本信息？** Product Owner 或 tenant admin。
3. **谁可以归档/恢复？** Product Owner 或 tenant admin。
4. **谁可以管理成员/移交负责人？** Product Owner 或 tenant admin；owner 不可自我移除（需 admin 或先移交——对齐 tenant owner 转让纪律 tenant.rs:49-51）。
5. **Product 权限是否独立于 Project Member？** **是，完全独立**。两套判定源：租户角色（core.tenant_member）与产品负责人（core.product.owner_principal_id）；project_member 不参与任何 Product 判定。
6. **Team 管理员是否天然拥有 Product 管理权？** **是**（Owner/Admin 兜底，先例一致）；但 tenant owner 独占操作（解散团队/所有权转让）不延伸到 Product——Product 无等价破坏性操作。
7. **Product 成员是否天然拥有关联 Project 访问权？** **否（V1 无 Product 成员概念，即使未来引入也须显式授权）**。任务书双向禁令遵守：Project 角色不自动获得 Product 管理权（第 5 点），Product 角色不自动获得 Project 访问权。跨域访问永远走各自域的显式成员/授权机制。

### 9.3 可见性（读权限）——已裁决（DR-06 ACCEPTED，2026-11-17）

- **裁决方案**：**最小可见 + 详情受限**两层——
  - 列表 `GET /products`：最小字段集（key/name/lifecycle/owner，**不含 description**），全租户成员可见——支撑 P4-02 关联选择器、项目页产品徽标、P4-07 目录浏览。
  - 详情 `GET /products/{id}`：全字段（含 description 及未来 P4-03/04/05 聚合面），**Owner ∪ tenant admin，其余 403**（存在性已由列表暴露，用 403 对齐 team_llm `err_forbidden` 惯例）。
  - 修订理由：**不对称性——收紧接口破坏既有调用方，放宽纯增量**；Product 是后续切片的敏感聚合枢纽（战略/商业信息），权限默认值必须落在将来想要的位置。原「全租户读」案已否决。
  - Viewer 角色不建表（V1）：将来加授权记录即可开放详情，契约零变更。
- 原案备选（全租户读）：**已否决**（DR-06 决议记录）。
- 与 GAP-08 的区别仍然成立：docproc 争议在项目文档产物跨项目暴露；本分层针对 Product 自身承载的商业信息，从第一天即受控。

### 9.4 冻结权限矩阵（DR-06 已裁决，2026-11-17）

| 操作 | Product Owner | Tenant Admin | 普通 Team Member |
|---|---|---|---|
| 查看 Product 列表（最小字段） | 允许 | 允许 | 允许 |
| 查看 Product 详情 | 允许 | 允许 | **拒绝** |
| 创建 Product | —（依赖 Admin） | 允许 | 拒绝 |
| 修改 Product | 允许 | 允许 | 拒绝 |
| 归档 / 恢复 | 允许 | 允许 | 拒绝 |
| 移交 Product Owner | 允许 | 允许 | 拒绝 |
| 管理 Product 成员 | 不适用（V1 无成员体系） | 不适用 | 不适用 |

补充约束（随裁决生效）：①Owner 必须是本 Team 有效成员，离开前须完成移交（补充 A）；②Product–Project Association 为独立权限边界，**两侧权限取交集**、零穿透（补充 B，具体角色/API 由 P4-02 裁决）。

---

## 10. Product 与其他领域的边界（任务书 §三.P4-01-Q6 逐域）

| 领域 | P4-01 明确的边界 | 遵守的裁决 |
|---|---|---|
| **Project** | 不同业务主体：Product 是长期产品主体，Project 是建设/交付载体；本设计不建立任何关系字段/表 | P4-DP-03；基数等全部留给 P4-02 |
| **Product Cognition** | Product 身份 ≠ 认知对象。`core.product.id` 是主体锚点；pc product_* 对象保持项目私有现状零改动。未来关系=P4-03 独立裁决，本设计只保证「存在可被引用的稳定锚点 + 键不可变使引用可长期成立」 | C-3；任务书 §4.2 全部禁令 |
| **Project Cognition** | 项目技术/实施认知不自动归 Product；projc 零涉及 | C-4 |
| **KB** | 零涉及。不改 P3-KB-01 裁决、不预支 Product↔KB 关联（P4-04） | C-1 |
| **Requirement** | 项目需求不因 Product 存在而变成产品需求；需求域零改动 | 基线非目标 5 |
| **Design / ADR** | 领域权威不变；ADR 仍是 design 域业务实体（P4-00 GAP 区分双层语义） | S03 裁决 |
| **Product Plan** | 不实现、不定义、不重新归域；仅保持协议现状（PermanentlyExcluded） | Q-PC-13（C-2） |
| **Team/Tenant** | 见 §4 语义分层 | DR-01 |
| **Customer** | 客户是业务关系实体；Product 不挂 customer（若未来需要「产品卖给谁」，属 P4 后续切片，不由本设计预支） | 方案 C 拒绝理由 |

---

## 11. Schema Placement Options（方案比较，DR-02 摘要）

### 方案 A：core.product（推荐）

**实体结构（V1 完整 DDL 意图，非迁移）：**

```
core.product
  id                 uuid PK
  tenant_id          uuid NOT NULL FK → core.tenant(id)
  product_key        text NOT NULL   -- CHECK 格式；UNIQUE(tenant_id, product_key)
  name               text NOT NULL
  description        text NULL
  owner_principal_id uuid NOT NULL FK → core.principal(id)
  lifecycle_status   text NOT NULL DEFAULT 'active' CHECK (active|archived|dissolved)
  created_by         uuid FK → core.principal(id)
  updated_by         uuid NULL FK
  created_at / updated_at timestamptz
  UNIQUE(tenant_id, id)             -- 组合键支撑未来组合 FK（0018:10 惯例）
```

| 维度 | 评估 |
|---|---|
| 组合键与 FK | uuid PK + `UNIQUE(tenant_id,id)`；P4-02 关联表可直接 `(tenant_id,product_id)→core.product(tenant_id,id)` 组合 FK |
| Tenant 隔离 | 行级 tenant_id，与 customer/project 同构 |
| 生命周期 | CHECK 两态 + 服务层守卫；审计走 tenant_audit_log |
| 权限 | 复用 is_admin 判定 + owner 列；无新权限表 |
| 与 core 一致性 | 最高——字段集、约束风格、审计通道全部对齐 customer/tenant_member 先例 |
| 优点 | 完整性库级保证（避开 GAP-04 零 PK 模式）；权限/审计/路由零新建机制；实施面最小（1 表 + 1 迁移） |
| 缺点 | core 域承接新产品概念（域语义上 core=租户与主体层，Product 属主体层，成立） |
| 风险 | 若未来 Team:Tenant 解耦，需迁移 tenant_id 语义（接缝已收敛，见 §4） |
| 对 P4-02~07 | 02:关联表有干净 FK 目标；03:有稳定锚点；04:关联模型可复用组合键；05:Plan 可引用；06:单表迁移；07:目录 API 独立 |

### 方案 B：独立 product schema

- 结构：`product.product` 表，仿 design/plan/projc 惯例（零 FK + 应用层校验）。
- 优点：域边界纯净，认知/资产切片可共置。
- 缺点/风险：** propagates GAP-04（新域零 PK 债务）**——0010/0011/0025 零 PK 已被 P4-00 定级 P2，不应为主权主体复制该模式；权限/审计需新建通道（tenant_audit_log 写入点新增）；跨域事务（product+project 关联）无库级保障。
- 评估：**不推荐**。独立 schema 惯例的动机是「复用认知域机制、不复用业务表」（0025:9-12），Product 是治理主体不是认知域，语境不符。

### 方案 C：其他候选（逐一否定）

| 候选 | 否定理由 |
|---|---|
| Product 作为 customer 子类型（parent_id 挂客户） | 语义错位：customer 是**业务关系**实体（可变 code、无所有权语义、parent 表示集团层级），产品归属主体是 Team 不是客户；且客户可删改，不适合作长期主体锚 |
| PC 单例直接升格为 Product | 违反任务书 §4.2 禁令（不得将 product_profile/vision 认定为 Product 实体）；且单例当前逐项目私有，升格=P4-03 裁决 |
| 平台级全局产品（无 tenant） | 无跨租户共享场景；破坏现行租户隔离模型全线惯例 |
| Team 独立实体先行（先建 core.team 再挂 product） | 语义正确但引入一次纯映射层迁移（team↔tenant 1:1），AGENTS.md 规则 1 禁止无依据过渡层；Tenant 语义分层已由 DR-01 接缝收敛覆盖 |

### 结论

**推荐方案 A（PROPOSED）**；B/C 全部保留在 DR-02 供裁决推翻。

---

## 12. Recommended Domain Model（推荐模型汇总）

- **1 张新表**：`core.product`（§11 方案 A 字段集）。
- **0 张新权限表**：owner 用列，不用 member 表（V1）。
- **0 处既有表改动**：pc/projc/kb/delivery/design/plan/core.project 全部零触碰。
- **生命周期**：active/archived + restore + `dissolved` 系统终态（不可恢复）；服务层守卫 archived 禁改禁新关联。
- **权限**：create=tenant admin；mutate/archive/restore/owner-transfer=Product Owner ∪ tenant admin；**read 分层：列表最小字段=租户成员 / 详情=Owner∪admin（403）**（DR-06 已裁决）；Owner 须为 Team 有效成员（补充 A）。
- **审计**：product.created / product.updated / product.archived / product.restored / product.owner_changed → `governance.tenant_audit_log`（insert_tenant_audit 单点）。
- **显式不做**：层级列、别名、版本修订、row_revision、suspended、硬删除、member 表、customer 关联、任何 project-facing 字段。

---

## 13. Entity Relationship Diagram

```mermaid
erDiagram
    CORE_TENANT ||--o{ CORE_PRODUCT : "tenant_id（隔离承载）Team=Tenant当前1:1映射"
    CORE_PRINCIPAL ||--o{ CORE_PRODUCT : "owner_principal_id（显式授权负责人）"
    CORE_PRINCIPAL ||--o{ CORE_PRODUCT : "created_by / updated_by"

    CORE_PRODUCT {
        uuid id PK "全局唯一"
        uuid tenant_id FK "隔离承载（DR-01接缝①）"
        string product_key UK "租户内唯一且不可变（DR-03）"
        string name "允许重名"
        string description "可空"
        uuid owner_principal_id FK "产品负责人（DR-06）"
        string lifecycle_status "active | archived | dissolved（DR-04 + DR-01 解散终态）"
        uuid created_by FK
        datetime created_at
        datetime updated_at
    }

    CORE_PROJECT {
        uuid id PK "既有主体，本切片零改动"
    }

    PC_COGNITION_OBJECT {
        uuid id PK "认知对象，逐项目私有，本切片零改动"
    }

    CORE_PRODUCT ..> CORE_PROJECT : "P4-02 未裁决：基数/主次/解除机制全部开放"
    PC_COGNITION_OBJECT ..> CORE_PRODUCT : "P4-03 未裁决：认知权威是否上移至 Product 级"

    GOVERNANCE_TENANT_AUDIT_LOG }o..|| CORE_PRODUCT : "逻辑引用（target_type/target_id），无FK，审计单点"
```

> 虚线（`..>`）= 未裁决关系，**不构成已确定模型**（任务书 §六.13 要求）。

---

## 14. API Boundary Proposal（仅边界，不实现）

**推荐挂载（PROPOSED，二选一在 DR-02 附带裁决）：业务面形态，仿 `/v1/pm/customers`（无租户段，作用域取 caller.tenant_id）**：

| 端点 | 方法 | 守卫 | 语义 |
|---|---|---|---|
| `/v1/pm/products` | POST | tenant admin | 创建（key/name/owner/description） |
| `/v1/pm/products` | GET | 租户成员 | 目录列表（**最小字段集**，不含 description；含 lifecycle 过滤） |
| `/v1/pm/products/{id}` | GET | **Owner ∪ admin（其余 403）** | 详情（全字段；DR-06 已裁决） |
| `/v1/pm/products/{id}` | PATCH | Owner ∪ admin | 改 name/description（key 拒改→400） |
| `/v1/pm/products/{id}/archive` | POST | Owner ∪ admin | 归档（非终态可恢复） |
| `/v1/pm/products/{id}/restore` | POST | Owner ∪ admin | 恢复 |
| `/v1/pm/products/{id}/owner` | POST | Owner ∪ admin | 负责人移交（对齐 tenant_members owner-transfer 先例） |

- **无 DELETE 端点**（§6.1 硬删除禁止）。
- **无任何 project-facing 端点**（P4-02）。
- 归属列不接受请求体传入（P4-00 实证的统一纪律：tenant←caller，防越权）。
- 备选形态：tenant 管理面 `/v1/pm/tenants/{tenant_id}/products*` + `guard_tenant` + `is_admin`（team_llm 先例）——若裁决认为 Product 纯属治理面则选此；推荐前者，因读面开放给全体租户成员，管理面形态会把读也锁进 admin。

---

## 15. Audit & Lifecycle History

- **通道**：租户级单点 `pm_write::insert_tenant_audit`（pm_write/support.rs:101-131）——事务内写入、失败整体回滚、actor 四身份列齐备（coverage 测试 audit_actor_coverage.rs 钉死清单，新增写入点须登记）。
- **事件集**：`product.created` / `product.updated` / `product.archived` / `product.restored` / `product.owner_changed` / `product.dissolved`（随 dissolve_team 清扫写入；对齐 `llm_provider.*` 点分命名惯例 team_llm.rs:455+）。
- **载荷**：target_type="product"、target_id=产品 id；updated/archived/restored 携 before_data/after_data；owner_changed 携新旧 owner。
- **不建独立历史表**：Product 主体变更频率低，审计行已满足 P4-DP-05；版本化知识归 PC（P4-03）。

---

## 16. P4-02 Dependencies（对 Product–Project 切片的输出与约束）

**P4-01 交付给 P4-02 的**：

1. 稳定锚点：`core.product.id` + `UNIQUE(tenant_id,id)` 组合键 → P4-02 关联表可用组合 FK 干净引用。
2. 生命周期约束：archived Product **禁止新增关联**（服务层语义已定义，P4-02 落实）；既有关联在 Product archived 时不自动解除（关联生命周期独立）。
3. 权限锚点：关联管理授权**不在本切片定义**（任务书 §4.1 明令），但边界已锁（DR-06 补充 B）——**建立/解除关联必须同时满足 Product 侧与 Project 侧各自权限（交集原则），任一侧不得穿透另一侧**；Association 生命周期与审计独立于两侧主体。具体角色/审批/API 形态由 P4-02 裁决。

**P4-01 明确不决定的（P4-02 保持开放）**：基数（1:N/N:N）、Primary 标记、关联解除与历史机制、建项目是否强制选产品、无产品项目的合法性。

---

## 17. P4-03 Dependencies（对认知归属切片的输出与约束）

**P4-01 交付给 P4-03 的**：

1. 稳定主体锚点（Product.id/product_key 不可变）——PC 未来引用的候选锚。
2. 身份分离裁定：Product 主体 ≠ pc.product_* 认知对象（§5.2 第 6 问）。
3. DR-07 骨架（独立裁决，含选项占位与禁止事项清单）。

**P4-01 明确不改的**：pc.cognition_object 归属模型、PC 确认机制、版本/Revision 结构、逐项目单例 bootstrap（bootstrap.rs:325 现状保留）；P4-00 的混合推荐**不视为已确认决定**。

---

## 18. Risks & Open Questions

| # | 风险/开放问题 | 等级 | 处理 |
|---|---|---|---|
| R-01 | Team:Tenant 未来解耦的迁移成本（当前接缝已收敛但仍是物理耦合） | 低 | DR-01 登记接缝；解耦时仅动归属列+权限入口 |
| R-02 | archived 可恢复与代码库单向归档惯例不一致，评审可能被挑战 | 低 | DR-04 已列保守替代案，交裁决 |
| R-03 | ~~读可见性=全租户成员~~ → 已由 DR-06 修订案消解（最小可见+详情受限）；详情 403 面是否影响 P4-02/07 关联 UX 待验证 | 低 | P4-07 设计目录级页面；Viewer 增量授权兜底 |
| R-04 | P4-02 若需要「建项目即选产品」，创建链路（bootstrap）将触及项目创建事务——本设计未预支 | 中 | 留 P4-02；本设计保证 Product 可独立于 Project 存在 |
| R-05 | 迁移排号与 P3-11（0026）冲突窗口 | 低 | 实施期全分支扫号（C-9） |
| OQ-1 | product_key 是否需要系统生成备选（`PRD-NNN`，next_code 先例 pm_write/support.rs:134-150） | 开放 | DR-03 选项 B；推荐调用方提供 |
| OQ-2 | 是否需要 Product 级 `revision bigint`（core.project 先例 0001:1258）供未来 CAS | 开放 | V1 不加（§5.1）；需求出现时增量 |
| OQ-3 | 产品创建是否需要审批流（立项门） | 开放 | 无场景不设（AGENTS.md 规则 1）；出现时走 governance.approval_request 先例 |

---

## 19. Decision Records Required

全部 DR 草案落盘于 **[P4-01-decision-records-draft-v1.0.md](P4-01-decision-records-draft-v1.0.md)**（Status 全部 = PROPOSED）：

| DR | 裁决点 | 推荐（PROPOSED） |
|---|---|---|
| DR-01 | Product 的领域归属 | ✅ **ACCEPTED（2026-11-17，含修订 Consequences：Team 解散终结产品内容 / 载体复活隔离 / 不比 Team 长寿 / 无跨 Team 转移）**——见 DR 文件 |
| DR-02 | Product Schema 落点 | ✅ **ACCEPTED（2026-11-17）**：core.product（方案 A） |
| DR-03 | Product 身份与唯一性 | ✅ **ACCEPTED（2026-11-17）**：uuid 全局 ID + 租户内唯一不可变 product_key（调用方提供）；name 允许重名；身份与 PC 对象分离 |
| DR-04 | Product 生命周期 | ✅ **ACCEPTED（2026-11-17）**：active/archived + 可恢复（纯主体态翻转）+ 事件流审计；四生命周期分离 + 无传播原则（双向）；`dissolved` 系统终态（随 DR-01） |
| DR-05 | 层级结构 | ✅ **ACCEPTED（2026-11-17）**：V1 扁平，零预留列 |
| DR-06 | 权限边界 | ✅ **ACCEPTED（2026-11-17，含补充 A/B）**：创建=admin；变更=Owner∪admin；owner 单列且须 Team 有效成员；列表最小字段全租户 / 详情 Owner∪admin（403）；双向不穿透；关联授权=两侧交集原则（P4-02 裁决具体面） |
| DR-07 | （骨架）P4-03 Cognition Ownership Boundary | ✅ **定位已确认（2026-11-17）：留 P4-03 独立裁决，不提前裁决认知归属**；问题/约束/选项占位/依赖已登记 |

---

## 20. Implementation Readiness Assessment

| 维度 | 判定 |
|---|---|
| 模型完备性 | ✅ 字段级定义+约束+守卫语义齐备，可直接转 DDL |
| 依赖阻塞 | ✅ 无——不依赖 P3-KB-01 落地、不依赖 P3-11、不依赖 P4-02/03 结论 |
| 实施面估算 | 小：1 迁移（1 表+索引）+ 1 服务模块 + 7 端点 + 审计 5 事件 + 前端目录页（P4-07 另计） |
| 实施前置 | ✅ DR-01~06 已全部 ACCEPTED（2026-11-17 逐项人工裁决）；DR-07 定位确认留 P4-03；迁移号全分支扫号 |
| 测试要点预设 | archived 守卫矩阵、key 不可变、越权写 403/404、审计行 coverage、restore 幂等 |
| 结论 | **FROZEN V1.0（2026-11-17，架构评审 PASS）**——冻结记录见 [P4-01-freeze-review-v1.0.md](P4-01-freeze-review-v1.0.md)；**冻结≠数据库迁移或代码实施授权** |

---

## 21. Quality Self-Check（任务书 §九 逐项）

| 自检项 | 结果 |
|---|---|
| Product 与 Project 明确区分 | ✅ §4/§10/§13（关系画为虚线未裁决） |
| Team 与 Tenant 语义区分 | ✅ §4 专表分层；DR-01 强制条款 |
| 未裁决关系基数保持开放 | ✅ §16 开放清单；ERD 虚线 |
| 未将 PC 提前改 Product 级 | ✅ §5.2/§17；任务书 §4.2 禁令逐条遵守 |
| P3-KB-01 保持不变 | ✅ §3 C-1/§10 零涉及 |
| Q-PC-13 保持不变 | ✅ §3 C-2/§10 |
| Product 权限不自动穿透 Project | ✅ §9.2 第 5/7 问双向否定 |
| V1 无无据层级 | ✅ §7 拒绝 parent 预留列（AGENTS.md 规则 1） |
| 提供真正可选的备选方案 | ✅ DR 每项含 ≥2 备选+trade-off；§11 三方案 |
| 识别 P4-02/P4-03 依赖 | ✅ §16/§17 + DR-07 骨架 |

---

*本文件为裁决输入，不代表已批准架构。Recommendation ≠ ACCEPTED。完成后等待人工评审，不自动实施。*
