# Project AI Team V1.0 产品需求与架构设计基线（评审稿）· 记录索引

> 状态：评审稿（Review Draft）存档转录 · 未冻结 · Q2–Q4 待确认
> 记录日期：2026-11-08
> 来源：外部供稿粘贴文本（`C:\Users\leo\.zcode\tmp\paste-attachments\2026-11-08\pasted-text-20261108-104341-1a864848.txt`）
> 转录方式：按原稿结构与措辞忠实转录；粘贴过程中丢失的表格/图形结构按原稿文字项重建为 Markdown，并在相应位置标注。

---

## 1. 本稿性质与状态警示

- 本稿为 **Review Draft（评审稿）**，原稿自我声明：核心架构已有裁决，但 Q2–Q4 待确认，**不能视为已冻结的 V1.0 正式基线**（见分篇 5 §22.3）。
- **编号冲突警示**：本稿 §14 的 Q2/Q3/Q4（专业 Role 执行方式 / 项目自定义 Role / 独立管理界面）是**本次 AI Team 设计评审的私有编号**，与 `ai-pm-v2-architecture-baseline-v1.md` 中已裁决的 Q1=A / Q2=B / Q3=A（Agent Client 管理范围 / 任务分配方式 / Role 与 Client 关系）**不是同一组问题**，阅读时不得混用。原稿 §14 亦自带警示：「不是此前已确认的 Client 调度 Q2」。
- 本稿 §0.1 的 AT-D01~D11 与 V2.0 正式基线的裁决方向一致（个人级 Client 注册、Role 动态分配、Platform 控制面等）；AT-D12（AI Project Manager 是独立管理协调角色）为本稿新增确认项（状态：已确认 A）。
- 本稿含两轮内容：
  - 第一轮 §0–§15：设计主体 + 请求用户对 Q2–Q4 裁决（原稿推荐 Q2=A、Q3=A、Q4=A）。
  - 第二轮 §16–§22：用户回复「继续」后的深化。原稿本轮明确声明**不把 Q2–Q4 的推荐方案当成用户裁决**，只深化不依赖这三个选择的部分，使后续选择变化时底层架构不需要推倒重来。
- 本稿与本目录既有文档的先后关系及裁决覆盖范围未在本稿中声明；是否已被 V2.0 正式基线及后续裁决吸收，以用户确认为准。

## 2. 分篇目录

| 分篇 | 文件 | 覆盖章节 | 内容 |
| --- | --- | --- | --- |
| 1 | [ai-pm-project-ai-team-v1-review-draft-01-boundary-positioning-organization.md](ai-pm-project-ai-team-v1-review-draft-01-boundary-positioning-organization.md) | §0–§3 | 已有裁决边界（AT-D01~D12）、两个系统边界、产品定位、组织架构、Role Catalog |
| 2 | [ai-pm-project-ai-team-v1-review-draft-02-role-permission-dispatch-session.md](ai-pm-project-ai-team-v1-review-draft-02-role-permission-dispatch-session.md) | §4–§8 | Role Definition 六组字段与版本管理、权限矩阵、Task Dispatch、Task/TaskExecution 生命周期、Work Session |
| 3 | [ai-pm-project-ai-team-v1-review-draft-03-client-provider-knowledge-ui.md](ai-pm-project-ai-team-v1-review-draft-03-client-provider-knowledge-ui.md) | §9–§12 | Client Management、LLM Provider 与模型策略、知识写入治理、管理 UI |
| 4 | [ai-pm-project-ai-team-v1-review-draft-04-scope-decisions-summary.md](ai-pm-project-ai-team-v1-review-draft-04-scope-decisions-summary.md) | §13–§15 | V1.0 实施范围（S1~S4）、三个待裁决问题（Q2/Q3/Q4）、当前设计状态总结 |
| 5 | [ai-pm-project-ai-team-v1-review-draft-05-domain-dispatch-governance-acceptance.md](ai-pm-project-ai-team-v1-review-draft-05-domain-dispatch-governance-acceptance.md) | §16–§22 | 六子域 Domain 模型、实体与关系约束、Dispatch 细化、统一事件审计、审批红线、AT-AC-01~15 验收标准、本轮结论 |

## 3. 与本目录其他文档的关系

- `ai-pm-v2-architecture-baseline-v1.md`：V2.0 正式产品架构设计与裁决记录（已裁决）。本 V1.0 评审稿为同主题的产品模型设计稿，两者问题编号体系不同（见 §1 编号警示）。
- `ai-pm-v05-mvp-scope-baseline-v1.md` / `ai-pm-v05-execution-roadmap-v1.md`：V0.5 MVP 范围与排期；其中「AI Team 创建」目标与本稿组织模型相关。
- `ai-pm-v2-code-alignment-verification-v1.md`：本稿 §16.3 明确提示实体模型为建议稿、尚未对照当前代码库核验；实体复用核验参见该文档。
- **代码综合比对报告**：[ai-pm-project-ai-team-v1-vs-code-comparison-v1.md](ai-pm-project-ai-team-v1-vs-code-comparison-v1.md)（2026-11-08）——本稿全部主张与 2026-11-08 工作区代码的逐章比对、概念映射总表、缺口清单与处置建议。
- 本稿为本次新落盘文件，尚未 commit。
