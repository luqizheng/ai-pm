# 阶段 D · P1 真实业务链 E2E 判定（G3 / G4）与 G2 复判记录

> 2026-11-16 · 代码快照 master `e16e228d`（SEC-P0/P1 修复已在链上）+ 本任务新增测试代码（未提交）
> 上游：交付物 05 V1.1 §4/§5/§6；V-1/V-2 专项任务书 §13；D05/D06
> 结论：**G2 通过（复判）· G3 通过（10/10）· G4 证据包就绪**。

---

## 1. G2 复判（文书落章）

| 条件（交付物 05 §5） | 结果 | 证据 |
|---|---|---|
| 规定负向测试（Regression 类）通过 | ✅ | L2 三套件 29/29（P0 5 + V1 15 + V2 9） |
| 无未关闭 P0 | ✅ | SEC-P0-1~5 修复 + 逐项回归 |
| 无未关闭 P1 | ✅ | SEC-P1-1 修复（`require_human_decision_context`，两入口）；SEC-P1-2 测试落地 |
| 关键 UNKNOWN 关闭 | ✅ | V1-T07-A 架构事实关闭（无独立服务层）；V2-T06 无删除面；T08-C 特征化基线 |
| **G2** | **通过** | 允许进入 P1 业务链（G3） |

专项终判（任务书 §13 六分类）：**V-1 = MATCH**（差异类型：无；残留 `SEC-P2-4` append-only 纵深防御登记）；
**V-2 = PARTIAL**（仅 SEC-P2-1/2/3 跨域引用校验；P0/P1 = 0）。
允许关闭 V-1；V-2 待 SEC-P2 修复或专家记录 P2 例外。允许进入 G3：**是**。

---

## 2. 执行环境（可复现快照）

| 项 | 值 |
|---|---|
| 仓库/分支 | `D:\kudi-project\templates\ai-pm` · master · HEAD `e16e228d` |
| 数据库 | 一次性 `aipm_chain_e2e`（容器 `aipm-teamqa-pg`，127.0.0.1:16499；用毕 DROP） |
| 服务 | 真 pm-api(:18490, `AI_PM_AUTH_MODE=jwks`，本地 JWKS :18491) + 真 ai-pm-worker + mock-llm(:18497) |
| 夹具 | 本地 file:// Git 仓库（固定 commit `c2aee36e…`）；FakeModel 版本 `fake-model-v1`；worker 单实例串行 |
| 迁移/业务代码 | 零 Schema、零 Migration、零业务代码改动（阶段 D 只新增测试与文档） |
| 命令 | `pnpm exec playwright test -c playwright.chain.config.ts`（env 见 §5） |
| 结果 | **10 passed (32.5s)**（6 主链 + 4 分支） |

---

## 3. G3 判定表（S1~S6）

| 步 | 判定 | 关键证据（DB / API） |
|---|---|---|
| S1 analyze | **PASS** | 202 + report `f617c31f…`；重复 analyze 409 `report_open`（同 id）；报告 analyzed，commit=`c2aee36e…`，run_mode=full |
| S2 KB draft | **PASS** | `kb.document_revision` `89cedecb…` status=draft、analysis_mode=derived；确认前索引 0 行、`/v1/retrieval/query` 不命中 |
| S3 PC 起草 | **PASS** | proposal `c4e4e9e9…` origin_kind=ai_agent、submitted；`pc.approval` pending；对象 lifecycle=draft（无 active）；`pc.source`/`pc.evidence` 钉 `89cedecb…`；通知 `pc_proposal_pending` 留痕 |
| S4 人工门 | **PASS** | AI(ai_pm) decision 403；api_key decision 403；真人 KB confirm→published；索引 FTS `chain` 命中；真人 PC approve→approved（`published_version_no=null`）；通知 `kb_document_revision_published`（dedup=revision）；`audit_log` 发布 actor=真人 owner |
| S5 close | **PASS** | applied=1 / failed=0 / withdrawn=0 / rejected_kb=false；version `5167eefa…` + version_member + snapshot_hash；lifecycle→active；锚点=HEAD；通知 `pc_version_published`（dedup=version）；`audit_log` close actor=真人 owner |
| S6 幂等 | **PASS（实际行为登记）** | 重复 close→409（报告非 analyzed）；版本数不变；候选契约 E2-3 待裁决 |

