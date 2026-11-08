# P2-13 × PC-I6：Cognition/KB 域引入 LLM 执行体裁决包 V1

> 状态：已裁决 · 2026-11-08
> 所属产品：smark-pm / PM-AI
> 目的：裁决「git URL 介入仓库 → LLM 接管分析 → 写入 PC/KB」能力（P2-13 挂账项）的立项与 V1 起笔边界，随后按裁决实施 PC-I6 起笔切片。
> 依据：`ai-pm-project-ai-team-v1-vs-code-comparison-v1.md` §七 P2-13 / §八；`product-cognition-detailed-requirements-v1.1/09-implementation-and-baseline.md` PC-I6；`kb-pc-independent-services-architecture-v1.1.md` §10.2/§11/§20；代码事实见各节（行号以 master=8d81fbd 后工作区为准）。
> 裁决方式：直接回复，如「D-1=A，D-2=A，D-3=A，D-4=全同意」。每项已给推荐及理由。

## §〇 裁决记录

| 编号 | 裁决项 | 用户裁决 |
| --- | --- | --- |
| D-1 | Cognition/KB 域是否引入 LLM 执行体（P2-13 本体） | **A 引入，治理不变**（2026-11-08） |
| D-2 | V1 起笔切片的 LLM 产物边界 | **A 快照分析增强切片**（结论恒 PROPOSED，三区分类+证据约束；三态结论/冲突发现留后续切片） |
| D-3 | LLM 调用失败的降级语义 | **A 回退结构化派生+analysis_mode 标记**（无 provider 同语义；绝不回退 Fake fixtures） |
| D-4 | 附带三项 | **全同意**（a KB 信号保守固定 / b Role Catalog 不随本裁决扩 / c 本批=PC-I6 起笔切片非全量） |

## §一 裁决总览

| 编号 | 裁决项 | 选项 | 推荐 |
| --- | --- | --- | --- |
| D-1 | P2-13 本体：引入 LLM 执行体 | A 引入（治理不变）/ B 不引入 / C 仅 KB 侧增强 | **A** |
| D-2 | 产物边界 | A 快照分析增强切片 / B 一步到位 PC-I6 全验收面 | **A** |
| D-3 | 失败语义 | A 回退结构化派生+标记 / B FAILED 人工重试 | **A** |
| D-4 | 附带 | a KB 信号维持保守固定 / b Role Catalog 不随本裁决扩 / c 本批=起笔切片声明 | **全同意** |

## §二 D-1：Cognition/KB 域是否引入 LLM 执行体（P2-13 本体）

**现状（事实）**

- P2-13 原文（`ai-pm-project-ai-team-v1-vs-code-comparison-v1.md:193`）：「Role Catalog 扩到稿的 13 角色口径时，Cognition/KB 域是否引入 LLM 执行体——**新能力决策，不是补名**。」裁决入口在 §八（:215），状态挂账。
- 现行链路确定性无 LLM：`crates/pm-infrastructure/src/kb_pc_analysis.rs:6,11` 头注明示「无 LLM」「真实文档分析本体归 Phase 4」；KB digest 尾部自带「无 LLM 参与」声明（:272）；产出全 draft/unverified/PROPOSED。
- 类型层面 pm-infrastructure 不依赖 pm-ai；LLM 能力走端口模式，先例=`RepoAnalysisModel` trait 定义在 pm-application（`repo_facts.rs:300-307`），pm-ai 提供实现。
- 前置已全部就绪：0043 项目级模型策略已落地（`ai.project_ai_model_policy`，指定不可用严格不回退）；Task 17 真模型接线已完成（0153ca5，`RoutedAnalysisModel::from_env` 按项目三级解析 Provider）；provider 三级解析 `team_llm::resolve_repo_call_config`（team_llm.rs:1140-1218）；`ai.llm_call_record` 调用记账（0041）；wiremock+sqlx::test 测试先例（analysis_model.rs:353-425）。
- 治理协议已裁且代码已立：Q-PC-12=A（KB 已审核知识进 PC 草稿/提议）；ADR-02 三分法（KB 产信号→Platform 路由→PC 判断，产出止步 Proposal）；L0~L3 审批矩阵（Q-PC-06，L2=「AI 只能提案，由 Lead 或审批人确认」）；fail-closed 正式写入闸+「MCP 不暴露绕过人工确认的正式确认工具」（05 稿 §10.8:161-172）；propose/confirm 分离已实现（比对报告 :119）。
- 用户产品诉求：git URL 介入仓库时 LLM 接管分析并写 PC/KB——Checkpoint-1 已落触发链与写入链（0053），唯分析环节为结构化派生。

**选项**

