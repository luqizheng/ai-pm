# P4-02 — V1.0 Final Architecture Freeze Review

> **评审对象（三份文件均已全文直读，非摘要评审）：** [P4-02-product-project-relationship-design-v1.0.md](P4-02-product-project-relationship-design-v1.0.md) · [P4-02-decision-records-draft-v1.0.md](P4-02-decision-records-draft-v1.0.md) · [P4-02-一致性修订报告-v1.0.md](P4-02-一致性修订报告-v1.0.md)
> **冻结输入核对：** P4-01 V1.0 FROZEN @ `002c825c`（git show 验证：三份 P4-01 文件随该提交入库；DR 文件含 19 处 ACCEPTED 记录）
> **评审日期：** 2026-11-17 · **评审性质：** 最终架构一致性评审与冻结评审 · **实施授权：无**

## 评审结论：**PASS — 12/12 项核查通过，允许正式冻结**

冻结版本 **V1.0 FROZEN（2026-11-17）**。无阻断项；冻结前修正 3 处文档残留（§4 冻结前修正清单）+ 1 处计数标注，均不改变任何 DR 语义。

---

## 1. 强制核查 12 项

### 01 决策状态与冻结边界 — **PASS**

- DR-01~06 状态行+决议记录全部 ACCEPTED（DR 文件 :11/:27/:53/:77/:93/:129，尾行 :145/:169）；无「全部 PROPOSED」残留（评审发现设计 §4/§5/§16 三处 PROPOSED 措辞残留 → 冻结前已修正）。
- 「决议记录为唯一真源」：DR 文件头注 ：3；「Freeze 前不构成实施授权」→ 冻结后改为「冻结不构成实施授权」（:3/:169）；设计 §17 结论「冻结≠实施授权」。
- 未将评审通过误写为实施授权（全部边界语句核查通过）。

### 02 P4-01 冻结输入兼容性 — **PASS**

- Team≠Tenant 分层：P4-02 经 §3 F 表引用 P4-01 冻结裁决，无任何 Team=Tenant 永久等同表述。
- 正常生命周期仅 active/archived + dissolved 为系统终态：设计 §12 ERD（lifecycle_status 三值标注「P4-01 冻结主体」）、F-6/F-7 继承行（§3 :49-50）；P4-02 未把 dissolved 增设为业务态。
- archived 禁新增、既有关联保留：F-2（:45）+ §6.2（:142）+ §10.1 建立行（:233）。
- 无传播：F-4（:47）+ §6.2（:143/:145）+ §9 语义澄清（:200）。
- 身份分离 / PC 零裁决：§2 Non-goals（:33）+ §12 ERD 虚线（:284）+ §14（:304）。

### 03 N:M 与独立 Project 合法性 — **PASS**

- N:M、无 is_primary、独立 Project 合法、多值展示、主产品需独立决策：DR-01（:9-21，决议 :20）+ 设计 §5（:108-115，已改「裁决结论（ACCEPTED）」）+ §12 ERD 注（:287）。

### 04 Link 聚合实体与数据完整性 — **PASS**

- 独立聚合实体（非连接表）：DR-02（:25-47）+ 设计 §4（:54-104）+ §13 落点分析（:291-298）。
- 字段齐备：UUID PK/tenant_id/双侧主体/status/linked_by/linked_at/ended_by/ended_at/end_reason/时间戳（设计 :60-79）。
- 双侧组合 FK + **C-01 前置（先为 core.project 补 UNIQUE(tenant_id,id)）**：设计 :64-67 + §19 C-01（:369，证据 0001:4030-4034 本会话亲验）。
- 部分唯一索引 uq_pp_link_active（:77-78）；重建=新 id 不复用（DR-02 补充 B :40）。

### 05 Link 状态约束与四禁 — **PASS**

- 四禁逐项：设计 :92-99（①身份列 ②DELETE ③ended→active ④OLD.status='ended' 拒绝任何 UPDATE/整行冻结/历史纠错须显式审计机制）= DR-02 补充 C（:41，F-03 补强标注）。
- 状态一致性 CHECK：ck_pp_link_state / ck_pp_link_reason（:84-87）；end_reason 仅 manual|team_dissolved（:86、:104）。
- 设计 §19 C-03/C-04（:371-372）。

### 06 建立与解除状态守卫矩阵 — **PASS**

