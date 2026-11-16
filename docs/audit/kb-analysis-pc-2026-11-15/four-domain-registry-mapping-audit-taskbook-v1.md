# 四域对象注册表与现有数据库映射审计任务书 V1.1

> 编制日期：2026-11-15 · 编制依据：[四域目标对象模型 V1.0](four-domain-object-model-v1.md) §9/§12 + [三域架构裁决 V1.0](architecture-ruling-v1.md)
> 证据基线：本目录 9 分册只读审计（00~08，2026-11-15，master 05ce33d8 快照）——**映射矩阵已用该证据预填**，剩余验证点单独列出。
> 定位：§16.1 交付物 01。产出物供 02（Analysis 详细设计）与 06（Migration 任务书）使用。**执行纪律：禁改 Schema；本任务书本身亦不定义 DDL。**
>
> **版本记录**：V1.0 专家审查=**有条件通过，待四项补充后冻结**；V1.1 纳入 A-01~A-04（§7）；**V1.1 已冻结为正式审计执行基线**（专家终审：建议通过；三个执行期补充检查要求见 §8，不必推迟冻结）。
> **2026-11-16 追记**：交付物 02《Analysis 对象模型详细设计 V1.0》已产出
> （[deliverable-02](deliverable-02-analysis-object-model-design-v1.md)），本文 §4 的
> Migration 候选 1~3 已有设计落点（A-01 registry / Report Revision / result_item）；
> 开放项 O-01~O-03 仍待裁决，06 不得在裁决前转 DDL。

## 1. 审计方法与纪律（承接对象模型 §12）

- 每个数据库对象完成**七项检查**：业务归属 / 身份性质 / 内容权威 / 状态权威 / 引用关系 / 权限边界 / 实现状态。
- 结论六分类（对象模型 §12.3）；与本仓 9 分册词汇映射：**GAP→MISSING、CONFLICT→MISMATCH、桩/占位→STUB**，MATCH/PARTIAL/UNKNOWN 同义。
- 每条结论八要素：DB 对象名 / 当前语义 / 目标对象 / 实证依据 / 差异说明 / 风险等级 / 推荐处理 / 是否需 Migration。
- **A-04 差异四分**（正式报告统计口径，防止过度 Migration）：每条结论必须加标差异类型——
  - **Schema Gap**：确实缺少数据库结构（才可能需要 Migration）；
  - **Contract Gap**：结构存在，缺统一业务契约；
  - **Implementation Gap**：结构存在，但写入/读取/生命周期未完成；
  - **Verification Gap**：证据不足，无法判断是否真正实现。
- 判定禁止仅凭表名；一切以系统目录 + 代码路径:行号 + 数据样本为准（证据锚点见 [08 证据索引](08_evidence_index.md)）。

## 2. 审计对象清单

**注册表 25 对象**（对象模型 §9）+ **建议补登 5 对象**（附录 A 缺口：pc.relationship / pc.relationship_revision / pc.relationship_type / pc.source / 正文载体 pc.document_revision）+ **跨域基础 7 项**（ObjectReference / Run Contract / Finding Contract / Evidence Reference / Human Decision Contract / 事件信封 / 权限租户）。

## 3. 预填映射矩阵

> 列：目标对象 → 现有载体（表·代码）｜七检查摘要｜结论｜关键差异｜需 Migration？

### 3.1 KB

