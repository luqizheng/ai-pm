# D4 · 多仓库交付与结果回写 v1.1（T-040 设计稿 / T-041 已实施）

> **标注图例**：〔评审收编〕= 2026-11-01 外部专家评审意见收编进正文（评审原文与逐条对照见 `docs/slice-d/ai-pm-slice-d4-delivery-review.md`；逐项核对报告经用户确认 + Q1/Q2 补充裁决后收编）。
> 状态：**V1.1 评审收编完毕 + T-041 已实施（2026-11-02）**——4 项 P0 + 4 项 P1 + 数据模型/管线/审计修订 + 验收 C85–C97 已并入正文；T-041 准入条件（P0 修复完成、相关 P1 规则明确）已在设计层面满足，经用户过稿确认后进入 T-041 实施。实施记录见 §9。
> 日期：2026-11-01（V1.1 评审收编）/ 2026-11-02（T-041 实施回填 §9）
> 版本：V1.1（2026-11-01 外部评审收编；V1 = 同日过稿）
> 一句话：`delivery.task_repository_commit` 逐仓落「基线 → 结果 commit → 证据」（基线事实取 `task_run_repository` 领取快照，不重复落列），`pm_submit_result` 契约定稿（只加 `integration_status`），提交管线以 **⓪ 幂等识别（`request_fingerprint`）+ 事务内 fencing（`lock_lease` 复用）+ `SubmitOutcome` 事务结果模型 + 契约仓全集校验 + 实际工作区 HEAD 校验** 重构，任务状态迁移统一经 D1 `update_task_status` 命令管线 + `validate_transition`；外键 `ON DELETE RESTRICT` 保交付历史；应交付 vs 实交付对账由聚合视图陈述事实（分母只计可写仓），任务裁决归 D5。
> 上位依据：`target.md` §P5 八模块、`docs/architecture/ai-pm-multi-repo-model-v1.md` §3.2/§3.3/§4.4/§4.5、`docs/slice-d/ai-pm-slice-d-execution-pipeline-v1.md` §三 D4 行 + Q2/Q5 + §15.5（不变量：`write_access=false` 不得出现 Commit）、D2 冻结的 `pm_submit_result` draft 形状（D4 拥有定稿权）、`0011_run_dispatch.sql`（`task_run_repository` 基线快照 + `PRIMARY KEY (run_id, repository_id)`）、todo.md T-040、评审记录 `docs/slice-d/ai-pm-slice-d4-delivery-review.md`。

---

## 1. 六项裁定（用户逐项确认；V1.1 按评审收编修订）

| # | 裁点 | 结论 |
|---|---|---|
| Q-D4-1 | `task_workspace` 落表与否 | **不建**。D3 已把工作区路径确定性派生（`{root}/{run_id}/{workspace_path}`）且写入 `task_run_repository.workspace_path`；工作区审计由 D3 pid 文件 + 既有列承担。D4 只建 `task_repository_commit` |
| Q-D4-2 | 基线防篡改强度 | **双保险**：① 库层——声明 `base_commit` 必须等于 `task_run_repository` 领取快照，不等即整单驳回；② git 层——在**当前 Run 的实际 Git 工作区**校验（镜像仓仅提供基线与共享对象，不假定拥有执行器新产生的对象，见 §3.3）：`commit_sha` 为 Commit 类型对象（`git cat-file -e "${commit_sha}^{commit}"`）+ 是 `base_commit` 后代（`merge-base --is-ancestor`）+ **必须等于工作区实际 HEAD**，回读 author/时间入库。〔评审收编 P0-4/4.3〕 |
| Q-D4-3 | 变更文件清单形态 | **计数 + git 现算**：落 `commit_sha` + `changed_files` 计数（沿用 draft 契约）；`changed_files` 的**事实来源是提交时 Git 现算**（`git diff --name-status base_commit..commit_sha` 实际计数），执行端上报计数仅做一致性比对、**不一致即拒绝**；文件级清单不落库，对账/展示时现算。〔评审收编 P0-4〕 |
| Q-D4-4 | 测试证据形态 | **draft 原样落 JSONB（V1.1 收敛）**：`tests[{name, passed, evidence_ref}]` 落 `task_repository_commit.test_evidence`（含 submit 时间戳）；`integration_status` **不再内嵌**于仓级 JSONB——Run 级列是单一事实来源（见 Q-D4-6）；结论≠证据的区分留给 D5。〔评审收编 P1-3〕 |
| Q-D4-5 | 交付历史聚合 | **表 + 按 task 聚合视图**：`task_repository_commit` 天然 per-run；视图 `v_task_delivery_manifest` 按 task × 契约仓对账，T-070 直接读。V1.1 修订对账规则：分母只计可写仓、`latest_run`/`latest_submitted_run` 分离（§2.3）。〔评审收编 P1-1/P1-2〕 |
| Q-D4-6 | `integration_status` 落点 | **契约补字段 + run 级列 + 视图算整单**：D4 定稿 `pm_submit_result` 补 `integration_status`（`passed/failed/not_run`），落 `task_run` 新列且为**单一事实来源**；仓级 JSONB 不重复保存；partial 整单状态由视图计算，D4 不改任务状态机（裁决归 D5） |

