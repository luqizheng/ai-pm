# v1.0 修复回归验收计划（docs/acceptance/v1.0-remediation/）

- 日期：2026-11-24
- 性质：五项 P1 修复后的**重新业务验收**（原始报告 `docs/acceptance/v1.0-business/` 只读保留）
- 被验收系统：AI-PM 1.0（full-dev 全栈，代码 = `remediation/integration` 集成分支 = 四修复合并）
- 验收项目：TaskFlow（复用原始验收项目与遗留对象；含原 REVIEW_REQUIRED 会话已完成态）

## 1. 集成与门禁（自动化，已完成）

| 门禁 | 结果 |
|---|---|
| 四修复分支合并（R1+R2+R3+R4 → remediation/integration） | 无冲突 |
| 后端全量（5 包，--skip q10 按 AGENTS 规则 6） | 见 §2 记录 |
| 前端全量 vitest + vue-tsc + check:i18n | 见 §2 记录 |
| q10 系列单独跑（规则 6） | 见 §2 记录 |
| full-dev 三镜像重建（pm-api / ai-pm-worker / web） | 见 §3 记录 |

## 2. 业务回归场景（对照原始验收 §十）

| # | 场景 | 对应缺陷 | 状态 |
|---|---|---|---|
| 1 | 项目创建与首次 AI 使用（未配置引导） | B01-01/B01-04 | 待运行态 |
| 2 | 需求梳理（发现/裁决/草案确认） | — | 复验 |
| 3 | **REVIEW_REQUIRED 消息恢复**（死信场景反转） | B03-01 | 待运行态 |
| 4 | 知识沉淀 | — | 复验 |
| 5 | 新对话读取项目记忆（总结引用真实记录） | B09-01 | 待运行态 |
| 6 | **Design/ADR 创建→修改→审批→门禁（全 UI）** | B04-02 | 待运行态 |
| 7 | 任务规划 | — | 复验 |
| 8 | 需求变更影响分析 | — | 复验 |
| 9 | 项目总结 | — | 复验 |
| 10 | 六道审批闸门回归（需求确认/设计审批/ADR 接受/任务批/mark-ready 403/B5 阻断） | 统一安全 | 自动化+运行态 |
| 11 | 工程执行 / 交付审查 | — | **未验收**（无真实 Repository/Runner/MCP/Codex——同原始验收口径，不伪造） |

证据目录：`evidence/`（截图 + API 交互记录）。

## 3. 结论口径

四态：PASS / CONDITIONAL PASS / FAIL / NOT READY（同 FINAL-REPORT §11）。