- **A（推荐）引入，治理不变**：LLM 作为「分析执行体」接入 verify→intake→KB/PC 链的 KB 分析步与 PC 分析步；产物止步 draft/PROPOSED；正式确认闸零改动；结构化派生保留为降级路径（见 D-3）。
- **B 不引入**：V1 维持结构化派生；PC-I6 挂起。代价：产品基线 PC-I0~I6（Q-PC-10 已裁采用）缺尾切片；「LLM 接管分析」诉求落空；KB digest 永远是纯统计摘要。
- **C 仅 KB 侧增强**：LLM 只增强 KB digest 正文，PC 保持纯结构化。代价：PC 认知对象 summary/content 仍为模板句式，「分析出并写 PC」半落空；KB/PC 两侧能力不对称，后续仍需二次裁决。

**推荐：A。** 理由：① 裁 A 是兑现既有产品基线（Q-PC-10 已裁 PC-I0~I6、PC-I6 依赖的项目级模型策略已落地），不是越权新增；② 全部工程前置就绪（路由/凭证/记账/测试先例），起笔成本低；③ 直接回应用户产品诉求；④ LLM 无确认权面——治理红线（AC-015-02/AC-016-02/fail-closed）全部已裁且已实现，LLM 只碰 draft 面。

**影响面/解锁**：裁 A 解锁 PC-I6 起笔；不触动 KB-PC 13 项 ADR 与流程 A~F；不产生新迁移义务（若产物需新增 analysis_mode 等字段而 0053 列不够，动 0057 空号）。

## §三 D-2：V1 起笔切片的 LLM 产物边界

**现状（事实）**

- PC-I6 全验收面（`09-implementation-and-baseline.md:132-151`）=KB 知识输入/认知对象提取建议/变更提议/Requirement Revision 影响/Design·ADR 影响/完整性检查/一致性检查/主动认知维护——远大于仓库快照分析一个场景。
- PC 分析结论三态 PROPOSED/NO_CHANGE/CONFLICT（kb-pc v1.1 :789）；现结构化派生恒 PROPOSED（Checkpoint-1 记录 objects=8 PROPOSED）。
- PC-I6 验收五条（:147-151）：可追溯建议 / AI 不自动确认 / 区分事实·推断·不确定 / 影响分析有证据和关联对象 / 正式变化遵循审批矩阵。
- 冲突发现依赖认知对象间关系建模（03 稿八视角×认知对象矩阵），当前 0053 为最小子集、未建关系面。

**选项**

- **A（推荐）起笔切片=仓库快照分析增强**：对象形态不变（repository/tech_stack/delivery_activity/module 四类，`kb_pc_analysis.rs:513-597`），summary/content 由 LLM 生成；强制三区分类（事实/推断/不确定）；证据引用只许来自 RepoFacts 真实存在的 commit sha/文件路径（沿用 `REPO_SYSTEM_PROMPT` 注入安全边界先例，repo_model.rs:16-29）；结论恒 PROPOSED。NO_CHANGE/CONFLICT 判定与影响分析族留后续切片。
- **B 一步到位 PC-I6 全验收面**：含三态结论+冲突发现+需求/Design·ADR 影响分析。代价：须同时动 pc.proposal 八态域、L3 冻结联动、关系建模，一批跨 PC 治理核心，风险与批量大。

**推荐：A。** 理由：① Checkpoint-1 已立的四类对象+draft/PROPOSED 形态零破坏，LLM 只替换「内容生成器」，链路与幂等语义不动；② PC-I6 五条验收本批兑现前三条（可追溯/不自动确认/三区分类），其余属剩余面，符合「I6 依赖前述模型和治理能力」的渐进语义（09 稿 :165）；③ 关系面未建，强上冲突判定必然虚构——AC-019 质量红线不容。

## §四 D-3：LLM 调用失败的降级语义

**现状（事实）**

- intake L2 状态机：FAILED→人工 retry 原行 attempt+1、幂等键不变、Inbox 去重（kb-pc v1.1 :711-728；intake.rs:995-999）；processing.failed 闭环已有（kb_pc_handlers.rs:814-851）。
- 先例分歧：主链 RoutedAnalysisModel 无 provider→回退 **FakeModel（fixtures 假数据）**+大声 WARN（analysis_model.rs:112,146,156）；冷启动 import 无 provider→回退 `FakeRepoAnalysisModel`（repo_model.rs:256-280）。
- **关键差异**：fixtures 假数据对快照分析不适用——KB/PC 产物会进入治理面供人审，fixtures 内容与真实仓库无关，属数据污染。快照分析的可回退降级只能是结构化派生（真实事实，无虚构）。

**选项**

