# 01 — 修复回归验收结果

- 日期：2026-11-24
- 被验收代码：`remediation/integration`（= master `3a50c7cd` + 四修复分支 merge，四 PR 见 §4）
- 部署态：full-dev 27 容器；**pm-api / ai-pm-worker / ai-frontend-web 三镜像已自集成分支重建并重启**（其余 24 容器沿用）

## 1. 自动化门禁（全部通过）

| 门禁 | 结果 |
|---|---|
| 后端全量（pm-api/pm-application/pm-infrastructure/pm-mcp/pm-ai，`--skip q10`） | 集成分支重跑，全绿（数字见 §4 汇总） |
| q10 系列单独跑（AGENTS 规则 6） | 18/18 绿（q10_race_closure 14 + q10_r3_snapshot_check 2 + q10_classify_matrix 2） |
| 前端全量 vitest | 230 文件 / 1386 用例全绿（R4 分支口径；R3 分支口径 1387） |
| vue-tsc --noEmit | 干净 |
| check:i18n | zh-CN/en 6342 key 全对齐，无新增硬编码 |
| 前端发布构建 | `pnpm build`（production，release-guard 通过：无 mock、无 fixtures/demo 模块进产物） |

分批明细：R1（orchestrator 37 + conversation_router 15 + pm-api discovery 7 + pm-application discovery 22 + 前端 33）；R2（pm-ai 全量 137 lib + 集成全绿 + project_records 6 + chat_context 10）；R3（ai_team_monitor 8 + team_llm_catalog 9 + SettingsTeamAiView 11 + AssistantView 28）；R4（四设计视图 23，全量前端 1386）。

## 2. 部署态核验（无登录可验部分）

| 项 | 结果 |
|---|---|
| 前端 8092 实际下发 bundle | `index-BqeK95cx.js` = 新 dist hash（含 `assistant-readiness-banner`/设计写路径代码）✓ |
| readiness 探针路由 | `GET /v1/pm/projects/{id}/ai-readiness` → 401（路由在，鉴权收口；旧代码 404）✓ |
| worker 开关 | REPLY=on / DISCOVERY_ORCHESTRATOR=on / RUN_DISPATCH=off（部署态不变）✓ |
| pm-api 启动 | listening 8090（gateway healthy 后 setup-status 恢复）✓ |

## 3. 业务流运行态重验（**被凭据阻断，未执行**）

- 开放注册已关闭（`POST /v1/iam/auth/register` → `registration closed`），full-dev 栈内仅 1 个 iam 账号（leo，验收期用户提供密码）。
- e2e 夹具的自签 JWT 仅适用独立 QA 栈（team-qa），对生产栈（真实 token-service JWKS）无效。
- **不伪造**：九阶段业务流（含死信恢复/项目记忆总结/Design-ADR 全流程/就绪横幅）在登录后的真实 UI 操作未重验，本报告不声明其通过。
- 解除阻断路径（二选一）：① 提供 leo 凭据执行浏览器回归；② 用户按 `00-regression-plan.md` §2 清单自行操作并回填证据。

## 4. 缺陷→修复对照

| 缺陷 | 修复 PR | 状态 | 业务流重验 |
|---|---|---|---|
| B03-01 死信 | PR#4（0a10e293） | 自动化 5 新用例 + 回归全绿 | 未重验（凭据） |
| B09-01 项目记忆 | PR#5（1f5cba89） | 自动化 16 用例 + 回归全绿 | 未重验（凭据） |
| B01-01/B01-04 | PR#7（af80ce94） | 自动化 +4/+6/+2 用例全绿；部署级路由/前端核验 ✓ | 未重验（凭据） |
| B04-02 | PR#8（cd71f234） | 前端 +18 用例全量绿 | 未重验（凭据） |

六道审批闸门：后端闸门零改动（require_human/SC-05/SC-06 代码未触碰），既有测试全量回归绿；运行态重验同样被凭据阻断。
