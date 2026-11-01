# Slice D4（T-040 交付事实）设计稿专家评审

> 评审对象：`docs/ai-pm-slice-d4-delivery-v1.md`（D4 · T-040 设计稿）
>
> 评审结论：**有条件通过，暂不建议直接实施**——总体架构方向正确，但存在涉及数据完整性、幂等性和交付事实真实性的问题，建议在 T-041 开工前修正。
>
> 记录说明：本文件为专家评审意见的原文记录，仅做格式整理（Markdown 化），不改动措辞与结论。评审修订**暂未并入设计稿正文**，待确认后收编。
>
> ⚠️ 记录状态：**评审已完整落盘**（含第八章最终裁决与原文配图转写）。评审修订**未并入设计稿正文**，待确认后收编。

---

## 总体评价

这份 D4 设计稿的架构思路是成熟的，职责边界也划分得比较清楚。但目前有几个涉及数据完整性、幂等性和交付事实真实性的问题，建议在 T-041 开工前修正。

我认可六项裁定的大方向，尤其是：

1. 不建 task_workspace，复用 D3 已经建立的工作区事实。
2. 每个仓库独立记录 Commit，不追求跨仓 Git 原子事务。
3. 数据库基线快照 + Git 祖先校验，双重防止基线漂移。
4. D4 只记录交付事实，不负责 Review 裁决。
5. 用聚合视图向 T-070 提供交付清单。
6. 不让平台自动 merge 或推送远端。

这与此前 D1/D2/D3 确立的职责划分一致：D3 管执行与工作区，D4 管交付事实，D5 管任务裁决。

不过，我发现 6 个需要重点修正的问题，其中 4 个建议列为 P0。

---

## 一、P0：必须修正的问题

### P0-1：幂等设计存在逻辑矛盾

这是目前最明显的问题。

设计规定：

```
① verify_lease
② 行锁 Run + 任务
...
⑦ 落 commit 行
   UNIQUE 冲突 → 幂等返回既有清单，200
```

但成功提交之后，系统已经执行：

```
task: running → submitted
```

此时再次提交：

- Run 可能已经不属于活跃执行状态。
- Task 已经不是 running。
- verify_lease 可能直接拒绝。
- 根本到不了第七步的 UNIQUE 冲突处理。

因此，C78 所要求的重复提交幂等，实际上可能无法实现。

建议：把幂等识别提升到提交管线前部。

建议区分两种情况：

| 情况 | 处理 |
| --- | --- |
| 相同 Run、相同执行会话、相同提交内容 | 返回原提交结果，200 |
| 相同 Run、相同仓库，但提交内容不同 | 拒绝，409 |
| 不同执行会话或旧租约 | 拒绝，409 |
| 首次提交 | 正常执行 fencing 和七步管线 |

这里还需要明确一个问题：

幂等不能简单等同于 `UNIQUE(task_run_id, repository_id)`。

因为 UNIQUE 只能证明数据库中已经存在该仓库的提交记录，不能证明第二次请求与第一次请求是同一个提交。

建议引入：

```
request_fingerprint
```

或者对规范化后的完整提交清单计算摘要。

至少需要比较：

- integration_status
- repository_id
- base_commit
- commit_sha
- changed_files
- tests

如果相同则返回原结果，不同则拒绝。

验收 C78 应当增加：成功提交后重复调用，而不是仅在同一事务内模拟 UNIQUE 冲突。

### P0-2：驳回审计与整单事务原子性存在冲突

原文：

> 任何驳回不落任何 commit 行，但写 delivery.rejected 审计。
>
> 同时：
> 提交管线单事务。

这是一个需要明确处理的事务问题。

例如：

```rust
return Err(DeliveryError::BaselineMismatch);
```

如果事务框架在遇到 Err 时自动回滚，那么：

```
delivery.rejected
```

也会跟着回滚。

最终结果：

- 没有 commit 行。
- 也没有驳回审计。
- D5 无法获取失败证据。

这直接违背设计目标。

建议采用明确的事务结果模型：

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

如果通过：
    写 commit 行
    更新 integration_status
    执行任务状态迁移
    写 delivery.submitted

如果驳回：
    不写 commit 行
    写 delivery.rejected

