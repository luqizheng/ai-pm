# P1 真实业务链 E2E 测试用例与执行记录 V1.0

> 2026-11-16 · 阶段 D（交付物 05 V1.1 §4.1 / §4.3；G3/G4）
> 代码快照：master `e16e228d` + 本任务新增测试代码（未提交）
> 执行环境：一次性库 `aipm_chain_e2e`（127.0.0.1:16499 容器 `aipm-teamqa-pg`）+
> 本地 JWKS + 真 pm-api(:18490, jwks) + 真 ai-pm-worker + mock-llm（FakeModel v1）
> 执行入口：`pnpm exec playwright test -c playwright.chain.config.ts`
> 汇总：**10/10 PASS**（6 主链 + 4 分支）；证据 `test-case/evidence/chain/`。
>
> 11 字段标准（`docs/e2e-spec/00-e2e-testing-framework-v1.md` §4）：
> Case ID / Suite / Priority / Preconditions / Test Data / Steps / Expected Result /
> Actual Result / Status / Evidence / Cleanup。

## 主链（S1~S6）

| 字段 | CHAIN-S1 | CHAIN-S2 | CHAIN-S3 |
|---|---|---|---|
| Suite | P1 业务链 G3 | P1 业务链 G3 | P1 业务链 G3 |
| Priority | Must | Must | Must |
| Preconditions | 团队+项目已建；本地 file:// 仓库夹具 verified、HEAD 固定 | S1 analyzed | S2 draft KB 已生成 |
| Test Data | `chain-repo` @ `c2aee36e…`（固定 commit） | report `f617c31f…` | 同左 |
| Steps | 1) POST `/v1/pm/repositories/{id}/analyze`；2) 立即重复 analyze；3) 轮询报告终态 | GET `/v1/pm/analysis-reports/{id}`；查 KB 状态/索引；确认前检索 | GET 报告详情；查 `pc.proposal`/`pc.approval`/`pc.source`/`pc.evidence` |
| Expected Result | 202 + report_id；重复 409`report_open`+同 report_id；报告 analyzed 且 commit_sha=HEAD | KB revision=draft、analysis_mode∈{derived,llm}；确认前索引 0 行、检索不命中 | origin_kind=ai_agent、submitted、approval pending；对象 lifecycle 无 active；source/evidence 钉 KB revision；通知 `pc_proposal_pending` |
| Actual Result | 202；409 report_open 同 id；analyzed，commit=`c2aee36e…`，run_mode=full | draft / derived；索引 0；检索空 | ai_agent / submitted / pending；lifecycle=draft（+identity 单例）；source+evidence 钉 `89cedecb…`；通知已留痕 |
| Status | **PASS** | **PASS** | **PASS** |
| Evidence | `playwright-results.json`；`p0-pm-api.log` | 同上 | 同上 |
| Cleanup | 一次性库 teardown 自动 DROP | 同左 | 同左 |

| 字段 | CHAIN-S4 | CHAIN-S5 | CHAIN-S6 |
|---|---|---|---|
| Suite | P1 业务链 G3 | P1 业务链 G3 | P1 业务链 G3 |
| Priority | Must | Must | Should |
| Preconditions | S3 pending approval | S4 已 approve + KB published | S5 closed |
| Test Data | proposal `c4e4e9e9…`；KB rev `89cedecb…` | report `f617c31f…` | 同左 |
| Steps | 1) AI(ai_pm) decision；2) api_key decision；3) 真人 KB confirm；4) 等索引构建；5) 真人 PC approve | POST `/v1/pm/analysis-reports/{id}/close`；查 version/version_member/lifecycle/锚点 | 再次 POST close；对比版本数 |
| Expected Result | AI/api_key 403；KB→published；索引 FTS 命中；PC→approved 且未发布版本；通知+审计主体可追溯 | applied=1；version+snapshot_hash+members；lifecycle→active；锚点=HEAD；发布通知+close 审计可追溯 | 不重复发布（实际行为记录） |
| Actual Result | 403/403；published；索引>0 且 FTS `chain` 命中；approved，`published_version_no=null`；通知 `kb_document_revision_published`，发布审计 actor=owner | applied=1、failed=0、withdrawn=0、rejected=false；version `5167eefa…` 快照非空；active；锚点=`c2aee36e…`；通知 `pc_version_published`，close 审计 actor=owner | **409**（报告非 analyzed）；版本数不变；报告仍 closed |
| Status | **PASS** | **PASS** | **PASS**（实际行为登记；E2-3 候选契约待裁决） |
| Evidence | 同上 | 同上 | 同上 |
| Cleanup | 自动 DROP | 同左 | 同左 |

## 分支场景（B1~B4）

| 字段 | CHAIN-B1 | CHAIN-B2 |
|---|---|---|
| Suite | P1 分支（G3 附件） | P1 分支（G3 附件） |
| Priority | Should | Should |
| Preconditions | 新仓库 b1 analyzed | 仓库 b2 首份报告已关闭（锚点固化） |
| Test Data | report `a837d818…` | anchor `124c4990…`；新 commit `be6faa0d…` |
| Steps | 不决策/不确认 → close | 增量 commit + `mode=incremental` analyze → 查报告 |
| Expected Result | 提案 withdrawn、KB draft rejected、零版本 | run_mode=incremental；since_commit=锚点；commit=新 HEAD |
| Actual Result | applied=0、withdrawn=1、rejected_kb=true；KB rejected；版本数不变 | incremental；since=`124c4990…`；commit=`be6faa0d…` |
| Status | **PASS**（证据 `branch-b1-close-residual.json`） | **PASS**（`branch-b2-incremental.json`） |
| Evidence | 同左 | 同左 |
| Cleanup | 自动 DROP | 同左 |

| 字段 | CHAIN-B3 | CHAIN-B4 |
|---|---|---|
| Suite | P1 分支（G3 附件） | P1 分支（G3 附件） |
| Priority | Should | Should |
| Preconditions | 新仓库 b3，无历史锚点 | 新仓库 b4，url 置为不可达 file:// |
| Test Data | `mode=incremental` | `file:///definitely-missing-chain-repo-9402` |
| Steps | analyze → 查报告 | analyze → 轮询 failed；查 intake task；修复 url 后重试 |
| Expected Result | 无锚点退 full（since 空） | status=failed、error_code/error_detail 可见、任务 FAILED；失败不占门 |
| Actual Result | full / since 空 | failed；`kb_analysis_failed`；`git: git authentication failed`；任务 FAILED；重试 202 |
| Status | **PASS** | **PASS**（`stage_detail` 为空 = DEF-CHAIN-01；错误分类不准确 = DEF-CHAIN-02） |
| Evidence | `playwright-results.json` | `branch-b4-failure.json` |
| Cleanup | 自动 DROP | 同左 |

## 未覆盖/不适用

| 项 | 结论 |
|---|---|
| 两段式取消（仓库分析） | **NOT_APPLICABLE**：仓库分析/报告链无 cancel 端点（取消仅在 imports/docproc）；不伪造通过 |
| E2-1/2/4/5 Close 部分失败/恢复/重复返回语义 | 候选契约（F4）；本轮只记录实际行为（S6=409），不作强制断言 |
| Conflict 冻结 / KB Git 权威 / PCGIT | 阶段边界（M2）：STUB/后续 Phase，不作 P1 断言 |