---

## 2. 数据模型（迁移 `0022_delivery_manifest.sql`）

> 迁移号对账〔收编修正〕：T-032 已用 0023（`run_executor_hardening`），T-083 已顺移 0024（discovery），**0022 归本稿**（与 todo.md「T-041 保留 0022 / T-083 顺移 0024」一致），提交前核对。

### 2.1 `delivery.task_repository_commit`

```sql
CREATE TABLE delivery.task_repository_commit (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          UUID NOT NULL,
    project_id         UUID NOT NULL,
    task_run_id        UUID NOT NULL,
    repository_id      UUID NOT NULL,
    commit_sha         TEXT NOT NULL,
    branch             TEXT,                                 -- 平台从 Run 工作区 git HEAD 读取，不取执行端声明（§3.4）
    changed_files      INT  NOT NULL DEFAULT 0
                       CHECK (changed_files >= 0),           -- Git 现算为准（Q-D4-3 V1.1）
    test_evidence      JSONB NOT NULL DEFAULT '{}'::jsonb,   -- {submitted_at, tests:[{name,passed,evidence_ref}]}；integration_status 不内嵌（Q-D4-4 V1.1）
    commit_author      TEXT,
    commit_authored_at TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (task_run_id, repository_id),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (project_id, task_run_id)
        REFERENCES delivery.task_run (project_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (project_id, repository_id)
        REFERENCES integration.project_repository (project_id, id)
);
```

- 仓库身份用 `project_repository.id`（C3 裁定：multi-repo 的全局 `integration.repository` 作废）。
- `review_status` / `pr_url` / `merged_sha` **不加**（审查字段归 D5，避免落了没人写的列）。
- 前置：给 `delivery.task_run` 补 `UNIQUE (project_id, id)`（Q5 裁定 D4 一并处理，组合外键得以成立）。
- 〔评审收编 Q2 裁决〕`ON DELETE RESTRICT`：Run 历史属于交付与审计的重要关联事实，不因 Run 删除级联丢失交付记录。Run 生命周期保持 reconcile/terminate 终态语义，**不增加物理删除机制**。
- 〔评审收编 3.1〕除 `CHECK (changed_files >= 0)` 外，迁移一并补齐必要的字段长度、非空与 JSONB 结构校验；`task_repository_commit → task_repository` 组合外键**列为评估项**——T-041 落地时按 `task_repository` 现有 `PRIMARY KEY (task_id, repository_id)`（0011）形态决定能否成立，**不得为此外键破坏既有模型**。
- 〔评审收编 3.3〕`base_commit` **不在本表重复保存**：基线事实 = `task_run_repository.base_commit`（D3 领取快照；其 `PRIMARY KEY (run_id, repository_id)` 保证每 Run × Repository 唯一基线记录）；Manifest 按 `task_run_id + repository_id` 关联取得基线，避免双基线字段漂移。
- 〔评审收编 3.4〕`branch` 由平台从实际 Git 工作区读取；`pm_submit_result` 契约**不增** `branch` 字段。

### 2.2 `delivery.task_run.integration_status`

```sql
ALTER TABLE delivery.task_run
    ADD COLUMN integration_status TEXT
        NOT NULL DEFAULT 'not_run'
        CHECK (integration_status IN ('passed', 'failed', 'not_run'));
```

〔评审收编 P1-3〕本列是 `integration_status` 的**单一事实来源**（Run 级集成事实）；仓级 `test_evidence` 不再内嵌同名字段。

### 2.3 视图 `delivery.v_task_delivery_manifest`

按 task × 契约仓聚合：`task_id / repository_id / repository_name / write_access / base_commit(最新 Run) / commit_sha / changed_files / test_evidence / delivery_state`。

**对账分母（V1.1 重定义）**〔评审收编 P1-1〕：

```
expected_delivery_count = COUNT(task_repository WHERE write_access = true)
actual_delivery_count   = COUNT(task_repository_commit)
```

**`delivery_state` 判定规则（Q1 裁决，2026-11-01）**：

| 条件 | `delivery_state` |
|---|---|
| `expected_delivery_count = 0`（含全只读仓任务：空集即完备） | `delivered` |
| `expected_delivery_count > 0` 且当前任务从未产生有效提交 | `never_submitted` |
| `0 < actual_delivery_count < expected_delivery_count` | `partial` |
| `actual_delivery_count = expected_delivery_count` | `delivered` |