COMMIT
```

也就是说，业务驳回不是数据库事务失败。

它是一个成功提交的业务结果。

数据库异常才真正触发事务回滚。

这样才能同时保证：

- 驳回审计可靠存在。
- 驳回时没有部分 commit 行。
- 成功时 commit 行、状态变更和审计原子一致。

### P0-3：只读仓校验存在绕过可能

当前第四步：

```
只读仓出现 commit → 整单驳回
```

但设计没有明确检查范围。

假设任务有三个仓库：

| 仓库 | write_access | 实际情况 |
| --- | --- | --- |
| A | true | 有 commit |
| B | false | 执行端偷偷提交了 commit |
| C | true | 有 commit |

如果执行端提交时只上报 A、C：

```json
{
  "repositories": [
    {"repository_id": "A", "...": "..."},
    {"repository_id": "C", "...": "..."}
  ]
}
```

如果第四步只遍历上报的 repositories，那么 B 就会被绕过。

我的建议：校验范围必须是完整的任务仓库契约，而不是执行端上报的仓库集合。

具体：

1. 从 task_repository 读取完整契约仓集合。
2. 对每个仓库找到当前 Run 对应的 D3 工作区。
3. 对 write_access=false 的仓库检查实际 HEAD 是否发生变化。
4. 发现未经授权的提交，整单驳回。
5. 对可写仓库再进行正常提交校验。

另外，建议把检查范围明确为：

```
base_commit → 当前工作区 HEAD
```

而不只是执行端声明的 commit_sha。

否则执行端可以通过不提交某个只读仓库的声明，绕过只读约束。

这里还涉及一个 D3/D4 集成细节：必须确认工作区路径、仓库身份和镜像仓对象之间存在确定性映射，不能仅凭执行端提供的路径进行校验。

### P0-4：Git 祖先校验不能完全证明提交属于当前 Run

目前设计：

```
git cat-file -e <commit_sha>
git merge-base --is-ancestor <base_commit> <commit_sha>
```

这能证明：

- Commit 对象存在。
- Commit 是基线的后代。

但不能证明：

> 这个 Commit 确实是当前 Run 工作区产生的交付结果。

例如：

```
base_commit
    |
    +--- commit_A（其他分支或其他 Run）
    |
    +--- commit_B（当前 Run）
```

执行端上报 commit_A，祖先校验依然可以通过。

这会让平台记录一个不属于当前执行过程的提交。

建议增加一项校验：

```
commit_sha 必须等于当前 Run 工作区实际 HEAD
```

如果业务确实允许非 HEAD 的交付 Commit，则至少要求它属于当前 Run 工作区当前交付分支的可达历史，并且明确约束分支选择规则。

同时建议：

```
git cat-file -e "${commit_sha}^{commit}"
```

确保对象类型确实是 Commit，而不是仅仅存在某个 Git 对象。

另外：

changed_files 当前由执行端上报，但设计又要求文件清单由 Git 现算。

建议明确：

```
changed_files = git diff --name-status base_commit..commit_sha 的实际计数
```

执行端的计数只能用于一致性比对，不能作为最终可信数据。

否则执行端可以上报 changed_files=0，但实际提交了大量文件。

---

## 二、P1：建议在实施前修正的问题

### P1-1：只读仓与交付状态的分母定义存在矛盾

原文：

> 应交付数（契约仓数）vs 实交付数（最新已提交 Run 的 commit 行数）

但同时规定：

> write_access=false 不得出现 Commit

那么假设：

| 仓库 | write_access |
| --- | --- |
| A | true |
| B | true |
| C | false |

执行端提交 A、B 后：

```
实际提交数 = 2
契约仓数 = 3
```
视图就会得出：
```
partial
```

然而 A、B 已经完成全部允许的交付。

因此，分母必须重新定义。

建议：

```
expected_delivery_count =
    COUNT(task_repository WHERE write_access = true)
```

```
actual_delivery_count =
    COUNT(task_repository_commit)
