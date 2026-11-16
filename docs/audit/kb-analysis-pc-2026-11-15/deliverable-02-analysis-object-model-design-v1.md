# 交付物 02：Analysis 对象模型详细设计 V1.0

> **状态**：设计基线（待专家评审；不含 DDL，不授权 Migration）
> **日期**：2026-11-16 · **代码快照**：master `dfc7b7f4`
> **上游真源**（均在 `docs/audit/kb-analysis-pc-2026-11-15/`）：
> `architecture-ruling-v1.md`（Q1~Q12）· `four-domain-object-model-v1.md`（下称「对象模型」）·
> `four-domain-registry-mapping-audit-taskbook-v1.md`（下称「映射任务书」A-01~A-04）·
> `03_analysis_domain_audit.md`（现状三链三模型）· `05_cross_domain_relationships.md`（10 项交叉关系）·
> `phase-d-g3-g4-verdict.md`（G3 真实链证据）
> **边界**：本任务只产出设计；不改业务代码/Schema、不写 DDL、不执行 Migration。
> **纪律**：Q1~Q12 与三条不可变更边界为硬约束；O-01~O-03 只给建议、不自行定终态。

---

## 0. 设计结论速览

| 设计点 | 结论 | 上游依据 |
|---|---|---|
| Run 身份 | **统一 Run = `analysis.run` 业务头 + `analysis.run_registry` 映射**；一个 Run 对应 0..N 条底层执行记录；**不加 `repo_analysis_report.analysis_run_id`** | Q1、A-01、对象模型 §4.1/§10 |
| Report | `integration.repo_analysis_report` **原地演进**为稳定业务身份；不绑定单一 Run；保留 `intake_id`（Q1=B） | Q1、Q2、A-02 |
| Report Revision | 新对象 `analysis.report_revision`：**正文锚 Git Commit**、DB 只存身份/状态/引用；`source_commit_sha`（源码提交）与 `content_commit_sha`（正文提交）分离 | Q2、A-02、对象模型 §5.3 |
| 历史内容 | `report.kb_revision_id` **降级为 Compatibility Ref**（`content_authority='kb_legacy'`），不再承担正文权威 | Q2、映射任务书 §8.1 |
| Finding/Conclusion/Recommendation | 新载体 `analysis.result_item`（一个身份表、三种 `item_kind`）；Finding 通过 Finding Contract 对外表达；`cognition_item` 归 Project Cognition，不充当 Analysis Finding | Q3、Q5、对象模型 §4.2 |
| 开放项 | O-01/O-02/O-03 只给推荐方案与备选，标注「建议待裁决」 | 对象模型 §13.1 |
| Migration 输入 | **7 项 Schema Gap + 2 项 Contract Gap**（另 1 项禁止项 M-09）；按 A-04 四分口径逐项标注 | 映射任务书 §5/§7 |

---

## 1. 现状与硬约束

### 1.1 现状：三条链、三种执行/结果模型

| 链 | 执行载体 | 报告载体 | 结果载体 | 正文/结果位置 |
|---|---|---|---|---|
| ① 仓库再分析 | `integration.intake_task`（kb_analyze / pc_analyze）+ 报告状态推进 | `integration.repo_analysis_report` | PC 提案 + KB 草稿（无 Finding 实体） | `kb.document_revision.content` |
| ② 冷启动导入 | `ai.analysis_run`（`repo_import.analysis_run_id`） | 无 | `ai.change_proposal` + `result jsonb` 内嵌 findings | PG jsonb |
| ③ 任务执行/评审 | `delivery.task_run` | 无 | `delivery/design.review_finding` | 评审产物行 |

现状证据：
- Run 双写入者：`crates/pm-ai/src/context.rs:67,153-161`（创建 A）与
  `crates/pm-application/src/project_import/proposal.rs:312-324`（创建 B）；
  `ai.analysis_run` 列见 `migrations/0001_baseline.sql:675-692`。
- 仓库链无 Run 实体：报告仅 `intake_id` 间接关联；`repo_analysis_report` 列见
  `migrations/0023_repo_analysis_report.sql:16-35`，run_mode/since_commit 见
  `migrations/0024_analysis_report_run_mode.sql:13-16`。
- 报告正文借住 KB：`crates/pm-application/src/analysis_report.rs:9-13,318-332`；
  `kb_revision_id` / `pc_source_id` 为裸 uuid（0023 无 FK）。
