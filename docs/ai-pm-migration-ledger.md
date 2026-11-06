# 迁移台账（Migration Ledger）

> 统一登记迁移号段的预留与占用（评审 P1-1 收编产物，2026-11-02）。
> 规则：① 设计稿预留**不锁定**实施——实施以先到者为准，后到者按台账顺移并回填本表；② 取号前必须先对账本表与 `migrations/` 实况；③ 实施完成后把「状态」置为「已应用」并填文件名。

## 一、已应用（权威 = `migrations/` 目录）

| 号 | 文件 | 任务 | 状态 |
|---|---|---|---|
| 0001–0021 | `migrations/0001_core.sql` … `0021_task_dependency.sql` | 既有 | 已应用 |
| 0041 | `0041_team_llm_provider.sql` | Team LLM Provider Management Phase 1（DEC-LLM-001~004；R-LLM-01/02 修复） | 已应用（2026-11-06） |
| 0042 | `0042_s02_i2_projection.sql` | S02-I2 Markdown 反向投影（并行会话） | 已应用（2026-11-06） |
| 0043 | （预留→本批） | Project AI Model Policy（§10.2 项目指定 Provider） | 实施中（2026-11-06） |
| 0044 | `0044_team_onboarding.sql` | Team Onboarding（Team≡tenant 扩展：slug/description + 邀请三表；feat/team-onboarding 已合入 master） | 已应用（2026-11-06） |
| 0045 | `0045_s02_i3_source_traceability.sql` | S02-I3 来源与追溯（5936105 曾以 0044 提交，与 Team Onboarding 撞 sqlx 版本号 → DEF-01 P0，2026-11-07 顺移） | 已应用（2026-11-06） |
| 0022 | `0022_delivery_manifest.sql` | T-041（D4） | 已应用（2026-11-02） |
| 0023 | `0023_run_executor_hardening.sql` | T-032 | 已应用 |
| 0024 | `0024_knowledge_section_index.sql` | T-060（知识索引 V2） | 已应用（2026-11-02） |
| 0025 | `0025_discovery.sql` | T-083（Discovery 数据模型） | 已应用（2026-11-02） |
| 0026 | `0026_delivery_review.sql` | T-043（D5） | 已应用（2026-11-02） |

## 二、已预留（设计稿已过稿/提交，未实施）

| 号 | 预留者 | 设计稿 | 内容 | 备注 |
|---|---|---|---|---|
| ~~0024~~ | ~~T-060（知识索引 V2）~~ → **已实施** | `docs/ai-pm-t060-knowledge-index-v1.md` §4 | `0024_knowledge_section_index.sql` | 2026-11-02 先到先得，已落「一、已应用」 |
| ~~0025~~ | ~~T-083（Discovery 数据模型）~~ → **已实施** | `docs/ai-pm-project-discovery-v1.md` §附录A | `0025_discovery.sql` | 2026-11-02 已落「一、已应用」；实施时补 `ai.document_proposal` 租户内唯一键（0019 清单遗漏） |
| 0027 | 空闲（缓冲） | — | — | 供下一任务（注：0022–0040 已由各任务就地取号应用，本表「二」仅存历史；新取号以 `ls migrations/` 实况为准） |

## 三、纪律

1. 新设计稿取号：先读本表 → 再 `ls migrations/` 核实 → 预留号 = `migrations/` 最大号 + 1 与本表最大预留号 + 1 的较大者。
2. 实施撞号：后实施者无条件顺移，不得改已应用迁移；顺移后同步回填本表 + 自己的设计稿。
3. 本表由每个取号/顺移动作的执行任务就地维护（含在对应 `chore(todo)` 或 `feat` 提交内）。