全只读仓示例：

```text
expected_delivery_count = 0
actual_delivery_count   = 0
delivery_state          = delivered
```

**`latest_run` / `latest_submitted_run` 分离（V1.1）**〔评审收编 P1-2〕：

- `latest_run`：该 Task `attempt_no` 最大的 Run，**不考虑是否提交**。
- `latest_submitted_run`：最近一次产生有效交付记录的 Run。
- 主 Manifest 展示 `latest_run` 的现状（未提交即如实显示 `never_submitted`/`partial`），历史有效交付经 `latest_submitted_run` 提供；**不得因最新 Run 未提交而以旧 Run 的交付顶替最新交付**（对 D5 判断尤其重要）。

**只读仓单独展示**：`write_access = false` 的仓在 Manifest 中显示 `delivery_required = false / commit_sha = null`——它出现在 Manifest，但不是预期 Commit 的交付对象。

- `delivery_state ∈ {'never_submitted', 'partial', 'delivered'}`：**视图只陈述事实，不改状态机**（`partial/changes_requested` 的任务裁决归 D5）。〔评审收编 Q1 裁决〕`delivered` 仅表示仓库交付契约完备，**不代表任务已验收或已完成**，最终裁决仍归 D5；不引入第四态（如 `not_applicable`）。
- 整单集成状态取 `task_run.integration_status`（Run 级单一事实来源）+ 视图计算。

---

## 3. 提交管线（`pm-application::delivery::submit_delivery`，单事务；V1.1 重构）

| # | 步骤 | 失败 → |
|---|---|---|
| ⓪ | **幂等识别**（管线最前部，先于 fencing）：按 `UNIQUE(task_run_id, repository_id)` 查既有提交 → 重算 `request_fingerprint` 比对 + 执行会话比对（§3.1 四情况分派） | 幂等 200 / `submit_conflict`（409）/ lease 409 |
| ① | **fencing（事务内）**：`BEGIN` → `lock_lease`（Run `FOR UPDATE`，**复用 D3 `run_context` 既有 helper**）→ 锁 Task（顺序 run → task，B2）→ 在锁内重验 token + `lease_generation` + `executor_session` | `lease_invalid` / `lease_expired`（409） |
| ② | **契约仓全集展开**：从 `task_repository` 读取完整契约仓集合（**校验范围 = 契约仓全集，而非执行端上报集合**，§3.3） | — |
| ③ | 上报校验：上报仓 ∈ 契约集合 + 输入完整性校验（§4.2 九项） | `repo_not_in_contract`（409）等 |
| ④ | **只读仓事实校验**：逐仓定位当前 Run 的 D3 工作区，对 `write_access=false` 仓检查实际 HEAD 是否变化 | `readonly_repo_commit`（409，整单驳回） |
| ⑤ | 声明 `base_commit` ≠ 领取快照 → **整单驳回**（Q-D4-2 库层） | `baseline_mismatch`（409） |
| ⑥ | 逐仓 git 校验（**实际 Git 工作区**）：`${commit_sha}^{commit}` 类型确认 + `merge-base --is-ancestor`（后代）+ **`commit_sha` == 工作区实际 HEAD** + `changed_files` 现算一致性比对 + 回读 author/时间 | `commit_not_descendant`（409） |
| ⑦ | **落库（`SubmitOutcome` 事务结果模型，§3.2）**：Accepted → commit 行 + `task_run.integration_status` + 任务 `running→submitted`（经 D1 `update_task_status` 命令管线 + `validate_transition`）+ 审计 `delivery.submitted`；Rejected → **0 行 commit** + 审计 `delivery.rejected`（原因 + 上报摘要）；**Accepted 与 Rejected 均正常 `COMMIT`** | 数据库异常 → 事务回滚 |

**四情况分派（步骤⓪）**〔评审收编 P0-1〕：

| 情况 | 处理 |
|---|---|
| 相同 Run、相同执行会话、相同提交内容（指纹一致） | 返回原提交结果，200（幂等，不产生第二份审计） |
| 相同 Run、相同仓库，但提交内容不同（指纹不一致） | 拒绝，409 `submit_conflict` |
| 不同执行会话或旧租约 | 拒绝，409（沿用 lease 语义） |
| 首次提交 | 正常执行 ①~⑦ 管线 |

**`request_fingerprint`**〔评审收编 P0-1〕：对规范化后的完整提交清单计算摘要——`integration_status` + 每仓 `repository_id / base_commit / commit_sha / changed_files / tests`。Run 级落列。理由：`UNIQUE(task_run_id, repository_id)` 只能证明「数据库已存在该仓提交记录」，**不能证明第二次请求与第一次是同一个提交**，指纹必配。契约不增字段（平台侧计算）。

