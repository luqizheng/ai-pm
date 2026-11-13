# AI-PM：D2.1 完整策略合成与策略版本口径（V1 · 裁定与实现）

> 状态：已实现并验证。
> 范围：`ai-pm` 后端策略链路（迁移 `0016`、`pm-application::policy_composition`、`run_dispatch` 的策略评估与 `policy_snapshot`）。
> 不在范围：D3 进程启动、D4 交付落库、D5 状态裁决；执行端能力档位与预注册；策略 DSL。

---

## 一、一句话结论

D1 文档把「完整策略合成」与 `policy_version` 口径划给 D2，D2 文档又把它们推给「策略阶段」，两份文档互相矛盾。本轮裁定：**以 D1 §九 的归属为准，另开 D2.1 真做**，而不是继续挂账。实现上补齐 `delivery.policy_scope` 的能力类与额外校验字段，按「能力类 AND、约束类累加」四层合成，并把版本改为内容寻址（`policy_hash` = canonical 有效策略的 sha256，`policy_version` = 前 12 位）。

---

## 二、前因：这个缺口是怎么留下的

时间线（均为 2026-11-01）：

1. **D1 成文**（`docs/slice-d/ai-pm-slice-d-execution-pipeline-v1.md`）时，策略引擎只有任务级 `delivery.task_policy_override`，组织/项目/任务类型三层没有存储位。D1 §九 #1/#2 因此把「完整策略合成」与「`policy_version`/`policy_hash` 生成口径」标为留口并**明确归属 D2**（§九 表格第 694–695 行、§十三 表格第 780–781 行）。
2. **D2 成文**（`docs/slice-d/ai-pm-slice-d2-mcp-context-v1.md`）时，D2 的主线是「MCP 契约 + Run 级凭据 + 进度上报」，策略合成被视为另一个能力域，于是 §1.3 第 52 行写成「需策略引擎类型化，另立阶段」，§6.4 第 354 行写成「`0014` 不引入策略版本表，留策略阶段」。
3. **D2 按自己的文档交付**：`run_dispatch` 写入的 `policy_version` 是 `null`，`snapshot_scope` 是 `d1-minimal`，`execution_enabled` 是字面量 `true`（见 `crates/pm-application/src/run_dispatch.rs` 迁移前实现）。
4. **冲突暴露**：D1 的独立评估器把 ①② 划给 D2，D2 文档却写「不做」；这不是实现遗漏，而是**两份设计文档的归属口径不一致**。

---

## 三、裁定

### 3.1 以 D1 §九 为准

理由按优先级：

1. **显式验收项不得降级推迟**。D1 的验收口径把两项列为「转 D2 / D3」的未解决风险，D2 若以「另立阶段」结案，等于把已承诺的验收项再往后推一次。
2. **缺口是实打实的数据模型缺口**，不是「策略引擎不够类型化」的抽象问题。`delivery.policy_scope`（迁移 `0013`）只有 `require_human_approval` 一列；组织/项目/任务类型三层根本没有 `auto_accept` / `auto_merge` / `auto_publish_knowledge` / `additional_checks` 的存储位。没有字段就没有合成，继续等待不会让字段自己出现。
3. **D2 的原始范围并未被本轮破坏**。D2 的契约（`pm_get_run_context` 返回体、租约、工具清单）本轮**一个字都不改**；改动只发生在「领取时写入的快照内容」与「策略读取的 SQL」。

### 3.2 D2 文档的「不做」条目如何处置

D2 §1.3 与 §6.4 的原文保留为历史记录，但追加「V1.2 修订」标注：该项已并入 D2.1 并完成，后续读者以本文件为准。具体回填位置见 §九。

---

## 四、实现方案

### 4.1 数据模型（迁移 `0016_policy_composition.sql`）

只补列，不改 0013 的既有语义：

