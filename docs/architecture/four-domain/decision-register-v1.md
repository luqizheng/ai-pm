# 架构差异与待裁决清单 V1.0（审计产出）

> commit `05ce33d8` · 2026-11-15 · 只读审计产物
> 等级：P0 权威混乱/越权写入/绕过人工审批 ｜ P1 核心边界错误/状态机冲突/历史不可溯 ｜ P2 引用不完整/对象重复/接口缺失 ｜ P3 命名/文档/一致性
> 标注：CONFIRMED（直接证据）/ LIKELY / UNVERIFIED / DECISION_REQUIRED

## 1. P0（4 项）

| Issue ID | 事实 | 证据 | 违反 V2.0 | 策略建议 | 标注 |
|---|---|---|---|---|---|
| P0-1 | **ai.project_knowledge 确认端点无主体门**：decide_item 仅查项目成员可见性，未调 require_human/可信上下文；持 api_key 且为项目成员的 AI 主体可确认认知并写入正式对象 | apps/pm-api/pm_cognition.rs:279-318（对照 baseline 三重门 pm_baseline.rs:183-238）；repo_cognition.rs:263-276 | §7.2 原则 1/7、任务书 §2.3-7 | 补 auth_mode+principal.kind 双门（对齐 pm_baseline 模式）+ 补 AI 拒绝测试 | CONFIRMED |
| P0-2 | **ai.analysis_run 双写入者状态分叉**：pm-ai 用 queued/analyzing/completed/failed；导入链直插 succeeded 终态行；无 CHECK/触发器/乐观锁；start_run 幂等复用只认 completed，succeeded 行游离其语义外 | pm-ai/context.rs:80-165 vs project_import/proposal.rs:312-316；BL:675（无约束） | D02、§3.2 统一契约 status | 补 status CHECK（统一词表）+ 导入链改走 pm-ai 同一入口或 ADAPTER 登记映射（U20） | CONFIRMED |
| P0-3 | **ai.cognition_item 决策无领域审批权威承接**：governance.approval_request 仅服务 change_proposal；cognition 决策既无审批对象也无审计 outbox 事件（三表零触发器零事件） | BL:7202-7222；pm_cognition/repo_cognition 无 notify/outbox | §7.2、B08 | 决策落 governance.audit_log + 审阅中心接入（U25） | CONFIRMED |
| P0-4 | **报告→KB/PC 落点链真实数据未走通**：kb_revision_id/pc_source_id 在唯一真实样本中为 NULL；报告正文借 kb 修订且裸引用无 FK | integration.repo_analysis_report 实测 1 行 failed、引用列 NULL；0023 | §4.5、任务书 §12.3-6 | 端到端演练（V2.0 场景 A/C）前不得宣布闭环 | CONFIRMED（NO_PROD_DATA） |

## 2. P1（9 项）

| Issue ID | 事实 | 证据 | 策略 |
|---|---|---|---|
| P1-1 | ProjectCognition 四死列：project_knowledge.version 恒 1、supersedes_id 零写入；cognition_item.superseded 零写入、has_conflict 恒 false | BL:1101/770；grep 零写入 | 启用或删列（U10） |
| P1-2 | 冲突治理双桩：pc.conflict 无生产写入者（事件常量无生产者、resolve 无入口）；cognition 冲突检测不存在 | tests 直插 pc_proposal.rs:274；kb_pc_handlers.rs:908-922 只消费 | 单独冲突治理设计（U17） |
| P1-3 | repo_analysis_report 无 analysis_run_id、无 Report Revision、cancelled 无写入者 | 0023 DDL；grep | D01 第一刀：补 run 关联列+Revision 设计（U01/U02/U21） |
| P1-4 | Analysis 无独立 Finding/Evidence：5 套异构模型，severity 三口径、大小写混用、result JSONB 无稳定 ID | lifecycle-matrix §2/§5-1 | D03 基础契约（U05/U07），大小写归一可先行 |
| P1-5 | pc L3 冻结守卫为死代码且反向风险：冲突恒空守卫不生效；若手工插入冲突行则无解冻入口（P6 501），永久冻结需 SQL 干预 | pc/proposal.rs:792-798/1030-1039、publish.rs:325-345；P6 501 | 补 resolve 流程后才可依赖 L3 |
| P1-6 | pc.relationship 越位+空转：字典含 owner_domain='kb' 域内边；写入侧不校验 owner_domain；create_relationship_draft 无生产调用者；字典 full-dev 0 行（种子被清库清除） | 0007:63-74；relationship.rs:219-310；实测 count=0 | U18 裁决归属；种子分级防清库丢失（P2 运维） |
| P1-7 | ProjectCognition 溯源弱：evidence 仅 repository_id，无 commit/路径/行号；repo_versions 记 "current" 非具体 revision；确认落 project_knowledge 不复制 evidence | pm_cognition.rs:130-157、repo_cognition.rs:265-268 | ARC-05/§5.4 证据绑定版本要求 |
| P1-8 | DomainRef 六项校验缺四项：type 无注册表、id/revision 无存在性、tenant/project 仅 KB 局部 | relationship-registry §1.2 | 统一引用校验（V2.0 §11 末行） |
| P1-9 | 旧 ai 栈零触发器零 CHECK 守卫层（状态全应用层），与 kb/pc 的 DB 守卫纪律不一致 | ai.* 无 CREATE TRIGGER | 评估补触发器或登记豁免 |