- §10.1 矩阵逐行核验（:229-238）：建立=product active ∧ project∈{planning,initializing} 未知默认拒绝 ∧ 无 active 行；解除=仅 link active，主体状态不限（archived 产品/项目均可解除；未错误要求 product active 或 project 可建态）；清扫=系统身份不限主体状态。
- 与 DR-03 实时权限检查无冲突：矩阵注（:238）明示权限交集对建立与解除同等适用，仅主体状态条件按方向分离。

### 07 生命周期联动 — **PASS**

- Product archived→保留禁新增（§6.2 :142）；restored→不变+重开新增（:143，ended 行构造性不复活 :134）；Project archived→不动（:145/:194）。
- Team dissolve：清扫集合=全部 active Link 不论 archived（§9 :192，F-08）；同事务（:199）；幂等跳过已 ended（:192）；reason=team_dissolved 统一（:197）；旧 Link 永久保留不可恢复（四禁②③④）。
- 无主体状态自动传播（§9 :200 语义澄清 + DR-06 :135）。

### 08 双侧权限交集与实时授权 — **PASS**

- 布尔式（PO∪TA）∧(ProjOwner∪TA)：DR-03（:58）+ 设计 §7.2（:163）。
- 同 Team 域/实时验证/不依赖历史与缓存/403 有意阻断/V1 无双侧确认工作流：DR-03 补充 A~C（:65-67）+ 设计 §7.3（:171-173）。
- 读权限未套用写交集：§10 分离表（:208-214）。

### 09 Actor 身份与审计责任 — **PASS**

- 人工=caller（kind='human'）、清扫=DISPATCH_SERVICE_PRINCIPAL（kind='service'）：设计 §4 F-06（:101）+ §9（:198）。
- **kind='service' 直接证据**：`DISPATCH_SERVICE_PRINCIPAL: Uuid = Uuid::from_u128(1)`（crates/pm-application/src/run_dispatch/lease.rs:214）= 种子行 `('00000000-…-0001','service','ai-pm-dispatch')`（0001:10134）；principal_kind_check 含 'service'（0001:1228）。
- 跨表 kind 校验职责归服务层/清扫路径，表 CHECK 不声称承担（F-06，:101）；API 无参数可指定清扫身份（:101③）。
- tenant_audit_log 单点 + 两事件 + 同事务边界：§11（:244-247）+ §9（:199）。

### 10 API 路由、读取权限与数据最小化 — **PASS**

- 五端点与指令清单逐一比对（:216-222）：POST 嵌套建 / POST /end / **GET /product-project-links/{link_id}（F-04，无双 id 历史路由残留）** / GET 产品侧 / GET 项目侧。
- 读按所属侧（任一侧满足，非写交集）：分离表 :214 + 详情行 :222。
- 最小字段：产品侧仅 code/name/status（:220）；项目侧仅 key/name/lifecycle（:221）；无 description 泄露（:224）。
- 无 DELETE 解除端点（:225）。

### 11 Team dissolve 与兼容迁移完整性 — **PASS**

- 清扫集合明确（§9 :192，F-08）；同事务协调（:199）；顺序无关归因（:197）；team_dissolved 保留（:86/:270）；DISPATCH 执行路径可验证（lease.rs:214 + 0001:10134 种子）。
- 实施前置明确：C-01（§19 :369）+ 部分唯一索引 + 触发器四禁 + 迁移号全分支扫号（§17 :345）。
- **未以数据近零豁免迁移风险评估**：R-03 状态风险与 C-01 前置独立在案（:316/:369），全文无「数据近零故免评估」表述（grep 核验）。

### 12 F-01~F-10 闭合与整体可实施性 — **PASS**

- 逐项回查（修订报告 §1 表 + 正文直读比对）：F-01（DR 头 :3/尾 :169）✅ · F-02（枚举两值，残留仅存「取消说明」3 处，grep 验证 :104/:197/DR:139）✅ · F-03（四禁 :92-99）✅ · F-04（路由 :222）✅ · F-05（矩阵 :229-238）✅ · F-06（:101）✅ · F-07（allow-set :218/:233/:316）✅ · F-08（:192）✅ · F-09（ERD ended_at :269）✅ · F-10（:163 无「先补授权」）✅。
- 全文扫描：无路由冲突、无守卫矛盾、ERD 与 §4 一致（ended_at 补齐 :269）、实施前置无遗漏、无未经裁决的新设计。

## 2. F-07 / F-04 专项验证（指令 §三.12）