| 列 | 类型 | 语义 |
| --- | --- | --- |
| `auto_accept` | `BOOLEAN` 可空 | `NULL`＝该层不表态；`false`＝该层禁止自动验收 |
| `auto_merge` | `BOOLEAN` 可空 | 同上，作用于自动合并 |
| `auto_publish_knowledge` | `BOOLEAN` 可空 | 同上，作用于知识自动发布 |
| `additional_checks` | `JSONB NOT NULL DEFAULT '[]'` | 约束类：只能追加，空数组＝不追加 |
| 约束 `policy_scope_additional_checks_array` | `CHECK` | 非数组写入被数据库拒绝，不靠应用层自觉 |

`require_human_approval` 仍是 `NOT NULL`（0013 原状），缺行＝该层不表态。迁移用 `ADD COLUMN IF NOT EXISTS` + `pg_constraint` 元数据判断后加 CHECK，可重入；不依赖隐式约束名（R2）。

### 4.2 合成规则（`crates/pm-application/src/policy_composition.rs`）

四层输入：`organization → project → task_type → task`。

| 类别 | 字段 | 运算 | 说明 |
| --- | --- | --- | --- |
| 能力类 | `auto_accept` / `auto_merge` / `auto_publish_knowledge` | 所有表态层取 **AND** | `NULL` 视为不表态，不额外收紧；任务级 `true` 不能放宽高层的 `false` |
| 约束类 | `require_human_approval` | 任一层 `true` 即 `true` | 低层 `false` 不能放宽高层 `true` |
| 约束类 | `additional_checks` | **并集 + 去重 + 稳定排序** | 「只能增加，不能减少」 |
| 交叉否决 | `require_human_approval → auto_accept` | 要求人工审批时 `auto_accept=false` | 按 §15.6 示例函数实现 |

**一个刻意的取舍**：`auto_merge` 与 `auto_publish_knowledge` **不**共用 `require_human_approval` 的判断。§15.6 原文明确建议「使用独立的 `resolve_auto_merge`、`resolve_knowledge_publish` 函数，避免不同动作错误共用同一个审批判断」。当前只有单一审批布尔字段，强行共用会把「审批」错误地解释成「三种自动化全部关闭」。待 D5 出现分动作审批字段后，再各自接入。

### 4.3 版本口径：内容寻址

- 对**不含时间字段**的 canonical 有效策略取 sha256：
  `schema + auto_accept + auto_merge + auto_publish_knowledge + require_human_approval + additional_checks`（字段顺序由结构体定义固定）。
- `policy_hash` = 全量 64 位小写 hex；`policy_version` = 前 12 位。
- `captured_at` **不进入哈希**。否则同一策略在不同时刻领取会产生不同「版本」，版本号就失去「策略内容标识」的语义。
- 因此：**同一有效策略由不同层级拆分得到，hash 相同**（内容寻址的本意）；任一策略字段变化，hash 与 version 同时变化。
- `POLICY_SCHEMA = "composed-v1"` 参与哈希：将来策略语义变更（例如引入分动作审批）必须升版本，避免同 hash 不同含义。

不新增策略版本表，也不引入序列号。这样没有额外写入路径与状态一致性债。

### 4.4 快照结构

`delivery.task_run.policy_snapshot` 现在写入：

```json
{
  "policy_schema": "composed-v1",
  "policy_version": "4f9c1a0b2d3e",
  "policy_hash": "4f9c1a0b2d3e...(64 位)",
  "captured_at": "2026-11-01T...",
  "approval_required": false,
  "approval_status": "approved",
  "execution_enabled": true,
  "repository_baselines": [ ... ],
  "snapshot_scope": "composed-v1",
  "effective_policy": {
    "auto_accept": true,
    "auto_merge": false,
    "auto_publish_knowledge": true,
    "require_human_approval": false,
    "additional_checks": ["architecture_review"]
  }
}
```

`execution_enabled` 改为由「策略门 + 审批证据」推导（`approval_status == "approved"`），不再是字面量。它**不等于** `auto_accept`：后者决定验收是否自动化，前者决定本次能否启动。

---

## 五、落点表