| 目标对象 | 现有载体 | 摘要 | 结论 | 关键差异 | Mig? |
|---|---|---|---|---|---|
| KB Source | kb.source + kb.source_project；kb_service/source.rs:156-210 | 团队级来源池，uri XOR inline_content，0 行 | PARTIAL | 与 Document **无引用链**（交叉关系#1）；无对象存储通道。**A-02**：Source→Document→Revision 来源链列入 P2/P3 详细设计，允许多对多+修订级来源证据+Source 退役后可追溯；设计定前不判"无需 Migration" | 待设计（A-02） |
| KB Document | kb.document；document.rs:541-620 | doc_key 项目内唯一，0→1 行 | MATCH | — | 否 |
| KB Document Revision | kb.document_revision；insert_kb_revision（kb_pc_analysis.rs:683-700） | content 列=Markdown 全文权威，append-only 触发器，analysis_mode 三值 | PARTIAL | 内容权威=PG（Q4 目标 Git）；canonical_path 虚拟路径；**偏差未登记** | 是（P3 Git 化） |
| KB Knowledge Proposal | 无独立表；提案=kb.memory_revision(review_status=PENDING_REVIEW) | 触发器强制提案态插入 | PARTIAL | 提案内嵌于 Revision，无独立 Proposal 身份 | 视 02/03 设计定 |
| KB Memory | kb.memory；memory.rs:349-414 | 对象壳，0 行 | MATCH | 与 Document 零关联（目标模型要求的提取关系 MISSING，另列） | 否 |
| KB Memory Revision | kb.memory_revision；memory.rs:285-343 | 双层版本（revision_no+version_no）、审核状态机、CAS | MATCH | version_status='MERGED' 预留无实现 | 否 |
| KB Knowledge Review | 无独立实体；memory.rs:618-780 review + document.rs:60-248 confirm_revision | 双真人门 + CHECK/触发器 | PARTIAL | 无独立 Review 对象；待审未进审阅中心（Q8 排期） | 视 P4 接入方案 |

### 3.2 Analysis

| 目标对象 | 现有载体 | 摘要 | 结论 | 关键差异 | Mig? |
|---|---|---|---|---|---|
| Analysis Run | ai.analysis_run（context.rs:143-161 创建A / proposal.rs:312-324 创建B）；仓库链**无 Run 实体** | 双写入者分叉；result jsonb 内嵌 findings | **MISMATCH**（Schema+Contract Gap） | 三链三模型（03 分册 §0）；repo_analysis_report 无 analysis_run_id（Q1=B 维持 intake 关联）。**A-01**：统一 Run Contract 时必须建 Run↔底层执行实体映射注册表（一个统一 Run 可对应多条底层执行记录；明确幂等键/失败重试/终态映射）；**禁止**直接给 repo_analysis_report 加 analysis_run_id | 是（P2 统一 Run Contract + 适配器） |
| Intake Task | integration.intake_task + repo_import（status 10 值） | 独立工作流实体，与 analysis_run 有 FK（repo_import.analysis_run_id） | MATCH | 与目标语义一致（Intake≠Run 已天然成立） | 否 |
| Analysis Report | integration.repo_analysis_report（0023/0024） | 独立 ID/状态机/409 门/close 真人门；UNIQUE(intake_id) | PARTIAL | 正文借住 kb（裸指针无 FK）；无版本；run_mode/since_commit 已有 | 是（P2：Report Revision + 正文迁 Git；**历史 kb_revision_id 兼容保留**） |
| Report Revision | **不存在** | — | MISSING（Schema Gap） | Q2=B+C 目标实体；正文 Git 权威、DB 存身份/状态/引用。**A-02 关联约束**：Report Revision 须能关联产生它的 Run，但 **Report 身份不得绑定到某一个 Run**（防止重新分析与修订混淆）；五身份分立：Report=稳定业务身份 / Revision=内容版本身份 / Run=执行活动身份 / Git Commit=正文锚点 / 历史 kb_revision_id=兼容引用不再承担长期权威 | 是（P2 新增，整体设计） |
| Analysis Finding | ai.analysis_run.result jsonb 内嵌（pm-domain/proposal.rs:36-42）；无独立表 | 无独立身份/状态/生命周期 | **STUB** | 独立 Finding 实体缺失；cognition_item 语义归 Project Cognition（Q3），**不充当 Analysis Finding** | 是（P2，随 Finding Contract） |
| Analysis Conclusion | 内嵌 result jsonb / draft jsonb | — | MISSING | 同上 | 是（P2） |
| Analysis Recommendation | 内嵌；仓库链产物=pc.proposal（借 PC 域承载） | analysis_report.rs:334-351 经 PC 反查 | MISSING | 建议无 Analysis 侧身份 | 是（P2） |

### 3.3 PC