| 项 | 验证 | 结论 |
|---|---|---|
| F-07 deny→allow 等价性 | 现役值集={planning, initializing, archived, deleted}（证据：DDL 默认 planning 0001:1257；条件迁移 →initializing `WHERE status='planning'` lifecycle.rs:269-270；→archived archive.rs:79-80；→deleted 仅 dissolve team_onboarding.rs:1269；无其他生产写入者）。两形态在现值集上外延等价；allow-list 对未来未知状态更严格（更安全方向）；指令驱动、已入 DR-05 修订记录（:118-121），决议记录（:122-123）未改动 | **无实质冲突，不阻断**——allow-list 修订随冻结生效 |
| F-04 读写分离保持 | 详情路由改 link_id 后：读=任一侧权限满足（:214/:222），写交集仅适用建立/解除（:210-211）；未把读误写为写交集 | **语义保持，PASS** |

## 3. 冻结前一致性修正（评审发现，均不改变 DR 语义）

| # | 修正 | 位置 |
|---|---|---|
| 1 | 设计 §4 实体结构措辞 PROPOSED→已裁决 ACCEPTED | §4 |
| 2 | 设计 §5 表头「推荐（PROPOSED）」→「裁决结论（ACCEPTED）」 | §5 |
| 3 | 设计 §16 引言「（Status=PROPOSED）」→「（Status=ACCEPTED）」（01 号核查项残留清除） | §16 |
| 4 | DR-02 决议记录「三禁守卫」计数标注「F-03 后为四禁，见补充 C」+ 设计 §16 同步 | DR-02/设计 §16 |

## 4. 冻结声明

- **冻结版本：** V1.0 FROZEN · **冻结日期：** 2026-11-17
- **冻结基准：** 冻结输入 = P4-01 V1.0 FROZEN @ `002c825c`（已入库）；P4-02 三文档为工作区未提交态，以 **SHA-256 锚点**固定冻结内容：
  - `P4-02-product-project-relationship-design-v1.0.md` = `3de9138441fc76ec7fd6293f28cd729cd0c3279bf52db3d0b06c8899a9e4917a`
  - `P4-02-decision-records-draft-v1.0.md` = `6a72f9dcd2c1cc0b4055a4fddc47fe77272c4a69229e142a01320de214a60979`
  - `P4-02-一致性修订报告-v1.0.md` = `4c3c5eaa84175591bc3b2689d8cdb3be89199a654af3180cd7b4fd17c1f1198c`
- **入库建议：** 三份 P4-02 文档 + 本评审记录一并提交（提交后以 commit 锚点取代哈希锚点为强锚）。
- **修订纪律：** 冻结后任何语义变更必须走补充 DR 或原 DR 正式修订，不得改设计正文绕过裁决。

## 5. 授权边界声明

**P4-02 Freeze ≠ 代码/Migration/数据库/API/部署实施授权。** 实施启动须经 Phase 4 门禁（Gate 3/4）与单独实施任务书。

**实施期登记项（累计移交）：**
1. dissolve_team 清扫纳入 core.product 置 dissolved（P4-01）。
2. 租户成员移除流程加 Product Owner 持有检查（P4-01 补充 A）。
3. dissolve_team 清扫纳入 core.product_project_link 置 ended/reason=team_dissolved（P4-02 DR-06）。
4. 迁移前置：core.project ADD CONSTRAINT UNIQUE(tenant_id,id)（C-01）。

## 6. 冻结后的下游锁定输入

| 切片 | 可使用的 P4-02 锁定输入 | 禁止越界 |
|---|---|---|
| P4-03 | 「一 Product ↔ N Project 正式关系存在且可查询」为认知关系底座 | 不得回改 P4-02；PC 归属上移须专项 Decision Record + 评审 |
| P4-04 | 「聚合实体链接 + ended 终态 + 行级历史」模式取舍结论；读写分离与数据最小化原则 | 不强制同构，按资产域特性裁决；KB 关联硬前置仍为 GAP-02 |
| P4-06 | 迁移前置清单（C-01 / 部分唯一索引 / 四禁触发器 / 清扫扩展） | 迁移号全分支扫号；不因数据近零豁免风险评估 |
| P4-07 | 多值关联展示（不得推断主产品）；创建时选产品=两调用串联；最小字段口径 | 不引入 primary 语义；展示口径含 ended 与否按 OQ-1 裁决 |
