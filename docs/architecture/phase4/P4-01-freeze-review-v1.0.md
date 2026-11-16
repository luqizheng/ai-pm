# P4-01 Architecture Review & Freeze 记录 V1.0

> **评审对象（唯一）：** [P4-01-product-domain-model-design-v1.0.md](P4-01-product-domain-model-design-v1.0.md) · [P4-01-decision-records-draft-v1.0.md](P4-01-decision-records-draft-v1.0.md)
> **强制约束：** DR-01~06 已 ACCEPTED 的决议记录（含评审补充）；DR-07 仅作 P4-03 独立裁决骨架
> **评审日期：** 2026-11-17 · **Git 基准：** master @ `b0d8973a`（冻结文档为工作区未提交态，建议入库提交由人工执行）

## 评审结论：**PASS — 允许正式冻结**

冻结版本 **V1.0 FROZEN（2026-11-17）**。无 P0/P1 阻断项；2 处 P2 级文档一致性滞后已在冻结前修复（属已裁决语义的传播滞后，未改变任何 DR 内容，无需新 DR）。

---

## 1. 强制核查 12 项逐项结果

| # | 核查项 | 结果 | 证据位置 |
|---|---|---|---|
| 1 | 归属 Team，`tenant_id` 承载；Team 不永久等同 Tenant | ✅ | 设计 §4 语义分层表（Team=领域概念 / Tenant=隔离承载）；DR-01 Consequences④（解耦迁移点=归属列+权限入口）；DR-01 决议记录（dissolve 已分离二者生命周期） |
| 2 | `core.product` 落点；UUID PK + 组合唯一键 + 必要 FK/CHECK | ✅ | 设计 §11 方案 A DDL 意图（UUID PK、`UNIQUE(tenant_id,id)`、FK tenant/principal/owner/created_by/updated_by、CHECK key 格式与 lifecycle）；DR-02 决议记录 |
| 3 | `product_key` 租户内唯一不可变；身份与 PC 对象分离 | ✅ | 设计 §5.1（UNIQUE(tenant_id,product_key)+格式 CHECK+不可变）；§5.2 第 6 问；DR-03 决议记录（调用方提供；customer.code 反例不采） |
| 4 | 正常生命周期 active/archived、可恢复、禁硬删 | ✅ | 设计 §6.1/§6.2；DR-04 决议记录（restore=纯主体态翻转；审计锚定禁硬删纪律） |
| 5 | `dissolved` 系统终态：仅 dissolve_team 清扫写入、不可恢复 | ✅ | 设计 §6.1 状态机（系统路径专用迁移）+§6.2.1（restore 拒绝/列表排除）；§5.1 CHECK(active,archived,dissolved)；DR-01 评审补充决议；DR-04 补充④ |
| 6 | 载体复用后旧 Product 不进新 Team 目录 | ✅ | DR-01 Consequences②；设计 §6.1 dissolved 条目（载体复活后不进新目录）+§6.2.1（dissolved 列表不可见） |
| 7 | V1 扁平，不预留层级字段 | ✅ | 设计 §7（拒绝 parent 预留列，扩展路径=customer.parent_id 先例 0001:7786 仅登记）；DR-05 决议记录 |
| 8 | DR-06 权限：列表最小可见/详情受限/Owner∪Admin 治理/零穿透 | ✅ | 设计 §9.3（两层可见性）+§9.4 冻结权限矩阵 +§14 API 表（详情 403、无 DELETE）；DR-06 决议记录 1~6 条逐字入档 |
| 9 | Owner 须 Team 有效成员；成员移除前检查持有 | ✅ | 设计 §5.1 owner 行（创建校验+实施集成点）；DR-06 补充 A 全文（含租户成员移除流程持有检查登记） |
| 10 | Association 两侧权限交集；具体授权留 P4-02 | ✅ | DR-06 补充 B（交集原则+边界声明「不预裁决角色/API/审批」）；设计 §16 第 3 条 |
| 11 | 四生命周期独立，禁止自动传播 | ✅ | 设计 §6.3（无传播原则双向锁定）；DR-04 补充③（Product 恢复不恢复已解除关联 / Project 归档不归档 Product） |
| 12 | 不借 Freeze 修改 PC/ProjC/KB/ProductPlan 权威边界 | ✅ | 设计 §10 逐域边界表 + §2 Non-goals；DR-07 定位确认（留 P4-03）；本评审全部写操作仅限 P4-01 三份文档 |