```

只读仓仍然应该显示在 Manifest 中，但它不是预期 Commit 的交付对象。

建议状态定义：

| 状态 | 判定 |
| --- | --- |
| never_submitted | 当前任务从未有过有效提交 |
| partial | 当前有效交付仓数小于应交付仓数 |
| delivered | 当前有效交付仓数等于应交付仓数 |

只读仓单独展示：

```
write_access = false
delivery_required = false
commit_sha = null
```

还有一个边界情况：如果任务全部是只读仓，分母为零时不能直接用 0/0 判断 delivered，需要单独定义。

### P1-2：最新 Run 的选择条件不正确

原文：

> 最新 Run = 该任务 attempt_no 最大的、已落 commit 行的 Run。

我认为这个定义不够准确。

举例：

| Run | attempt_no | commit 情况 |
| --- | --- | --- |
| Run 1 | 1 | 3 个仓全部提交 |
| Run 2 | 2 | 没有提交 |
| Run 3 | 3 | 只提交 1 个仓 |

如果按最新的、有 commit 行的 Run 选择：

```
Run 3
```

没问题。

但如果：

| Run | attempt_no | commit 情况 |
| --- | --- | --- |
| Run 1 | 1 | 3 个仓全部提交 |
| Run 2 | 2 | 没有提交 |

视图会继续显示 Run 1 的完整交付。

这就掩盖了最新执行尝试没有提交的事实。

建议明确两个概念：

- latest_run：该 Task 最新的 Run，不考虑是否提交。
- latest_submitted_run：最近一次产生有效交付记录的 Run。

主 Manifest 应该明确展示当前最新 Run 的情况，同时可以提供历史有效交付记录。

不能因为最新 Run 没有提交，就自动将旧 Run 的交付当成最新交付。

这对于 D5 的判断尤其重要。

### P1-3：integration_status 出现了两个事实来源

Q-D4-4：

```json
{
  "integration_status": "passed",
  "submitted_at": "...",
  "tests": []
}
```

Q-D4-6：

`task_run.integration_status`

现在相当于保存了两份：

```
task_run.integration_status
task_repository_commit.test_evidence.integration_status
```

这会产生不一致：

```
task_run.integration_status = passed

commit.test_evidence.integration_status = failed
```

究竟哪个是真的？

建议明确单一事实来源：

```
task_run.integration_status
```

作为 Run 级集成状态。

`test_evidence` 只保存：

```json
{
  "submitted_at": "...",
  "tests": [
    {
      "name": "...",
      "passed": true,
      "evidence_ref": "..."
    }
  ]
}
```

如果确实需要保留每仓的测试证据，则每仓保存 tests 即可。

另外，integration_status 是 Run 级字段，不应该在每个仓库的 JSONB 里重复保存。

### P1-4：状态迁移入口需要与 D1 保持一致

原文：

> task running → submitted
> 经 apply_task_transition

这里需要特别注意 D1 已经冻结的约束：

任务状态迁移必须统一通过：

```
update_task_status
    ↓
validate_transition
```

不能因为 D4 新增了交付管线，就引入第二个任务状态写入入口。

建议 T-041 明确：

```
submit_delivery
    ↓
delivery validation
    ↓
update_task_status
    ↓
validate_transition
    ↓
submitted
```

如果 apply_task_transition 是现有统一入口的实际名称，需要明确它与 D1 冻结接口的关系，而不是再造一套机制。

还有一点：

D4 只允许在交付校验成功后发起状态迁移，不能直接 SQL 修改任务状态。

---

## 三、数据模型还有几个需要补充的地方

### 3.1 task_repository_commit 缺少数据约束

当前表结构总体合理，但建议补充：

```
CHECK (changed_files >= 0)
```

以及必要的字段长度、非空和 JSONB 结构校验。

更重要的是，建议明确以下关系：

```
task_repository_commit
    ├── task_run
    ├── project_repository
    └── task_repository（任务仓库契约）
```

目前 SQL 只对 project_repository 建立外键，没有对 task_repository 建立外键。

这意味着数据库只能保证仓库属于项目，不能保证它属于当前任务的仓库契约。

虽然第三步有应用层校验，但这里建议考虑增加组合外键，防止其他代码路径绕过业务校验。

当然，具体能否实现，要看现有 task_repository 的主键及唯一约束，不能为了这个外键破坏现有模型。

### 3.2 ON DELETE CASCADE 是否会破坏交付历史？

当前：

```
FOREIGN KEY (project_id, task_run_id)
    REFERENCES delivery.task_run (project_id, id)
    ON DELETE CASCADE
```

这意味着删除 Run 会删除所有交付 Commit 记录。

但 D4 的一个核心价值是保存交付历史。

建议确认：

- Run 是否允许物理删除？
- 是否存在清理历史 Run 的任务？
- 审计记录是否需要长期关联交付清单？

如果 Run 历史属于不可丢失的业务事实，建议使用 RESTRICT，或者通过软删除和归档策略保留交付数据。

不要让清理执行记录的操作顺便删除交付证据。

### 3.3 base_commit 的事实来源需要明确

表中没有：

```
base_commit
```

但设计的一句话是：

> 逐仓落「基线 → 结果 commit → 证据」

我认为不一定要在 task_repository_commit 重复保存 base_commit。

因为：

```
task_run_repository.base_commit
```

已经是 D3 领取时的基线快照。

这样更好，避免出现两个基线字段。

但需要在设计稿中明确：

```
task_repository_commit.base_commit
    =
