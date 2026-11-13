# S02-I1 · Requirement Core 详细设计稿 V1.0 · 优先评审

> 评审对象：`docs/scenarios/s02/ui/smark-pm-s02-i1-requirement-core-v1.md`
> 评审范围：按用户指定，仅评审四个影响 S02 后续切片的决策点；不逐字审全文。
> 代码基线：`ai-pm-s01`，`feat/s01-implementation`
> 版本：V1.0（2026-11-04）

---

# §0. 总评

四个问题中，**第 2 项（Confirmation 独立记录）和第 4 项（双重 CAS）方向正确，但当前稿不足以直接实施**。第 1 项需要收窄 `rejected` 的入口；第 3 项存在一个必须先解决的现有 schema 冲突：稿中新增 `lifecycle_status`，却没有处理现有 `delivery.requirement.status`。

| 问题 | 结论 | 级别 |
|---|---|---|
| 1. Revision 状态机 / rejected | **接受为不可复用终态，但只允许从 `pending_confirmation` 进入**；`draft` 的放弃不应复用 `rejected` | P1（语义） |
| 2. Confirmation 独立记录 | **接受独立实体作为正式确认凭据**；但必须同步取消 `approved_by` 的权威地位，并给出可执行的表结构/约束 | **P0** |
| 3. Requirement 生命周期 | **保留 withdrawn / archived 两态**；归档建议允许受控恢复；同时必须解决现有 `status` 与新增 `lifecycle_status` 的双状态冲突 | **P0** |
| 4. 并发控制 | **接受双字段校验**，但必须补 NULL-safe CAS、统一应用服务、`row_revision` 语义和幂等重放定义 | **P0** |

---

# §1. 问题 1：Revision 状态机与 rejected

## 1.1 结论

**`rejected` 应当是终态且不可复用，但只应表示“正式确认请求被人类拒绝”。**

当前稿 §5.2 同时画了两条边：

```text
pending_confirmation -> rejected
draft -> rejected
```

建议删除 `draft -> rejected`。理由：

1. `rejected` 是**对人类正式确认请求的否定结果**，必须绑定提交确认时的精确 Revision/Markdown 版本/hash；draft 尚未进入确认，缺少这些对象。
2. `draft` 的放弃是“起草人不再继续”，不是“人类拒绝了一个正式版本”。两者混在同一状态，审计会失去区分度。
3. 后续 I2 要按状态驱动 Markdown/Git 行为；如果 draft 也可以 rejected，I2 必须判断“这个 rejected 对应哪个 Markdown 版本”，而 draft 可能根本没有提交过版本。

**建议：**

- `rejected` 只能由 `pending_confirmation` 进入。
- `draft` 不设置终态；允许继续编辑，或由后续 I2 定义 `cancelled`/`abandoned` 的独立动作（若确需从工作列表隐藏）。
- 若业务坚持 draft 也要“关闭”，应新增 `cancelled`，不要复用 `rejected`。

## 1.2 不可复用的定义

建议在 §14 不变量中补全：

| 规则 | 说明 |
|---|---|
| 不可修改 | rejected Revision 的正文、hash、版本号不可再编辑 |
| 不可重开 | 不允许 `rejected -> draft` 或 `rejected -> pending_confirmation` |
| 不可复用 version | `(requirement_id, version)` 永不复用；重提必须新建 Revision |
| 不可成为 current | rejected 不能成为 `current_revision_id` |
| 不可作为新 Task 依据 | 新 Task 不得 pin rejected Revision；已存在的历史引用保留可读 |
| 可克隆 | 允许“从 rejected 复制为新 draft”，但新 Revision 使用新 version、新 ID |

## 1.3 当前稿缺失

§5.6 写明“拒绝记录必须保留拒绝人、时间和理由”，但 §11.2 的 Revision 字段清单没有对应字段，也没有独立的拒绝记录实体。需要二选一：

- 在 Revision 上增加 `rejected_by` / `rejected_at` / `rejection_reason`；或
- 明确拒绝信息只落审计事件，并在 §5.6 改成“由审计事件保留”，不得写成 Revision 字段级要求。

---

# §2. 问题 2：Confirmation 独立记录