### 3.1 幂等识别位置的正确性依据

原 V1 把幂等放在第⑦步 UNIQUE 冲突分支不可达：成功提交后 `task: running→submitted`、Run 已非活跃，步骤① fencing 会直接拒绝重试请求，永远到不了 UNIQUE 分支。故幂等识别**必须前移到管线最前部**（先于 fencing），且 C78 的验收方式是**成功提交后重复调用**，而非同一事务内模拟 UNIQUE 冲突。〔评审收编 P0-1〕

### 3.2 事务结果模型（`SubmitOutcome`）

〔评审收编 P0-2〕**业务驳回不是数据库事务失败**——它是成功提交的业务结果；数据库异常才真正触发回滚。

```rust
enum SubmitOutcome {
    Accepted(DeliveryManifest),
    Rejected(DeliveryRejection),
}
```

事务内部：

```
BEGIN

锁定 Run / Task
校验 fencing
执行提交校验

如果通过（Accepted）：
    写 commit 行
    更新 task_run.integration_status
    任务状态迁移（update_task_status 命令管线 + validate_transition）
    写 delivery.submitted

如果驳回（Rejected）：
    不写任何 commit 行
    写 delivery.rejected（原因 + 上报摘要）

COMMIT
```

三条保证同时成立：① 驳回审计可靠存在（D5 可取失败证据）；② 驳回时无部分 commit 行；③ 成功时 commit 行、状态变更和审计原子一致。

### 3.3 校验范围：契约仓全集与工作区 HEAD

〔评审收编 P0-3〕校验范围必须是**完整的任务仓库契约**，而不是执行端上报的仓库集合（否则漏报只读仓即绕过，如三仓只报 A/C）：

1. 从 `task_repository` 读取完整契约仓集合。
2. 对每个仓库找到当前 Run 对应的 D3 工作区。
3. 对 `write_access=false` 的仓库检查实际 HEAD 是否发生变化。
4. 发现未经授权的提交，整单驳回。
5. 对可写仓库再进行正常提交校验。

检查范围明确为 `base_commit → 当前工作区 HEAD`，而不只是执行端声明的 `commit_sha`。D3/D4 集成依赖**确定性映射**：工作区路径（`task_run_repository.workspace_path`，Q-D4-1）、仓库身份、镜像仓对象三者确定性关联，不仅凭执行端提供的路径校验。

**镜像仓与 Worktree 的对象可见性**〔评审收编 4.3〕：执行器在 Worktree 中新产生的 Commit，不假定镜像仓天然可见。git 校验目标明确为 **Run 对应的实际 Git 工作区**；镜像仓仅提供基线与共享对象能力。

### 3.4 Git 校验约束（事务锁持有）

〔评审收编 4.2〕单事务内执行 Git 校验需防止「数据库行锁 → Git 子进程 → 对象读取 → 祖先检查」链条无限占用锁：

- Git 命令设置明确超时。
- 禁止网络操作（不在事务中等待远端）。
- 限制提交仓库数量与校验规模。
- Git 异常、超时统一作为交付校验失败处理（走 §3.2 Rejected 路径，`delivery.rejected` 审计持久）。

### 3.5 审计事件摘要

〔评审收编 5.1〕`delivery.submitted` 审计事件保存摘要：

```json
{
  "repository_count": 3,
  "commit_count": 3,
  "knowledge_proposal_count": 2,
  "integration_status": "passed"
}
```

驳回时 `delivery.rejected` 保存对应摘要。D5 由此得知执行器提出过多少知识建议，但 D4 不创建正式知识提案。完整 `knowledge_proposals` 内容**不进**审计摘要，设明确大小上限，避免审计膨胀。

**`knowledge_proposals` 仍只记数不落提案**（知识回写走 D5 的 `KnowledgeBaseWriteRequested`，T-043 落地）。

**状态迁移入口**〔评审收编 P1-4〕：任务 `running→submitted` 统一经 D1 冻结机制——`update_task_status` 命令管线 + `validate_transition`（pm-domain）。V1 的 `apply_task_transition` 名称废弃（实测代码中不存在）。D4 只允许在交付校验成功后发起状态迁移，**禁止直接 SQL 修改任务状态**，不引入第二个任务状态写入入口。

- git 校验 helper 抽 pm-infrastructure 共享（工作区/镜像仓定位复用 D3），并施加 §3.4 约束。

---

## 4. 契约定稿（`pm_submit_result`）

在 D2 draft 形状上**只加一个字段**，其余原样冻结：