| 目标对象 | 现有载体 | 摘要 | 结论 | 关键差异 | Mig? |
|---|---|---|---|---|---|
| Cognition Proposal | pc.proposal(+member)；proposal.rs | origin_kind 五值、change_kind 六值、同事务 submit | MATCH | 0 行生产数据 | 否 |
| Cognition Object | pc.cognition_object；object.rs | lifecycle 五值、current_revision/superseded 链 | MATCH | 0 行 | 否 |
| Cognition Version | pc.cognition_version + version_member + 视图 v_revision_versioned；publish.rs:380-500 | snapshot_hash/predecessor 链/成员钉扎 | MATCH | 0 行 | 否 |
| Cognition Conflict | pc.conflict(+object)；**生产无写入者** | 冻结逻辑在（mod.rs:180-203），事件 producer=桩（outbox.rs:326-331） | **STUB** | Q6 五要件待实做 | 是（P4） |
| Cognition Evidence | pc.evidence；evidence.rs:121-208 | 分型定位器 CHECK、四元组跨域引用 | MATCH | 0 行 | 否 |
| Cognition Review | pc.approval；proposal.rs:1000-1039 | 真人门+三重乐观锁+不可变触发器 | MATCH | — | 否 |
| （补登）Relationship(+Revision/Type) | pc.relationship / relationship_revision / relationship_type | owner_domain=pc/kb、端点校验触发器 | MATCH | owner_domain='kb' 归属越位（Q10） | 否（字典归属修订，P2 小改） |
| （补登）Source | pc.source；evidence.rs:27-79 | 跨域来源登记 8 通道，唯一有真实数据的 PC 表 | MATCH | — | 否 |
| （补登）正文载体 | pc.document_revision | content=PG 权威，git_commit_sha 恒 NULL（PCGIT 已登记 pc/mod.rs:25-27） | PARTIAL | Q7 目标 Git | 是（P4） |

### 3.4 Project Cognition

| 目标对象 | 现有载体 | 摘要 | 结论 | 关键差异 | Mig? |
|---|---|---|---|---|---|
| Knowledge Proposal | ai.cognition_item（state=proposed 即提案形态） | 6 域 6 态、evidence jsonb | PARTIAL（Contract Gap） | 提案内嵌；无独立 Proposal 实体 | 视 O-07 |
| Project Knowledge | ai.project_knowledge；repo_cognition.rs:272-283 | 6 域、status=active/superseded/retired、supersedes 链 | MATCH | 归属 Project Cognition（Q3=C 已定）；正文权威待 O-05 | 否（结构） |
| Knowledge Version | ai.project_knowledge.version 整数列 | 无独立版本对象/revision 行 | **MISSING**（待 A-03 评估，未必是 Schema Gap） | **A-03**：目标是让版本概念拥有清晰可追溯身份与生命周期——独立表是手段之一而非必须；设计定前不预设 Migration | 视 A-03 设计 |
| Project Evidence | cognition_item.evidence jsonb；无独立证据表 | — | **MISSING**（同 A-03，身份先行） | **A-03**：证据须有独立可追溯身份；载体（独立表 vs 增强四元组引用）由设计裁决 | 视 A-03 设计 |
| Project Decision | governance.approval_request + audit_log（analysis_run_id 关联） | 审批承接存在（advance.rs:99） | PARTIAL（Contract Gap） | **A-03 关键约束**：governance.approval_request 的审批完成状态**不得自动证明** Project Cognition 正式对象已通过正确的人工确认——决策身份必须归属 Project Cognition 自有语义 | 视 O-08 |
| Project Cognition Review | pm_cognition.rs:279-310（AuthedPrincipal→decided_by，:12 "AI 无确认入口"） | **待复核**：本审计实证有真人主体判定；四域审计（另会话）P0 记"确认无主体门"——两处证据冲突 | **UNKNOWN** | O-11 优先核实：确认入口是否强制 principal.kind='human' 且不可被服务主体冒用 | 复核后定 |

### 3.5 跨域基础

