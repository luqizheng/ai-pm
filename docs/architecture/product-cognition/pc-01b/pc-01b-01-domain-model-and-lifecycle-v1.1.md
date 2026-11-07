# PC-01B — 领域模型与生命周期（V1.1 拆分件 01，原文档 §3~§4）

> **拆分声明**：本文件为《PC-01B 数据模型与数据库设计 V1.1》拆分件之一，内容为原文档 §3~§4，**零设计变更**；章节编号保持原文档编号。总纲/裁决/验收见拆分件 00（`pc-01b-00-overview-and-decisions-v1.1.md`）；拆分索引见该文件。

---

# 3. 领域模型图

## 3.1 核心领域模型（对象-关系-版本）

```mermaid
flowchart TB
    subgraph 类型注册〔平台级字典〕
        OT[pc.object_type<br/>14 类对象类型 + governance_level + singleton]
        RT[pc.relationship_type<br/>类型 + 方向性 + 端点合法性 + 治理等级]
    end

    subgraph 对象维度〔聚合 1〕
        O[pc.cognition_object<br/>稳定身份 + lifecycle_status + current_revision]
        OR[pc.cognition_revision<br/>append-only + Git 引用 + content_hash]
        O -- 1:N --> OR
        OT -.约束类型.-> O
    end

    subgraph 关系维度〔聚合 2〕
        REL[pc.relationship<br/>稳定关系身份 + 端点 + lifecycle]
        RR[pc.relationship_revision<br/>端点快照 + 语义/方向/属性 + content_hash]
        REL -- 1:N --> RR
        RT -.约束类型.-> REL
    end

    subgraph 版本层〔聚合 3〕
        V[pc.cognition_version<br/>version_no + snapshot_hash + predecessor + confirmed_by]
        VM[pc.version_member<br/>双维度成员，发布后不可变]
        V -- 1:N --> VM
    end

    VM -.复合 FK 固定.-> OR
    VM -.复合 FK 固定.-> RR
    OR -.端点复合 FK.-> O
    RR -.端点 Revision 快照复合 FK.-> OR
```

## 3.2 治理与追溯模型

```mermaid
flowchart TB
    subgraph 治理〔聚合 4/5〕
        P[pc.proposal<br/>提案 + proposal_content_hash + expected_version_no + L0-L2]
        PM[pc.proposal_member<br/>拟变更成员集（双维度 + change_kind）]
        AP[pc.approval<br/>不可变审批事实，锚定 proposal_content_hash + expected_version_no]
        CF[pc.conflict<br/>+ pc.conflict_object 冻结范围]
        P -- 1:N --> PM
        P -- 1:N --> AP
        CF -.L3 冻结阻断.-> AP
    end

    subgraph 追溯
        S[pc.source<br/>来源四元组 + 定位]
        EV[pc.evidence<br/>证据定位 + evidence_hash，钉 Revision]
        WO[pc.write_operation<br/>Git-PG 幂等账本三态]
        AL[governance.audit_log<br/>复用，source_kind='pc_*']
    end

    S -- 1:N --> EV
    EV -.object_revision_id / relationship_revision_id.-> OR2[pc.cognition_revision / pc.relationship_revision]
    S -.来源。KB/人工/需求。-> P
    P -- 发布事务 --> V3[pc.cognition_version]
    WO -.登记 Git 提交与 PG 产出.-> OR2
    AP -.发布锚点.-> V3
    AL -.记录.-> P
    AL -.记录.-> V3
```

## 3.3 对象类型体系（14 类种子，〔裁决〕基线 §3.2 六层）

| 层 | object_type | governance_level | singleton |
|---|---|---|---|
| L1 产品身份与方向 | `product_profile` | L1（初始化）/ 描述修改 L1 | true |
| L1 | `product_vision` | **L2** | true |
| L2 产品结构 | `business_domain` / `business_capability` / `feature_module` / `feature` | L1；废弃/合并 L2 | false |
| L3 使用与行为 | `actor` / `scenario` / `business_process` / `page` / `interaction` | L1；business_process 及状态转换 L2 | false |
| L4 业务语义 | `domain_object` / `business_rule` | L1 新增；**business_rule 全生命周期 L2** | false |
| L5 决策 | `product_decision` | L2 | false |

