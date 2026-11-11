# docs/e2e-spec · E2E 测试体系文档目录

本目录是 **E2E 测试的体系层真源**：定义测什么、怎么写案例、什么算通过、什么卡发布。

## 文档索引

| 文件 | 内容 | 状态 |
|---|---|---|
| `00-e2e-testing-framework-v1.md` | **E2E 测试体系总纲**（8 套测试集、案例 11 字段标准、P0/P1/P2 分级、PM-AI 专属套件、体系图、技术实现） | V1.0（2026-11-10） |
| `10-p0-test-specification-v1.md` | **P0 测试规格**（7 核心领域 / 24 案例 / 阶段 A–D / 结果判定 / 发布门禁 + **首轮评审记录 R1–R10**） | V1.0（2026-11-10）首轮评审完成 |
| `11-p0-01-playwright-spec-v1.md` | **P0-01 可执行 Playwright 规格**（选择器映射真实锚点、后端数据断言口径、Fixture 与清理、待裁决项 D1–D5） | V1.0（2026-11-10） |
| `12-p0-02-playwright-spec-v1.md` | **P0-02 可执行 Playwright 规格**（项目创建/Command Center/初始化 7 态派生/多项目隔离，评审 R11–R20，裁决 D6–D10） | V1.0（2026-11-10） |
| `13-p0-03-playwright-spec-v1.md` | **P0-03 可执行 Playwright 规格**（AI-PM 无外部工具可用、intake 分析链、人工确认三道闸，评审 R21–R26，裁决 D11–D16） | V1.0（2026-11-10） |
| `14-p0-04-playwright-spec-v1.md` | **P0-04 可执行 Playwright 规格**（三层权威：Markdown+Git / PG / PageIndex；绑定点=submit；reindex 重建；综合一致性 Rev1→2→3，评审 R27–R32，裁决 D17–D21） | V1.0（2026-11-10） |
| `15-p0-05-playwright-spec-v1.md` | **P0-05 可执行 Playwright 规格**（Task/Run/Delivery/Review/多仓 Commit/ACP 七例；Run 幂等 0012、delivery 4 值、AlreadyDecided、C2 缺口 DEF-P0-01，评审 R33–R40，裁决 D22–D26） | V1.0（2026-11-10） |
| `16-p0-06-playwright-spec-v1.md` | **P0-06 可执行 Playwright 规格**（外部 Agent/MCP 九例 040~048；MCP fallback+device_layer、5 固定工具、Skills 四件套、**回传通道=D4 未开通**、采纳管道待查证，评审 R41–R49，裁决 D27–D31） | V1.0（2026-11-10） |
| `17-p0-07-playwright-spec-v1.md` | **P0-07 可执行 Playwright 规格**（知识学习/Commit 采纳/统一协调 八例 049~056；门禁 8 规则后端强制、协调者幂等账本、kb.memory_ref 溯源、**051/052 管道 NOT_IMPLEMENTED**，评审 R50–R56，裁决 D32–D36） | V1.0（2026-11-10） |
| `18-p1-test-specification-v1.md` | **P1 测试规格 V1.1**（七组：初始化持久/登录落地/列表权限/Bootstrap/零外部依赖/Baseline 六裁决 R-BL-01~06/DEF-03·04 回归；**V1.1 按代码事实修正**：S27=需求层术语非运行时代理、`POST /v1/pm/tasks` 存在且 human 闸（R33 回改）、Baseline 确认无真人守卫 → **DEF-P0-02**；评审 R57–R64 + R69–R70（R65–R68 归 12 号落地实测），裁决 D37–D42） | V1.1（2026-11-11） |
| `19-p1-executable-cases-v1.md` | **P1 可执行用例集**：**七组用例 + E2E 实现全部完成**（2026-11-11）。落地评审 R71–R91；`ai-frontend/e2e/p1-01…07.spec.ts` ×7 共 **41 用例** + `playwright.p1.config.ts`（复用 p0 栈），`--list` 解析全通过；fixme ×3（DEF-P0-02 / DEF-P0-01 / D54 不可达 provider）；运行命令与待映射 D43–D58 见文内 | 已完成 |
| `20-p0-impl-facts-v1.md` | **P0 规格 ↔ 实现事实源偏差（R92–R102）**：2026-11-11 P0 阶段 B~D 实测对照表（分析链入口、终态词、承接人须 dev_agent、租约活跃期、知识门 8 规则构造要求、可领取性清单）+ 由偏差牵出的 DEF-P0-03/04 | V1.0（2026-11-11）｜裁决口径：**实现为准**，规格按本表回写 |

## 与 `test-case/` 的分工（不重复真源）

