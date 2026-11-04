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

> **编号冲突（待裁决）**：`docs/ui/s01/` 下存在两个 S01-01 ——「项目初始化总览」（新增）与「项目基线确认」。
> 建议统一为依赖顺序：S01-01 初始化总览 / S01-02 项目创建 / S01-03 Repository Management /
> S01-04 AI-PM Cognition / **S01-05 项目基线确认** / S01-06 Codex·MCP Bootstrap / S01-07 初始化交接 /
> **S01-08 Personal Home**。该动作涉及既有稿件改名与交叉引用，须裁决后统一执行，
> 详见 `s01-01-project-initialization-overview-ux-v1.md` §0.3。

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