task_run_repository.base_commit
```

Manifest 通过：

```
task_run_id + repository_id
```

关联取得基线。

同时，必须保证 task_run_repository 中每个 Run × Repository 只有一条基线记录。

### 3.4 branch 的来源没有定义

表结构有：

```
branch TEXT
```

但是 pm_submit_result 并没有 branch 字段。

需要说明：

- 是从当前工作区 Git HEAD 读取？
- 是从 D3 创建工作区时的目标分支读取？
- 还是由执行端声明？

我建议由平台从实际 Git 工作区读取，不信任执行端声明。

---

## 四、提交管线还需要明确三个执行细节

### 4.1 fencing 和行锁的顺序

当前顺序：

```
① verify_lease
② 行锁 Run + Task
```

存在一个潜在竞争窗口。

假设：

```
执行器 A 验证租约成功
        ↓
租约过期
        ↓
调度器回收 Run
        ↓
执行器 B 取得新租约
        ↓
执行器 A 继续提交
```

如果第一步只是普通查询，A 就可能带着过期验证结果继续执行。

建议：

```
BEGIN
    ↓
锁 Run（FOR UPDATE）
    ↓
锁 Task（FOR UPDATE）
    ↓
重新验证 fencing
    ↓
确认当前 Run、lease_token、executor_session
    ↓
继续提交
```

如果现有 verify_lease 已经包含锁定和原子校验，则需要在设计稿中明确这一点。

原则：fencing 的最终验证必须与提交事务处于同一个并发保护范围内。

尤其不能允许 D3 的租约回收与 D4 的交付提交互相穿透。

### 4.2 Git 校验期间的事务锁持有时间

当前方案是在单事务中执行 Git 校验。

逻辑上正确，但需要防止：

```
数据库行锁
    ↓
Git 子进程
    ↓
Git 对象读取
    ↓
Git 祖先检查
```

由于 Git 校验可能耗时，不能让它无限期占用数据库锁。

建议：

- Git 命令设置明确超时。
- 禁止网络操作。
- 限制提交仓库数量和校验规模。
- Git 异常、超时统一作为交付校验失败处理。
- 明确 Git 校验失败是否产生 delivery.rejected 审计。

特别是，不要让 Git 命令在数据库事务中等待远端网络。

### 4.3 镜像仓与 Worktree 的对象可见性

D3 已经有：

```
{AI_PM_WORKSPACE_ROOT}/git-mirror/{repo_id}
```

以及独立 Run 工作区。

这里要明确一个实现事实：

执行器在 Worktree 中产生的新 Commit，是否能够被 D4 从镜像仓直接读取？

如果 Worktree 和镜像仓共享 Git 对象库，需要确认 D4 使用只读方式访问对象时，新 Commit 已经可见。

如果 Worktree 使用独立对象库，则 D4 不能简单对镜像仓执行：

```
git cat-file -e <commit_sha>
```

而应该通过对应工作区的 Git 仓库校验。

我建议将校验目标明确为：

```
Run 对应的实际 Git 工作区
```

镜像仓用于提供基线及共享对象能力，不要假定它天然拥有执行器刚刚创建的所有对象。

---

## 五、契约与审计还有两个遗漏

### 5.1 knowledge_proposals 只记数，需要确定落点

原文：

> 收下但只记数不落提案。

方向正确，D4 不应该提前承担 D5 的知识回写职责。

但目前没有说明这个数量保存在哪里。

建议在 delivery.submitted 审计事件中保存：

```json
{
  "repository_count": 3,
  "commit_count": 3,
  "knowledge_proposal_count": 2,
  "integration_status": "passed"
}
```

如果有驳回，也可以保存对应的摘要。

这样 D5 后续能够知道执行器提出过多少知识建议，但不必在 D4 创建正式知识提案。

另外，不建议把完整 knowledge_proposals 内容直接放进审计摘要。应该有明确的大小限制，避免审计记录膨胀。

### 5.2 提交契约需要补充输入完整性校验

建议增加：

| 校验 | 处理 |
| --- | --- |
| repositories 存在重复 repository_id | 拒绝 |
| repository_id 格式非法 | 拒绝 |
| commit_sha 格式非法 | 拒绝 |
| changed_files 为负数 | 拒绝 |
| tests 缺少 name | 拒绝 |
| tests.passed 类型不正确 | 拒绝 |
| integration_status 非法 | 拒绝 |
| 超过允许的提交仓数量 | 拒绝 |
| evidence_ref 超出长度限制 | 拒绝 |

还要明确：

tests 为空是允许的，还是意味着没有提供测试证据？

这两者不能混为一谈。

not_run 也应该保留为事实，而不是在 D4 自动推断成 failed。

---

## 六、我建议调整后的核心设计

不建议推翻 D4，而是保持现有架构，修正几个关键约束。

我建议最终形成以下结构：

**D3 · 执行事实**

Run、Lease、Executor、Worktree、基线快照

**D4 · 交付事实**

逐仓交付记录

```
task_repository_commit
- repository_id
- commit_sha
- changed_files（Git 实际计算）
- test_evidence
- commit_author
- commit_authored_at
```

Run 级集成事实

```
task_run.integration_status
- passed
- failed
- not_run
```

交付 Manifest

```
v_task_delivery_manifest
```

按 Task × Repository 汇总应交付、实交付、最新 Run 和历史结果。

**D5 · 任务裁决**

Review、验收、changes_requested、completed、失败与重试决策。

这里有一个重要原则：

> D4 的 submitted 只代表提交管线接受了这次交付，不代表交付完整，更不代表任务完成。

例如：

```
Task = submitted

