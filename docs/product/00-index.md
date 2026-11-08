# PM-AI 业务逻辑终态登记 · 总索引

| 属性 | 值 |
| --- | --- |
| 目录用途 | 登记开发过程中**已裁决且已实现**的业务逻辑当前状态，作为用户手册（业务流程/状态图/交互图）的母本 |
| 建立日期 | 2026-11-08 |
| 维护纪律 | 每次用户裁决或代码合入改变某域行为后，须同步更新对应章节并刷新「证据基准」行 |

---

## 1. 本目录与 docs/ 其他目录的关系

`docs/` 下的绝大多数文档是**过程稿**：设计稿、评审稿、裁决包、验收记录。它们的结论分散且大量被后续评审取代，无法直接当「系统现状」阅读。本目录只登记**终态**：

- 每条业务规则 = 结论 + 证据锚点（迁移号 / 代码路径 / 裁决记录 §节）。
- 只写「已裁决且已实现」；「已裁决未实现」「未裁决挂账」单独标注，绝不混入正文。
- 设计稿与本文档冲突时，以本文档 + 代码为准；取代关系在每篇 §权威源 一节登记。
- 图（mermaid 时序图/状态图）按可直接进用户手册的标准绘制。

## 2. 章节清单

| 章节 | 文件 | 状态 |
| --- | --- | --- |
| Runner / Platform / AI Agent / MCP 通信协议 | [runner-platform-mcp-communication-v1.md](runner-platform-mcp-communication-v1.md) | ✅ 已登记（2026-11-08） |
| PC / KB 操作链（commit → 分析 → 人工确认） | [pc-kb-operation-v1.md](pc-kb-operation-v1.md) | ✅ 已登记（2026-11-08） |
| MCP 工具面与三通道分工（26 工具族谱 / vs PC/KB commit 监控） | [mcp-tool-face-channel-division-v1.md](mcp-tool-face-channel-division-v1.md) | ✅ 已登记（2026-11-08） |
| Team 与角色体系（Team≡tenant / 三层角色分离） | 待登记 | ⬜ |
| 五阶段业务体系（S01~S05 各阶段操作流） | 待登记（骨架见 `docs/ai-pm-five-stage-business-architecture-v1.md`） | ⬜ |
| 需求生命周期（S02：提出→确认→基线→验收） | 待登记 | ⬜ |
| 任务生命周期与派发策略（S04/S05 业务面） | 待登记 | ⬜ |
| AI Role 体系与对话编排（DK-0）对接 | [AI团队角色体系与对话编排对接-v1.md](AI团队角色体系与对话编排对接-v1.md) | ⚠️ 已裁决未实现（2026-11-08 裁决定版；代码零开工） |
| LLM Provider 管理与模型路由 | 待登记 | ⬜ |
| 首登 Onboarding 与邀请 | 待登记 | ⬜ |

## 3. 权威源登记表（「想看 X 该读哪份」）

按域登记当前权威依据；**加粗**为最高依据，冲突时以其为准。行号类锚点以 2026-11-08 工作区为准，仅作导航。

