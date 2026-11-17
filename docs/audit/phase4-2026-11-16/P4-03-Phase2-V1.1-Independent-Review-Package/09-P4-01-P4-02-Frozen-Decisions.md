# 09 — P4-01 / P4-02 冻结决定与继承映射（G 节）

**证据基准**：master `e05693afdc0a9ae534735c9b6f67117a6b92de8d`。本包只读取证，未修改任何文件。**本文件不复裁决 P4-01/P4-02——仅提供冻结事实与 P4-03 V1.1 的继承核对。**

## 1. 冻结决定（G.1/G.2）

### P4-01 Product 领域模型

- 状态（设计 :4 逐字）：「**FROZEN V1.0（2026-11-17，架构评审 PASS）** — DR-01~06 ACCEPTED，DR-07 留 P4-03；**冻结 ≠ 数据库迁移或代码实施授权**」。
- 冻结锚点：commit `002c825c`（2026-11-17 07:19:38 +0800，diff=三份 P4-01 文件，纯 .md，740 行）；文档内自记冻结基准 master @ `b0d8973a`（评审时未提交态）。
- DR-01~06 ACCEPTED（2026-11-17 人工逐项裁决）；DR-07=PROPOSED-DRAFT 骨架（「继续留在 P4-03 独立裁决」）。
- 对 P4-03 有约束力的条文（G.3）：
  - **DR-01**：Product 归属 Team（领域语义）/tenant_id（物理承载）；「V1 不变量：Product 不能活得比它的 Team 更久；不存在跨 Team 产品转移」。
  - **DR-02**：core.product（uuid PK+UNIQUE(tenant_id,id) 组合键；product_key 租户内唯一、创建后不可变）。
  - **DR-03**：「**产品身份与 PC 认知对象身份严格分离**（core.product.id 是主体锚点，pc.cognition_object.id 是认知对象标识，P4-03 决定引用方式而非身份合并）」。
  - **DR-04**：active/archived+可恢复+无硬删+dissolved 系统终态（不可恢复）；「archived 语义：可读、禁改、禁新增关联（服务层守卫）、既有关联不自动解除」；「任何方向零自动传播——Product 恢复不自动恢复已解除的 Project 关联…此约束对 P4-02/03/04 生效」。
  - **DR-06**：创建=tenant owner/admin；变更/归档/恢复/移交=**Product Owner ∪ tenant admin**；「可见性两层：列表（最小字段集）全租户成员；详情 Owner∪admin，其余 403」；「Product 权限与 Project 权限双向隔离，不自动穿透」。
  - **DR-07 骨架**：「上移触及 pc 已冻结模型 B01~B10 → 必须专项 Decision Record + 架构评审…逐项目单例骨架处置（数据近零，可直改契约）」。

### P4-02 Product–Project 关系

- 状态（设计 :4）：「**V1.0 FROZEN（2026-11-17，最终架构冻结评审 PASS，12/12 项核查通过）** — DR-01~06 ACCEPTED；冻结 ≠ 代码/Migration/数据库/部署实施授权」。
- 冻结锚点：commit `0a0e0e12`（2026-11-17 08:04:04 +0800，diff=四份 P4-02 文件，纯 .md，732 行）；评审记录 :4 验证「P4-01 V1.0 FROZEN @ `002c825c`」为其冻结输入。
- 一致性修订 F-01~F-10 全 ✅（含 F-03 ended 行四禁守卫、F-07 Project 状态守卫 allow-list {planning,initializing} 随冻结生效）。
- 对 P4-03 有约束力的条文（G.3）：
  - **DR-01**：N:M、V1 无 is_primary、独立 Project 合法。
  - **DR-02**：独立聚合实体 `core.product_project_link`（id PK+双侧组合 FK+status active|ended，ended 终态行永久保留；重建=新 id 不复用；部分唯一 active；C-01 前置=core.project 补 UNIQUE(tenant_id,id)）。
  - **DR-03**：Link 写操作=单一执行者**双侧授权交集**（`(PO∪TA) ∧ (ProjOwner∪TA)`）；双侧确认工作流=扩展路径（另立 DR）。
  - **DR-05**：嵌套建+扁平 end（不用 DELETE）；审计单点 `product_project.linked/.ended`。
  - **DR-06**：Team dissolve 同事务将全部 active Link 置 ended（reason=team_dissolved；清扫身份 DISPATCH_SERVICE_PRINCIPAL）；**archived 不动链接**；「清扫=关联自身生命周期非传播」；清扫扩展=实施期登记项 #3。
  - **§14**：「→ P4-03：本切片零假设 PC。交付的稳定事实=『一 Product ↔ N Project 的正式关系存在且可查询』——P4-03…以此为关系底座，但**不得回改本切片**（冻结纪律）」。

### 实施状态（两切片一致）

**均「文档 FROZEN、实施未开始」**：七份文档无一处声称已实现（grep 反向声明零命中+正向「零代码/零 Migration」声明多处分节）；冻结 commit diff 纯 .md；代码库核验 core.product/product_project_link/ProductOwner/consumption 零命中；迁移止于 0027。→ P4-03 V1.1 及本包涉及 Product/Link 的一切「现状」均为**不存在**（06/05 号文件已核）。

## 2. P4-03 V1.1 对冻结决定的继承映射（G.4）

