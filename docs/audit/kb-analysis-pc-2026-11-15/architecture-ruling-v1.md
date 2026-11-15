# 三域架构裁决记录 V1.0

> 状态：**已裁决（有条件通过）** · 裁决日期：2026-11-15
> 依据：[00 总报告](00_summary_report.md) + [expert-review-prompt-v1.md](expert-review-prompt-v1.md)（专家仅基于提示词内已实证事实裁决）
> 性质：**架构裁决记录，不是实施设计**。Q1/Q2/Q3 共同决定 Analysis 与 KB/PC/Project Cognition 的对象引用结构——须在下一轮形成正式目标对象模型后，再编制 Migration 任务书。

## 0. 裁决立场

有条件通过三域架构方向，**不建议立即大规模数据库重构**。总体原则（裁决原文）：

1. KB、Analysis、PC 保持独立领域身份。
2. **Project Cognition 不因当前位于旧 ai schema 而混入 KB 或 PC**（四域事实格局获认可）。
3. 统一业务契约 ≠ 统一数据库表。
4. 正式内容权威、执行状态权威、人工决策权威三者分别明确。
5. 先打通真实业务链，再决定结构迁移。

## 1. P0 裁决

### Q1 Run→Report 契约 → **B（阶段性采用 intake 关联）**

- 当前以 `intake_id` 为正式关联机制，**不立即**给 repo_analysis_report 加指向 ai.analysis_run 的 FK——两者目前不是同一执行模型，强行关联会制造错误语义。
- 未来统一 Run 时先明确三者不可互相替代：**Run=执行活动统一身份；Intake Task=导入编排/业务工作流实体；Report=Analysis 业务结果实体**。
- 后续建统一 Run Contract，经适配器关联现有执行实体，再决定是否加直接引用。

### Q2 报告正文归属 → **B+C 组合**

- Analysis 独立拥有 **Report + Report Revision** 业务身份；正文采用 **Git 内容权威**，DB 只存 Revision 身份/状态/Git 引用。
- Report Revision **不得依附** KB Document Revision；兼容历史 `kb_revision_id` 但不再作为长期权威引用。
- 目标关系：`Analysis Run → Analysis Report → Report Revision 1..N（各钉 Git Commit）`；Report 下挂 Findings / Conclusions / Recommendations。
- **重新分析（新 Run，可产生新 Report/轮次）与修订报告（同 Report 身份下新 Revision）是两种不同操作，不得混淆**（落实 ARC-10）。

### Q3 知识三落点 → **C（各自定位，不迁移合并）**

| 对象 | 领域归属 | 核心职责 | 权威内容 |
|---|---|---|---|
| ai.project_knowledge | **Project Cognition** | 项目技术认知（架构、技术约定、仓库事实） | 后续按其正式内容方案定 Git/Markdown 权威 |
| kb.memory | KB | 可复用知识、经验、规则、方法 | KB 正式知识内容 |
| pc.cognition_object | PC | 产品定位、业务领域、能力、模块、角色、流程、业务规则 | PC 正式认知内容 |

- 关键判词：**项目技术认知 ≠ 通用知识 ≠ 产品认知**。
- cognition_item 确认产物**不迁入** kb.memory；若具跨项目复用价值，由 KB 后续形成**独立 Memory 提案**（人工），不得自动转化。
- 三者各自拥有确认、版本、生命周期；须补充跨域引用与转换规则。

## 2. P1 裁决

| 题 | 裁决 | 要点 |
|---|---|---|
| Q4 kb 正文 Git 权威 | **B 排期 Git 化** | 不批准 PG 权威永久豁免；迁移前 PG 为现行权威，**迁移期间禁止双写/双主** |
| Q5 Finding 碎片化 | **C 保留领域独立 + 公共 Finding Contract** | 不统一表、不强制统一生命周期；合约统一稳定身份/来源/证据/严重度/关联引用/事件表达，状态经领域适配器映射。成员：Discovery Finding、Design Review Finding、Delivery Review Finding、Analysis Finding、Project Cognition Item（**最后一项是否属于 Finding 依业务语义定，不强改**）。状态词汇先统一 API/Event 外部表达与映射，不批量改历史 DB 状态 |
| Q6 pc.conflict 桩 | **A 排期实做** | 完整闭环五要件：Detection / Review / Resolution / Freeze Lifecycle / Audit——只加生产写入者不算完成 |
| Q7 PCGIT | **A 下一个 PC 内容权威迁移里程碑兑现** | git_committed 不可达状态不得当作已实现 |

## 3. P2 裁决