| 目录 | 层次 | 内容 | 生命周期 |
|---|---|---|---|
| `docs/e2e-spec/` | **体系层** | 规范、模板、标准、分级、映射 | 稳定，变更走版本号 |
| `test-case/` | **落地层** | 载体拓扑、用例矩阵、执行记录、缺陷登记 | 每批次滚动 |

| 事项 | 归 `docs/e2e-spec/` | 归 `test-case/` |
|---|---|---|
| 8 套测试集定义、11 字段标准 | ✅ | — |
| Suite 与用例模板 | ✅ | — |
| P0/P1/P2 分级与发布门禁 | ✅（口径） | 门禁执行清单 |
| L2/L3 载体拓扑、端口、私钥通路 | — | ✅ |
| 具体用例实现（`apps/pm-api/tests/e2e_flow.rs`、`ai-frontend/e2e/`） | — | ✅ |
| 执行报告、证据目录、缺陷单 | — | ✅ |

> 上位文件：`test-case/full-lifecycle-test-plan-v1.md`（全仓 L0–L4 测试策略总纲）。
> 落地计划：`test-case/e2e-test-plan-v1.md`（L2/L3 计划与用例矩阵）。
> 执行记录：`test-case/e2e-flow-execution-report-v1.md`。

## 命名与编号规范

| 对象 | 规则 | 示例 |
|---|---|---|
| 体系文档 | `NN-<主题>-v<主>.<次>.md` | `00-e2e-testing-framework-v1.md` |
| 套件明细文档 | `NN-<套件号>-<主题>-v<主>.<次>.md` | `01-basic-function-v1.md` |
| 通用案例 ID | `TC-<suite 两位><序号>` | `TC-301`、`TC-801` |
| 本仓落地案例 ID | `FLOW-*`（L2）、`UIFLOW-*`（L3） | `FLOW-04` |
| 缺陷 ID | `DEF-<批次>-<序号>` | `DEF-E2E-01` |

## 待展开（已登记，不静默）

| # | 文档 | 状态 |
|---|---|---|
| 1 | `01-basic-function-v1.md`（Suite 01 通用模板 + PM-AI 落地用例） | 待写 |
| 2 | `02-input-validation-v1.md` | 待写 |
| 3 | `03-state-lifecycle-v1.md` | 待写 |
| 4 | `04-permission-security-v1.md` | 待写 |
| 5 | `05-exception-resilience-v1.md` | 待写 |
| 6 | `06-concurrency-idempotency-v1.md` | 待写 |
| 7 | `07-page-interaction-v1.md` | 待写 |
| 8 | `08-business-flow-v1.md` | 待写 |
| 9 | `09-pm-ai-domain-suites-v1.md`（Team/Project/Repository/AI-PM/Task/Agent/Knowledge/ACP/MCP） | 待写 |
| 10 | `10-case-template-v1.md`（11 字段空表模板 + 填写规范） | 待写 |
| 11 | `12-p0-02-playwright-spec-v1.md`（P0-02 Team 与 Project 基础管理） | 已落盘 |
| 12 | `13-p0-03-playwright-spec-v1.md`（P0-03 Project AI-PM 核心能力） | 已落盘（R8/R10 断言收窄已写入；**D11/D12 需裁决：worker 进栈与 provider 配置**） |
| 13 | `14-p0-04-playwright-spec-v1.md`（P0-04 Requirement 与 Knowledge 权威性） | 已落盘（三层权威核验完成，10 号文档 §8 待核验 1 关闭） |
| 14 | `15-p0-05-playwright-spec-v1.md`（P0-05 Task 执行与交付） | 已落盘（D22：扩编 7 例） |
| 15 | `16-p0-06-playwright-spec-v1.md`（P0-06 外部 Agent/MCP，**P0-040~048**） | 已落盘（D27：原 ACP 并入 P0-021） |
| 16 | `17-p0-07-playwright-spec-v1.md`（P0-07 知识学习与采纳，**P0-049~056**） | 已落盘（D32：权限套件解散分散承接；总数 24→**38**；051/052/049b NOT_IMPLEMENTED 待转正） |

> **P0 全 7 套件规格已全部核验落盘**（11–17 号文档）。最终口径：38 案例 / 阶段 A–D / 门禁 38/38 PASS；NOT_IMPLEMENTED 项（045b、051、052、049b）不计 PASS，落地后按 16/17 号文档转正条件复验。
> **P1 规格**（18 号）：七组落盘；执行顺序建议 P1-01（Phase 3 实测）→ P1-02/03（可立即）→ P1-06（六裁决已实名）→ P1-04/05（增量）→ P1-07（先做 D38 端点盘点）。