| 目标 | 现有载体 | 结论 | 关键差异 | Mig? |
|---|---|---|---|---|
| ObjectReference | kb-pc-contracts DomainRef 四元组（domain/type/id/revision）+ registry 校验 + 通道状态机 | PARTIAL | 缺 tenant_id/project_id 显式字段（现经 project 反查）；DB 层无 FK（软引用，接受） | 否（应用层扩展） |
| Run Contract | 无 | MISSING | 各域 Run 词汇不一 | P2 契约规范 |
| Finding Contract | 无 | MISSING | 5 处 finding 词汇/生命周期不一（Q5） | P2 契约规范 |
| Evidence Reference | pc.evidence 结构化定位器 + discovery_finding_evidence（复合 FK 到 repo_import_source_attempt） | PARTIAL | 无统一信封（integrity_hash/captured_at 缺） | P2/P3 |
| Human Decision Contract | 决策记录分散：pc.approval / kb review / governance.approval_request / ai.change_proposal(executed_audit_id) | PARTIAL | 四套词汇；无统一表达；决策不跨域借用已有实例（PC≠ProjCognition） | P2 契约规范 |
| 事件信封 | integration.outbox_event + 域事件（kb.memory.approved / pc.change_proposed / pc.conflict.detected桩） | PARTIAL | 无 correlation_id/causation_id；O-10 | 视设计 |
| 权限/租户 | 新栈双键过滤；旧 ai 栈 project_id 单键（tenant 由 project 反查 knowledge.rs:424-438） | PARTIAL | Q11 P1 安全复核；坐实跨租户→P0 | 复核后定 |

## 4. 「真正需要修改的地方」汇总

> 统计口径=**A-04 差异四分**：Schema Gap 才进 Migration 候选；Contract/Implementation Gap 走契约与实现补齐；Verification Gap 走验证。**不得为了让四域整齐而统一表结构。**

**Migration 候选（全部排在 P2 之后，P0/P1 零迁移；均为 Schema Gap）**：
1. Analysis Run 统一（Run Contract + Adapter + **A-01 映射注册表**；是否加 analysis_run_id 列：**禁止**，维持 Q1=B）— P2 · **设计已落位**：交付物 02 §3
2. Report Revision 新实体（Schema Gap，P2 核心设计对象）+ 报告正文迁 Git + 历史 kb_revision_id 降级为兼容引用 — P2 · **设计已落位**：交付物 02 §4/§6
3. Analysis Finding/Conclusion/Recommendation 独立载体（随 Finding Contract）— P2 · **设计已落位**：交付物 02 §5
4. KB Git 化（kb.document_revision/memory 正文权威迁移 + 一致性/恢复机制 O-09）— P3
5. PC 正文迁 Git（PCGIT 兑现）+ Conflict 五要件 — P4
6. 字典归属修订（owner_domain='kb'，Q10）— 小改，P2

**待设计后判 Migration（A-02/A-03）**：
- KB Source→Document→Revision 来源链（P2/P3 详细设计；多对多+修订级来源证据+退役可追溯）
- Project Cognition 的 Version/Evidence/Decision 身份（业务身份先行，载体形式由设计裁决）

**Contract/Implementation Gap（不改结构，补契约与实现）**：
- 四项公共契约规范（Run/Finding/Evidence/Human Decision）— 交付物 03
- pc.conflict 五要件实做（结构在，Implementation Gap）— P4
- 审阅中心接入 kb.memory（Q8）— P4

**明确不需要动（避免不必要重构）**：
- pc.* 全部现有表结构（MATCH 为主，0 行=迁移红利窗口，结构无需返工；**但 PC MATCH ≠ PC 完成**：conflict 五要件与 PCGIT 兑现前不得标记领域完成）
- kb.memory/revision 状态机与触发器
- 四元组软引用机制（DB 层不加 FK，维持应用层校验）
- repo_import/intake 实体（Intake≠Run 目标已天然满足；**不为统一 Run 重构 Intake**）
- 三知识对象的表分离现状（Q3=C：不合并即不建公共表）

## 5. 待补验证点（关闭前不得冻结结论）

| # | 验证点 | 方法 | 关联 |
|---|---|---|---|
| V-1 | Project Cognition 人工确认主体门是否真实存在（两处审计证据冲突） | 代码复核 pm_cognition.rs 确认入口 + 服务主体冒用测试 | O-11 |
| V-2 | 旧 ai 栈跨租户访问可能性 | Q11 安全复核清单（后台任务/MCP/DB 路径） | Q11，坐实升 P0 |
| V-3 | 仓库分析→报告→KB 确认→PC 提案→审批→发布→Close 全链 | 真实 E2E（P1 Phase，禁绕人工门） | Q12；关闭引用链零数据问题 |
| V-4 | close 批量发布 / publish_version 在真实数据上的行为 | 同 V-3 | ARC-03/13 |
| V-5 | ai.project_knowledge 正文权威与 Git 化方案 | O-05 详细设计 | 不得套用 KB Q4 |

## 6. 执行步骤与产出（专家定序，V1.1）

