# PM-AI 四域架构裁决会审阅包 V1.0

> 依据：四域对象注册表审计 V1.0（commit 05ce33d8，docs/architecture/four-domain/ 七分册）+ 专家审阅意见（2026-11-15）
> 性质：裁决会议材料——每项给出事实/冲突/候选方案/推荐意见/需确认问题；**本包不是已批准基线，不授权实施**
> 纪律：审计发现的 P0 性质各异（安全缺陷/契约冲突/治理缺失/链路未验证），不得统一处理成一次数据库整改任务

---

## 0. 架构红线（裁决不得触碰）

1. 不把 kb.memory、ai.project_knowledge、pc.cognition_object 合并为通用 Knowledge。
2. 不让 Analysis Report 长期借用 KB Document Revision 作为版本权威。
3. 不允许 AI 主体确认 Project Cognition 正式对象。
4. 不允许跨域服务直接写其他领域正式业务表。
5. 不让 Review Center 成为领域审批权威（B08）。
6. 不因 PG 存储实现成熟改变 Markdown + Git 内容权威目标。
7. 不以单测通过/表存在宣称四域链路闭环。
8. 三个概念关系永久有效：Document≠Knowledge、Knowledge≠Cognition、Analysis≠Knowledge。

---

## 1. 建议新增裁决议题（3 项，扩展原 U 项）

### D-REF：DomainRef 统一契约（扩展 U19，落实 V2.0 §8.2）

| 项 | 内容 |
|---|---|
| 事实 | 四元组结构合理（9 domain 注册表、通道状态含 project_cognition=ReadOnly 预注册）；校验仅 domain+非空，type 无注册表、id/revision 无存在性、tenant/project 仅 KB 局部；引用三形态并存（TEXT 四元组/裸 UUID/FK）；任务列绕过 payload 校验 |
| 冲突 | 结构校验与解析授权校验混在口头约定里，无统一责任方 |
| 候选 | A. 两层校验：契约层（domain/type 注册、格式）+ 解析层（存在性/租户/项目/revision 有效性，按域注册 resolver）｜B. 各域自发校验维持现状｜C. DB 层补 FK |
| 推荐 | A。引用不等于拥有：解析校验通过也不产生写权；跨域操作仍走目标域服务/事件。C 否决（跨域 FK 违反领域自治） |
| 需确认 | ① resolver 注册归 kb-pc-contracts 还是各域 port？② 校验失败语义（拒写入 vs 记 revision_missing_reason 放行）哪些场景适用？③ 存量三形态引用是否统一收敛到契约层？ |

### D-CONTENT：共享内容权威（扩展 U13/U14）

| 项 | 内容 |
|---|---|
| 事实 | 正文权威三模式并存+第 5 处（ai.project_knowledge）；旧栈 Git 能力是 knowledge.rs crate 内私有实现，无共享抽象；PC 已预留 git_committed 中间态与两个闲置命令；kb.source_commit_sha=源码锚≠内容提交 |
| 冲突 | 业务领域权威（谁拥有对象/确认权）与内容存储权威（正文最终介质）两个概念未在架构上分离 |
| 候选 | A. 建 Shared Content Authority（统一 Markdown+Git 服务：commit/retrieve/verify(hash)/revert），各域 Revision 引用 content_ref｜B. 各域自建 Git 通道｜C. 维持 PG 权威+登记豁免 |
| 推荐 | A（专家示意：KB Document Revision / KB Memory Revision / Analysis Report Revision / PC Document Revision / Project Cognition Revision 共享底层，生命周期各自独立）。先裁决再建，不提前动表 |
| 需确认 | ① 统一服务形态（独立 crate？复用 KnowledgeRoot？）；② Source Commit vs Content Commit 双引用列设计；③ 迁移顺序（kb→pc→报告→project_knowledge？）；④ 各域确认门与内容提交的两阶段事务边界 |

### D-REVIEW：统一审阅聚合（扩展 U25，落实 B08）

| 项 | 内容 |
|---|---|
| 事实 | 审阅中心 human_action_summary 5 kind；kb.memory 审核与 cognition 决策无入口；各 kind 操作端点已指向各领域权威（B08 成立）；Review Card Contract（前端 reviewCenter.ts 页签归并 15-33） |
| 冲突 | 补齐待办类型若动「统一审批对象」会破坏 B08 |
| 候选 | A. 仅扩展聚合读模型+Card Contract，操作端点指向各域服务｜B. 建统一审批表 |
| 推荐 | A。B 否决（违反 B08 与领域自治） |
| 需确认 | ① 新增 kind 的展示上下文与深链；② cognition 决策补审计事件后是否进聚合；③ 卡片操作后的状态回流机制 |

