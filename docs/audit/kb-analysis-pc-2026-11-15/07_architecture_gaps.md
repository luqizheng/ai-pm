# 07 架构差异清单（对照《三域对象边界与协作架构设计基线 V1.0》）

> 基线状态=提案待确认（非已批准）。凡基线中标注"待讨论"的设计建议，本清单不以"现有实现错误"论处，只登记差异。
> 判定：MATCH / PARTIAL / GAP / CONFLICT / UNKNOWN。

## A. 原则级差异

| # | 基线要求 | 判定 | 差异说明 |
|---|---|---|---|
| A-1 | D01：Analysis 独立一级领域 | **GAP** | 无 analysis schema；分析对象散落 ai/integration/delivery/discovery，三条分析链三种模型（03 文件 §0） |
| A-2 | P1 领域权威独立 | **PARTIAL** | kb/pc 各有唯一写入方与真人门；但"知识"语义三处落点（ai.project_knowledge / kb.memory / pc.cognition_object）互不外键，权威边界靠代码约定 |
| A-3 | P2 来源与结论分离 | **PARTIAL** | kb.source 与 kb.document 无引用列（交叉关系 #1 部分实现）；pc.source/evidence 分离良好 |
| A-4 | P3 AI 生成≠人工确认 | **MATCH** | kb confirm/review 双真人门；pc approve/publish 真人门（principal.kind='human'）；cognition_item 人工决；MCP 无确认工具；CHECK/触发器兜底 |
| A-5 | P4 引用优于复制 | **MATCH** | 四元组软引用（DomainRef）+ 通道状态机；exception：kb.document_revision.content 全文入 PG（本身即正文权威，非复制） |
| A-6 | P5 Markdown+Git=正文权威 | **CONFLICT** | 仅 ai 栈 Git 权威；kb/pc 均为 PG 权威（pc 已显式登记 PCGIT 后置偏差；kb 未登记任何偏差，属未声明偏离） |
| A-7 | P5 PageIndex 可重建派生索引 | **PARTIAL** | FTS 章节索引可重建（published 事件驱动）；向量/嵌入未启用（G-3 门控） |
| A-8 | P6 分析成果可追溯（输入+版本） | **MATCH**（仓库链） | commit_sha/run_mode/since_commit/last_analyzed_commit_sha；ARC-05 输入版本引用 MATCH |
| A-9 | P7 跨域写入=目标领域治理 | **MATCH** | 分析→KB draft→confirm 门；分析→PC proposal→publish 门；无跨域直写 UPDATE |

## B. 红线级差异（ARC-01~15）

| 红线 | 判定 | 依据 |
|---|---|---|
| ARC-02 PC 正式认知人工确认 | MATCH | pc/mod.rs:152-176 + publish.rs:100,626-653 |
| ARC-03 报告不能自动成为正式认知 | MATCH | 仓库链产物=draft+proposal；close 批量发布仍走 pc publish 事务 |
| ARC-05 历史报告保留原始输入版本引用 | MATCH | repo_analysis_report.commit_sha + repository_version_confirmation + cognition_run.repo_versions |
| ARC-10 Run/Report 必须区分 | PARTIAL/CONFLICT | 仓库链区分但**无 analysis_run_id 关联列**（intake 间接）；导入链有 Run 无 Report |
| ARC-11 跨域引用四元组 | MATCH（应用层） | kb-pc-contracts DomainRef；DB 层无 FK（软引用） |
| ARC-13 报告发布≠业务对象确认 | MATCH | close 批量发布针对的是**已 approved 的提案**，未决→withdrawn |
| ARC-14 历史版本不得静默覆盖 | MATCH | 全部 revision 表 append-only+触发器；repo_analysis_report 无版本概念（按一次运行一份规避） |
| ARC-15（Finding 独立生命周期） | PARTIAL | Finding 4+ 套并存、词汇不一致；analysis_run.result 内嵌 findings 无状态 |

## C. 对象/职责混淆问题（审计任务 §八-1~8 逐项）

**C-1 三域对象职责混淆（确认存在）**
- "知识"三落点：ai.project_knowledge（cognition_item 确认落点，6 域枚举与 cognition_item 同词）、kb.memory（新 KB）、pc.cognition_object（正式认知）。互不外键；kb.memory_ref.target_domain 既有 knowledge_base 又有 project_cognition/pc——引用关系存在但对象语义未裁决。
- "发现（finding）"5 处：cognition_item / discovery_finding / delivery.review_finding / design.review_finding / analysis_run.result.findings（jsonb）——状态词汇、大小写、生命周期互不一致。
- "文档/修订"三栈：ai.project_document、kb.document、pc.document——同名同构不同权威（Git vs PG）。

**C-2 正文双重权威（不存在；但存在未声明偏离）**
- kb/pc 正文单份 PG；ai 栈 requirement 投影受哈希不变量保护。真正的问题是 kb/pc **不满足** Git 权威原则（A-6），而非双权威。

**C-3 Analysis Report 缺失/混入其他域（部分存在）**
- 报告正文住在 KB 修订（kb.document_revision.content），报告行只存裸指针（无 FK）。报告域归属未定：表在 integration、正文在 KB、产物在 PC、Run 在 ai（03 文件问题 14）。

**C-4 Finding 与 Report 混合存储（部分存在）**
- 仓库报告的 findings=pc.proposal 行（借 PC 域承载）；导入链 findings 嵌 ai.analysis_run.result jsonb（无独立状态）。仓库链 report↔finding 无关联表列。

**C-5 PC 自动确认/AI 绕过审批（不存在）**
- pc 全链真人门+触发器+乐观锁；MCP 无确认工具；cognition_item/kb 亦人工。DEF-P0-01/02 类缺口在本域未见同类问题。

**C-6 版本追踪缺失（局部）**
- repo_analysis_report 无版本列（"修订 vs 重新分析"无法区分，ARC-10 相关）；kb.document_revision 无回滚通道（ai 栈有 rolled_back）。其余版本链完整。

**C-7 跨项目数据隔离风险（低；两处口径不一致）**
- 新栈每查询双键过滤；旧 ai 栈仅 project_id 过滤（tenant 由 project 派生）——若未来出现跨租户 project 共享/合并逻辑，旧栈无独立防线。pc.source/kb.memory_ref 等软引用无 FK，引用完整性靠应用层校验（registry 校验 domain，不校验目标行存在性——除 pc.evidence document_section 组合键即时校验外）。

**C-8 跨域直接修改他域数据（不存在）**
- 全部跨域变更走目标域账本/门（knowledge_write_operation、pc.write_operation、outbox 事件）。

## D. 待裁决事项输入（供基线 D01~D12）

| 裁决 | 审计事实 |
|---|---|
| D01 Analysis 独立领域 | 三链三模型是最大结构债；统一 Run 模型需裁决 Run→Report 关联列（analysis_run_id）与 Report 正文归属 |
| D05 Team/Project 级 | kb.source 是租户级+source_project 关联；其余对象全部项目级 |
| D07 报告发布人工审核 | 现状=close 真人门+审阅流 v2 暂记；PC 发布另有独立门 |
| 报告正文归属 | 建议列入裁决：留在 kb.document_revision（现状）还是独立 report_revision |
| PCGIT（pc 正文 Git 权威） | 已登记后置；裁决兑现时点 |
| kb.document Git 化 | 未登记偏差，若维持 PG 权威需在基线中豁免或登记 |
| PC 字典承载 KB 关系类型（owner_domain='kb'） | 域字典越位，确认权归属需裁决 |
| kb.memory 审核待办进审阅中心 | D-6 统一待办中心尚未收列 kb.memory_revision |