- 报告门 409：`crates/pm-application/src/repository/repo.rs:813-825`；
  run_mode 归一化（incremental 无锚点退 full）：`repo.rs:784-800`。
- Finding 内嵌：`crates/pm-domain/src/proposal.rs:36-50`（Finding/AnalysisResult）。
- `intake_task` / `repo_import`：`migrations/0001_baseline.sql:2639-2659` / `:2754-2775`；
  `repo_import.analysis_run_id` FK 见 `:9238`。
- 三知识对象互不外键：`ai.cognition_item`→`ai.project_knowledge`（`repo_cognition.rs:272-283`）；
  `kb.*` 与 `pc.*` 独立。

上游依据：03 分册 §0/§7、映射任务书 §3.2、Q1/Q2/Q3。

### 1.2 硬约束（不得突破）

1. **Q1=B**：Report 与 Run 的正式关联继续经 `intake_id`；**禁止**给
   `repo_analysis_report` 加 `analysis_run_id`（`architecture-ruling-v1.md:19-23`）。
2. **Q2=B+C**：Report 与 Report Revision 归 Analysis；正文 **Git 权威**；DB 只存
   身份/状态/Git 引用；Report Revision 不依附 KB Document Revision
   （`architecture-ruling-v1.md:25-30`）。
3. **Q3=C**：`ai.project_knowledge` / `kb.memory` / `pc.cognition_object` 不合并；
   `cognition_item` 归 Project Cognition，不迁 `kb.memory`、不充当 Analysis Finding。
4. 三条不可变更边界：不合并三知识对象；公共合约≠公共表；禁止 Git/PG 长期双正文权威。
5. O-01~O-03 **保持开放**：本设计只给推荐方案，不得表述为已裁决。

---

## 2. 五身份分立

上游依据：Q2、A-02、对象模型 §2.2.3/§5.3。

| 身份 | 载体（设计后） | 回答的问题 | 不得混同 |
|---|---|---|---|
| **Run** | `analysis.run`（业务头）+ `analysis.run_registry`（执行映射） | 哪一次**执行活动** | 不是 Intake Task、不是 Report |
| **Report** | `integration.repo_analysis_report`（演进） | 哪一份**分析结果**（稳定业务身份） | 不绑定单一 Run |
| **Report Revision** | `analysis.report_revision` | 报告的哪一**内容版本** | 不是 KB Document Revision |
| **Git Commit** | `content_commit_sha + content_path + content_hash` | 正文的**不可变锚点** | 不是 DB 行版本 |
| **Compatibility Ref** | `compat_kb_revision_id`（只读） | 历史正文曾借住哪里 | 不再承担长期权威 |

补充：**Source Commit ≠ Content Commit**（专家 2026-11-15 更正）。`source_commit_sha`
记录“分析了哪个源码提交”；`content_commit_sha` 记录“报告正文在哪个 Git 提交”。
两者必须分列，禁止用一个列表达两种语义。

---

## 3. Run Contract + A-01 映射注册表

上游依据：Q1、对象模型 §4.1/§10/§10.1、映射任务书 A-01、03 分册 §0/§1。

### 3.1 Run Contract（对外统一视图）

```
RunContract:
  run_id            // Analysis 域生成的统一执行活动身份
  run_type          // repository_analysis | cold_start_import | task_review | ...
  owning_domain     // 默认 analysis；评审链可声明 delivery
  execution_source  // 见 §3.2 枚举
  subject_refs[]    // 项目/仓库/需求等
  input_refs[]      // commit / since_commit / intake / 文档引用
  output_refs[]     // report / revision / proposal / version 引用
  initiated_by      // 真人 / Agent / Service / System
  status            // queued | running | succeeded | failed | cancelled | blocked(建议)
  started_at? / completed_at?
```

规则（对象模型 §4.1）：Run 代表执行活动，不代表业务工作流；Intake Task 不因接入消失；
Report 不因接入变成执行实体；各领域保留自己的 Run 实现，经 Adapter 映射。

### 3.2 A-01 映射注册表

**设计：`analysis.run`（头）+ `analysis.run_registry`（多执行记录）**。

