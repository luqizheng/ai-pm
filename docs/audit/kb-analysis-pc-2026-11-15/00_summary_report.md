# PM-AI KB / Analysis / PC 三域数据库现状审计总报告

> 2026-11-15 · 只读审计（未修改任何代码/结构/数据）· 对照《三域对象边界与协作架构设计基线 V1.0》（提案待确认）
> 分册：01 清单 / 02 KB / 03 Analysis / 04 PC / 05 交叉关系 / 06 存储权威 / 07 差异清单 / 08 证据索引

## 1. 当前实际架构概述

数据库 19 schema / 209 表 / 2228 列 / 1398 FK / 53 触发器 / 2 视图 / 0 ENUM（全部 text+CHECK）。迁移唯一机制 sqlx，master 24 条全绿。三域相关现状：

- **KB**：新旧两栈并存。旧 ai 栈（ai.project_document/document_revision）Git 权威，服务需求/ADR/设计文档；新 kb schema（document/memory/source 三壳）**PG 权威**，append-only+真人确认门+章节 FTS 索引。实现质量高，人工门完备。
- **PC**：单栈（pc schema），**提案→审批→发布事务**全链实现且守卫最严（触发器+CHECK+三重乐观锁+真人主体强制），正式认知 vs 草稿分离达成基线红线。唯二缺口：正文 Git 权威后置（PCGIT 已登记）、conflict 链是桩。
- **Analysis**：**无独立领域**。三条互不相同的分析链（仓库再分析 / 冷启动导入 / 任务执行评审）散落 4 schema，Run/Report/Finding 模型三套各异；独立 Report 实体（repo_analysis_report）有 ID 有状态机但无正文无版本，正文借住 KB 修订。

数据现状：库刚清空重测，三域真实数据几乎为零（kb 1 篇快照、pc 1 行 source、报告 1 行 failed、导入 6 行）。**三域引用链尚未在真实数据上完整走通一次**。

## 2~4. 三域实现情况

详见分册 02 / 03 / 04。速览：

| 维度 | KB | Analysis | PC |
|---|---|---|---|
| 对象完备度 | Source✓ Document✓ Knowledge✓ Extraction(借docproc)△ | Run△(3套) Report△(仅仓库链) Finding△(5套) | Object✓ Revision✓ Version✓ Candidate(=draft)✓ Relationship✓ Evidence✓ |
| 人工确认 | ✓ confirm/review 双门 | 报告 close 真人门；cognition_item 人决 | ✓ proposal→approval→publish |
| AI 绕过 | 无 | 无 | 无 |
| 正文权威 | PG（偏离基线） | 借 KB | PG（PCGIT 后置） |
| 版本管理 | ✓（append-only+触发器） | 报告无版本 ✗ | ✓ 双层 |
| 生产数据 | 1 行 | 8 行 | 1 行（+字典） |

## 5. 三域对象关系图（实际存在的关系）

```
[integration.project_repository] --commit_sha--> [repo_analysis_report]
        |                                            | intake_id(间接)
        |                                            | kb_revision_id/pc_source_id (裸引用,样本NULL)
        v                                            v
[repo_import] --analysis_run_id FK--> [ai.analysis_run]   [kb.document_revision.content]<==报告正文
        |                                    |
        v (FK)                               v result jsonb{findings,proposals}
[repo_import_source(_attempt)]          [ai.change_proposal] --approval_request--> 人工
        ^ (FK)                                                    |
[discovery_finding_evidence]                                      v executed
                                                        core.project 概要落地
仓库分析链：
[repo_analysis_report] → intake_task(kb_analyze/pc_analyze)
    → kb_pc_analysis.insert_kb_revision → kb.document_revision(draft) --confirm 真人门--> published
    → kb_pc_analysis.derive_pc_cognition → pc.source / pc.evidence / pc.cognition_object(draft)
                                          → pc.proposal(ai_agent) --人工 approval--> publish_version(真人门)
                                            → pc.cognition_version/version_member (active)
[kb.memory(_revision/_ref)] ← 独立提案/审核（未与任何分析链连接）
[ai.cognition_item] --人工确认--> ai.project_knowledge（与 kb.memory、pc 互不外键）
```

## 6. 正文权威关系

| 栈 | 权威 | 风险 |
|---|---|---|
| ai（旧） | Git（DB 存 sha，无正文列） | requirement 投影靠哈希不变量维护（rebuild_projection 兜底） |
| kb | PG content（Markdown 全文） | 无 Git 通道、无偏差登记 → 与基线 P5 CONFLICT |
| pc | PG content（git_commit_sha 恒 NULL） | PCGIT 已显式登记后置 |
| 对象存储 | docproc→file-service（RustFS） | kb.source 不走对象存储 |
| PageIndex | 章节 FTS 可重建（kb published 事件驱动）；向量未启用（G-3 门控） | 无双重权威；索引含章节正文副本 |