Expected repositories = 3
Actual commits = 2
Integration = failed
```

这仍然可以是一个有效的 D4 提交事实。

但 D5 必须根据交付清单、测试证据和验收规则决定是否通过。

不要在 D4 把 partial 或 failed 直接转换为 Task 的最终失败状态。

---

## 七、建议给 T-041 增加的验收用例

原有 C71–C84 保留，我建议追加：

| 编号 | 场景 | 期望 |
| --- | --- | --- |
| C85 | 成功提交后重复请求 | 返回原结果，200 |
| C86 | 相同 Run 重复提交不同 Commit | 409 拒绝 |
| C87 | 只读仓未出现在上报列表，但实际 HEAD 已变化 | 整单驳回 |
| C88 | 提交其他 Run 已有的后代 Commit | 拒绝 |
| C89 | changed_files 与 Git 实际计数不一致 | 以 Git 事实为准或拒绝 |
| C90 | 最新 Run 未提交，之前 Run 已交付 | 不得错误显示旧 Run 为最新 |
| C91 | 项目内合法仓库但不属于任务契约 | 数据库或应用层拒绝 |
| C92 | 驳回后审计持久化 | 有 rejected 审计，0 行 Commit |
| C93 | fencing 校验与租约回收并发 | 旧执行器不能提交 |
| C94 | Git 校验超时 | 整单驳回，无部分 Commit |
| C95 | 只读仓与可写仓混合 | 只读仓不计入应交付数 |
| C96 | 重复 repository_id | 拒绝 |
| C97 | Run 删除 | 不得意外丢失交付历史 |

---

## 八、最终裁决建议

<!-- 配图转写：原文评审结论面板 -->

| 评审维度 | 状态 |
| --- | --- |
| 架构方向 | 通过 |
| 职责划分 | 通过 |
| 数据模型 | 需要修订 |
| 提交管线 | 需要修订 |
| 验收矩阵 | 需要扩充 |

### 实施建议

P0 修复完成、相关 P1 规则明确后，可以进入 T-041。

我建议向设计方提出以下修改要求：

1. 必须修正幂等机制：成功提交后的重试必须能够真正返回原结果。
2. 必须修正驳回事务：保证 rejected 审计与整单回滚语义不冲突。
3. 必须修正只读仓校验：检查完整契约仓，而非仅检查上报仓。
4. 必须修正 Commit 归属验证：证明 Commit 属于当前 Run 的工作区。
5. 修正只读仓不参与交付分母的问题。
6. 修正最新 Run 的聚合规则。
7. 明确 integration_status 单一事实来源。
8. 与 D1 的统一状态迁移入口保持一致。

我的额外建议：不要为了 D4 再引入一套新的交付状态机。

当前的：

- Run 状态
- Task 状态
- integration_status
- delivery_state

已经足够表达不同维度的事实。

它们应该各自承担明确职责，而不是互相复制状态。

尤其不要让 delivery_state 成为新的任务状态字段。

最后一个架构判断：

我认为 D4 是整个 AI 研发管理平台从「AI 执行代码」转向「平台可信地管理 AI 交付结果」的关键模块。

D3 能证明 AI 执行过什么；D4 必须证明它实际交付了什么；D5 才能判断这些交付是否满足任务要求。

这三层边界一定要守住。

> 建议：D4 保留原设计主体，按上述意见修订 V1.1，再正式交给 T-041 实施。