```json
{
  "project_id": "...", "run_id": "...", "lease_token": "...",
  "integration_status": "passed | failed | not_run",   // 新增，缺省 not_run
  "repositories": [
    { "repository_id": "...", "base_commit": "...", "commit_sha": "...",
      "changed_files": 12, "tests": [{ "name": "...", "passed": true, "evidence_ref": "..." }] }
  ],
  "summary": "...",
  "knowledge_proposals": [{ "document_id": "...", "change_summary": "..." }]
}
```

MCP 工具从 shape-first 的 `Unsupported` 变为真实落库；fencing 沿用 `accept_executor_result`（仅当前 Run 的当前执行会话可提交）。契约**不增** `request_fingerprint` / `branch` 字段（§3、§2.1）。

### 4.1 错误码

| code | 触发 |
|---|---|
| `lease_invalid` / `lease_expired` | token 错 / 过期（沿用 D2 语义；含⓪分派的「不同执行会话或旧租约」） |
| `readonly_repo_commit` | `write_access=false` 的仓出现 commit（契约仓全集校验，§3.3） |
| `baseline_mismatch` | 声明 base_commit ≠ 领取快照 |
| `commit_not_descendant` | commit 不存在 / 非 Commit 类型 / 非 base_commit 后代 / ≠ 工作区 HEAD |
| `repo_not_in_contract` | 上报仓不在任务契约集合 |
| `already_submitted` | 重复提交且指纹一致（幂等 200 返回既有清单，非错误） |
| `submit_conflict` | 相同 Run 重复提交但指纹不同（409）〔评审收编 P0-1〕 |
| `integration_status_invalid` | 取值不在三值集合 |

### 4.2 输入完整性校验

〔评审收编 5.2〕提交契约在管线步骤③统一校验：

| 校验 | 处理 |
|---|---|
| repositories 存在重复 repository_id | 拒绝 |
| repository_id 格式非法 | 拒绝 |
| commit_sha 格式非法 | 拒绝 |
| changed_files 为负数 | 拒绝 |
| tests 缺少 name | 拒绝 |
| tests.passed 类型不正确 | 拒绝 |
| integration_status 非法 | 拒绝 |
| 超过允许的提交仓数量 | 拒绝 |
| evidence_ref 超出长度限制 | 拒绝 |

**`tests` 语义**：空数组是允许的提交形态，但**不等于**「未提供测试证据」——两者在证据语义上区分，不得混同。`not_run` 保留为事实（Run 未执行集成测试），D4 **不自动推断**为 `failed`。

---

## 5. 状态语义与边界（V1.1 扩充）

```mermaid
stateDiagram-v2
    Running --> Submitted: submit_delivery 全部校验通过（守卫入口：update_task_status 命令管线 + validate_transition）
    Running --> Running: 整单驳回（审计 delivery.rejected，可修正重报）
    Submitted --> Reviewing: D5（归 T-042/T-043）
```

**`submitted` 的语义**〔评审收编六章〕：D4 的 submitted 只代表**提交管线接受了这次交付**，不代表交付完整，更不代表任务完成。例如：

```
Task = submitted
Expected repositories = 3
Actual commits = 2
Integration = failed
```

这仍然是一个有效的 D4 提交事实。D5 必须根据交付清单、测试证据和验收规则决定是否通过。**D4 不得把 partial 或 failed 直接转换为 Task 的最终失败状态。**

**四状态字段各司其职**〔评审收编八章〕：不为了 D4 再引入一套新的交付状态机。

| 字段 | 职责 |
|---|---|
| Run 状态 | D3 执行生命周期（租约、回收、终态） |
| Task 状态 | D1 状态机（唯一写入入口） |
| `integration_status` | Run 级集成事实（`passed/failed/not_run`） |
| `delivery_state` | 视图对账列（只陈述事实） |

它们各自承担明确职责，不互相复制状态；**`delivery_state` 永不成为新的任务状态字段**。

**三层边界**：D3 证明 AI 执行过什么；D4 证明它实际交付了什么；D5 才能判断这些交付是否满足任务要求。三层边界必须守住。

---

## 6. 验收矩阵（C70 → C97）