## 2. 评审发现问题及处置

| 问题 | 级别 | 处置 |
|---|---|---|
| §13 ERD lifecycle 属性仍写 `active \| archived`，未含 dissolved | P2（一致性滞后） | **冻结前已修复**（属性改为三态）——不改变 DR 内容（dissolved 已在 DR-01/04 裁决） |
| §15 审计事件集缺 `product.dissolved`（§6.2.1 已引用） | P2（一致性滞后） | **冻结前已修复**（事件集补入，随清扫写入） |

**无 P0/P1 未关闭项。**

## 3. 有意保留的开放项（均 P3，非阻断，已登记处理路径）

| 项 | 说明 | 路径 |
|---|---|---|
| OQ-1/2/3 | key 系统生成备选 / 主体 revision CAS / 立项审批门 | 无场景不实现（AGENTS.md 规则 1）；需求出现时增量+按需 DR |
| R-01 | Team:Tenant 未来解耦迁移成本 | 接缝已收敛（归属列+权限入口），DR-01 Consequences④ |
| R-05 | 迁移排号与 P3-11（0026）协调 | 实施期全分支扫号纪律 |
| DR-07 | 认知归属上移 | P4-03 独立切片裁决（骨架在，P3-11 落地后评审） |

## 4. 冻结声明

- **冻结版本：** V1.0 FROZEN
- **冻结日期：** 2026-11-17
- **冻结基准：** master @ `b0d8973a`（工作区未提交态；本记录与两份冻结文档建议一并入库提交）
- **冻结文件清单：** ①P4-01-product-domain-model-design-v1.0.md ②P4-01-decision-records-draft-v1.0.md ③本评审记录
- **修订纪律：** 冻结后的任何语义变更必须走新 Decision Record 或原 DR 正式修订（含版本递升），不得直接改动设计文档绕过裁决。

## 5. 授权边界声明

**P4-01 Freeze ≠ 数据库 Migration 授权 ≠ 代码实施授权。** 不执行任何 SQL 迁移、Rust/TypeScript/Vue 变更、数据库操作或导航/UI 改动。实施启动须经 Phase 4 门禁（Gate 3/4）与单独的实施任务书。

**实施期登记项（随冻结移交，非本阶段授权）：**
1. `dissolve_team` 清扫语句集纳入 `core.product` 置 dissolved（DR-01 集成点）。
2. 租户成员移除流程增加 Product Owner 持有检查（DR-06 补充 A 集成点）。

## 6. 对后续切片的锁定输入（不得擅自扩大范围）

| 切片 | 可使用的已锁定输入 | 禁止越界 |
|---|---|---|
| P4-02 | 稳定组合键锚点；archived Product 禁新增关联；**两侧权限交集原则**；无传播原则；关联生命周期独立 | 不得在 P4-02 内改 Product 主体模型或权限矩阵；基数/Primary/解除机制按其切片裁决 |
| P4-03 | 稳定身份锚点（id/key 不可变）；身份与 PC 对象分离裁定；DR-07 骨架 | 不得在 P4-03 内顺带改 core.product；PC 归属上移须专项 Decision Record + 评审 |
| P4-04 | product_key 不可变性（引用稳定）；archived 禁新增关联语义；审计单点惯例 | KB 关联仍以 GAP-02（P3-KB-01 落地）为硬前置；关联历史设计不得低于 DR-04 无传播原则 |