## 2.1 结论

**接受独立 Confirmation 实体。** `approved_by` 只能证明“谁最后写了 approved”，不能证明：

- 确认的是哪一个 Markdown 版本；
- 确认的是哪一个 `content_hash`；
- 确认时的可信身份上下文；
- 确认是否被重放；
- 确认与 Revision / Project 的完整绑定。

因此，Confirmation 必须是独立、不可变、可查询的正式凭据。

## 2.2 但必须同时处理 `approved_by`

当前 `approved_by` 的消费者已经明确存在：

| 类型 | 落点 |
|---|---|
| 读 | `apps/pm-api/src/pm_artifacts.rs:50`（产物列表回显） |
| 读 | `apps/pm-api/src/pm_lists.rs:511`（修订列表回显） |
| 读 | `crates/pm-application/src/rows.rs:144`（Row 类型） |
| 读 | `frontend/src/api/artifacts.ts:17`、`frontend/src/api/types.ts:125` |
| 写 | `crates/pm-application/src/pm_write/revision.rs:98`（REST 定稿） |
| 写 | `crates/pm-application/src/command_executor/executor.rs:901`（执行器定稿） |

**建议：**

1. **不要保留 `approved_by` 作为第二套权威**。`AGENTS.md` 规则 1 已明确无存量数据，最干净的做法是直接删列，不做 backfill。
2. 读侧 API 由 Confirmation 提供 `confirmed_by` / `confirmed_at`；如要减少前端改动，可在 API 响应中保留 `approved_by` 作为**派生字段名**，但其值必须来自 Confirmation，不得再落库。
3. 写侧两处（REST / Executor）必须收敛到同一个确认应用服务。仅删除 `approved_by` 写入不够，必须同时删除两条独立的批准路径。
4. 若暂不删列，必须在稿中明确：`approved_by` 是派生投影，不是权威；但这是下策，后续极易复发双写。

## 2.3 Confirmation 表结构必须在本稿定到可实施程度

当前稿 §11.3 只给“建议逻辑字段”，并把表名/唯一约束推给 I6-B 与 I6-A“最终确定”。这不足以让 I6-A/I6-B 并行开工。至少应冻结以下约束：

| 项 | 建议 |
|---|---|
| 表名 | `delivery.requirement_confirmation`（或明确由 I6-A 命名，但 I1 必须给出契约） |
| 业务唯一键 | `UNIQUE (revision_id)`：一条 Revision 最多一条有效正式确认 |
| 复合外键 | `(project_id, requirement_id, revision_id)` 指向 Revision；需要 Revision 上补 `UNIQUE (project_id, requirement_id, id)` 或等效唯一索引 |
| 人类主体 | `human_principal_id` FK `core.principal`，`identity_id` FK `principal_identity` |
| 内容绑定 | `markdown_version_ref`、`content_hash` 必须非空，写入时与 Revision 当前值一致 |
| 幂等 | 增加 `operation_id`（可选但建议必填），用于识别同一确认请求重放 |
| 不可变 | 禁止 UPDATE/DELETE；若使用数据库触发器，需明确触发时机与错误码 |
| 历史性 | Revision 转 `superseded` 后，历史 Confirmation 仍然有效，不因 Approver 后续被撤销而失效 |

## 2.4 `approved` 与 Confirmation 的数据库一致性

稿中写 `approved` 表示“存在有效人类正式确认”，但 PostgreSQL 的 CHECK 不能跨表校验。必须明确采用以下之一：

- **应用服务单事务保证**：插入 Confirmation 后，才允许 `UPDATE revision SET state='approved'`；所有入口共用同一服务。测试覆盖“无 Confirmation 时直接 UPDATE 被拒”。
- **数据库触发器**：在 Revision 表 `BEFORE INSERT/UPDATE` 校验 `NEW.state='approved'` 时必须存在匹配 Confirmation；在 Confirmation 表 `BEFORE DELETE` 阻止删除已被 approved Revision 引用的记录。

若两者都只写“应保证”，则 RI-10 无法验证。建议至少采用应用服务 + 数据库触发器双保险；若 V1 不引入触发器，必须在验收中明确“直接 SQL 绕过”属于已知缺口，不得声称 RI-10 已满足。

