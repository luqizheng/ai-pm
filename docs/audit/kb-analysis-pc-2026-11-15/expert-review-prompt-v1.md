# 三域数据库审计 · 专家裁决提示词（expert-review-prompt-v1）

> 用法：整段复制给专家（人/AI）。专家只需基于本提示词内的事实裁决，无需读仓。
> 事实来源：docs/audit/kb-analysis-pc-2026-11-15/ 只读审计（2026-11-15，全证据带文件:行号）。

---

你是资深软件架构评审专家。以下是对 PM-AI 项目数据库与实现的只读审计结论（事实均已用代码与数据库系统目录实证，非推测）。请逐题给出裁决：**选项 + 理由 + 影响面**。只裁决，不写代码。

## 背景（30 秒版）

- KB/Analysis/PC 三域架构基线 V1.0 为提案（待确认）。审计对照其红线 ARC-01~15 与原则 P1~P7。
- 实现现状：KB 与 PC 两域实现质量高（append-only 触发器 53 个、真人确认门完备、AI 无任何绕过通道）；**Analysis 无独立领域**，散落 4 个 schema；全库生产数据近零（引用链从未在真实数据上走通）——重构窗口期。
- 正文权威现状：旧 ai 栈=Git 权威（DB 只存 sha）；新 kb 栈与 pc 栈=PG content 列权威（Markdown 全文入 PG）。

## P0（阻断三域协作成立，必须裁决）

**Q1. Analysis Run→Report 契约断裂**
事实：仓库分析链有独立报告实体 `integration.repo_analysis_report`（独立 ID/状态机/409 报告门/close 真人门），但**无 analysis_run_id 列**，与执行仅经 intake_id 间接关联；冷启动导入链有 `ai.analysis_run`（repo_import.analysis_run_id 有 FK）但无 Report 实体；任务执行链（delivery.task_run）两者皆无。三条链三种 Run/Report 模型。
选项：A) 给 repo_analysis_report 补 analysis_run_id（对齐导入链先例）；B) 承认 intake 关联为正式机制，不补列；C) 裁决统一 Run 抽象（成本高）。

**Q2. 报告正文归属**
事实：报告行自身无正文/版本列；正文借住 `kb.document_revision.content`（报告行只存裸指针 kb_revision_id，无 FK；唯一真实样本该列为 NULL）。报告"修订 vs 重新分析"无法表达（无版本，重分析=新行）。
选项：A) 维持 kb 修订承载并升级为显式契约（FK 化+基线登记），报告无版本按"一次运行一份"豁免；B) 独立 report_revision 表（自管版本）；C) 报告正文入 Git。

**Q3. "知识"三落点未裁决**
事实：三个知识对象互不外键——ai.project_knowledge（cognition_item 人工确认后落此，旧 ai 域）、kb.memory（新 KB，提案→人工审核→版本号推进）、pc.cognition_object（正式认知，proposal→approval→publish）。AI 确认入口三处均无。但 cognition_item 的确认产物仍落旧 ai 域，不入 kb/pc；kb.memory 与任何分析链零连接。
选项：A) cognition_item 确认落点迁 kb.memory，ai.project_knowledge 退役；B) 保留 ai.project_knowledge 为导入链内部对象，登记边界；C) 三者各自定位（需给出语义分工表）。

## P1（架构债，建议裁决优先级）

**Q4. kb 正文 Git 权威偏离未登记**：kb.document_revision.content 是 PG 权威、canonical_path 是虚拟路径（不对应磁盘文件）、kb_service 全目录无 git 调用。pc 栈同类偏差（git_commit_sha 恒 NULL）已显式登记后置（PCGIT），kb 未登记。选项：A) 登记"KB=PG 权威"为永久豁免；B) 排期 Git 化；C) 仅登记偏差、择期裁决。

**Q5. Finding 碎片化**：5 处 finding 对象（ai.cognition_item / discovery.discovery_finding / delivery.review_finding / design.review_finding / ai.analysis_run.result 内嵌 findings）状态词汇互不一致（如 discovery 用大写 PENDING/CONFIRMED，其余小写），生命周期各异；基线 ARC-15 要求的统一 Finding 独立生命周期（OPEN/ACKNOWLEDGED/…/SUPERSEDED）无法直接落。选项：A) 词汇归一先行（低成本）；B) 统一 Finding 表；C) 维持碎片+登记分工。

**Q6. pc.conflict 全桩**：表+冻结逻辑（frozen_by_conflict 三处 403）在，但生产无写入者，事件 producer 标"pc 桩"。选项：A) 排期实做；B) 明确降级为非目标。

**Q7. PCGIT 兑现时点**：pc.write_operation 有 pc_commit_* 命令与 git_committed 状态，均不可达。选项：A) 下一个里程碑兑现；B) 长期后置。

## P2（登记即可，不需深裁，但可给优先级）

Q8 kb.memory 待审未接入统一审阅中心（六 kind 之外）；Q9 /v1/pc/analyses、/v1/pc/decisions 501 占位；Q10 pc.relationship_type 承载 owner_domain='kb' 的 KB 域内边（字典越位，确认权归属）；Q11 旧 ai 栈租户隔离仅靠 project_id（新栈双键）；Q12 全链路零生产数据——建议先走通"仓库分析→报告→KB 确认→PC 提案→审批→发布→close"一整条再动结构。

## 已达标项（无需裁决，供确认）

ARC-02（PC 人工确认）/ARC-03（报告不自动成认知）/ARC-05（输入版本引用）/ARC-11（四元组跨域引用）/ARC-13/ARC-14 均 MATCH：AI 只能提案（origin_kind=ai_agent）、审批行不可变触发器+三重乐观锁、跨域写入全走目标域账本、历史版本 append-only。

## 输出要求

1. 每题一行裁决结论 + ≤3 行理由。
2. Q1~Q3 必须给出明确选项（可组合，组合须自洽）。
3. 若认为某"事实"判断有误，指出需复核的具体证据点。
4. 最后给一个实施顺序建议（Phase 化，标注每 Phase 是否需要新迁移）。