## 7. 版本管理情况

append-only+守卫触发器是全仓纪律（kb/pc/delivery/design/docproc/viz 共 53 个触发器），ARC-14 MATCH。唯一无版本概念的对象是 repo_analysis_report（"一次运行一份报告"规避）与 cognition_version 快照链（健全）。kb.memory_revision 是唯一双层版本（提案序+审核通过版本号）。

## 8. 跨域写入和权限机制

- 写操作账本三套：ai.knowledge_write_operation（旧栈，幂等 operation_id+项目级写锁）、pc.write_operation（发布幂等）、ai.change_proposal（导入链，配 governance.approval_request）。
- 真人门判定统一 `principal.kind='human'`（ensure_formal_authorized / require_human / confirm_revision 三重门）。
- MCP/执行端无确认凭据；前端无绕过通道。
- 跨域引用统一四元组（kb-pc-contracts DomainRef）+ 通道状态机（NotYetOpen/PermanentlyExcluded），DB 层无 FK（软引用）。

## 9. 与 V1.0 基线差异（Top 项）

| 判定 | 内容 |
|---|---|
| MATCH | P3/P4/P7/A-8、ARC-02/03/05/11/13/14——确认门、引用优先、可追溯、目标域治理 |
| PARTIAL | A-2/A-3/A-7、ARC-10/15——Run↔Report 无关联列；Finding 5 套并存；PageIndex 只有 FTS |
| GAP | D01（Analysis 非独立领域）；KB Document→Knowledge 提取关系；报告版本管理；memory 审核待办未进审阅中心；向量通道 |
| CONFLICT | P5 正文 Git 权威（kb/pc 两栈 PG 权威；kb 未登记偏差）；报告正文住在 KB、域归属未定 |
| UNKNOWN | （基线中属待讨论设计建议的项，不判定对错，见 07 §D 裁决清单） |

## 10. P0 / P1 / P2 问题清单

**P0（阻断三域协作成立）**
1. **Analysis 三链三模型**：仓库链有 Report 无 Run 关联列（intake 间接）；导入链有 Run 无 Report；任务链两者皆无。做三域协作前必须统一 Run→Report 契约（加 analysis_run_id 或裁决 intake 关联为正式机制）。
2. **报告域归属未定**：正文借住 kb.document_revision（报告行裸指针无 FK），kb_revision_id/pc_source_id 在唯一真实样本中为 NULL——报告→KB/PC 的落点链路未真实走通。
3. **"知识"三落点未裁决**：ai.project_knowledge / kb.memory / pc.cognition_object 互不外键，cognition_item 确认仍落旧 ai 域（不入 kb/pc）。

**P1**
4. kb 正文 Git 权威偏离未登记（PCGIT 至少有显式登记）。
5. Finding 词汇/生命周期碎片化（5 套、大小写不一致），基线 ARC-15 要求的统一 Finding 独立生命周期无法直接落。
6. pc.conflict 全桩：表+冻结逻辑在，检测无生产写入者。
7. 报告无版本（修订 vs 重新分析不可表达，ARC-10 关联问题）。

**P2**
8. kb.memory 审核待办未接入统一审阅中心（D-6 六 kind 之外）。
9. /v1/pc/analyses、/v1/pc/decisions 501 占位。
10. pc.relationship_type owner_domain='kb' 承载 KB 域内边（字典越位）。
11. 旧 ai 栈租户隔离仅靠 project_id（新栈双键），口径不一。
12. 全链路零生产数据：PC 确认链、报告 close 批量发布、memory 审核从未在真实数据走通。
13. kb.memory_revision.version_status='MERGED' 预留无实现。

## 11. 建议后续架构裁决事项

1. **Analysis 收敛**（D01 落地第一刀）：统一"一次分析执行=Run"——推荐给 repo_analysis_report 补 analysis_run_id（对齐 repo_import 先例），并把"报告正文=kb 修订"升级为显式契约（FK 化或登记）。
2. **报告正文归属**：维持 kb.document_revision 承载（推荐，复用确认门与索引管线）并在基线登记，或独立 report_revision（成本高、收益存疑）。
3. **知识对象归一**：裁决 ai.project_knowledge 的退役路径（cognition_item 确认落点迁 kb.memory 或保留为导入链内部对象）。
4. **PCGIT 兑现时点**与 **kb PG 权威豁免/登记**。
5. **Finding 统一词汇**（大小写归一 + 状态机对齐基线 7 态）——低成本高收益，可先行。
6. **owner_domain='kb' 关系类型的确认权归属**。
7. **真实数据演练**：P0-2 的验证即"full-dev 重测"清单项——走通 仓库分析→报告→KB 确认→PC 提案→审批→发布→close 一整条。

---
*审计过程零写入：所有 DB 访问为只读 SELECT/系统目录查询；原始导出留存于审计工作目录（out-01~out-12）。*
