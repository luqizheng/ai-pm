# S01 · 项目初始化与接入 UX 文档

本目录沉淀 S01 场景的前端 UX 规范。

| 编号 | 文档 | 状态 |
| --- | --- | --- |
| S01-00 | [全局主菜单与信息架构设计基线 V1.0](s01-00-global-navigation-information-architecture-v1.md) | **用户裁决完成，设计基线冻结**（Q1~Q3；全产品导航基础规范） |
| S01-01 · 初始化总览 | [Project Initialization Overview UX 详细设计稿 V1.0](s01-01-project-initialization-overview-ux-v1.md) | 设计稿完成，待评审；编号冲突待裁决（见该稿 §0.3） |
| S01-01 | [项目基线确认 UX 规范 V1.0](s01-01-project-baseline-confirmation-ux-v1.md) | 待评审 |
| S01-02 | [Project Creation UX 设计稿 V1.1](s01-02-project-creation-ux-v1-1.md) | 设计稿完成，待整体冻结 |
| S01-03 | [Repository Management UX 设计稿 V1.1](s01-03-repository-management-ux-v1-1.md) | 设计稿完成，待代码事实核验及整体冻结 |
| S01-04 | [AI-PM Cognition UX 详细设计稿 V1.1](s01-04-ai-pm-cognition-ux-v1-1.md) | 用户裁决已收编，待设计冻结 |
| S01-05 | [Personal Workspace UX 设计稿 V1.1](s01-05-personal-workspace-ux-v1-1.md) | 用户裁决已收编，待整体基线冻结；**命名已裁决=Personal Home**（Workspace 为旧称，全文修订待做） |
| S01-05 · 高保真 | [Personal Home 高保真 UI 设计稿 V1.0](s01-05-personal-home-high-fidelity-ui-v1.md) | **已实施（2026-11-04，见该稿 §13）**：页面落地 `ai-frontend/src/views/personal/`，登录默认落地 `/personal`；651 测试全绿 + 三门禁通过 |
| 裁决记录 | [R-S01-01-01 · Native AI-PM Independence 裁决记录 V1.0](s01-01-native-ai-pm-independence-ruling-v1.md) | **已记录（2026-11-05，专家裁决·方案 A，只记录未评审）**：内置 AI-PM 不依赖 Repository/MCP/Codex/外部 Agent；可用性矩阵已收编 I5 §4.7 与 S01-01 总览稿 §9.5；专家后续稿待到 |
| S01 设计修订 · 第一份 | [Project Command Center 设计修订稿 V1.1（专家供稿记录）](s01-command-center-design-revision-v1-1.md) | **已记录（2026-11-05），待基线确认**：Create Project = Ready to Work；五区结构、Needs Attention 优先于 Initialization；InitItem 读模型/六态不变；§九 更正编号——S01-01 仍为 Initialization Overview，修订后顺序 01~07 与下方建议顺序同向（改名执行仍待统一） |
| S01 设计修订 · 第二份 | [S01-02 Project Creation 设计修订稿 V1.1（专家供稿记录）](s01-02-project-creation-design-revision-v1-1.md) | **已记录（2026-11-05），待基线确认（§十 建议冻结 V1.1）**：Q1~Q3 方案 A 保留（**正文已在既有稿 ：412-414 定位**，§一 担忧解除）；Q3 落地修订=创建后直接进 Command Center、初始化内嵌；四状态独立；含 Figma AI 提示词与成功流程图（assets/ 已归档） |
| S01 设计修订 · 第三份 | [S01-03 Repository Management 详细设计稿 V1.2（专家供稿记录）](s01-03-repository-management-design-revision-v1-2.md) | **已记录（2026-11-05），修订建议稿待代码核验+最终 UX 核验后冻结**：R-03-01~04 四裁决收编（正文已在既有稿 ：48/:68-88 定位）；新增=长期管理定位、Branch/Commit 三概念治理（Observed HEAD vs Confirmed Commit）、页面 IA、AC-01~15、17 项代码核验清单、§15 原型五点纠正（**原型原件未入仓，位置待确认**）；G-03/G-04/G-05 仍是三项裁决的实施前提 |
| S01 设计修订 · 第四份 | [S01-04 AI-PM Cognition 详细设计稿 V1.2（专家供稿记录）](s01-04-ai-pm-cognition-design-revision-v1-2.md) | **已记录（2026-11-05），设计基线候选待技术现状核验**：Cognition=长期认知治理（六领域/提案逐项确认/变化治理/混合 Context）；Q2=C·Q3=B·Q4=C·Q5=C 显式映射与既有裁决全一致（Q1=B 落点=§1.2）；§15 十项核验（勿复制 Discovery/Knowledge/Document 三套既有提案状态机）；与已实施 CognitionPage V1 的范围差登记待收敛 |
| S01 设计修订 · 第五份 | [S01-05 Project Baseline 设计稿 V1.1（专家供稿记录）](s01-05-project-baseline-design-revision-v1-1.md) | **已记录（2026-11-05），内嵌 V1.1=已裁决的产品设计基线（实施冻结待四项核验）**：R-BL-01~06 六裁决收编（B/A/A/A/A/B，§十二 建议值=§十三 收编值）；边界=Project 级共识基线≠S03 Implementation Baseline；Requirement 只引用 Revision（R-BL-01=B）；未决事项允许且标记（R-BL-02=A）；无 Repository/Cognition 不完整均可确认（R-BL-03/04=A）；完整逻辑快照（R-BL-05=A）；确认权限与创建者分离（R-BL-06=B）；**§九 Figma 提示词正文缺失+§六 6.1/6.2「略」登记挂账**；核验对象须含实现仓既有 0037 基线纵切片（记忆来源，以代码为准） |
| S01-08 | [Project Knowledge Repository & Git Backup 裁决记录及详细设计稿 V1.0](s01-08-knowledge-repository-git-backup-v1.md) | **已记录（2026-11-05），R-S01-08-01~04 四裁决收编；原文于 Part D.2 第 5 步截断，剩余待补发**。占号冲突：README 既有建议顺序 S01-08=Personal Home，两案待统一裁决（见评审稿 §3.1） |
| S01-08 · 评审 | […Git Backup 专家评审稿 V1.0](s01-08-knowledge-repository-git-backup-review-v1.md) | **已记录（2026-11-05）**：4 P0（与已实施知识空间 V2 关系未声明·禁止第二套存储／同步须锚定已治理版本防绕过红线／元数据不得平行建表／远程目录镜像待裁决）+ 6 P1 + 4 P2；Q-08-01~06 待用户裁决；Part D 补发后补评 |