---

## 2. KB-01~KB-09（专家建议裁决，尚未冻结——供会议确认）

| 编号 | 建议裁决 | 与 U/D 项关系 | 现状事实（审计） |
|---|---|---|---|
| KB-01 | 保留 Source/Document/Memory 三对象，不合并 | 对应 U09 | 三对象链路完整（KB 域 6 COMPLETE）；memory_ref 桥接 |
| KB-02 | Document 与 Knowledge 分离，补知识形成关系 | U08 + Q1 补审 | Extraction 对象不存在；memory 生成侧缺位（唯一 REST 提案入口） |
| KB-03 | Document 确认与 Memory 审核双门分离 | — | 两套治理机制均完整（confirm vs review，双 revision 体系） |
| KB-04 | Analysis Report 不归 KB | D01/U01-04 | 正文借住 CONFIRMED（kb_revision_id 裸引用，样本 NULL） |
| KB-05 | KB 正文逐步迁 Markdown+Git，PG 作结构状态权威 | D-CONTENT/U13 | PG content 权威 CONFIRMED；source_commit_sha≠内容提交（专家指正采纳：区分 Source Commit/Content Commit） |
| KB-06 | 保留 memory_ref 四元组 | D-REF | 设计合理但零行（NO_PROD_DATA） |
| KB-07 | Memory 审核接入统一审阅中心 | D-REVIEW/P2-2 | 现无入口（human_action_summary 不含） |
| KB-08 | FTS 保留，PageIndex/Vector 分阶段 | — | 现状=章节 FTS+事件驱动重建（架构文档命名改为 KB Section Full-Text Index） |
| KB-09 | 旧 ai 栈进入兼容与迁移治理，不立即删除 | U22 | 旧栈 Git 权威真实存在、独立运行零互通；DEPRECATE 候选仅 ai.knowledge_reference |

另采纳专家两项技术更正进审计口径：
- kb 新栈 canonical_path 为虚拟路径、无实际 Git 写入——**不得据 Git 相邻字段认定 Git 权威**（authority-matrix 已按此口径，进一步在 KB-02 分册确认）。
- 现有章节 FTS 不得称完整 PageIndex。

---

## 3. U01~U25 按依赖重排（七组议程）

> 每项格式：事实 → 冲突 → 候选 → 推荐 → 需确认。推荐均为审计意见，非既定裁决。

### 第 1 组：报告族（解锁 D01 实施）——最优先

**U01 Run/Report 基数**
- 事实：intake_id UNIQUE=一次分析运行一份报告；无 analysis_run_id；导入链 run 是记账快照。
- 冲突：UNIQUE intake_id 事实约束可能被误当目标关系冻结。
- 候选：A. 1 Run:1 Report 保持（intake 升级为正式关联列）｜B. 1 Run:N Report｜C. Report 可累积多 Run 贡献（经 Revision）。
- 推荐：暂按 A 落关联列，把 C 留给 Revision 机制（U02），不冻结基数。
- 需确认：报告的业务连续性单位是「仓库+分析目标」还是「intake 事件」？

**U02 重跑策略**
- 事实：现状重跑=新报告行+新 KB 修订；报告无版本。
- 候选：A. 重跑恒新 Report｜B. 同一业务报告下新 Revision｜C. 按分析目标/范围判定（同目标=新 Revision，异目标=新 Report）。
- 推荐：C（与 V2.0 §4.4 一致），判定字段需 U03 状态机承载。
- 需确认：判定由谁做（发起人显式指定 vs 系统按 repository+range 推断）？

**U03 报告状态机**
- 事实：analyzing/analyzed/closed/failed/cancelled，其中 cancelled 无写入者；守卫全应用层。
- 候选：A. 现 5 态补齐 cancelled 入口｜B. 收敛为 4 态（删 cancelled）｜C. 引入 Ready/Confirmed 细分。
- 推荐：B 或 A 均可，倾向 A（保留取消语义，补入口+审计）；Ready/Confirmed 留给 close 语义（U04）。
- 需确认：cancelled 语义是否=「放弃本次分析」且需保留产物？

**U04 close 语义与独立确认门**
- 事实：close=单事务批量发布 PC 提案+撤回未决+拒 KB 草稿+固化锚点；ensure_closable 仅 interactive 真人。
- 冲突：close 同时承担「分析完成确认」与「认知批量发布」两个语义，是否需要拆分。
- 候选：A. close 一步到位（现状）｜B. close（确认分析完成）与 publish（认知发布）拆两口｜C. close 可配置。
- 推荐：先 B 的方向讨论——报告发布≠业务对象确认（V2.0 ARC-13），但拆分成本与现有审阅流 v2 兼容性要评估。
- 需确认：① 批量发布失败中途回滚的 UX；② KB draft rejected 自动处置是否保留。