| V1.1 位置 | 继承的冻结条文 | 核对结论 |
|---|---|---|
| §2.2（L75）「P4-01 DR-03 身份分离裁定原样保持：core.product（主体）≠任何认知对象；PC-P 对象以 product_id 为**归属锚**，不是主体身份的延伸」 | DR-03 原文 | ✅ 语义一致（「引用方式而非身份合并」） |
| §7.1（L243）「PC-P 身份锚=(tenant_id, product_id)；core.product 为归属锚（P4-01 DR-02 形态）」 | DR-02 组合键惯例 | ✅ 一致（F1/F2 物理形态未裁，不违 DR-02） |
| §9.2-H13/§7.9「认知版本列为四生命周期之一，响应不传播」 | DR-04 无传播双向锁定 | ✅ 一致 |
| §7.9 archived 行「不自动解除 Link（P4-01 DR-04：archived 禁新增关联，既有关联不自动解除）；restore 不自动恢复曾解除的 Link（P4-01 补充③）」 | DR-04 | ✅ 逐点一致 |
| §7.9 Team dissolved 行「Product→dissolved（P4-01 DR-01/04）…与 P4-01 DR-01/DR-04、P4-02 DR-06 清扫模式对齐」 | DR-01 不变量+DR-06 | ✅ 一致；但 PC-P 侧清扫细节（含 approved→withdrawn 声明）存在与代码状态机的偏差（见 11 号文件 D-1） |
| §7.8 双语义分离「有权建立 Link ≠ 已授权认知读取」 | P4-02 DR-03（交集管 Link 写）+P4-01 DR-06（双向不穿透） | ✅ 未把关联权限解释为消费权限 |
| §9.2-H14「DR-06 管产品实体详情；DR-08 管认知消费，两平面不相混淆…不重解 DR-06、不自动穿透」 | DR-06 两层可见性 | ✅ 边界声明清晰；Product Owner∪tenant admin 角色集复用=RC-3 一致 |
| §7.9 Link 重建行「不自动恢复旧引用…新 Link=新授权决定+新钉扎显式建立（与 P4-02 DR-02 重建身份语义同构）」 | DR-02 B | ✅ 同构引用恰当 |
| §9.3-H16~H19「Link 底座/交集授权/终态清扫…以其为底座，零回改（P4-02 §14 约束遵守）」 | §14 | ✅ 无回改迹象（P4-02 条文未被修改） |
| §5-S1/§7.3「一 Product 多 Project…各项目独立钉扎」 | P4-02 DR-01 N:M | ✅ 一致（含独立 Project 合法=S2） |
| §3 方案 A「Product…仅经 P4-02 Link 聚合关联项目」 | P4-02 交付稳定事实 | ✅ 引用恰当 |

## 3. 四项特别检查（G 节明示）

| 检查 | 结论 | 证据 |
|---|---|---|
| 是否把 Product 身份与 PC 认知对象身份合并？ | **否** | §2.2/§7.1 逐字保持 DR-03 分离；PC-P 对象 id 为认知对象标识、product_id 仅为归属锚；未发现任何「认知对象=主体身份」表述 |
| 是否把 Product 与 Project 的主体关系误作认知所有权关系？ | **否（有边界）** | §7.1 区分「产品真相权威=PC-P（锚 Product）」与「项目上下文真相权威=Project（经 PC-J）」，内容不同、权威不重叠；Link 仅作「关系底座」引用（P4-02 §14 授权用法）。复审注意点：R1「权威归属 PC-J←Project」的表述将认知权威挂接在主体上，属设计主张（PROPOSED），与 P4-02 无冲突但与 P4-01 DR-03 的「分离」需靠 §2.3 铁律 1/2 维持——边界声明在文，实质由阶段 3 DR 裁决 |
| 是否把 P4-02 的关联权限解释为认知消费权限？ | **否** | §7.8「授权双语义分离（V1.1 闭合审查 P1-3）」+六问第 5/6 问（Link ended 必然撤销、授权⊆Link）；G1 重新定位声明「不是把 Link 写权限升级为读取授权」；P4-02 DR-03 条文未被重释 |
| 是否违反 P4-01 已冻结的主体身份和权限约束？ | **未发现违反；一处需复审确认** | 身份：无违反（上）。权限：H14 边界声明+RC-3 复用 DR-06 角色集=一致。需复审确认点：§7.8 给 Product Owner∪tenant admin 新增「认知消费授权决定权」，这是在 DR-06 冻结角色集之上**新增职责面**（DR-06 原文只管 Product 实体 CRUD 与详情可见性）——V1.1 以「不重解 DR-06、新资源面显式增量（DR-08）」自我处理，逻辑自洽，但其「PO∪TA 天然是授权主体」的推荐仍是 PROPOSED，须 DR-08 正式裁决；未发现与 DR-06 冻结文本的直接冲突 |

## 4. 底座缺失对 P4-03 的影响（事实登记）

P4-03 以 P4-02 Link 为「关系底座」、以 P4-01 core.product 为 PC-P 归属锚，但两者**零实现**：P4-03 的全部消费/授权/生命周期设计在实施序列上必须后置于 P4-01/P4-02 实施任务包（C-01 迁移前置、Link 表、Product 表均不存在）。V1.1 未显式声明该实施顺序依赖（其 §11 只列自身迁移影响）——登记至 11 号文件 §6（非设计缺陷，属实施排期输入）。