| # | 场景 | 期望 |
|---|---|---|
| C71 | 迁移断言 | `integration_status` 列默认 `not_run`；视图存在；`changed_files` CHECK；外键 RESTRICT；tenant_schema 收编新表 |
| C72 | 正常三仓交付 | 3 行 commit + `passed` + 任务 `running→submitted`（`update_task_status` 命令管线 + `validate_transition`）+ 审计 `delivery.submitted` |
| C73 | 只读仓出现 commit | 整单驳回 `readonly_repo_commit`；0 行落库 |
| C74 | 基线漂移 | `baseline_mismatch`；0 行 |
| C75 | commit 非后代 | `commit_not_descendant` |
| C76 | commit 不存在 | `commit_not_descendant` |
| C77 | 仓不在契约 | `repo_not_in_contract` |
| C78 | 重复提交（**成功提交后重复调用**，非事务内模拟 UNIQUE 冲突）〔评审收编 P0-1〕 | 幂等 200 返回既有清单；指纹比对通过；无第二份审计 |
| C79 | integration_status 非法 | 拒绝 |
| C80 | 租约过期 | `lease_expired`；不落行 |
| C81 | not_run 交付 | 落库成功（事实记录；不推断 failed；completed 禁令归 D5） |
| C82 | 视图对账三态 | `never_submitted` / `partial`(2/3) / `delivered`(3/3)（分母只计可写仓） |
| C83 | 视图历史 | 两次 Run 提交：`latest_run` 现状与 `latest_submitted_run` 历史分离展示；最新 Run 未提交不得以旧 Run 交付顶替〔评审收编 P1-2〕 |
| C84 | MCP 端到端 | `pm_submit_result` 真实落库；draft 兼容（缺省 `not_run`） |
| C85 | 成功提交后重复请求 | 返回原结果，200 |
| C86 | 相同 Run 重复提交不同 Commit | 409 `submit_conflict` |
| C87 | 只读仓未出现在上报列表，但实际 HEAD 已变化 | 整单驳回（契约仓全集校验） |
| C88 | 提交其他 Run 已有的后代 Commit | 拒绝（≠ 工作区 HEAD） |
| C89 | changed_files 与 Git 实际计数不一致 | 拒绝（Git 现算为准） |
| C90 | 最新 Run 未提交，之前 Run 已交付 | 不得错误显示旧 Run 为最新 |
| C91 | 项目内合法仓库但不属于任务契约 | 数据库或应用层拒绝 |
| C92 | 驳回后审计持久化 | 有 `rejected` 审计，0 行 Commit |
| C93 | fencing 校验与租约回收并发 | 旧执行器不能提交（`lock_lease` 行锁内重验） |
| C94 | Git 校验超时 | 整单驳回，无部分 Commit |
| C95 | 只读仓与可写仓混合 | 只读仓不计入应交付数 |
| C96 | 重复 repository_id | 拒绝 |
| C97 | Run 删除 | 不得意外丢失交付历史（RESTRICT 生效） |

---

## 7. 实施顺序（T-041）

1. 迁移 `0022`（表 + `CHECK` + `RESTRICT` 外键 + run 列 + `UNIQUE(project_id,id)` + 聚合视图〔含 Q1 判定规则与 latest 分离〕+ 组合外键评估）+ tenant_schema 收编。
2. `pm-application::delivery` 模块：⓪~⑦ 管线（幂等识别 + `request_fingerprint` + `SubmitOutcome` 事务模型 + `lock_lease` 复用 + D1 状态入口对接）。
3. git 校验抽 pm-infrastructure 共享 helper（实际工作区定位 + §3.4 超时/禁网/规模约束）。
4. MCP `pm_submit_result` 定稿实现（去掉 `Unsupported`）。
5. 用例 C71–C97 + 文档回填 + `cargo test --workspace` 全绿。

**门禁**：D1/D2/D3 用例不回归；默认环境行为不变；Run 生命周期保持终态语义、无物理删除（Q2 实施要求）。

---

## 8. 明确不做

- 不做 Review 裁决（D5）：`review_status` / 四值结论 / 验收条目逐条裁决全部归 T-042/T-043。
- 不做自动 merge / 推远端（multi-repo §2.3「禁止整单回滚」的另一半：平台不代写远端）。
- 不追求跨仓原子事务（多仓各自 commit，平台只做「整单一致性」判定）。
- 不建 `task_workspace`（Q-D4-1）；不做文件级清单落库（Q-D4-3）。
- 不做全局仓库实体（C3 裁定维持）。
- 〔评审收编八章〕不引入新的交付状态机：Run 状态 / Task 状态 / `integration_status` / `delivery_state` 各司其职，`delivery_state` 仅是视图列，永不成任务状态字段。
- 〔评审收编 Q2〕不做 Run 物理删除：生命周期保持终态语义（reconcile/terminate），交付历史受 RESTRICT 保护；清理执行记录的操作不得连带删除交付证据。

---

## 9. 实施记录（T-041，2026-11-02）

> 依据：本稿 V1.1 已过稿；todo.md T-041 行于 2026-11-02 领用。
> 验收命令：`DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm cargo test --workspace`（74 个测试二进制全绿）+ `cargo check --workspace --all-targets`（无警告）。

### 9.1 落地物清单