```
analysis.run
  run_id uuid PK
  tenant_id / project_id
  run_type text NOT NULL
  owning_domain text NOT NULL DEFAULT 'analysis'
  idempotency_key text            -- 逻辑触发键；唯一（project_id, idempotency_key）
  initiated_by uuid NOT NULL
  status_internal text            -- 领域状态快照
  status_contract text            -- RunContract 映射结果
  subject_refs jsonb NOT NULL DEFAULT '[]'
  input_refs   jsonb NOT NULL DEFAULT '[]'
  output_refs  jsonb NOT NULL DEFAULT '[]'
  started_at / completed_at / created_at / updated_at

analysis.run_registry
  id uuid PK
  run_id uuid NOT NULL REFERENCES analysis.run(run_id)
  tenant_id / project_id
  execution_source text NOT NULL  -- analysis_run | repo_import | intake_task | delivery_run | external
  source_record_id uuid NOT NULL  -- 底层实体主键
  source_record_kind text         -- kb_analyze / pc_analyze / import / ai_run / task_run
  attempt int NOT NULL DEFAULT 0
  status_internal text NOT NULL
  status_contract text NOT NULL
  failure_code text
  error_detail text
  started_at / completed_at / updated_at
  UNIQUE (execution_source, source_record_id)
```

设计要点：

| 点 | 设计 |
|---|---|
| 一对多 | 一个 `run_id` 对应多条 registry 行（如仓库链 kb_analyze + pc_analyze；导入链 repo_import + ai.analysis_run） |
| 身份唯一 | `(execution_source, source_record_id)` 唯一：一条底层执行记录只归属一个统一 Run，防止重复归属 |
| 幂等键 | `analysis.run.idempotency_key` 承接现有 `repo_analyze:{repository_id}:{commit}:{trigger_token}`（`repository/repo.rs:832`）；同键重放返回同一 run_id，不新增 registry 行 |
| 重试 | 同 run_id、同 source_record_id，`attempt += 1`；**不新建 Run**；耗尽按注册表的失败策略落 `failed` |
| 终态映射 | 见 §3.3；由 Adapter 把内部状态映射为 `status_contract`，不反向改底层状态机 |
| 不侵入 | 不改 `ai.analysis_run` / `repo_import` / `intake_task` 的既有状态机；registry 只读它们的状态并映射 |
| Q1 维持 | `repo_analysis_report` **不加** `analysis_run_id`；Report 与 Run 经 Revision.produced_by_run_id 与 registry 间接关联 |

### 3.3 终态映射表

| RunContract | 仓库链（intake_task） | 导入链（repo_import / ai.analysis_run） | 评审链（task_run） |
|---|---|---|---|
| queued | PENDING | queued / queued | queued |
| running | DISPATCHED / PROCESSING | cloning / reading / analyzing / analyzing | claimed / running |
| succeeded | COMPLETED | draft_ready → proposed → applied / succeeded | succeeded |
| failed | FAILED | failed | failed |
| cancelled | CANCELLED | cancelled | cancelled |
| blocked（建议） | NEEDS_HUMAN_REVIEW | proposed（待人工审批） | 待人工评审 |

说明：`blocked` 是否进入公共 RunContract 枚举**建议待裁决**（可退化为 running + 领域子状态）。

### 3.4 三个底层实体适配

| execution_source | source_record_id | Adapter 读取 | 产出映射 |
|---|---|---|---|
| `analysis_run` | `ai.analysis_run.id` | status/result/attempt_count/failure_code | result jsonb → 兼容 Finding（只读）；新 Finding 由 `analysis.result_item` 承载 |
| `repo_import` | `integration.repo_import.id` | status 10 值（`migrations/0001_baseline.sql:2774`） | draft 版本 → 兼容输出；提案链 → output_refs |
| `intake_task` | `integration.intake_task.id` | task_status / result_ref（`:2639-2659`） | kb_analyze→KB revision；pc_analyze→PC proposal/version |

---

## 4. Report 与 Report Revision

上游依据：Q1/Q2、对象模型 §2.2.3/§5.2/§5.3、映射任务书 A-02/§8.1、专家 F-01。

### 4.1 Report（稳定业务身份）

**演进路径：`integration.repo_analysis_report` 原地演进，不新建平行 Report 表。**