> **编号冲突（待裁决）**：`docs/ui/s01/` 下存在两个 S01-01 ——「项目初始化总览」（新增）与「项目基线确认」。
> 建议统一为依赖顺序：S01-01 初始化总览 / S01-02 项目创建 / S01-03 Repository Management /
> S01-04 AI-PM Cognition / **S01-05 项目基线确认** / S01-06 Codex·MCP Bootstrap / S01-07 初始化交接 /
> **S01-08 Personal Home**。该动作涉及既有稿件改名与交叉引用，须裁决后统一执行，
> 详见 `s01-01-project-initialization-overview-ux-v1.md` §0.3。

> **R-S01-01-01 结构预告（2026-11-05，用户转交专家裁决，后续稿待到，编号暂不动）**：专家将陆续产出
> ① S01-01 Project Command Center（总控台独立成稿）、② Project Initialization（降为总控台内部的初始化进度模块）、
> ③ Project Native AI-PM（项目原生 AI 能力）、④ External Integrations（独立的外部工具接入能力）。
> 裁决正文与落地影响见 [裁决记录](s01-01-native-ai-pm-independence-ruling-v1.md)；
> 上述结构与上方「编号冲突（待裁决）」注的关系在新稿到达后一并收敛。
> **更正（2026-11-05，第一份设计修订稿 §九 到达）**：编号以上述修订稿为准——S01-01 工作包**仍为 Initialization Overview**（非 Command Center）；
> Command Center 设计约束由[修订稿](s01-command-center-design-revision-v1-1.md)承载（初始化总览=Command Center 内模块 + 完整详情页双形态）；
> 修订后顺序 01 初始化总览 / 02 创建 / 03 仓库 / 04 认知 / 05 基线 / 06 Bootstrap / 07 交接，与本节上方建议顺序同向（稿件改名执行仍待统一）。