---

# §3. 问题 3：Requirement 生命周期

## 3.1 结论

**保留 `withdrawn` 与 `archived` 两态，但必须给 `archived` 定义受控恢复路径。**

两态语义建议：

| 状态 | 语义 | 新 Revision | 新 Task | 默认列表 | 恢复 |
|---|---|---|---|---|---|
| `active` | 正常需求 | 允许 | 允许 | 显示 | — |
| `withdrawn` | 人类决定不再推进，但保留可恢复性 | 禁止 | 禁止（已有 Task 可继续完成） | 默认隐藏，可筛选 | 允许恢复 |
| `archived` | 退出工作集，进入冷态历史 | 禁止 | 禁止 | 默认隐藏 | 建议允许 Project Lead 显式恢复并留审计；若 V1 不允许，必须二次确认归档 |

不建议把 `withdrawn` 与 `archived` 合并：二者回答的是不同问题——“还要不要做”与“还要不要在活跃工作集里显示”。合并后无法区分“暂停但可能恢复”与“历史归档”。

## 3.2 P0：现有 `status` 与新增 `lifecycle_status` 冲突

这是本稿目前最具体的 schema 风险：

- `delivery.requirement.status` 当前默认 `'draft'`，定稿时被写成 `'approved'`：`migrations/0002_delivery.sql:3-14`、`crates/pm-application/src/pm_write/revision.rs:113`、`crates/pm-application/src/command_executor/executor.rs:909`。
- 该列被需求列表直接读取：`apps/pm-api/src/pm_lists.rs:254`。
- 本稿 §11.1 又新增 `lifecycle_status`（`active/withdrawn/archived`），但没有说明旧 `status` 的去留。

如果直接新增 `lifecycle_status`，同一张表会出现两个“状态”：

```text
status = 'approved'
lifecycle_status = 'active'
```

这会让“需求是否有效”与“是否存在正式版本”混在一起，且现有 `pm_lists` 仍读旧列。

**建议：**

1. 将现有 `status` **重命名/替换**为 `lifecycle_status`，取值 `active/withdrawn/archived`；现有 `approved` 语义改由 `current_revision_id` 表达。
2. 删除定稿路径中对 `delivery.requirement.status='approved'` 的写入（`pm_write/revision.rs:113`、`executor.rs:909`）。
3. 同步修改 `pm_lists.rs:254`、种子数据与测试断言。
4. 增加 CHECK：`lifecycle_status IN ('active','withdrawn','archived')`，并补状态迁移守卫。

## 3.3 归档守卫

建议在 §14 增加：

- 只有 `Project Lead` 或显式授权的人类可 withdraw/archive/restore。
- withdraw/archive 必须 bump `row_revision`，否则并发确认可能绕过生命周期状态。
- 确认时必须校验 `lifecycle_status='active'`。
- `archived` 状态默认只读；历史 Revision、Task/Run/Delivery 引用继续可追溯。
- 若归档时尚有非终态 Task 关联该 Requirement，建议返回 409，除非人类显式强制归档并留 reason。

---

# §4. 问题 4：并发控制

## 4.1 结论

**接受 `expected_current_revision_id + expected_row_revision` 双重校验，但当前稿还不足以关闭 DEF-03。**

理由：双字段只能提供“预期值”，真正的正确性来自**同一事务内的原子 CAS + 所有入口共用同一服务**。

## 4.2 必须补的四点

### (1) NULL-safe 比较

首次确认时 `current_revision_id` 为 NULL。如果 SQL 写成：

```sql
WHERE current_revision_id = $expected
```

当两边都是 NULL 时结果为 UNKNOWN，UPDATE 影响 0 行。必须使用：

```sql
WHERE current_revision_id IS NOT DISTINCT FROM $expected
```

这是最容易在实施时踩中的语义陷阱。

### (2) 原子 CAS 语句

建议把确认收敛为一条带条件的 UPDATE：