### 3.1 E1 对象身份关联表

| 对象 | 本次链身份 |
|---|---|
| Analysis Report | `f617c31f-68f6-42f8-b675-b4601bb2eeab`（= repo_analysis_report 行） |
| Intake | `1d2e901f-9c75-40cc-8a04-f951159c8aca`（E2E 追踪键 = report_id + intake_id） |
| KB Document / Revision | `c948e71d-8226-4f9e-ab92-d5b8f018ae94` / `89cedecb-24e1-4e4e-8769-e0b66a1f91d5` |
| PC Proposal / Approval | `c4e4e9e9-6f99-4101-adfe-4a05f4de0998` + pending approval（ai_agent 起草） |
| Cognition Version | `5167eefa-1712-4cbe-be60-8a5db4038994`（snapshot_hash + version_member） |
| Close | 采用版本集合 = 上述 version_member 明细；锚点 `c2aee36e…` |
| **Compatibility Ref（现状偏差）** | Report 身份=report 行；正文由 `kb.document_revision` 持有；`kb_revision_id` 为历史兼容引用，**不得视为独立 Report Revision 对象**（P2 落地前） |

完整机器可读版：`test-case/evidence/chain/e1-object-identity.json`。

---

## 4. 分支场景

| 编号 | 场景 | 结果 |
|---|---|---|
| B1 | close 残留（不决策/不确认） | applied=0、withdrawn=1、rejected_kb=true；KB rejected；零版本（`branch-b1-close-residual.json`） |
| B2 | 增量链（新 commit + incremental） | run_mode=incremental、since_commit=上一关闭锚点、commit=新 HEAD（`branch-b2-incremental.json`） |
| B3 | 无锚点 incremental | 退 full，since 空 |
| B4 | 仓库不可达失败恢复 | 报告 failed、error_code=`kb_analysis_failed`、error_detail=`git: git authentication failed`；intake task FAILED；修复 url 后重试 202（失败不占门） |
| 取消 | 两段式取消 | **NOT_APPLICABLE**：仓库分析/报告链无 cancel 端点；不伪造通过 |
| E2-1/2/4/5 | Close 部分失败/恢复契约 | 候选契约（F4），仅记录实际行为，不作强制断言 |

### 4.1 E2-1~E2-5 实际行为登记（候选契约，不作强制断言）

| 编号 | 场景 | 本轮实际行为 |
|---|---|---|
| E2-1 | 一批 Proposal 部分成功/部分失败 | **NOT_TRIGGERED**：主链单提案一次发布成功；未构造混合批次 |
| E2-2 | 部分成功→Worker 中断→重启→重复 Close | **NOT_TRIGGERED**：未注入 Worker 中断 |
| E2-3 | 重复 Close | S6 实测 **409**（报告已 closed，非 analyzed），版本数不增；「返回原结果」未实现 |
| E2-4 | `pc-decision:{id}` 幂等键并发 | **NOT_TRIGGERED**：单决策/单发布；幂等键在 close 发布事务内使用，重复 close 不会二次发布（E2-3 已间接验证） |
| E2-5 | Close 成功 vs 发布状态一致性窗口 | **NOT_TRIGGERED**：单提案，无中间窗口可观测 |

---

## 5. G4 证据签收包

### 5.1 可复现命令

```powershell
cargo build -q -p pm-api --bin pm-api
cargo build -q -p ai-pm-worker --bin ai-pm-worker
$env:TEAM_QA_PM_API_BIN="<repo>\target\debug\pm-api.exe"
$env:P0_WORKER_BIN="<repo>\target\debug\ai-pm-worker.exe"
$env:P0_DB="aipm_chain_e2e"
$env:P0_LOG_DIR="<repo>\test-case\evidence\chain"
$env:AI_PM_SECRET_KEY="e2e-chain-secret"
$env:NO_PROXY="127.0.0.1,localhost"
cd ai-frontend
pnpm exec playwright test -c playwright.chain.config.ts
```