> **修正状态（2026-11-05「开始修正」执行登记）**：
> ✅ 已执行——① **统一技术核验清单**建立：[s01-technical-verification-unified-checklist-v1.md](s01-technical-verification-unified-checklist-v1.md)（合并总览稿 G-01~G-23 + S01-03 §10 十七项 + S01-04 §15 十项 + S01-05 §八/§11/冻结条件；含 U-B01 时间快照差警示：G-08「无 baseline 表」vs 记忆载实现仓 0037 已落，以当前分支代码核验为准）；② 六份既有稿文首修订指针注：s01-02 / s01-03 / s01-04 / s01-01-基线确认 / S01-01 总览稿 §0 / I5 §0。
> ⛔ 仍门禁中——稿件改名与编号收敛（待裁决统一执行）；各稿冻结动作（待基线确认+代码核验）；原型修订五点（原件未入仓，位置待确认）；S01-05 §九 Figma 提示词正文与 §六 6.1/6.2 布局（待专家补件）；代码侧核验（按统一清单逐项执行，结论回写来源稿）。

> **S01-00 基线地位（2026-11-04，用户裁决「以这份为主」）**：S01-00 是全产品导航基础规范（S01～S08 共用）与导航口径唯一权威，其「冻结范围」章节明确
> 不引入 Workspace 业务概念、不设全局 AI 协作中心。两处既有口径已按其约束落档：
> ① 根目录 `smark-pm-menu-roles-permission-model-v1.md` 的「7 个一级菜单」菜单结构部分被本基线的
> 6 入口结构取代（差异=AI 协作中心移除，Q1=A；该稿文首已加取代注，其余权限模型内容不受影响仍待评审）；
> ② S01-05 挂账的 Personal Workspace vs Personal Home 命名裁决已下（2026-11-04）：**定名 Personal Home / My Home**，Workspace 退出业务/入口命名。

> **S01-05 现存两份文档，裁决前并存不合并**：
> `s01-05-personal-workspace-ux-v1-1.md` 是正文 UX 基线；`s01-05-personal-home-high-fidelity-ui-v1.md` 是其高保真页面设计稿
> （视觉原型 `assets/s01-05-personal-home-prototype.html`，含 7 种状态可切换）。
> 二者在**命名口径**上冲突（`Personal Workspace` / `My Workspace` vs `Personal Home` / `My Home`），
> 并在**概览指标**与**Watchlist 是否进首页**上口径不同；差异与倾向见高保真稿 §0.1 与 §11。
> 裁决后需同步修订另一份，不得产生两份互相矛盾的正本。
> **命名裁决已下（2026-11-04 用户）：Personal Home / My Home 定名**；正文 V1.1 中 Workspace 命名为旧称，
> 全文修订待执行，修订前以两稿文首裁决注为准。概览指标与 Watchlist 归属两处口径差仍待裁决。

## S01 后续推进顺序

| 顺序 | 工作项 | 交付物 |
| --- | --- | --- |
| 1 | S01-01 项目初始化总览 | UX 规范 |
| 2 | S01-01 基线确认 | UX 规范与状态模型 |
| 3 | S01-02 项目创建 | 用户流程与 UX 规范 |
| 4 | S01-03 Repository Management | 仓库接入与生命周期规范 |
| 5 | S01-04 AI-PM Cognition | 项目认知建立与人工确认规范 |
| 6 | S01-05 Personal Workspace UX | 个人工作台 UX 设计稿 |
| 7 | （原）项目基线管理 | 正式基线及版本演进规范，编号待 S01 总体目录统一修订 |
| 8 | S01-06 Codex / MCP Bootstrap | 开发者接入与授权规范 |
| 9 | S01-07 初始化完成与交接 | 项目总控台交接规范 |

> S01-01 的初始化基线确认与（原）S01-05 的长期基线管理职责不同：前者解决项目首次建立正式基线，后者解决项目长期演进中的基线变更。
> 长期基线管理不再占用 S01-05 编号，避免两个同编号的正式设计项；其重新编号在 S01 总体目录及实施计划中统一修订。

## 冻结的设计原则

1. 基线允许持续演进。
2. 基线版本不可覆盖。
3. 当前有效基线始终唯一。
4. 正式确认必须由人类完成。
5. AI 负责分析、提案、修订和证据整理。
6. 基线变更必须保留历史及影响追溯。