1. **任务书 V1.1**：纳入 A-01~A-04（本文档），不改原有架构裁决。✅
2. **V-1 / V-2 优先复核**：Project Cognition 人工主体门 + 旧 ai 栈租户隔离——不等到 P5，尽早开展；坐实跨租户→升 P0。
3. **编制 P1 E2E 测试方案**（交付物 05）：明确真实业务链的输入/输出/人工操作/失败恢复/验收条件。
4. **Analysis 对象模型详细设计**（交付物 02）：Run/Intake/Report/Revision/Finding + A-01 映射注册表。
5. **生成最终映射审计报告**：逐项核对 9 分册证据，按 A-04 四分口径冻结六分类结论。
6. **编制 Migration 任务书**（交付物 06）：只对有实证依据的 **Schema Gap** 制定迁移任务。

全程禁止顺手修改 Schema；发现新表/新列事实只登记不重构。

## 7. 专家审查意见记录（V1.0 → V1.1）

**审查结论**：有条件通过。四域边界/Analysis 模型/PC 模型/公共契约通过；对象注册表基本通过（补登五对象必要）；Project Cognition 与内容权威有条件通过；权限安全待验证；E2E 未验证（**不能用表存在代替业务闭环成功**）；Migration 暂不批准（先 P1+P2 设计闭合）。

**六项意见（要点）**：
1. **不为统一 Run 重构 Intake**（本审计最佳发现之一）；补 A-01 映射注册表要求；继续禁止给 repo_analysis_report 加 analysis_run_id。
2. **Report Revision 是 P2 核心设计对象**：五身份分立（Report/Revision/Run/Git Commit/兼容引用）；Revision 可关联 Run 但 Report 身份不绑定单一 Run。
3. **KB Source↔Document 零关联是真实业务缺口**：不简单加外键，先设计来源链（多对多+修订级证据+退役可追溯），列入 P2/P3。
4. **Project Cognition 是最需补齐的对象模型**：Version/Evidence/Decision 三缺口——目标是业务概念拥有清晰可追溯身份与生命周期，**不是必须三张新表**；approval_request 完成状态不得自动证明正式确认。
5. **PC MATCH ≠ PC 完成**：结构符合/有生产数据/有真实调用/E2E 成功四者不可互替；conflict 五要件与 PCGIT 兑现前不标记领域完成。
6. **Q11 独立于 Migration 提前推进**：不等 P5，尽早验证，坐实升 P0。

**A-01~A-04 补充项**：已分别落位于 §1（A-04 统计口径）、§3.2（A-01/A-02）、§3.4（A-03）、§4（四分口径重排）。

## 8. 冻结时执行期补充检查要求（专家终审，2026-11-15）

**冻结结论**：V1.1 建议通过 → **冻结为正式审计任务书**。三个细节作为审计执行中的补充检查要求，不推迟冻结。

### 8.1 Report Revision ↔ Git Commit 关系（补入 §3.2 语义）

- 正式发布的 Report Revision 必须有**确定的正文版本锚点**；Git Commit 是正文内容的不可变定位依据。
- **不要求**每 Revision 产生独立 Commit：多个 Revision 可引用同一 Commit，但必须保证 Revision 对应正文内容可确定。
- 采用目录/文件级版本定位时，必须能**无歧义恢复**对应正文。
- 禁止把 Git Commit 误当业务 Revision 本身。

### 8.2 A-02 来源关系的时间语义（补入 §3.1）

三态区分：**来源登记**（Source 被 Document 引用）≠ **版本使用证据**（某 Document Revision 实际使用了哪些 Source）≠ **来源生命周期**（Source 退役后历史 Revision 的来源证据仍有效）。

硬约束：**Source 与 Document 的关联不能代替 Revision 级来源证据**——正式知识版本必须能追溯其实际使用的来源及对应证据，即使 Source 后续退役/替换/失去活动状态。（影响 KB 可信度、审核与知识回溯。）

### 8.3 V-1 验证范围扩大为完整确认链（补入 §5）

`AuthedPrincipal`/`decided_by` 存在 ≠ 人工确认门成立。六个检查点：