## 3. P2（11 项）

| Issue ID | 事实 |
|---|---|
| P2-1 | kb.memory 生成侧缺位：唯一入口 REST 提案，无事件/命令自动候选；场景 B 无法端到端（U08/U16 相关） |
| P2-2 | 审阅中心不含 kb.memory 审核与 cognition_item 确认（human_action_summary 仅 5 kind） |
| P2-3 | 501 占位 6 端点：/v1/pc/analyses、/decisions、/conflicts/{id}、/versions/{id}；/v1/kb/analyses、/supply-requests fulfil（另 K3 PortNotWired） |
| P2-4 | 双起点/双词表：change_proposal 两写入者不同初始状态（proposed vs redline_pending）；analysis_run completed vs succeeded |
| P2-5 | intake_task.result_ref/signal_ref 手拼 JSON 绕过 payload 校验 |
| P2-6 | 引用三形态并存：TEXT 四元组（pc.source）/裸 UUID（repo_analysis_report 等）/FK（change_proposal）；version_member 域内裸引用 |
| P2-7 | pc.evidence.verification_status 4 态无人工验证程序 |
| P2-8 | 旧栈 ai.knowledge_reference 仅测试调用（DEPRECATE 候选，UNVERIFIED 生产计划） |
| P2-9 | 旧 ai 栈租户隔离仅 project_id 单键（新栈双键），口径不一 |
| P2-10 | 前端 History Drawer 为 SSE 合成只读视图，与权威事件源一致性无校验 |
| P2-11 | Dictionary/种子与业务数据未分级：清库重测清空 pc.relationship_type 种子 |

## 4. P3（4 项）