| 关注点 | 落点 |
| --- | --- |
| 迁移与 CHECK | `migrations/0016_policy_composition.sql` |
| 合成规则 | `crates/pm-application/src/policy_composition.rs:64`（`EffectivePolicy::compose`） |
| 能力类 AND | `crates/pm-application/src/policy_composition.rs:126`（`resolve_capability`） |
| 约束类并集 | `crates/pm-application/src/policy_composition.rs:134`（`union_checks`） |
| 内容寻址 hash / version | `crates/pm-application/src/policy_composition.rs:94` / `:109` |
| 四层读取 | `crates/pm-application/src/run_dispatch.rs:1016`（`load_policy_layers`） |
| 单层读取 | `crates/pm-application/src/run_dispatch.rs:962`（`load_scope_layer`）、`:990`（`load_task_layer`） |
| 脏 `additional_checks` 上抛 | `crates/pm-application/src/run_dispatch.rs:1038`（`decode_checks`） |
| 审批证据评估 | `crates/pm-application/src/run_dispatch.rs:1045`（`assess_approval`） |
| 领取判定携带策略结果 | `crates/pm-application/src/run_dispatch.rs:1078`（`ClaimAssessment`）、`:1098`（`claimability_assessment`） |
| 快照写入 | `crates/pm-application/src/run_dispatch.rs:868` |
| 合成单测（6 例） | `crates/pm-application/src/policy_composition.rs:165` 起 |
| 真实 PG 集成测试（3 例） | `crates/pm-application/tests/policy_composition.rs` |
| 迁移测试（5 例） | `crates/pm-infrastructure/tests/migration_0016.rs` |

---

## 六、测试与证据

见文末「验证记录」一节，全部为原始命令输出，未截断改写。

覆盖点：

| 覆盖 | 测试 |
| --- | --- |
| 全允许 / 任一层关闭 / 缺层不收紧 | 合成单测 |
| 审批否决 `auto_accept`、不否决另外两项 | 合成单测 |
| `additional_checks` 并集去重排序 | 合成单测 |
| 同内容不同层拆分 → 同 hash | 合成单测 + 真实 PG（跨 Run） |
| 四层混合落快照、低层不能重开高层关闭 | 真实 PG 集成 |
| 策略变化 → hash/version 变化 | 真实 PG 集成 |
| 迁移可重入、缺省语义、CHECK 拒绝非数组 | 迁移测试 |

### 一个必须说明的既有断言变更

`crates/pm-application/tests/run_dispatch.rs` 的 `policy_snapshot_records_the_minimal_fields` 首跑红：

```
assertion `left == right` failed
  left: String("composed-v1")
 right: String("d1-minimal")
```

这是**有意的业务语义变更**（快照模式从 `d1-minimal` 升级为 `composed-v1`），不是为让实现通过而放宽断言。处理方式：测试改名为 `policy_snapshot_records_the_composed_policy`，并把断言**加强**为校验 `policy_schema`、`effective_policy`、`policy_version` 长度、`policy_hash` 长度。`crates/pm-application/tests/run_context.rs:385` 同步改为 `composed-v1`。

---

## 七、已知边界（明确未越界）

| 边界 | 归属 | 本轮做法 |
| --- | --- | --- |
| 启动/终止执行进程 | D3 | 未触碰 |
| 逐仓 commit 与交付清单落库 | D4 | 未触碰 |
| 失败判定与任务状态裁决 | D5 | 未触碰；`auto_accept` 只落快照，不驱动状态 |
| 执行端能力档位 / 预注册 | 后续 | 未做 |
| 策略 DSL / 外部策略引擎 | — | 不引入；只做 §15.6 的两分运算 |
| 仓库级策略（§15.5 逐仓库保护规则） | D4 及后续 | 未做；`repository_baselines` 仍只是基线快照，不是策略 |
| 策略写入入口（REST/MCP） | 平台管理面 | 未新增；本轮只读策略、写快照 |
| `pm_get_run_context` 契约暴露 `policy_version`/`policy_hash` | 后续 | **未改** D2 契约；版本与 hash 目前只落库 |

---

## 八、口径覆盖关系（谁覆盖谁）