等价入口：`bash test-case/evidence/chain/run-chain.sh`（Git Bash；本机未装 Git Bash 时用上方 PowerShell）。

### 5.2 证据清单（`test-case/evidence/chain/`）

| 文件 | 内容 |
|---|---|
| `playwright-results.json` | 10 用例逐条状态/耗时 |
| `p0-pm-api.log` | 真 pm-api 全量日志（含报告/PC/KB 状态推进） |
| `p0-worker.log` | 真 worker 消费日志（intake/outbox/索引） |
| `e1-object-identity.json` | E1 关联表机器可读版 |
| `branch-b1-close-residual.json` / `branch-b2-incremental.json` / `branch-b4-failure.json` | 分支场景 DB 快照 |

**DB 前后状态**：断言口径为「写前快照 → 操作 → 写后终态」，DB 为唯一终态证据；
一次性库在 teardown 自动 DROP（`TEAM_QA_KEEP_DB=1` 可保留复现）。

### 5.3 失败日志

本轮无失败用例。首次迭代的 4 处断言偏差（analyze 回执 status=ROUTED、bootstrap identity 单例、
retrieval planner 默认关闭、intake_task 列名）均为测试实现修正，非业务缺陷；
修正后全量 10/10 绿。

---

## 6. 缺陷登记（八要素）

| 编号 | DEF-CHAIN-01 |
|---|---|
| 专项/等级 | P1 业务链 / **S3（一般，非阻断）** |
| 影响对象 | 分析报告失败可见性（`integration.repo_analysis_report`） |
| 复现路径 | 仓库 url 不可达 → analyze → 报告 failed |
| 证据 | `branch-b4-failure.json`：error_code=`kb_analysis_failed`、error_detail 有值、**stage_detail 为空** |
| 实际影响 | 失败原因可经 error_detail 定位；阶段描述缺失，前端需兼容空值 |
| 差异分类 | **Verification Gap**（观测字段完整性），非安全、非数据一致性 |
| 推荐修复 | `mark_failed` 同时写入阶段描述（如 `stage_detail='KB 分析失败'`）；单独报批 |
| 是否阻断 G3 | **否** |

| 编号 | DEF-CHAIN-02 |
|---|---|
| 专项/等级 | P1 业务链 / **S3（一般，非阻断）** |
| 影响对象 | 仓库分析失败原因分类（`error_detail`） |
| 复现路径 | 仓库 url 指向不存在的 file:// 路径 → analyze |
| 证据 | `branch-b4-failure.json`：error_detail=`git: git authentication failed`（实际是路径不存在） |
| 实际影响 | 错误分类误导排障（把「不可达/不存在」报成「认证失败」）；不改变失败判定 |
| 差异分类 | **Implementation Gap**（错误分类/消息准确性） |
| 推荐修复 | 区分 clone 的 not-found / auth / network 错误并给稳定错误码；单独报批 |
| 是否阻断 G3 | **否** |

其他观察（非缺陷）：
- `/v1/retrieval/query` 在 planner 默认关闭时确定性降级、证据为空——配置行为；本轮"可检索"以 KB 章节索引 FTS 命中为证据。
- S6 重复 close 返回 409——E2-3「返回原结果」为候选契约（F4），实际行为已登记，不判缺陷。

---

## 7. 终局状态

| 门 | 状态 | 允许后续 |
|---|---|---|
| G1 P0 修复 | ✅ 通过 | — |
| G2 V-1/V-2 专项 | ✅ 通过（V-1 MATCH / V-2 PARTIAL 仅 P2） | 进入 G3 |
| G3 P1 业务链 | ✅ 通过（S1~S6 全链成功 + 人工门负向成立 + 分支有结果） | 进入 P2 Analysis 详细设计 |
| G4 证据签收 | ✅ 就绪（可复现命令/日志/DB 终态/环境快照） | 专家签收 |

**允许进入 P2 Analysis 详细设计：是。**
**允许编制后续 Migration 任务书：是（仅编制；执行须另批）。**
**不允许：** 启动 Schema Migration、提前实现 KB Git 化/PCGIT/Conflict 完整生命周期、修复 SEC-P2/DEF-CHAIN-01（均须单独报批）。