### 第 2 组：Finding/Evidence（D03 契约）

**U05 Finding 基础契约范围**：事实=5 套模型矩阵（severity 三口径、大小写双词表、JSONB 无稳定 ID）；候选=A. 最小契约（id/type/owner_domain/run_ref/subject_ref/severity/evidence_refs）+领域扩展｜B. 仅词表归一不动结构；推荐=A，先行词表归一；需确认=各域 finding 表是否保留（D03 已裁保留）。
**U06 Finding 跨域转化身份**：事实=唯一人工桥 requirement_finding（裸引用）；候选=转化即新对象+来源引用保留｜转化复用原对象；推荐=新对象+DomainRef 回指；需确认=原 finding 状态是否联动。
**U07 Evidence 对象模型**：事实=pc.evidence 最完整（三形态 CHECK+四元组+hash）；discovery 有独立 evidence 表；候选=A. 共享 Evidence 契约+域内实体｜B. 统一 evidence 大表；推荐=A（B 违反红线 1/4）；需确认=文件/代码/口述三形态是否够用。

### 第 3 组：KB 知识形成（配合 Q1 补审结果）

**U08 KB Extraction**：事实=文档解析借 docproc（Processing Job/Artifact），知识提取对象不存在；专家已区分 Document Extraction vs Knowledge Extraction 两过程；候选=A. 建 Knowledge Extraction 对象（独立 run/revision/审核状态）｜B. 仅建 Document→Memory 关系列｜C. 维持 memory_ref 隐式表达；推荐=先 Q1 审计结论出来再定（可能 A 最小版）；需确认=提取过程是否需要独立 Run（D02 关联）。
**U09 Memory/Knowledge 语义**：事实=kb.memory 双层版本最成熟；四对象语义表（专家版：kb.memory=可复用知识 / cognition_item=ProjCog 候选 / project_knowledge=ProjCog 正式 / pc.cognition_object=PC 正式）已获专家认可；推荐=采纳专家四对象表进注册表 V1.1；需确认=Memory 是否承载「产品业务规则」类内容（专家意见=否，归 PC）。
**U12 Source/Evidence 边界**：事实=同名异义（kb.source 原文池 / pc.source 域锚 / evidence 各域）；推荐=采纳专家五概念表（Original Source/Document/Analysis Evidence/Cognition Evidence/PC Evidence），底层共享 URI/Hash/Revision 引用类型；需确认=kb.source 大文件 URI 生命周期与访问权限（专家提出）。

**U16 KB 自动化边界**：事实=提案无门/审核有门；生成侧缺位；候选=A. 受控自动整理白名单（低风险操作）｜B. 全人工；推荐=待 U08 落地后一起裁；需确认=哪些整理操作算低风险。

### 第 4 组：Project Cognition（解锁 P0-1 修复方案定稿+死列处置）

**U10 project_knowledge/cognition_item 正式映射**：事实=写入链证据已锁定归属（ProjCognition）；version/supersedes 死列；溯源仅 repository 级；候选=A. EVOLVE（启用 version/supersedes+补 evidence 钉 commit/路径/行号）｜B. 简化（删死列，保持追加式）｜C. 重建为 PC 同款 revision 模型；推荐=A（认知会演进，需修订链+溯源，但证据粒度先裁到 commit 级还是行级）；需确认=① 修订是否需要提案-审核两步（对照 U15）；② evidence jsonb 是否升独立对象。
**U15 ProjectCognition 审批模式**：事实=P0-1（无主体门）；审批权威无承接（无 approval 对象/无审计事件）；候选=A. 复用 PC proposal→approval→publish 模式｜B. 轻量门（require_human+审计，无独立审批对象）｜C. 沿用 decision 端点+补门+审计；推荐=C 最小修复先行（工作流 A），A/B 留给工作流 B 长期设计；需确认=① 决策可否改期（对照 pc 审阅流 v2）；② 是否需要 evidence review 前置步骤。
**附**：P0-1 修复本身不待裁决（安全缺陷，工作流 A 直接做）——补 auth_mode+principal.kind+AI 拒绝测试+审计记录。

### 第 5 组：关系与冲突