| 落点 | 文件 | 内容 |
|---|---|---|
| 迁移 | `migrations/0022_delivery_manifest.sql` | ① `delivery.task_run` 补 `UNIQUE (project_id, id)`（组合外键父键，Q5）；② 补 `integration_status`（Run 级单一事实来源）；③ 补 `request_fingerprint`（幂等指纹）；④ 建 `delivery.task_repository_commit`；⑤ 建视图 `delivery.v_task_delivery_manifest` |
| 管线 | `crates/pm-application/src/delivery/mod.rs` | ⓪~⑦ 单事务管线、`SubmitOutcome` 结果模型、`request_fingerprint`、§4.2 九项输入校验 |
| git 校验 | `crates/pm-application/src/delivery/git_verify.rs` | 四项校验（对象类型 / 后代 / == 工作区 HEAD / 变更计数现算）+ 超时约束 |
| 读模型 | `crates/pm-application/src/delivery/manifest.rs` | `read_manifest` / `list_manifest`（只读投影，20 列） |
| git 通道 | `crates/pm-application/src/git_access.rs` | 新增 `GitOutput` + `git_run`（非零退出码原样带回——`merge-base --is-ancestor` 用退出码表达「是/否」） |
| 租约 | `crates/pm-application/src/run_context.rs` | `lock_lease` 由私有改为 `pub(crate)`（D4 复用做事务内 fencing） |
| 契约 | `crates/pm-mcp/src/tools.rs` | `pm_submit_result` 定稿（去 `Unsupported`、补 `integration_status`、改 async 真实落库）+ `map_delivery_error` + `submit_result_with`（可注入工作区根） |
| 只读接口 | `apps/pm-api/src/pm_delivery.rs` | `GET /v1/pm/tasks/{task_id}/delivery`（前端页归 T-070） |
| 租编 | `crates/pm-infrastructure/tests/tenant_schema.rs` | `TENANT_TABLES` 收编 `delivery.task_repository_commit`（41 张） |

### 9.2 实施中的具体裁定

| # | 裁点 | 结论与理由 |
|---|---|---|
| I-D4-1 | `task_repository_commit → task_repository` 组合外键 | **不成立**。`task_repository` 主键是 `(task_id, repository_id)`，本表无 `task_id` 列（Task 身份经 `task_run` 传递）。为此外键新增 `task_id` 会复制 Run 已有的关联、破坏单一事实源。仓归属由 `(project_id, repository_id)` 与 `(project_id, task_run_id)` 两条组合外键保证〔评审收编 3.1 的「不得为此外键破坏既有模型」〕 |
| I-D4-2 | 事务所有权建模 | `finish_rejected` **按值**接收事务（`Tx = Transaction<'static, Postgres>`）并自行 COMMIT。理由：驳回是业务结果（要落审计），不能靠调用方 `?` 提前返回把事务丢掉；按值接收让「收尾」与「提交」同处一个函数，不存在半提交路径 |
| I-D4-3 | 可写仓漏报**不驳回** | D4 §5 明示 `submitted` ≠ 交付完整。提交 2/3 个仓仍是有效的 D4 交付事实，视图据此显示 `partial`；「应交付 vs 实交付」的最终裁决归 D5。驳回权只保留给**事实冲突**（只读仓有变化 / 基线漂移 / git 校验失败 / 仓不在契约） |
| I-D4-4 | 落库的 `commit_sha` 取值 | 取**工作区实际 HEAD**（`rev-parse HEAD`），不取执行端声明。执行端声明只做一致性比对——不等即驳回。依据：Q-D4-2 双保险的 git 侧 |
| I-D4-5 | `changed_files` 的权威 | Git 现算（`diff --name-status base..commit`）；执行端上报计数只做比对，不一致即驳回（Q-D4-3） |
| I-D4-6 | 审计 actor 回退 | 项目 human 成员 → 平台 service principal（`DISPATCH_SERVICE_PRINCIPAL`）。保证「项目无 human 成员」时提交不会因 `RowNotFound` 失败 |
| I-D4-7 | `request_fingerprint` 落 Run 列 | Run 级落列（`delivery.task_run.request_fingerprint`），不落仓级。理由：指纹覆盖整单（含 `integration_status`），仓级无处安放 |
| I-D4-8 | `pm_submit_result` 的返回形状 | 业务驳回**不是异常**：管线把驳回作为 `SubmitOutcome::Rejected` 返回（审计已持久），工具层照样回 200 并带 `outcome: "rejected"` + 机器可读 `code`。调用方据此知道「没有交付」而不是「调用失败」。真正的异常（租约失效 / DB 错误）才映射到 `ToolError` |
| I-D4-9 | git 校验异常的落点 | git 失败与超时统一走 Rejected 路径（`commit_not_descendant`），**不**用独立的 `git_verification_failed` 错误码——错误码表是 §4.1 的九项，不额外扩张 |