保留：`id`（Report 身份）、`tenant_id/project_id/repository_id/commit_sha/intake_id/status/
stage_detail/error_code/error_detail/closed_at/closed_by/run_mode/since_commit`。
新增（P2）：`current_revision_id uuid NULL`（指向当前 Revision，允许先建 Report 后建 Revision）。
语义降级：`kb_revision_id` 保留列但标记 **Compatibility Ref**（只读，不再作为正文权威）；
`pc_source_id` 保留为跨域输出引用。

**硬规则**：
- Report 不绑定单一 Run（Q2/A-02）；`commit_sha` 是“本次分析锚定的源码提交”，
  不是“报告正文提交”，后者只存在于 Revision。
- 报告门/close/真人门语义不变（`analysis_report.rs:387-551`；`repo.rs:813-825`）。
- 重新分析默认**新建 Report**（O-02 推荐，见 §8）。

### 4.2 Report Revision（内容版本身份）

```
analysis.report_revision
  id uuid PK
  tenant_id / project_id
  report_id uuid NOT NULL REFERENCES integration.repo_analysis_report(id)
  revision_no int NOT NULL
  content_authority text NOT NULL        -- git | kb_legacy
  content_commit_sha text                -- git 必填；kb_legacy 为空
  content_path text                      -- canonical path（如 analysis/reports/{report_id}/report.md）
  content_hash text NOT NULL             -- sha256(正文)
  git_ref text                           -- 可选：branch/tag
  source_commit_sha text                 -- 本次分析的源码提交（与 content 分列）
  status text NOT NULL                   -- draft | published | superseded | rejected
  produced_by_run_id uuid NULL REFERENCES analysis.run(run_id)
  compat_kb_document_id uuid NULL
  compat_kb_revision_id uuid NULL
  supersedes_revision_id uuid NULL
  change_reason text
  created_by uuid NOT NULL
  created_at / published_at
  published_by uuid NULL
  UNIQUE (report_id, revision_no)
```

**正文权威与恢复**：
- 新 Revision 必须 `content_authority='git'`，`content_commit_sha` + `content_path` 定位，
  `content_hash` 校验；DB 不存正文。
- 多 Revision 可引用**同一 Git Commit**：仅当 `content_path`+`content_hash` 一致（纯元数据
  变化）；正文恢复算法 = 按 commit 取 path → 校验 hash → 失败即 fail-closed。
- 同一 Commit 下正文不同（路径不同/哈希不同）不得声明为同一 Revision 内容。

**Compatibility Ref（历史）**：
- 迁移时为既有 Report 生成一条 `content_authority='kb_legacy'` 的兼容 Revision：
  `compat_kb_revision_id = 旧 report.kb_revision_id`，`content_commit_sha=NULL`，
  `content_hash = kb.document_revision.content_hash`（`0001_baseline.sql:2878-2894`）。
- 兼容 Revision 只读、不得再发布新修订；新 Revision 一律 Git。
- `report.kb_revision_id` 列保留但不再写入新值（写入即违约）；新链路只写 Revision。

**与 KB Document Revision 的边界**：
- Report Revision 不依附 KB Document Revision（Q2）；`compat_kb_revision_id` 只是历史指针。
- KB 草稿/发布仍归 KB：仓库链当前先写 `kb.document_revision`（`kb_pc_analysis.rs:495-553`），
  P2 文本迁移后 Report Revision 是 Analysis 权威，KB 只保留“知识建议”的独立对象。

### 4.3 发布与人工决策

| 动作 | 主体 | 语义 |
|---|---|---|
| Revision draft→published | 交互式真人（复用 Human Decision Contract） | 发布报告版本；审计 `decided_by` |
| Report close | 交互式真人 | 已有：批量发布 approved PC 提案、未决 withdrawn、KB 草稿 rejected（`analysis_report.rs:416-501`） |
| Report 修订 | 真人 | 同 Report 新 Revision（不改 Report 身份） |
| 重新分析 | 真人/Agent 触发 | 新 Run + 默认新 Report（O-02） |

---

## 5. Analysis Finding / Conclusion / Recommendation

上游依据：Q3/Q5、对象模型 §4.2/§10、03 分册 §4、映射任务书 §3.2。

### 5.1 Finding Contract 在 Analysis 侧的落地

公共契约（对象模型 §4.2）只统一外部表达/身份/证据引用，不统一内部状态机。
Analysis 侧实现为 `analysis.result_item`，`item_kind='finding'` 时适配
`FindingContract.finding_id = item_id`。