| Issue ID | 事实 |
|---|---|
| P3-1 | 状态词大小写两套并存（大写：kb.memory_revision/discovery/intake；小写：pc/kb.document_revision/delivery） |
| P3-2 | advisory lock 命名撞车：ai.project_knowledge_write:{project_id} 是知识空间锁，与 ai.project_knowledge 表无关 |
| P3-3 | /v1/knowledge/* 路径不存在（旧栈实际挂 /v1/documents），文档与代码口径需对齐 |
| P3-4 | repo_analysis_report.run_mode 与 error_code/error_detail 在唯一真实样本中语义未经验证（数据不足） |

## 5. 已冻结决策的合规核对（D01-D04/B01-B10）

| 决策 | 判定 | 说明 |
|---|---|---|
| D01 Report Revision+Git 正文 | ✗ 未实现 | AN-REPORT-REVISION 不存在；正文借 kb（P1-3） |
| D02 统一 Run 契约+Adapter | ✗ 未实现 | 无任何 Adapter；analysis_run 分叉（P0-2） |
| D03 Finding 基础契约 | ✗ 未实现 | 5 套模型无共享契约（P1-4） |
| D04 Project Cognition 独立域 | △ 方向成立治理不足 | 独立对象存在但人工门缺失（P0-1）+死列（P1-1） |
| B01-B05（独立/分离/不自动/平级） | ✓ 代码层成立 | AI 提案面收敛、无自动升级路径（全写入链审计零反例） |
| B06 PC 人工门保留 | ✓ | ensure_formal_authorized 三调用点+触发器+CAS 完整 |
| B07 版本化引用优先 | △ | 四元组契约在但校验缺四项（P1-8） |
| B08 审阅中心=入口非权威 | ✓ | 各 kind 操作端点指向各自领域服务 |
| B09 索引可重建 | ✓ | kb/ai section_index+pc.search_tsv 全部派生面 |
| B10 兼容迁移 | ✓（审计自身） | 本审计未动结构；后续实施须遵守 |

## 6. U01~U25 与审计发现的对应（供裁决会引用，审计不代裁决）

- **U01/U02（Run-Report 基数/重跑）**：现状证据=一次 intake 一报告（UNIQUE intake_id）+ 重跑=新报告行+新 KB 修订；无 Revision。→ P1-3。
- **U03/U04（报告状态机/close 语义）**：现状=5 态+cancelled 死态；close=批量发布+锚点固化单事务。→ P1-3、lifecycle §5-3。
- **U05/U06（Finding 契约/身份）**：5 套模型矩阵与大小写清单已备。→ P1-4。
- **U07（Evidence 对象模型）**：pc.evidence 为最完整参照（三形态 CHECK+四元组+hash）。→ P1-4/P2-7。
- **U08（KB Extraction）**：现状=docproc.artifact+source_ref 两列，无对象。→ P2-1。
- **U09（Memory/Knowledge 语义）**：kb.memory 双层版本最成熟；ai.project_knowledge 是另一对象（勿混同）。→ P0-1/P1-1。
- **U10（project_knowledge/cognition_item 映射）**：写入链证据=候选（cognition_item）→确认→正式（project_knowledge），追加式无修订。→ P0-1/P1-1/P1-7。
- **U11（PC↔ProjectCognition 关系）**：唯一代码桥=identity-synthesis 产 PC 提案（R-16）。→ registry R-16。
- **U12（Source/Evidence 边界）**：kb.source（原文池）与 pc.source（DomainRef 锚）同名词异义。→ registry §1.3。
- **U13/U14（Git 迁移顺序/基础设施）**：现状三模式并存；PC 侧换源清单已在 authority-matrix §2 列出。→ 正文权威结论 1。
- **U15（ProjectCognition 审批模式）**：现状弱门；PC proposal→approval→publish 模式为候选参照。→ P0-1。
- **U16（KB 自动化边界）**：现状 KB 提案无门/审核有门；生成侧缺位。→ P2-1。
- **U17（冲突治理共用）**：pc.conflict 桩+cognition 无冲突对象。→ P1-2。
- **U18（关系字典归属）**：owner_domain='kb' 2 行越位+写入侧零校验。→ P1-6。
- **U19（引用失效生命周期）**：revision_missing_reason 机制已存在（pc.source/kb.memory_ref），无统一策略。→ P1-8。
- **U20（Run 状态映射）**：completed/succeeded 分叉是现成反面教材。→ P0-2。
- **U21（Analysis Schema）**：无 analysis_run_id、无 Revision、无 Finding 表。→ P1-3/P1-4。
- **U22（兼容层）**：旧 ai 栈独立运行零互通，兼容面小。→ database-mapping §3。
- **U23（数据迁移）**：全库核心对象 0 行，迁移负担≈0（唯一窗口期红利）。→ 行数事实。
- **U24（端到端验收）**：P0-4 即验收缺口本体；V2.0 场景 A-D 场景化清单就绪。
- **U25（审阅中心接入）**：缺口=kb.memory、cognition_item 两权威未接入（P2-2）。

## 7. 建议实施顺序（最小风险，供参考）

1. **P0-1 主体门补齐**（小改动，纯收严，无迁移）
2. **P0-2 analysis_run 状态词统一**（词表归一+CHECK，存量 0 行无迁移成本）
3. **U05 Finding 词汇归一**（纯命名层，先行铺路）
4. **P1-3 D01 第一刀**：repo_analysis_report 补 analysis_run_id + Report Revision 设计裁决（U01/U02/U21 一起裁）
5. **U10/U15 ProjectCognition 治理裁决**（门模式+死列处置）
6. **U13 Git 正文迁移顺序裁决**（kb→pc→报告 或按消费面排序）
7. **U24 端到端演练**（场景 A/B/C/D）——真实数据走通后才可宣布闭环