### 9.3 验收矩阵落地情况（C71–C97，27 条）

| # | 用例 | 位置 |
|---|---|---|
| C71 | 迁移形状断言 | `crates/pm-application/tests/delivery.rs::c71_migration_shape_is_in_place` |
| C72 | 三仓交付 | `delivery.rs::c72_three_repository_delivery_lands_commits_and_submits_the_task` |
| C73 | 只读仓有 commit → 整单驳回 | `delivery.rs::c73_readonly_repository_with_a_commit_rejects_the_whole_order` |
| C74 | 基线漂移 | `delivery.rs::c74_baseline_drift_is_rejected` |
| C75 | commit 非后代 | `delivery.rs::c75_commit_that_is_not_a_descendant_is_rejected` |
| C76 | commit 不存在 | `delivery.rs::c76_nonexistent_commit_is_rejected` |
| C77 + C91 | 仓不在契约（应用层 + 库层外键） | `delivery.rs::c77_c91_repository_outside_the_contract_is_rejected` |
| C78 + C85 | 幂等重放（成功提交后重复调用） | `delivery.rs::c78_repeated_submission_after_success_replays_the_same_manifest` |
| C79 | `integration_status` 非法 | `delivery.rs::c79_invalid_integration_status_is_rejected` |
| C80 | 租约过期 | `delivery.rs::c80_expired_lease_does_not_land_a_row` |
| C81 | `not_run` 如实记录 | `delivery.rs::c81_not_run_delivery_is_recorded_as_a_fact` |
| C82 | 视图三态 | `delivery.rs::c82_manifest_view_reports_the_three_states` |
| C83 | `latest_run` / `latest_submitted_run` 分离 | `delivery.rs::c83_view_separates_the_latest_run_from_the_latest_submitted_run` |
| C84 + C85 | MCP 端到端落库 + 幂等重放 | `crates/pm-mcp/tests/contract.rs::c84_c85_submit_result_lands_the_delivery_and_replays` |
| C86 | 同 Run 不同 commit → 冲突 | `delivery.rs::c86_same_run_with_a_different_commit_is_a_conflict` |
| C87 | 只读仓漏报但实际已变 | `delivery.rs::c87_readonly_repository_moved_but_not_reported_rejects_the_whole_order` |
| C88 | 上报非 HEAD 的 commit | `delivery.rs::c88_commit_that_is_not_the_workspace_head_is_rejected` |
| C89 | `changed_files` 与 Git 计数不一致 | `delivery.rs::c89_changed_files_must_match_the_git_count` |
| C90 | 最新 Run 未提交不得借用旧 Run | `delivery.rs::c90_latest_run_without_submission_does_not_borrow_the_previous_run` |
| C92 | 驳回审计持久 | `delivery.rs::c92_rejection_is_audited_with_a_summary` |
| C93 | 旧 generation 不能提交 | `delivery.rs::c93_a_stale_lease_generation_cannot_submit` |
| C94 | git 超时整单驳回 | `delivery.rs::c94_git_timeout_rejects_the_whole_order` |
| C95 | 分母只计可写仓 | `delivery.rs::c95_readonly_repositories_do_not_count_towards_the_delivery_denominator` |
| C96 | 重复 `repository_id` | `delivery.rs::c96_duplicate_repository_id_is_rejected` |
| C97 | RESTRICT 保交付历史 | `delivery.rs::c97_deleting_a_run_must_not_drop_the_delivery_history` |

C11（D2 遗留）随 D4 定稿重写：`crates/pm-mcp/tests/contract.rs::c11_submit_result_rejects_a_repository_outside_the_contract`——从「shape-first 不碰表」改为「真实落库 + 整单驳回 + 0 行 commit + 任务状态不动 + 驳回审计 1 条」。

### 9.4 与后续切片的口径

- **D5 的接口点**：D5 通过 `delivery.task_run` 加 `run_kind` 列区分 review run 并重建 `uq_task_run_active` 索引。D4 的视图按 `attempt_no` 排序取 `latest_run` / `latest_submitted_run`，与该重建不冲突。
- **`integration_status` 的消费方**：D4 只落事实。`passed` / `failed` / `not_run` 的**判读**（何时算失败、何时算完成）归 D5 的完成判定器；`not_run` 在 D4 不被推断为 `failed`。
- **`partial` 的裁决方**：D4 视图如实显示 `partial`，**不**改任务状态。`changes_requested` 的裁决归 D5。
- **`DeliverySubmitted` 事件**：D1 已在 `submitted` 迁移边登记 `outbox_event_for_task_target`（默认只开 `TaskReady`），D4 不新增生产者；T-061 的「搭车」项随 T-041 落地。