说明：governance_level 是**对象类型的缺省治理等级**，单次变更可按影响上浮（服务层策略）；`business_rule`、`product_vision`、`business_process` 固定 L2 不可下调（〔裁决〕基线 §9.4 L2 范围）。singleton = 每项目至多一个非废弃对象（部分唯一索引承载，§5.3）。

---

# 4. 聚合与生命周期

## 4.1 五个聚合（〔裁决〕01A §6.3，本节落为库层边界）

| 聚合 | 聚合根 | 成员 | 库层一致性边界（本设计落实） |
|---|---|---|---|
| 1 认知对象 | pc.cognition_object | 其 cognition_revision 全集 | Revision 复合 FK `(tenant_id, project_id, object_id)`；append-only 触发器 |
| 2 认知关系 | pc.relationship | 其 relationship_revision 全集 | Revision 复合 FK；端点快照复合 FK（PC 内端点）；append-only 触发器 |
| 3 正式版本 | pc.cognition_version | version_member 全集 | Member 四复合 FK + 双维度互斥 CHECK + 部分唯一；发布后全行不可变触发器 |
| 4 提案 | pc.proposal | proposal_member 全集 + approval 历史 | Member 目标复合 FK（修复 m-3 型缺陷）；approval 锚定 hash + 单向状态 |
| 5 冲突 | pc.conflict | conflict_object 冻结范围集 | 冻结范围复合 FK；发布/确认事务前置校验 |

## 4.2 生命周期状态机

**Cognition Object / Relationship（身份层，同构）**：

```text
identity ──(首次 Revision 进入提案/草稿)──> draft ──(首次进入正式版本)──> active
active ──(L2 正式废弃)──> deprecated
active ──(L2 正式合并/替代)──> superseded（superseded_by_id 指向承接对象）
```

- `identity`：初始化占位（Profile/Vision 由项目初始化自动建），尚无 Revision。
- `draft`：有 Revision 草稿，未进入任何正式版本。
- `active`：已进入至少一个正式版本。
- `deprecated`/`superseded`：终态，只能由 L2 提案经审批达成（安全闸写路径）；不删除行、不级联（〔裁决〕基线 §13.4）。
- CHECK：`lifecycle_status IN ('identity','draft','active','deprecated','superseded')`。
- 终态转换规则（库层约束触发器）：`active → deprecated|superseded` 合法；`deprecated/superseded` 不可回到 active（复活 = 新对象或新提案，历史不可改写）。

**Revision 层（对象与关系同构）**：append-only，无状态字段——「是否已进入正式版本」由 version_member 存在性派生（§5.2 设计裁决：不冗余存储 revision_status，避免双写漂移）。

**Cognition Version**：创建即发布（发布事务原子写入 version + member + audit，§18）；`status CHECK ('published','archived')`，V1 只写 `published`，`archived` 为账本语义预留（成员与锚定列仍不可变）。

**Proposal**：

```text
draft ──submit──> submitted ──approve──> approved ──publish(发布事务)──> applied
                     │                    └─publish 失败─> failed
                     ├──reject──> rejected
                     ├──withdraw──> withdrawn
                     └──expire──> expired
```

单向触发器承载（§10.2）；`rejected/withdrawn/expired` 为终态（V1 重新发起 = 新提案）。

**Approval**：`pending → approved | rejected`（单向，0026:285-291 范式），行级锚定列 once-written（§10.3）。

**Conflict**：`detected → under_review → awaiting_human → resolved | dismissed`；`resolved/dismissed → reopened` 允许（复核发现误判）。`detected/under_review/awaiting_human/reopened` 视为**未解决**，冻结生效（§18.1 步骤⑥）。

**pc.write_operation**：`pending → git_committed → completed`；异常分支 `pending|git_committed → failed → completed(恢复后) | abandoned`（§12.3）。