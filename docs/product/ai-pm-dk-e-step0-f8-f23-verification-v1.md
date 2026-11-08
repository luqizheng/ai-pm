# DK-E 第 0 步：F8 ↔ F23 张力核实（V1.0，2026-11-09）

背景：设计稿 §8.2 场景 2 要求「更正 → `add_proposal(action=modify,
target_document=kb doc_key, base_revision_id=当前)`」；设计稿 §1 F8 又说
`discovery_proposal` 的三个对象引用全部指向 `ai.project_document` /
`ai.document_revision` / `ai.document_proposal`，而 F23 记录 checkpoint-1 的
KB digest 落在 `kb.document_revision`、PC 对象落在 `pc.*`。本文件按 DK-E
任务书「先核实后动码」逐项取证，给出处置结论。

## 一、结论

**走 (b) 路径**：`discovery_proposal` 的 FK 承载不了 kb/pc 目标，且目标域没有
`ai.project_document` 载体。因此本批：

- **不**为绕 FK 在 `ai.project_document` 造影子行；
- **不**自行迁移（零迁移）；
- 更正止步于 **findings 双行**（`contradicted` + `user_confirmed`）+ 会话
  `REVIEW_REQUIRED` + **「更正待知识应用」结构化标记**（复用既有
  `discovery_question` 面，`status='deferred'`，`finding_id` 指向 contradicted
  行）；
- KB 应用侧显式挂账 **KP-3**（k4 评审链为承接方）；PC 侧挂账 **KP-2**；
- F2 验收口径改为「**零提案行 + 标记可见**」（原口径「base_revision_id 指向当时
  KB revision」在现 schema 下不可满足）。

## 二、逐项核实

### 2.1 迁移实况：0025/0031/0053 已 squash 进 0001_baseline.sql

2026-11-09 冻结点 squash 后，`migrations/` 只有 `0001_baseline.sql` 与
`0002_conversation_discovery_link.sql`。原 0025（discovery 七表）、0031
（project_id/evidence 实体）、0053（KB/PC 最小面）的终态可在基线中逐表核对
（基线头注声明：pg_dump 终态 + schema/data diff 为零）。本文件引基线行号。

### 2.2 `discovery.discovery_proposal` 的 FK 清单（psql `\d` 实况）

```
Foreign-key constraints:
    discovery_proposal_tenant_id_base_revision_id_fkey
      FOREIGN KEY (tenant_id, base_revision_id) REFERENCES ai.document_revision(tenant_id, id)
    discovery_proposal_tenant_id_document_id_fkey
      FOREIGN KEY (tenant_id, document_id) REFERENCES ai.project_document(tenant_id, id)
    discovery_proposal_tenant_id_document_proposal_id_fkey
      FOREIGN KEY (tenant_id, document_proposal_id) REFERENCES ai.document_proposal(tenant_id, id)
    fk_discovery_proposal_tenant_project_session
      FOREIGN KEY (tenant_id, project_id, session_id)
      REFERENCES discovery.discovery_session(tenant_id, project_id, id) ON DELETE CASCADE
```

基线落点：`migrations/0001_baseline.sql:2423`（建表）、`:8938-8957`（四条 FK）；
`ai.project_document` = `:1083`，`ai.document_revision` = `:907`，
`ai.document_proposal` = `:872`。

**FK 目标是 ai 家族，不是 kb/pc。** 且 `base_revision_id` 是复合 FK
`(tenant_id, id) → ai.document_revision(tenant_id, id)`——把 `kb.document_revision.id`
填进去不可能通过约束。

### 2.3 KB 目标的真实载体

- checkpoint-1 仓库快照 digest：`INSERT INTO kb.document` +
  `INSERT INTO kb.document_revision`（`crates/pm-infrastructure/src/kb_pc_analysis.rs:418,606`）。
- 表定义：`kb.document` `0001_baseline.sql:2860`、`kb.document_revision` `:2878`。
- **kb schema 对 `ai.project_document` 零 FK**（基线中 `REFERENCES ai.*` 在
  kb/pc 建表段内零命中）。
- 反向镜像也不存在：没有任何代码把 `kb.document*` 镜像写进
  `ai.project_document`。`ai.project_document` 的写入方只有
  requirement 投影 / project-goal / 知识提案链（`requirement_projection.rs:182`、
  `project_goal.rs:419`、`knowledge_proposal/submit.rs`），doc_key 与 KB digest 的
  `repo-<repository_id>` 不同源。
- 唯一「过渡期 ai.document*」的表述在
  `crates/kb-pc-contracts/src/registry.rs:38`（目标态 kb.document*、过渡期
  ai.document*）——这是**通道注册表的历史口径**，不构成 checkpoint-1 digest 的
  载体：`kb_pc_analysis.rs` 已实写 kb.*，且 `kb.document_revision` 是
  `kb.analysis.completed` 事件的 `document_ref` 所指（`kb_pc_analysis.rs:515-535`）。