| 域 | 权威源 | 已被取代/有失实的材料 |
| --- | --- | --- |
| ACP 通信面 | **checkpoint2-r4-acceptance-record-v1.md**；派发竞争裁决记录（`project-ai-team-v2/ai-pm-v05-dispatch-competition-adjudication-record-v1.md`）；migrations 0046/0049/0050/0051/0052/0054/0055/0059 | R4 记录 §六-1 三处失实已勘误（派发默认 Off 非 On；不存在 `no_client_available` 字面量，真枚举在 `run_dispatch/policy.rs:135-231`）；「run_executor 硬编码 On」已被 a1bd7f4 接线取代 |
| PC / KB | **P2-13 裁决包**（2026-11-08 已裁）；checkpoint1-verification-record-v1.md；`architecture/knowledge-services/kb-pc-independent-services-architecture-v1.1.md`（13 项 ADR 已批）；migrations 0048/0053/0062 | kb-pc v1.0 架构稿（被 v1.1 取代）；product-cognition-detailed-requirements-v1.md（被 v1.1 取代）；v1-review.md 为过程稿 |
| MCP 工具面 | **`crates/pm-mcp/src/server.rs`**（26 工具声明+description 原义，pm-api `/mcp` 挂载）+ [mcp-tool-face-channel-division-v1.md](mcp-tool-face-channel-division-v1.md)；边界=`docs/ai-pm-five-stage-business-architecture-v1.md`（MCP 非运行时）+ I6-A A*（需求确认 fail-closed） | runner 章 §2「关键工具」10 个为本章族 A/B 的子集视图（不冲突）；工具行号锚点为 2026-11-08 工作区快照，仅导航 |
| Team / 角色 | **docs/team-onboarding/ 两稿 + 产品裁决 2026-11-06**（Owner 独占解散等）；migrations 0019/0030/0044/0058/0061；`crates/pm-domain/src/tenant.rs` / `project_member.rs` | team-domain-model-database-design-v1.md 中 core.team 建表方案（已被「Team≡core.tenant、Extend 不重建」裁决取代，仅邀请三表按稿落地） |
| 五阶段基线 | **docs/ai-pm-five-stage-business-architecture-v1.md**（2026-11-08 核定） | — |
| V0.5 范围 | **ai-pm-v05-mvp-scope-baseline-v1.md + 执行路线图 v1** | — |
| AI Team（AI Role/Capability） | **kickoff 裁决包**（D-1=A）+ migrations 0058/0061 + **`docs/project-ai-team-v2/ai-pm-v2-architecture-baseline-v1.md` §四**（Role 体系，已裁未实现）| v1 review draft 分篇为**未冻结评审稿**，不得当现状引用 |
| 对话编排（DK-0） | **`ai-pm-dk0` worktree `docs/product/ai-pm-dialog-orchestration-design-v1.md`**（D-DK-1~11 已裁 2026-11-08，未合 master） | 终态摘要见 [AI团队角色体系与对话编排对接-v1.md](AI团队角色体系与对话编排对接-v1.md) §2 |
| S02 需求域 | S02-04 总体设计 V1.2 + 各切片实施记录（0039/0040/0042/0045/0056） | S02-04 V1.0（截断稿）、S02-01/03 中被 V1.2 收编的条目 |
| 登录与鉴权链 | runbook-login-chain.md + lmms token-purpose 结论（aud 钉死 service JWT） | — |

## 4. 阅读方法

1. 想知道「**系统现在是怎么工作的**」→ 直接读对应章节正文，正文即终态。
2. 想知道「**为什么是这样 / 哪里拍板的**」→ 按章节内证据锚点回溯裁决包与迁移注释。
3. 想知道「**哪些还没做**」→ 各章 §边界 一节；本索引不登记愿望清单。
4. 引用格式约定：`文件:行` 为代码锚点；`迁移 NNNN` 为 `migrations/NNNN_*.sql`；裁决引用写「裁决包名 §节」。

## 5. 本目录在 PC/KB 模型中的位置

本目录的存在本身是 PM-AI 要解决的痛点的实例（definition-architecture §2：文档不等于产品认知 / 历史知识分散 / 认知冲突）。按 PC 产品模型（§4.4 Plan / Reality / Gap）：

| PC 概念 | 对应物 |
| --- | --- |
| Product Reality | **本目录**（人工核定快照，系统当前真实状态） |
| Product Plan | 设计稿 / 评审稿 / 裁决包（决策历史） |
| Product Gap | 各章 §边界 / §未做 一节 |

分工演进：今天本目录由「代码+迁移+裁决记录」三方对齐**人工蒸馏**（一次性 bootstrap）；PC-I6 后续切片（KB 知识输入、关系建模、冲突发现）到货后，本目录进入 Team Source Pool 作为**权威输入源**，稳态维护归 PC——代码/裁决漂移 → PC 出 PROPOSED 更新提案 → 人工确认回写。此闭环实现前，维护纪律以 §0 的人工规则为准。