```
analysis.result_item
  id uuid PK                          -- 对象身份（Finding/Conclusion/Recommendation 共用）
  tenant_id / project_id
  report_id uuid NULL                 -- O-03：建议必填，保留 NULL 备选
  revision_id uuid NULL               -- O-03：可绑定具体 Report Revision
  item_kind text NOT NULL             -- finding | conclusion | recommendation
  finding_type text NULL              -- finding 专用（如 requirement/risk/consistency）
  severity text NULL                  -- info | low | medium | high | critical
  status text NOT NULL                -- proposed | accepted | rejected | obsolete | superseded（建议）
  title text NOT NULL
  content text NOT NULL               -- 结构化文本 / Markdown 片段（非报告正文）
  structured jsonb NOT NULL DEFAULT '{}'
  source_refs jsonb NOT NULL DEFAULT '[]'    -- ObjectReference 数组
  evidence_refs jsonb NOT NULL DEFAULT '[]'  -- EvidenceReference 数组（O-04 未裁决前用引用结构）
  subject_refs jsonb NOT NULL DEFAULT '[]'
  target_ref jsonb NULL               -- Recommendation → PC proposal/version 等
  created_by_run_id uuid NULL REFERENCES analysis.run(run_id)
  created_at / updated_at
```

设计说明：
- **一个身份表、三种 kind**：对象身份分离（finding_id/conclusion_id/recommendation_id
  都可映射到 item_id），但存储不必三张同构表（“公共合约≠公共表”）。
- 读取面按 `item_kind` 投影为三类对象；写入面要求 `finding_type/severity` 仅 finding 使用。
- `evidence_refs` 在 O-04 裁决前使用 EvidenceReference JSON，不新建证据表。

### 5.2 与邻近对象的边界

| 对象 | 归属 | 与 Analysis Finding 的关系 |
|---|---|---|
| `ai.cognition_item` | **Project Cognition**（Q3=C） | 项目技术认知候选；**不充当** Analysis Finding，不迁 `kb.memory` |
| `discovery.discovery_finding` | Discovery / Project Cognition 相关业务 | 独立生命周期；仅通过 FindingContract 对外适配，**不合并表** |
| `delivery.review_finding` / `design.review_finding` | Delivery / Design | 领域独立；Analysis 不接管评审状态机 |
| `pc.proposal` / `pc.cognition_revision` | PC | 分析产物落 PC 提案；Analysis Recommendation 通过 `target_ref` 引用，不复制 PC 对象 |
| `ai.analysis_run.result.findings` | 旧 Analysis 内嵌 | Migration 时作为兼容来源；新链只写 `analysis.result_item`（禁止双写长期并存） |

### 5.3 与 KB/PC 的转化路径（Q3/Q5 边界）

- Analysis Finding → **PC Proposal**：经 `pc.proposal(origin_kind='ai_agent')` + 人工审批
  （现状 `kb_pc_analysis.rs:815-1027` → `proposal.rs:1401-1541`）。
- Analysis Finding → **Project Cognition**：只能作为 Evidence/Proposal 输入，
  经人工确认后成为 `ai.project_knowledge`；禁止直接写正式认知。
- Analysis Finding → **KB Memory**：必须经独立知识沉淀流程；禁止自动转 Memory。

---

## 6. 与 G3 真实链路的兼容映射

上游依据：`phase-d-g3-g4-verdict.md` §3.1、Q1/Q2、A-02。

G3 实际对象（2026-11-16，`test-case/evidence/chain/e1-object-identity.json`）：

| G3 对象 | 真实 id | 迁移后映射 |
|---|---|---|
| Report | `f617c31f-68f6-42f8-b675-b4601bb2eeab` | Report 身份不变（`repo_analysis_report.id`）；新增 `current_revision_id` |
| Intake | `1d2e901f-9c75-40cc-8a04-f951159c8aca` | 不变；`analysis.run_registry` 以 intake_task 行登记（一个 Run ↔ kb_analyze/pc_analyze 两条） |
| KB Revision | `89cedecb-24e1-4e4e-8769-e0b66a1f91d5` | 降级为 `report_revision.compat_kb_revision_id`（`kb_legacy`） |
| Proposal | `c4e4e9e9-6f99-4101-adfe-4a05f4de0998` | PC 域不变；Analysis 以 `result_item.target_ref` 引用 |
| Version | `5167eefa-1712-4cbe-be60-8a5db4038994` | PC 域不变；Run `output_refs` 登记 |
| 源码 commit | `c2aee36e28cbc0ec527769c39f9d263c14062fbb` | `report.commit_sha` 与 Revision.`source_commit_sha` |
| 正文 | 现为 `kb.document_revision.content` | 迁移后：历史兼容 Revision；新报告写 Report Content Git |