| 题 | 裁决 |
|---|---|
| Q8 kb.memory 进审阅中心 | 保留独立审核类型，**排期接入**统一入口（审阅中心管入口，KB 管审核语义） |
| Q9 PC 501 占位 | 保留占位、登记未实现；先定业务契约/权限/对象模型再实施 |
| Q10 owner_domain='kb' | **清理归属规则**（暂不删数据）：KB 关系类型由 KB 领域定义管理；跨域关系须明确定义/确认/维护主体 |
| Q11 旧 ai 栈租户隔离 | **升级 P1 安全复核**，按证据修复；检查面=项目归属校验/跨租户引用/后台任务/MCP/API/DB 访问路径；若存在跨租户访问可能 → **升 P0** |
| Q12 零生产数据 | **先 E2E 后重构**——当前最重要实施约束 |

## 4. 已达标项确认（附两项澄清）

ARC-02 / ARC-03 / ARC-05 / ARC-11 / ARC-13 / ARC-14 的 MATCH 结论接受，持续用回归测试保护。两项强制澄清：

1. **PC 人工确认完备 ≠ Project Cognition 人工确认完备**——不同领域、不同确认入口，审计结论不得互相借用（对应四域审计 P0"ProjCognition 确认无主体门"）。
2. **数据口径四分**：有生产数据 ≠ 有真实业务链路数据 ≠ 非孤立样本 ≠ 端到端成功样本。存在一条记录不等同业务闭环已验证。

## 5. 目标架构（裁决附图文字化）

```
┌─ KB（知识权威）──────┐  ┌─ Analysis（分析权威）─┐  ┌─ PC（产品认知权威）──┐
│ Source / Document /  │  │ Run / Report / Finding │  │ Proposal / Object /  │
│ Memory               │  │                        │  │ Version              │
└──────────────────────┘  └────────────────────────┘  └──────────────────────┘

Project Cognition（独立项目技术认知）
  保留自身 Proposal / Formal Object / Evidence / Version 语义，
  不因三域数据库审计而并入 KB 或 PC。
                    ⇕
        公共基础契约（业务契约，非公共表）
        Run Contract · Finding Contract · Evidence Reference · Human Decision Contract

基础设施与权威分工：
  Git/Markdown  = 正式内容权威（按领域迁移完成情况生效）
  PostgreSQL    = 身份、状态、版本引用、审批、审计
  Object Storage= 原始文件与转换产物
  Index         = 可重建的派生数据
```

核心：三域不共享同一套数据模型，而通过**稳定引用、明确权限、可追溯的人工决策**协作。

## 6. 实施顺序（六 Phase）

| Phase | 内容 | 新 Migration | 完成标准 |
|---|---|---|---|
| P0 | 核实 Q1~Q12 证据、补 E2E 测试设计、冻结风险分级 | 否 | 裁决与实施范围冻结；**不做迁移、不允许边审计边改 Schema** |
| P1 | 打通 仓库分析→报告→KB 确认→PC Proposal→审批→发布→Close | 视缺口，优先不新增 | 至少一条真实完整业务链成功；**不为测试绕过人工确认**；先找真断点再定新表 |
| P2 | Analysis Run/Report 关系、Report Revision、Finding Contract（整体设计，**不先单独加 analysis_run_id**） | 预计是 | Analysis 对象身份与版本引用完整 |
| P3 | KB Git 权威迁移 | 是 | KB 正文可由 Git 追溯，PG 不再是正文权威 |
| P4 | PCGIT 兑现 + Conflict 完整流程 + 审阅中心接入 | 是 | PC 内容权威与冲突治理闭环 |
| P5 | 旧 ai 栈收敛、租户隔离加固、旧状态清理 | 视审计结果 | 无未经批准的旧写入通道；退役前须证明无生产调用/历史依赖，并备迁移与回滚方案 |

P3/P4 约束：分领域各自迁移，须有 Git Commit ↔ DB Revision 一致性方案、失败恢复、历史兼容；全程单一权威，**禁止长期双主**。

## 7. 三个不可变更的实施决策边界（程序员不得自行改变）

1. **不合并三种知识对象**（project_knowledge / kb.memory / pc.cognition_object 保持独立业务身份）。
2. **公共合约 ≠ 公共数据库表**（Run 与 Finding 同此）。
3. **不允许 Git 与 PostgreSQL 长期双重正文权威**（每领域须明确当前权威/迁移期权威/最终权威）。

## 8. 裁决汇总

| Q1 | Q2 | Q3 | Q4 | Q5 | Q6 | Q7 | Q8 | Q9 | Q10 | Q11 | Q12 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| B | B+C | C | B | C+合约 | A | A | 接入 | 保留 | 修订归属 | P1 安全复核 | E2E 先行 |

**下一步**：下一轮产出正式目标对象模型（Analysis 对象引用结构 + 四域边界 + 公共契约定义）→ 再编 Migration 任务书。本记录不直接作为实施设计使用。
