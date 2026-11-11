# test-case · 实测用例目录

存放**需要真实环境执行**的测试用例（CI 覆盖不到的两机/进程/网络层）。

| 文件 | 任务 | 状态 |
|---|---|---|
| `../docs/e2e-spec/00-e2e-testing-framework-v1.md` | **E2E 测试体系总纲**（8 套测试集、案例 11 字段标准、P0/P1/P2 分级、PM-AI 专属套件） | V1.0（2026-11-10）；体系层真源在 `docs/e2e-spec/` |
| `full-lifecycle-test-plan-v1.md` | **测试全流程测试计划**（五层载体、业务流覆盖矩阵、状态机基准、批次门禁、覆盖缺口登记） | V1.0（2026-11-10） |
| `e2e-test-plan-v1.md` | **E2E 测试计划**（L2/L3 载体、SSH 私有key 三层通路、用例矩阵、执行方式、判定口径） | V1.0（2026-11-10） |
| `e2e-flow-execution-report-v1.md` | E2E 全流程执行报告（L2 7/7 + L3 8/8 + P0 阶段A 7/7 PASS + DEF-E2E-01~04） | 2026-11-10 三批执行完毕：L2 服务级 7 PASS（真 router + 真 Git 远端）+ L3 UI 8 PASS（真实仓库链路）+ P0 发布验收阶段 A 7 PASS（规格 11/12 落地，R57~R60 断言修正）；修复 DEF-E2E-01（P1 导入大批次超限）、DEF-E2E-02（P2 标题重复少落地）、DEF-E2E-03（P3 vitest 假红）、DEF-E2E-04（P2 网关环境泄漏）；一键复跑 `evidence/e2e-flow/run-e2e-flow.sh`（L2）/ `run-e2e-flow-ui.sh`（L3）/ `evidence/p0/run-p0.sh`（P0-A） |
| `t090-two-machine-test-cases-v1.md` | T-090 Bootstrap Skill + 自检闭环（两机实测） | 用例已写定；**逐条确认后执行**，每条回填「实际结果」 |
| `team-qa-test-plan-v1.md` / `team-qa-test-cases-v1.md` / `team-qa-execution-report-v1.md` | 团队功能（Team Onboarding）QA：计划 / 88 条用例执行记录 / 报告 | 2026-11-06 执行完毕：73 PASS / 7 FAIL（DEF-01 P0 迁移撞号、DEF-02 P1 被移除成员残留访问）；一键复跑 `evidence/team-qa/run-team-qa.sh` |

## 约定

- 用例文件即执行记录：每条用例带「实际结果」行，执行后回填通过/失败 + 证据。
- 全部通过后由实施会话回填 `todo.md` 对应条目、设计稿验收表与代码事实台账。
- 失败即登记缺陷（现象/复现/怀疑点），修复后重跑该条 + 对应主链用例。
- **用例实现位置**：L2 服务级 E2E 的用例代码在 `apps/pm-api/tests/e2e_flow.rs`（真 axum router + 真 PostgreSQL 临时库 + 真 Git 远端 over SSH）；L3 UI E2E 在 `ai-frontend/e2e/flow.spec.ts` + `ai-frontend/playwright.flow.config.ts`（Playwright + 真 vite :8860 + 真 pm-api :18495，与 team-qa :8850/:18490 端口互斥）；P0 发布验收套件在 `ai-frontend/e2e/p0-*.spec.ts` + `playwright.p0.config.ts`（复用 team 栈 :8850/:18490，规格 `docs/e2e-spec/11~17` 的落地载体，证据落 `evidence/p0/`）。