**Report 与 Report Revision 不得混同**：
- Report 身份 = `repo_analysis_report` 行；Revision = 内容版本对象。
- G3 的 `kb_revision_id` 不是 Report Revision，只是 Compatibility Ref；E2E 追踪键仍为
  `report_id + intake_id`（交付物 05 §4.2）。

### 6.1 对照 05 分册 10 项交叉关系

| # | 05 分册结论 | 本设计的处理 | 是否改变结论 |
|---|---|---|---|
| 1 | KB Source→Document 部分实现 | 不变（不在 Analysis 范围） | 否 |
| 2 | Document→Knowledge 未实现 | 不变（不新增提取管道） | 否 |
| 3 | KB Knowledge→Report（方向相反：kb_revision 作正文） | `kb_revision_id` 降级 Compatibility Ref；新正文为 Report Revision 的 Git Commit | **是（Q2 目标态）** |
| 4 | PC Cognition→Report 未实现 | 不变（不新增 PC→Analysis 输入） | 否 |
| 5 | Run→Report 部分实现（intake 间接） | 保留 `intake_id` 间接；新增 registry 显式映射；**不加 analysis_run_id** | 否（强化） |
| 6 | Report→Finding 部分实现（经 PC 反查） | 新增 `analysis.result_item(report_id)`；PC 反查保留兼容 | **是（新增 Finding 身份）** |
| 7 | Finding→PC Candidate 已实现 | 保留（`pc.proposal origin_kind=ai_agent`） | 否 |
| 8 | PC Candidate→PC Cognition 已实现 | 不变 | 否 |
| 9 | Finding→KB Knowledge 部分实现 | 保留 KB draft/confirm 真人门；P2 后 Report Revision 与 KB 知识建议分离 | 否（边界更清晰） |
| 10 | Report→PC Revision 部分实现（pc.source 钉 KB 修订） | 兼容保留现状；目标态改钉 Report Revision 属 03 契约/PC 域设计，本设计不改 `pc.*` | 否（标注目标态） |

结论：本设计只改变 #3（正文权威）与 #6（Finding 身份）两处，均已由 Q2/Q5 裁决覆盖；
其余 8 项维持现状边界，不新增跨域所有权。

**零数据假设**：设计不依赖“当前无数据”。迁移时必须支持：
1. 既有 Report 逐行生成一条兼容 Revision；
2. 既有 intake_task 逐行重建 registry（execution_source='intake_task'）；
3. 既有 `analysis_run.result.findings` 可选择性生成 `analysis.result_item`（或保留只读兼容，
   由 06 决定，不双写）。

---

## 7. Migration 需求清单（交付物 06 输入，不含 DDL）

上游依据：映射任务书 §5/§7、A-04 四分口径（Schema / Contract / Implementation / Verification Gap）。

| # | 对象/变更 | 变更类型 | 依赖裁决 | A-04 差异 | 阻塞 06？ |
|---|---|---|---|---|---|
| M-01 | 新 schema/命名空间（建议 `analysis`） | 新增 schema | 设计决策（物理落点） | Schema Gap | 是（先定名） |
| M-02 | `analysis.run`（统一 Run 头） | 新增表 | A-01、O-01 | Schema Gap | 是 |
| M-03 | `analysis.run_registry`（执行映射） | 新增表 | A-01、O-01 | Schema Gap | 是 |
| M-04 | `analysis.report_revision` | 新增表 | Q2、A-02、O-02、O-09 | Schema Gap | 是 |
| M-05 | `repo_analysis_report` + `current_revision_id` | 新增列 | Q2 | Schema Gap | 是 |
| M-06 | `report.kb_revision_id` 降级兼容引用 | 语义变更（无列变更） | Q2、§8.1 | Contract Gap | 否（文档/契约先） |
| M-07 | `analysis.result_item`（Finding/Conclusion/Recommendation） | 新增表 | Q5、O-03、O-04 | Schema Gap | 是 |
| M-08 | 索引/约束：run 幂等键、registry 唯一、revision_no 唯一 | 新增约束 | A-01/Q2 | Schema Gap | 是 |
| M-09 | `repo_analysis_report.analysis_run_id` | **禁止新增** | Q1=B | — | 否（阻止项） |
| M-10 | 旧 `ai.analysis_run.result` jsonb | 只读兼容（不双写） | Q1/Q5 | Contract Gap | 否（实施策略） |
| M-11 | `pc.source` / `kb` 兼容引用 | 不改结构 | Q3/三条边界 | — | 否 |