**U11 PC↔ProjectCognition 正式关系**：事实=唯一桥 identity-synthesis 产 PC 提案（代码级，无关系对象）；候选=A. 注册 proposes/supports 关系类型（记录：来源认知对象+版本+目标 PC 对象+提案结果+是否成 Revision）｜B. 不建关系，靠 pc.proposal.source 链；推荐=A（专家建议方向），语义命名待定；需确认=挂 pc.relationship_type 还是独立注册表（连带 U18）。
**U17 冲突治理共用**：事实=pc.conflict 桩+L3 死代码+cognition 无冲突对象；候选=A. PC/ProjCog 共用冲突基础设施｜B. 各建；推荐=先补 PC 冲突生产写入者+resolve 流程（否则 L3 永久冻结风险成立），ProjCog 视 U10 结果；需确认=冲突检测的触发时机（分析时？确认时？）。
**U18 关系字典归属**：事实=owner_domain='kb' 2 行越位+写入侧零校验+字典被清库；候选=A. 字典拆域内/跨域两册｜B. 保持单册+owner_domain 语义强化+写入校验；推荐=B+补种子分级运维（P2-11）；需确认=跨域关系确认权威如何登记。

### 第 6 组：基础设施

**U13 Git 迁移顺序**：依赖 D-CONTENT 裁决与 Q2 补审结果；候选顺序=报告→kb→pc→project_knowledge（按 D01 依赖）或 kb→pc（按成熟度）；推荐=Q2 分册出来后定。
**U14 Git 内容基础设施**：=D-CONTENT 主体。
**U19 引用失效生命周期**：事实=revision_missing_reason 机制已存在（pc.source/kb.memory_ref）；=D-REF 主体扩展。
**U20 Run 状态映射**：事实=completed/succeeded 分叉；候选=A. 词表归一+CHECK（导入链改走统一入口）｜B. Adapter 映射表；推荐=A（零存量成本）；需确认=succeeded 行是否需要兼容读取（当前 0 行，无需）。

### 第 7 组：实施与验收

**U21 Analysis Schema**：依赖第 1/2 组裁决；不提前设计。
**U22 兼容层**：事实=旧栈独立零互通，兼容面小；KB-09 承接。
**U23 数据迁移**：核心对象 0 行，负担≈0；唯一要求=迁移设计仍须含对账/回滚（纪律不豁免）。
**U24 端到端验收**：采纳专家三层次验收表（Schema/Unit→Service/Integration→E2E）；四场景 A/B/C/D 为正式验收门槛，非可选测试；kb.memory_ref 真实业务验证纳入场景 B/D。
**U25 审阅中心接入**：=D-REVIEW 主体；缺口 kind=kb.memory 审核+cognition 确认。

---

## 4. 两条并行工作流（专家结构，采纳）

**工作流 A：先关闭已确认安全与状态风险（不待裁决）**
1. P0-1 ProjectCognition 人工主体门（补 auth_mode+kind=human+成员校验保留+AI 拒绝测试+审计记录——安全修复与审计追溯两件事都做，不互替）
2. P0-2 Analysis Run 状态词统一（先定统一契约词表→补 CHECK→统一双写入者）
3. P2-2 审阅中心补 kb.memory/cognition 两待办类型（不改各域审批权威）

**工作流 B：冻结目标对象契约（裁决驱动）**
1. U01~U04 → D01 实施（analysis_run_id+Report Revision）
2. U05~U07 → D03 契约
3. U10/U15 → ProjectCognition 正式对象演进与审批权威
4. U13/U14（D-CONTENT）→ 统一内容权威迁移方案

**共同验收**：U24 四场景真实数据。

**实施纪律**：可先修已确认安全缺陷；目标对象契约未裁决前不提前大规模改表或迁移正文。

---

## 5. 请会议逐项确认的问题清单（汇总）

1. U01：报告业务连续性单位？
2. U02：重跑判定由谁做？
3. U04：close 与 publish 是否拆分？
4. U05：Finding 最小契约字段集？
5. U08：Knowledge Extraction 是否独立 Run？
6. U09：Memory 是否排除产品业务规则类内容？
7. U10：认知修订链与证据粒度（commit 级 vs 行级）？
8. U15：决策可改期？审批对象形态（B vs C）？
9. U11：跨域关系挂哪个注册表？
10. D-REF：resolver 责任方与失败语义？
11. D-CONTENT：服务形态与迁移顺序？
12. D-REVIEW：新 kind 的卡片契约改动范围？

---

*证据引用：审计七分册（object-registry/database-mapping/lifecycle-matrix/relationship-registry/authority-matrix/decision-register/four-domain-audit-report）+ KB 补审三分册（KB-01/02/03，产出中）。本包状态=待会议裁决。*