```sql
UPDATE delivery.requirement
SET current_revision_id = $new_revision_id,
    row_revision = row_revision + 1
WHERE id = $requirement_id
  AND current_revision_id IS NOT DISTINCT FROM $expected_current_revision_id
  AND row_revision = $expected_row_revision
RETURNING row_revision;
```

若返回 0 行 → 版本冲突。目标 Revision 状态另用：

```sql
UPDATE delivery.requirement_revision
SET state = 'approved'
WHERE id = $revision_id
  AND requirement_id = $requirement_id
  AND state = 'pending_confirmation';
```

若返回 0 行 → 状态冲突。两处必须同一事务。

### (3) `row_revision` 的 bump 语义

当前稿说 `expected_row_revision` 防止“Requirement 元数据并发覆盖”，但没有定义哪些操作会 bump。必须冻结：

- `delivery.requirement` 行的任何变更都 bump：`current_revision_id`、`lifecycle_status`、`title`、`code`、`type`。
- 仅插入 Revision 行不 bump Requirement 行。
- 确认事务同时更新 current 指针与 row_revision。

否则出现 `withdraw` 与 `confirm` 并发时，确认可能成功，生命周期被绕过。

### (4) 唯一应用服务

`S02-02` 的 DEF-03 根因不只是缺 CAS，而是 REST 与 Executor 两套路径各自实现。必须：

- `ConfirmRequirementRevision` 只存在于统一应用服务；
- REST/MCP/Executor 仅做传输适配；
- 回归测试同时覆盖两个入口的并发确认；
- 第二次确认必须是 409（或同一 `operation_id` 的幂等 200），不能两个 200。

## 4.3 幂等重放

稿中 §7.6 把重放语义推给 I6-B。但 Confirmation 有 `UNIQUE(revision_id)`，如果同一请求重放，第二次会撞唯一约束。必须提前定义：

| 场景 | 期望 |
|---|---|
| 同一 `operation_id`、同一 revision、同一 hash | 200，返回原 Confirmation，不新增审计/事件 |
| 不同 `operation_id`、同一已 approved revision | 409 |
| 同一 revisio
n、不同 hash | 409 |
| 不同 revision | 各自独立 CAS，后到者按 current/row 冲突 |

## 4.4 版本号分配的并发安全

当前 REST 创建修订使用 `SELECT COALESCE(max(version),0)+1` 且没有行锁：`crates/pm-application/src/pm_write/revision.rs:27`。两个并发 create_revision 可能算出同一 version，撞 `UNIQUE(requirement_id, version)`，表现为 500 或非预期冲突。

建议：

- 创建修订前对 Requirement 行加锁；或
- 用 `INSERT ... SELECT max+1` 后失败重试；或
- 使用独立的 per-requirement sequence。

由于 `rejected` 版本“永不复用”的语义依赖版本分配，这一点必须写进 I1，而不是留到实施时发现。

---

# §5. 额外 P0：迁移号已过期

本稿 §11.5、§16.1 仍写“S02 新 migration 从 0034 开始”，但当前分支实际已有：

- `migrations/0034_s01_command_center_indexes.sql`（已入库）
- `migrations/0035_s01_run_cleanup.sql`（工作区未提交）

因此 **S02 新迁移应从 `0036+` 起**。这是实施前必须修正的口径，否则迁移登记与实际分支冲突。

---

# §6. 建议的修改顺序

1. 先修 §3（`status` vs `lifecycle_status`）——这是 schema 级冲突，影响最大。
2. 再修 §2（Confirmation 表结构 + `approved_by` 退出方案）——直接影响 I6-A/I6-B。
3. 再修 §4（NULL-safe CAS、单服务、row_revision bump、幂等、版本分配）——直接回应 DEF-03。
4. 最后收 §1（rejected 入口收窄 + 拒绝字段）。
5. 迁移号统一改为 `0036+`。

---

# §7. 评审结论

**本稿的领域骨架成立，不能直接进入实施。**

四项中：

- **第 2、4 项方向正确但需要补硬约束**；
- **第 3 项必须先解决现有 schema 的双状态冲突**；
- **第 1 项需要收窄 `rejected` 语义**。

最低放行条件：P0 项全部在 V1.1 修正，且 §14 的每条不变量都有对应的数据库约束或可执行测试映射。