明确不做：不改 `ai.analysis_run`/`repo_import`/`intake_task` 状态机；不改 `pc.*`/`kb.*`
现结构；不删 `kb_revision_id`；不做 Git/PG 双正文权威；不新增 `analysis_run_id`。

---

## 8. 开放项设计建议（建议待裁决，不得自定终态）

上游依据：对象模型 §10.1/§13.1 O-01~O-03。

### 8.1 O-01 Run ↔ Intake Task 基数

| 方案 | 基数 | 优点 | 风险 |
|---|---|---|---|
| **推荐 A** | Run 1:N Intake Task；Intake Task 0..1 Run | 一个 Run 可聚合 kb_analyze/pc_analyze 两任务；任务可先于 Run 存在 | 需在 registry 唯一约束里表达“一任务一 Run” |
| B | Intake Task 1:N Run | 支持同一导入多次执行 | 与“一个 Run 聚合多任务”冲突，易重复计数 |
| C | 允许脱离 | 最大灵活 | 审计断链，Run 无法解释来源 |

**建议 A（待裁决）**：registry 的 `UNIQUE(execution_source, source_record_id)` 已保证
“一条底层记录只归属一个 Run”；Run→Task 为 1:N；Task→Run 为 0..1。

### 8.2 O-02 重新分析轮次规则

| 操作 | 推荐方案 | 备选 |
|---|---|---|
| 重新分析（新 Run） | **新 Report**（旧 Report 保持 closed，历史可追溯） | 同 Report 新轮次（需 `round_no`，报告门语义变更） |
| 修订原报告（措辞/格式） | 同 Report 新 Revision | — |
| 修正分析结果 | 新 Run + 新 Report，旧报告 superseded 关系由 Revision/Report 链接表达 | 同 Report 新 Revision（会混淆执行与内容） |

**建议“新分析=新 Report；同 Report 只做内容修订”（待裁决）**。理由：Report 是业务结果
身份，不绑定单一 Run；沿用同 Report 会让“报告身份”与“仓库时间线”耦合，并改变
报告门/close 语义（G3 已验证的门不建议在 P2 改动）。

### 8.3 O-03 Finding 生命周期与版本绑定

| 维度 | 推荐 | 备选 |
|---|---|---|
| 是否必须挂 Report | `report_id NOT NULL`（Finding 属于一次分析结果） | 允许独立 Finding（跨报告） |
| 是否绑定 Revision | `revision_id NULL`：产生该发现的 Run 有 Revision 则绑定，否则报告级 | 强制绑定具体 Revision |
| 状态机 | proposed → accepted / rejected / obsolete / superseded | 仅 created（无状态） |
| 是否版本化 | 不单独版本化；变更以新 item + supersedes 表达 | 独立 revision 表 |

**建议“挂 Report、Revision 可空、状态四值”（待裁决）**。Finding 的“独立生命周期”
指状态可独立演进，不要求脱离 Report；这样既满足 Finding Contract，又不制造第四套
知识对象。

---

## 9. 自检表

| 检查 | 结果 | 依据 |
|---|---|---|
| 每节标注上游依据 | ✅ §0~§8 均含“上游依据” | 本文件 |
| 不违反 Q1（不加 analysis_run_id） | ✅ §3.2/§7 M-09 | `architecture-ruling-v1.md:19-23` |
| 不违反 Q2（Report+Revision、Git 正文） | ✅ §4/§6 | `:25-30` |
| 不违反 Q3（三知识对象不合并） | ✅ §5.2/§5.3 | `:32-38` |
| 三条不可变更边界 | ✅ 未合并知识对象；合约≠表；单正文权威 | 对象模型 §0.2/§16 |
| O-01~O-03 未自定终态 | ✅ §8 全部标注“建议待裁决” | 对象模型 §13.1 |
| 与 05 分册 10 项交叉关系一致 | ✅ 保留 KB/PC/Analysis 现状边界；不新增跨域所有权 | 05 分册 §1~§10 |
| 引用代码先 grep 实证 | ✅ 所有路径:行号均来自本仓 | — |
| 兼容 G3 真实 id | ✅ §6 逐项映射 | `phase-d-g3-g4-verdict.md` §3.1 |
| 不依赖零数据假设 | ✅ §6 给出兼容重建规则 | 本文件 |
| 不含 DDL/Migration 执行 | ✅ §7 仅列需求 | 任务边界 |

