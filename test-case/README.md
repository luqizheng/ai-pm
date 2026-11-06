# test-case · 实测用例目录

存放**需要真实环境执行**的测试用例（CI 覆盖不到的两机/进程/网络层）。

| 文件 | 任务 | 状态 |
|---|---|---|
| `t090-two-machine-test-cases-v1.md` | T-090 Bootstrap Skill + 自检闭环（两机实测） | 用例已写定；**逐条确认后执行**，每条回填「实际结果」 |
| `team-qa-test-plan-v1.md` / `team-qa-test-cases-v1.md` / `team-qa-execution-report-v1.md` | 团队功能（Team Onboarding）QA：计划 / 88 条用例执行记录 / 报告 | 2026-11-06 执行完毕：73 PASS / 7 FAIL（DEF-01 P0 迁移撞号、DEF-02 P1 被移除成员残留访问）；一键复跑 `evidence/team-qa/run-team-qa.sh` |

## 约定

- 用例文件即执行记录：每条用例带「实际结果」行，执行后回填通过/失败 + 证据。
- 全部通过后由实施会话回填 `todo.md` 对应条目、设计稿验收表与代码事实台账。
- 失败即登记缺陷（现象/复现/怀疑点），修复后重跑该条 + 对应主链用例。
