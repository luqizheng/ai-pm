# Phase 3 — 专家反馈包 V1.0

- 日期：2026-11-16 · 提交人：Phase 3 设计/评审组
- 依据：基线 V0.1 + 基线评审意见 V1.0（源码核验，基准 d3b30739 / 注册表 05ce33d8）+ 基线校准与裁决台账 V1.0
- 用途：向专家通报本轮结论，并请求对 §B 六项给出意见（确认 / 反对 / 补充）

---

## A. 通报事项（本轮已定，无需再议）

| # | 事项 | 结论 |
|---|---|---|
| A-1 | 三域基线 V0.1 评审 | READY WITH CONDITIONS，已接受；P3-D01 保留为记录非闭合基线 |
| A-2 | KB 草案四修正 | Git=目标态非现状约束；不宣称 kb.review 对象；Memory↔Document 不假定已有引用能力；Proposal 双轨不合并。绑定表（document/memory_project_binding）暂缓。原则：**产品对象 / 现行库对象 / 目标架构对象分述** |
| A-3 | P3-11 方向 | 方案③为正式裁决起点：**共享数据库基础设施 ≠ 共享领域权威**（五边界）；仓库分析产物按**业务含义**分流（技术栈/模块结构/架构/技术约定→ProjC；业务能力/规则/产品模块→PC）；**Finding 仅候选，不自动成正式认知** |
| A-4 | 导航 | 恢复冻结 IA V1.2（四域入口在项目区）；reports 页签按 Phase 2 D-1 保留；Team 级 KB 全局入口如要做须单独裁决（Team 级=资产归属，Project 级=导航入口，不同维度） |
| A-5 | P3-KB-01 | 已确认（Team 级统一资产，Project 作关联范围），不重开；下一步与 kb.source_project 真实实现对齐 |
| A-6 | KB P3-D02 骨架 | Memory=独立审核确认的正式知识单元，Document 发布≠Memory 成立；Extraction 六步链路入 V1；四红线（Revision/人工审核/Accept≠Confirm/禁直写）；require_human+K4 CAS+行锁+触发器白名单作为既有实现约束继承 |
| A-7 | 优先级 | P0=P3-11+P3-08 → P1=D02/D03/D04 → P2=导航拆分+统一 UX。P3-11 未决前不冻结 PC/ProjC 边界 |
| A-8 | 编号 | 台账为 Phase 3 编号真源；P3-03/04/05/10 语义相对基线 §9 有漂移（注记在台账 §7） |

## B. 请专家给出意见的事项

### B-1【最重要】P3-11 子题①：物理复用形态

pc.* 核心对象全库 0 行（迁移红利窗口）；`pc.object_type`/`pc.cognition_object` 均无 owner_domain 列，加列须穿透发布事务、CAS、触发器与 CHECK。

- 我方建议：**独立 schema（projc.*）复用 DDL 模式与服务层模式，优先于 pc.* 加域判别列**；理由=无数据迁移成本（新项目禁兼容分支纪律）+ 各域状态机显式。
- **请专家重点评估**：独立 schema 对 **跨域关系遍历与统一检索** 的影响——0007 已终裁 relationship_type.owner_domain=确认权威域 + 双端隔离遍历；跨 schema 后 DomainRef 四元组、FTS/PageIndex、未来 P3-10（跨域搜索）的实现路径是否仍然成立，是否需要检索层适配。

### B-2 P3-11 子题③：过渡期处置

裁决落地前，现行 close_report→publish_version 链仍以 PC charter 写 snapshot 类对象（repository/module/tech_stack/delivery_activity）。

- 我方建议：审阅流 v2 确认/发布 UX 不动（Phase 2 冻结语义），仅落点域按裁决切换；不立即冻结写入。
- 请确认：过渡期「snapshot 类继续写 pc.*」是否可接受，还是需要立即收紧。

### B-3 delivery_activity 归属

`pc.object_type` snapshot 层的 `delivery_activity`（近期 commit 流摘要，checkpoint-1 派生）更像交付动态指标而非认知对象。

- 我方倾向：不作为 Project Cognition 认知对象（可留 Analysis 快照能力或裁掉）。
- 请专家裁定去留。

### B-4 Technical Conflict 归属（P3-05）

`pc.conflict/conflict_object` 表在 PC schema、无生产写入者，且注册表警示：若手工插行将触发 L3 冻结守卫且**无解冻入口**。

- 选项：沿用 pc.conflict（改语义挂 ProjC）vs ProjC 自建冲突对象。
- 请专家结合「冲突治理需要跨域证据」的特点裁定，并给出 pc.conflict 死表/死守卫的处置口径。

### B-5 P3-KB-02：Team Knowledge Visibility

P3-KB-01 已确认 Team 级统一资产，但可见性未裁决、不预设方案。

- 请专家给出候选维度：Team 级资产默认对谁可见（Team 成员/项目成员/两者）、项目关联（source_project）是否引入可见性门、与现有成员可见性模型（K4）如何衔接。

### B-6 上轮专家议题的衔接确认

| 上轮议题 | 本轮进展 | 请确认 |
|---|---|---|
| Q1（Document→Memory 断链） | KB-01 审计结论被采纳：Extraction 概念对象补 STUB、生成链路入 P3-D02 V1 范围 | 断链修复口径是否认可 |
| Q2（Git 内容权威） | 维持「目标态非现约束」；KB-02 两硬阻碍（0023 不可变触发器 / 发布事务 hash 锚链）认账不排期 | 是否同意 Git 迁移不进 Phase 3 关键路径 |
| KB-01~09 建议裁决（未冻结） | P3-D02 重整后可一并冻结其中 KB 域条目 | 哪些条目随 P3-D02 冻结、哪些保留待 P3-08 |
| 四动作映射 | Accept/Approve/Confirm/Apply 须映射到既有 DecisionBody action 枚举（confirm/reject/defer/investigate），不新造平行动词 | 映射方向是否认可 |

## C. 附：本轮文档序列（同目录）

1. `Phase3-三域产品设计基线-V0.1.md`（记录）
2. `Phase3-基线评审意见-v1.0.md`（源码核验评审）
3. `Phase3-基线校准与裁决台账-v1.0.md`（校准决定 + 台账真源 + P3-11 输入包）