### 9.1 已裁决 vs 待裁决

**已裁决（设计必须遵守）**

| 编号 | 内容 | 本设计落点 |
|---|---|---|
| Q1=B | Intake 关联，不加 analysis_run_id | §3.2、§7 M-09 |
| Q2=B+C | Report+Revision 独立，Git 正文 | §4、§6 |
| Q3=C | 三知识对象不合并 | §5.2/§5.3 |
| Q5 | Finding 领域独立 + 公共契约 | §5 |
| Q12 | 先 E2E 后重构 | 全文（G3 证据为基线） |
| A-01 | Run↔执行实体映射注册表；禁止 analysis_run_id | §3.2 |
| A-02 | Revision 可关联 Run；Report 不绑定 Run；五身份分立 | §2、§4 |
| A-04 | 差异四分口径 | §7 |
| 三边界 | 不合知识对象 / 合约≠表 / 禁双正文 | §0、§9 |

**待裁决（本文件只给建议）**

| 编号 | 待裁决问题 | 本文件建议 | 落点 |
|---|---|---|---|
| O-01 | Run ↔ Intake Task 基数 | 推荐 A：Run 1:N Task，Task 0..1 Run | §8.1 |
| O-02 | 重新分析轮次规则 | 新分析=新 Report；同 Report 仅内容修订 | §8.2 |
| O-03 | Finding 生命周期与版本绑定 | 挂 Report、Revision 可空、四状态 | §8.3 |
| D-02-01 | `blocked` 是否进 RunContract 枚举 | 可退化 running + 领域子状态 | §3.3 |
| D-02-02 | 物理落点（`analysis` schema 命名） | 建议 `analysis` | §7 M-01 |

---

## 10. 交接给交付物 06（Migration 任务书）

1. **先裁决 O-01~O-03 与物理 schema 名**，再展开 DDL；未裁决项不得写成 migration 指令。
2. M-01~M-08 按依赖顺序：run → registry → revision → report 列 → result_item → 约束。
3. 迁移必须包含：既有 Report 的兼容 Revision 回填、既有 intake_task 的 registry 重建、
   旧 result jsonb 的只读/可选导入策略（**禁止双写**）。
4. 验收以 G3 同构链复跑为准：报告门 409、人工门 403/允许、KB/PC 引用钉扎、
   close 幂等与版本快照不得回归。
5. 本设计不授权 Schema Migration；06 任务书须另行批准。

---

## 11. 版本记录与交付物序列状态

| 版本 | 日期 | 说明 |
|---|---|---|
| V1.0 | 2026-11-16 | 首次产出：Run Contract + A-01 注册表、Report/Revision（Git 正文 + 兼容引用）、Finding/Conclusion/Recommendation 载体、O-01~O-03 建议、Migration 需求清单、G3 兼容映射与自检。 |

| 交付物 | 状态 | 与本文关系 |
|---|---|---|
| 01 映射审计任务书 V1.1 | ✅ 冻结 | 本文输入（§3.2/§4/§7） |
| 02 Analysis 详细设计 V1.0 | ✅ 本文 | — |
| 03 公共契约规范 | 未开始 | 消费本文 §3/§5 的 Run/Finding Contract 字段 |
| 04 权威迁移设计 | 未开始 | 消费本文 §4 的 Git 正文与兼容引用 |
| 05 E2E 方案 V1.1 | ✅ 冻结 | 本文 §6 复用 G3 证据 |
| 06 Migration 任务书 | 未开始 | **待 O-01~O-03 与物理 schema 名裁决后**，按 §7 展开 |

**本文状态**：V1.0 待专家评审；评审通过前不得作为 06 的 DDL 依据。