| 文档 | 原口径 | 本轮处置 |
| --- | --- | --- |
| D2 §1.3 第 52 行 | 完整合成「另立阶段」 | **被 D1 §九 覆盖**：已由 D2.1 完成 |
| D2 §6.4 第 354 行 | `policy_version` 仍写 `NULL` | **被覆盖**：已内容寻址 |
| D2 §九 第 435 行 | 只透传 `d1-minimal` | **被覆盖**：已透传 `composed-v1` |
| D2 §4.2 第 169/184 行 | 示例与映射写 `d1-minimal` | 示例更新为 `composed-v1` |
| D1 §九 #1/#2、§十三 #1/#2 | 归属 D2 | 标注「已由 D2.1 完成」 |

---

## 九、验证记录

### 9.1 合成单测（6/6）

命令：`cargo test -p pm-application --lib policy_composition`

```
running 6 tests
test policy_composition::tests::absent_layers_do_not_narrow_capabilities ... ok
test policy_composition::tests::additional_checks_are_unioned_deduplicated_and_sorted ... ok
test policy_composition::tests::human_approval_vetoes_auto_accept_only ... ok
test policy_composition::tests::any_layer_forbids_a_capability ... ok
test policy_composition::tests::content_hash_is_stable_and_moves_with_any_field ... ok
test policy_composition::tests::all_layers_allow_capabilities ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.00s
```

### 9.2 真实 PostgreSQL 集成测试（3/3）

命令：`bash scripts/cargo-test.sh -p pm-application -E 'binary(policy_composition)'`

```
        PASS [   4.093s] (1/3) pm-application::policy_composition four_layer_capability_composition_lands_in_snapshot
        PASS [   4.105s] (2/3) pm-application::policy_composition policy_hash_is_content_addressed_across_runs
        PASS [   4.110s] (3/3) pm-application::policy_composition policy_hash_moves_when_policy_content_changes
────────────
     Summary [   4.110s] 3 tests run: 3 passed, 0 skipped
```

### 9.3 迁移测试（5/5）

命令：`bash scripts/cargo-test.sh -p pm-infrastructure -E 'binary(migration_0016)'`

```
        PASS [   0.168s] (1/5) pm-infrastructure::migration_0016 migration_file_is_present_and_forward_only
        PASS [   3.959s] (2/5) pm-infrastructure::migration_0016 migration_is_reentrant
        PASS [   3.985s] (3/5) pm-infrastructure::migration_0016 policy_scope_gains_the_composition_columns
        PASS [   3.988s] (4/5) pm-infrastructure::migration_0016 defaults_are_null_capabilities_and_empty_checks
        PASS [   3.989s] (5/5) pm-infrastructure::migration_0016 additional_checks_rejects_non_array_values
────────────
     Summary [   3.990s] 5 tests run: 5 passed, 0 skipped
```

### 9.4 D1/D2 回归（33/33）

命令：`bash scripts/cargo-test.sh -p pm-application --no-fail-fast -E 'binary(run_dispatch) | binary(run_context)'`

```
     Summary [  13.132s] 33 tests run: 33 passed, 0 skipped
```

其中含本轮改名的 `policy_snapshot_records_the_composed_policy`（PASS）。

### 9.5 既有断言首跑红（真实原因）

```
    thread 'policy_snapshot_records_the_minimal_fields' (6348) panicked at crates\pm-application\tests\run_dispatch.rs:488:5:
    assertion `left == right` failed
      left: String("composed-v1")
     right: String("d1-minimal")
```

见 §六末段的处理说明。

### 9.6 后端全量测试

命令：`bash scripts/cargo-test.sh --no-fail-fast`

```
FULL_TEST_EXIT=0
     Summary [ 154.947s] 388 tests run: 388 passed (1 slow), 0 skipped
```

### 9.7 clippy（改动 crate，`-D warnings`）

命令：`cargo clippy -p pm-application -p pm-infrastructure --all-targets -- -D warnings`

```
    Checking pm-application v0.1.0 (D:\kudi-project\templates\ai-pm\crates\pm-application)
    Checking pm-infrastructure v0.1.0 (D:\kudi-project\templates\ai-pm\crates\pm-infrastructure)
    Checking pm-ai v0.1.0 (D:\kudi-project\templates\ai-pm\crates\pm-ai)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.02s
```

退出码 0，无 warning。