### 2.4 PC 目标的真实载体

- `pc.cognition_object` `0001_baseline.sql:3041`、`pc.cognition_revision` `:3064`、
  `pc.proposal` `:3240`。
- pc schema 对 `ai.project_document` 同样零 FK；`pc.proposal` 只引用 pc 自身对象。
- PC 侧确认端点（KP-2）未建，`pc.*` 在本批必须零写入（D-DK-10 / F24）。

### 2.5 `add_proposal` 的服务层校验（第二道门）

`crates/pm-application/src/discovery/proposal.rs:90-124`：

- `action='modify'` **强制**要求 `document_id` 与 `base_revision_id`，且
  `base_revision_id` 必须是 `document_id` 所属 `ai.project_document` 的
  `ai.document_revision` 行（JOIN 校验）；
- `action='add'` 若带 `document_id`，必须在 `ai.project_document` 且同租户。

即：即使绕过 DB FK，服务层也会拒绝 kb/pc 的 revision id。**「造影子
ai.project_document 行」是唯一能让 modify 通过的旁路**——任务书明令禁止。

### 2.6 结论性判定

| 候选 | 判定 | 依据 |
|---|---|---|
| (a) FK 可承载 kb/pc 目标 | **否** | 2.2~2.5：三方 FK 全在 ai 家族，服务层二次校验同向 |
| (a') 目标域有 ai.project_document 载体 | **否** | 2.3/2.4：kb/pc 与 ai.project_document 无 FK、无镜像写入；digest 的 `document_ref` 指向 `kb.document_revision` |
| (b) 止步 findings + 标记 + 挂账 | **是（采用）** | 零迁移、零影子行、pc.* 零写入 |
| (c) 需迁移才能兑现提案语义 | 不需要 | 标记面可复用 `discovery_question(deferred)`；KB/PC 应用本就不是本批范围 |

## 三、处置口径（本批实现）

1. **更正落库**：`add_finding(kind=contradicted, 指向旧 finding)` +
   `add_finding(kind=user_confirmed, 新事实)`；旧 finding 零删改（F1）。
   - 「指向旧 finding」的零迁移载体：contradicted 行的
     `discovery_finding_evidence` 一条 `system_observation`，
     `description='contradicts:<旧 finding uuid>'`；同时把用户更正原话作为
     `user_statement` 证据。不改 `actor_role`（该列是槽位标签的语义位）。
   - 两条新行 `verification_status` 均为 `PENDING`（`FindingCreator::AgentRelayed`）；
     人工裁决面 `record_finding_verification` 是唯一改状态的入口（F7）。
2. **会话**：本轮更正成立且无剩余阻塞问题 → `REVIEW_REQUIRED`（AI 可达三态之内，
   既有边）。
3. **结构化标记**：`discovery_question`，`status='deferred'`，
   `finding_id=contradicted 行`，正文前缀 `[更正待知识应用]`，内容含目标
   （KB/PC）、被否定事实摘要、新事实与两侧挂账（KP-3 / KP-2）。
   `deferred` 不阻塞跃迁（§4.3 补充规则），但复核面必须可见。
4. **零提案行**：本批不写 `discovery.discovery_proposal`（F2 验收改口径）。
5. **知识应用**：不属本批。KB 侧由 KP-3 的 k4 评审链接收；PC 侧等 KP-2。

## 四、偏差登记（已回写设计稿 §12.2）

- 设计稿 §8.2 时序第 4 步（`ORC->>PROP: add_proposal(action=modify,
  base_revision_id=当前)`）**在现 schema 下不可执行**，本批以 deferred 标记替代；
- F2 验收口径由「提案 `base_revision_id` 指向当时 KB revision」改为
  「**零提案行 + 标记可见 + KB 行零变化**」；
- 该偏差是**设计稿内部 F8 与 §8.2 的不一致**（不是实施缩水）：F8 已把
  `discovery_proposal` 定义为 ai.project_document 家族的知识文档提案，
  §8.2 却要求它承载 kb 文档 revision。修复方向（目标态）是 KB 提案链自身
  建 proposal 面（KP-3 承接），或未来把 `discovery_proposal` 的 FK 泛化为
  域引用（需要迁移，不属本批）。

## 五、对后续批次的影响

- **DK-F 演示**：场景 2 的「更正 → 回流」在 V0.5 内止步于「复核页可见的更正
  待办」。演示话术不得声称 KB/PC 已被更新。
- **KP-2 / KP-3**：两处挂账是显式的——KP-3 的评审链需要消费
  `discovery_question`（`[更正待知识应用]`）或直接消费 contradicted finding；
  KP-2 建 PC 提案面后，本标记可平移为正式提案。