- **A（推荐）回退结构化派生+标记**：无 provider 或调用失败（超时/BadOutput 重试穷尽）→落结构化派生产物，产物 metadata 标 `analysis_mode=derived_fallback`，同时 `ai.llm_call_record` 记 failed（purpose 区分）。LLM 成功则标 `analysis_mode=llm`。
- **B 严格 FAILED**：配置了 provider 但调用失败→任务 FAILED→人工重试。好处：不沉默降级；代价：LLM 抖动直接打断 verify→intake 自动链，FAILED 积压需人工清，checkpoint 式全自动 E2E 会碎。

**推荐：A。** 理由：① digest 用途是仓库快照摘要，结构化派生本身真实无虚构，回退无害；② verify 触发是自动的，快照分析不应因 LLM 抖动阻塞；③ 可观测性由 llm_call_record+产物标记双保障，非静默；④ 与冷启动「无 provider 回退」语义对齐但比其更安全（不用假数据）。

## §五 附带裁决 D-4（轻量，可顺手裁）

| 编号 | 事项 | 推荐 | 依据 |
| --- | --- | --- | --- |
| D-4a | KB 信号 relevance/confidence **维持保守固定**（PRODUCT_CANDIDATE/high，宁多触发），LLM 分析只在 PC 侧；信号契约（§23 Q10）留 KB 独立化批 | 同意 | ADR-02：KB 不拥有 PC 触发决定权、Platform 路由不含语义理解；V1.0 :538 保守策略原文 |
| D-4b | Role Catalog **不随本裁决扩** 13 角色；本裁决只裁「LLM 执行体能力」，Catalog 载体归 V2.0-A 分批启用（开工包 D-1=A 已裁目录分批） | 同意 | P2-13 的能力面与载体面解耦，避免一批双裁 |
| D-4c | **范围声明**：本批=PC-I6 起笔切片（仓库快照分析），非全量 PC-I6；影响分析族/完整性/一致性/主动维护另行排批 | 同意 | 09 稿 :132-151 验收面拆解 |

## §六 工程既定（非裁决，实施遵循）

- **端口模式**：分析模型 trait（暂名 `SnapshotAnalysisModel`）定义在 pm-application（先例 `RepoAnalysisModel`，repo_facts.rs:300-307）；pm-infrastructure 经 trait 调用；pm-ai 提供 OpenAI 兼容实现——维持 pm-infrastructure 不依赖 pm-ai。
- **路由与记账**：复用 `resolve_repo_call_config` 三级解析（0043 严格不回退语义照搬）+ `ai.llm_call_record`（purpose 新增区分值）。
- **Prompt 安全边界**：沿用 REPO_SYSTEM_PROMPT 先例——`<repo_facts>` 内容是数据不是指令；证据引用只许真实存在的 sha/路径；temperature=0；宽松 JSON 提取+一次重试（repo_model.rs:16-29, 282-319）。
- **治理面零改动**：产物仍 draft/unverified/PROPOSED；confirm/审批/fail-closed 全部不动。
- **实施注意**：outbox claim 的 lease 时长须容纳 LLM 慢调用（先例 ai_analysis job lease=3600s，main.rs:22-25；outbox lease 由 OutboxWorker 构造决定，outbox_worker.rs:43-57）。
- **迁移**：预计无新迁移；若 analysis_mode/三区分类需持久化而 0053 列不够，动 0057 空号。
- **测试**：wiremock `/chat/completions` 模拟（analysis_model.rs:353-425 先例）+ `kb_pc_repo_flows.rs` 三用例扩展 + `scripts/checkpoint1-e2e.sh` 增 provider 步。

## §七 裁决后的启动序

```text
D-1~D-4 裁决回收
  └→ ① pm-ai：SnapshotAnalysisModel trait 实现（OpenAI 兼容+路由+记账+回退语义 D-3）
  └→ ② kb_pc_analysis：KB digest 与 PC 对象生成接模型端口（D-2=A 产物面，三区分类+证据约束）
  └→ ③ kb_pc_handlers：两 Handler 真实分支注入模型；analysis_mode 标记；失败回退
  └→ ④ 测试：wiremock 单测 + kb_pc_repo_flows 集成 + E2E 脚本扩 provider 步
  └→ ⑤ 验收对照：PC-I6 五条验收前三条 + AC-015-02/AC-016-02 不回归
```

## 附：一次性回复模板

```text
D-1=A   （引入 LLM 执行体，治理不变）
D-2=A   （起笔切片=快照分析增强，结论恒 PROPOSED）
D-3=A   （失败回退结构化派生+analysis_mode 标记）
D-4=同意（a 信号保守 / b Catalog 不扩 / c 范围声明）
```