| 检查点 | 问题 |
|---|---|
| 身份入口 | 是否强制 principal.kind='human'？ |
| 服务主体 | Service Principal 能否冒用人工身份？ |
| API | 是否存在绕过确认门的其他入口？ |
| 数据写入 | 是否只有授权人工决策能改变正式认知状态？ |
| 审计记录 | 能否追溯实际决策人/时间/对象？ |
| 测试 | 是否有 AI/服务/未授权主体的拒绝测试？ |

### 8.4 冻结执行边界（并行规则）

P0 安全与证据核实（V-1/V-2，零 Schema）/ P1 真实业务链（V-3/V-4，禁绕人工门）/ P2 Analysis 与跨域对象设计（Run Contract+Adapter、Report Revision、Finding Contract、Source 来源链、ProjCognition 三身份设计）/ P3 KB Git 迁移（禁双写双主）/ P4 PC 完整性（PCGIT、Conflict、审阅中心）/ P5 旧栈收敛。

**特别调整**：V-1/V-2 是优先验证任务，不阻塞 P2 纯设计工作的并行准备；P0/P1 优先级不变。

## 9. V-1 复核结果（2026-11-16，代码级，零改动）

**结论：Project Cognition 人工确认门 = PARTIAL（有外围防线、无显式主体门）**——两处审计证据冲突裁定：比"无主体门"缓和（四域审计 P0 表述过重），比"完备"严格（本审计先前判断需下调）。

| 检查点 | 结果 | 证据 |
|---|---|---|
| 1 身份入口 | ❌ 无显式 kind 检查 | `AuthedPrincipal` 无 kind 字段（auth.rs:166-182）；decide_item 只做 `require_project_visible`（pm_cognition.rs:285-291）=tenant+成员行存在（support.rs:184-195），不查 `core.principal.kind` |
| 2 服务主体 | ⚠️ 部分防住 | JWT service token 被拒（resolve_principal auth.rs:327-332 "service tokens are not accepted on business endpoints"）+ 跨租户 token 被拒（:333-335）；**但 `cpk_` 客户端凭据通道以 owner 真人 principal 直接放行**（auth.rs:318 注释"直接放行"+ :240-284 构造 AuthedPrincipal/human_actor），执行端 AI 持 cpk 凭据即以 owner 身份到达业务端点，且 decide_item 无权限码/auth_mode 闸 |
| 3 API 绕过 | ✅ 无旁路 | MCP 工具面不含 cognition 决策（crates/pm-mcp/src/tools.rs 无 cognition 路由）；cognition_item 状态变更唯一入口=decide_item（repo_cognition.rs:218 FOR UPDATE，仅 proposed 可决，重复 409）；AI 侧只 INSERT（start_run :161-187，state 恒 proposed） |
| 4 数据写入 | ✅ 单点 | 同上；confirm 落 ai.project_knowledge 且回写 source_item_id（repo_cognition.rs:272-283） |
| 5 审计记录 | ✅ 可追溯 | decided_by/decided_at 落行；audit_actor 含 client/auth_mode（auth.rs:185-193）——cpk 通道 client="cpk"/auth_mode="client_credential"，可区分执行端行权与真人交互 |
| 6 拒绝测试 | ❌ 缺失 | apps/pm-api/tests/pm_cognition.rs 7 个测试全为 happy-path+状态规则，无 AI/服务/未授权主体 403 用例 |

**风险定级与建议**：
1. **R1（P1，非 P0）**：cpk 通道以 owner 真人身份放行执行端到达全部 /v1 业务端点（含决策端点）——授权模型本意"授权主体永远是真人"（caller.rs 红线），但决策类端点应加 `auth_mode` 闸（interactive-only）或显式 kind='human' 校验（对齐 pc/mod.rs:157-162 ensure_formal_authorized 口径）。需要真实凭据+成员身份方可触达，非匿名可利用 → 不升 P0。
2. **R2（P2）**：decide_item 无 require_human 对齐——建议统一三层 caller 语义（HumanActor 已存在，决策端点改收 HumanActor 强类型即可结构性关闭）。
3. **R3（P2）**：补三类主体拒绝测试（AI 主体/服务 token/未授权）。
4. 对照锚点：pc 域同问题已正确解决（ensure_formal_authorized 查库验 kind='human'+触发器+乐观锁），Project Cognition 应对齐该口径。

---

*本任务书与 9 分册审计同源同方法；矩阵预填结论均为 2026-11-15 快照事实，代码演进后需按 08 证据索引重新锚定。*
