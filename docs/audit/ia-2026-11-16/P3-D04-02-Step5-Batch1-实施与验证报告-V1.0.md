# P3-D04-02 Step 5 Batch 1 实施与验证报告 V1.0

- 编制日期：2026-11-18（实施跨 2026-11-17~18）
- 性质：**实施与验证报告**——Batch 1 全部实施内容、实测结果与发现项登记
- Git 基点：`7fa64dd`（批准令正式登记 + 任务书 V1.0~V1.2 入库，工作树干净）
- 授权依据：①《Step 3 设计冻结批准令 V1.0》（2026-11-17 负责人签发，commit 7fa64dd 入库）②负责人正式实施指令（2026-11-17，两门齐备后下达）
- 任务书基线：**V1.2 + V1.3 定向修订**（修订记录见 §〇，2026-11-17 负责人批准）
- 实施范围遵守：仅 GAP-04 + GAP-37-A/B + Migration 0029/0030 + T-1~T-5；零行为变化、纯增量、无数据回填；未触碰 GAP-37-C、未跨批、未自行创建 GAP

---

## 〇、任务书 V1.3 定向修订记录（2026-11-17 负责人批准）

**触发**：实施前置检查 §五.4 快照重导发现基线表与实际结构不一致（实施指令第 7 条停止-报告流程；证据三重：迁移链文本、dev 库实测、aipm_mig_full 实测三方一致）。

| # | 修订项 | V1.2 原文 | V1.3 修订 | 证据 |
|---|---|---|---|---|
| V3-1 | §4.5 镜像清单漏列 | A 组 7 行，无 cognition_version | **A 组补 1 行**：cognition_version / UPDATE / BEFORE+ROW / forbid_version_mutation / pc=trg_pc_version_immutable（0001:7064 段）/ projc=trg_projc_cognition_version_immutable（0025:435）——**两侧既有，非本批新增**，属漏列非漂移 | 活库 2026-11-17 实测 pc=17/projc=10（三重：pg_trigger 计数、pg_get_triggerdef 全量导出、迁移链文本推演一致） |
| V3-2 | §4.3 计数勘误 | 「实施后 projc 触发器 11→13」 | **实际 10→12**（0025 建 8 − 0026 drop 1 + 0026 建 3 = 10；本批 +2） | 同上三重证据 |

**对实施的影响**：T-2 按修订后清单做全量反向枚举断言（防漂移规则不弱化）；其余范围零变化。

## 一、实施前置检查（§五，2026-11-17）

| # | 检查项 | 结果 |
|---|---|---|
| 1 | GAP-04 重复预检复跑 | **PASS**：dev 库 aipm dup=0/0（空库，见 E-1）；aipm_mig_full dup=0/0（非空证据：projc 2 行 0 重复） |
| 2 | 迁移编号占用 | **PASS**：0029/0030 空闲（migrations/ 止于 0028） |
| 3 | Git 门 | **PASS**：HEAD=origin/master=7fa64dd；工作树干净；批准令已入库 |
| 4 | §4.5 清单快照重导核对 | **发现→已处置**：漏列 cognition_version 镜像对（V3-1）＋计数失真（V3-2）→ 停止-报告 → 负责人批准 V1.3 修订后继续 |
| R-1 | NOT NULL 双证（V1.1） | **PASS**：0001/0025 建表文本 + 活库 information_schema `nullable=NO` ×4 |
| E-1 | 环境登记 | dev 库 aipm 为空壳（无 pc/projc schema；容器 aipm-dev-postgres 在位）→ 按既定重建流程恢复 0001→0028 全链（环境操作，非本批实施），重建后复验全过 |

## 二、实施内容（零偏离任务书 §3）

### 2.1 Migration 0029（GAP-04）`migrations/0029_cognition_revision_no_unique.sql`

- SQL 与任务书 §3.1 逐字一致（仅注释块增记实施日复跑结果一行）；两域 `ADD CONSTRAINT cognition_revision_object_id_revision_no_key UNIQUE (object_id, revision_no)`。
- 约束名两域均无既有冲突（既有 5 约束清单核实，0029 约束名列于其外）。

### 2.2 Migration 0030（GAP-37-A/B）`migrations/0030_projc_mirror_guards.sql`

- SQL 与任务书 §3.2 逐字一致；零新函数（projc.forbid_row_update 在位 0025:329-334）。
- 新增 `trg_projc_revision_immutable` / `trg_projc_vm_immutable`，均为 BEFORE DELETE OR UPDATE、FOR EACH ROW、projc.forbid_row_update。
- 既有挂载零调整（trg_projc_vm_closure 等 10 个触发器未动）。

### 2.3 应用记录

| 目标库 | 方式 | 结果 |
|---|---|---|
| sqlx::test 临时库（每测试一个，全新） | sqlx 按序 0001~0030 全序应用 | 全部成功（测试即证据） |
| dev 库 aipm（127.0.0.1:16433 容器） | docker exec psql 逐文件独立事务 | 0029 OK / 0030 OK；目录断言过 |

## 三、测试面与实际结果

### 3.1 新增测试清单

`crates/pm-application/tests/projc_mirror_guards.rs`（新增文件，3 用例）：

| # | 用例 | 覆盖 |
|---|---|---|
| T-1 | `t1_gap37_update_delete_probes` | GAP-37 验收①：A=cognition_revision、B=version_member 各自独立：UPDATE 普通字段/DELETE 均 4 次探针被拒，异常文本逐条含 `in projc.<table> is immutable`（=forbid_row_update RAISE 语义实测）；INSERT 成功断言随附（T-5）；全部探针显式事务 ROLLBACK＋收尾零残留断言 |
| T-2 | `t2_mirror_guard_inventory` | GAP-37 验收②+D-4 追加维度：pc 17 / projc 12（含本批 +2）触发器 pg_get_triggerdef 规范串逐字比对（六维：名/目标表/事件/时机+CONSTRAINT+DEFERRABLE/行级/执行函数）＋**完备性反向枚举**（两侧总数与清单集合完全相等，清单外新增/删减即失败）。基准表=§4.5 V1.3（A/B/C/D 分组注记内嵌代码注释；[V1.3] 行显式标记）。R-5 边界：结构一致性核对，不重审既有函数业务语义；B 组分歧按注记断言差异本身 |
| T-3 | `t3_gap04_revision_no_unique_probe` | GAP-04：①两域目录断言 pg_get_constraintdef=逐字 `UNIQUE (object_id, revision_no)`；②pc 同域重复 INSERT 拒（跨域因 (tenant,project,object) 复合外键不可构造，注记内嵌）；③projc 同域重复 INSERT 拒且**约束名精确命中新约束**（0025 建表零唯一约束→拒绝者唯一）；④projc 跨 tenant/project 重复 INSERT 拒且命中新约束（作用域=精确 (object_id, revision_no) 的直接证据，任何既有约束均不覆盖） |

### 3.2 实际执行结果（非代码检查，真实运行）

**T-1~T-3**（`cargo test -p pm-application --test projc_mirror_guards`，RUSTC_WRAPPER=""，DATABASE_URL=127.0.0.1:16433）：

```
running 3 tests
test t2_mirror_guard_inventory ... ok
test t3_gap04_revision_no_unique_probe ... ok
test t1_gap37_update_delete_probes ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.78s
```

### 3.3 T-4 全量回归（既有套件，临时库全序应用 0001~0030）

| 套件 | 结果 |
|---|---|
| `projc_formal_chain`（提案/审批/发布/关系/冲突全链） | **ok. 6 passed; 0 failed**（8.11s） |
| `pc_model`（PC 正式模型） | **ok. 6 passed; 0 failed**（8.21s） |
| `pc_proposal`（PC 提案链） | **ok. 8 passed; 0 failed**（8.67s） |

T-4 合计 **20/20 通过**——发布链（INSERT version_member + revision）在新增守卫下零影响。

### 3.4 T-5 追加维度（融合）

- INSERT 与发布流程不受影响：T-1 内嵌 INSERT 成功断言 + T-4 发布链全量回归承载 ✔
- 无绕过路径：测试边界说明——服务层全库 UPDATE/DELETE 这两表零命中（总账 V1.3 GAP-37 行既有证据维持）；DB 层守卫对任意会话/角色无条件拒绝（forbid_row_update 无条件 RAISE，探针实测）✔
- PC/ProjC 保护语义一致：T-2 承载（结构逐字一致）+ T-1 承载（projc 实测拒绝文本与 pc 真源同款 RAISE 语义，dev 库 pc 侧探针同文本验证）✔

## 四、两域数据库验证（dev 库 aipm，2026-11-18）

目录断言（应用后实测）：

```
constraint|pc|cognition_revision_object_id_revision_no_key|UNIQUE (object_id, revision_no)
constraint|projc|cognition_revision_object_id_revision_no_key|UNIQUE (object_id, revision_no)
trigger|projc|trg_projc_revision_immutable|CREATE TRIGGER trg_projc_revision_immutable BEFORE DELETE OR UPDATE ON projc.cognition_revision FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update()
trigger|projc|trg_projc_vm_immutable|CREATE TRIGGER trg_projc_vm_immutable BEFORE DELETE OR UPDATE ON projc.version_member FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update()
trg|pc|17 ；trg|projc|12（=10+2，与 V1.3 基准一致）
```

活体探针（每探针独立事务，全部 ROLLBACK）：

| 探针 | 实测结果 |
|---|---|
| P1 UPDATE projc.cognition_revision | **拒绝**：`row … in projc.cognition_revision is immutable`（projc.forbid_row_update） |
| P2 DELETE projc.cognition_revision | **拒绝**：同上 |
| P3 projc 同 (object_id, revision_no) 二次 INSERT | **拒绝**：`duplicate key value violates unique constraint "cognition_revision_object_id_revision_no_key"` |
| P4 UPDATE projc.version_member | **拒绝**：`in projc.version_member is immutable` |
| P5 DELETE projc.version_member | **拒绝**：同上 |
| C1 UPDATE pc.cognition_revision | **拒绝**：`in pc.cognition_revision is immutable`（真源同款语义） |
| C2 DELETE pc.cognition_revision | **拒绝**：同上 |
| C3 pc 同域重复 INSERT | **拒绝**：命中既有 `cognition_revision_tenant_project_object_no_key`（旧索引先查属预期；新约束在位由目录断言与 T-3 测试承载——与任务书 §3.3 T-3 设计注记一致） |

探针零残留：两域 cognition_revision / version_member / 探针种子行数全 0。

## 五、测试临时库清理

`_sqlx_test%` 泄漏检查：**0 个泄漏库**（sqlx::test 正常收尾自动清理；收尾实测 `pg_database WHERE datname LIKE '_sqlx_test%'` = 空）。dev 库探针种子经 ROLLBACK 零残留（§四）。

## 六、§4.4 新发现项处置状态记录

**trg_pc_version_delete（BEFORE DELETE ON pc.cognition_version）projc 缺位**：本批**未实施、未扩批、未创建 Migration**（遵守任务书 R-7）；事实与 D 组例外登记维持（T-2 清单 [D] 组注记内嵌）；**待总账维护者核对是否已有对应 GAP → 负责人决定是否设立 GAP-37-C**。裁决前清单测试将持续断言该差异（pc 有/projc 无）防漂移。

## 七、批外新发现登记（未实施、未扩批，移交总账维护者）

| # | 事实 | 影响 | 建议 |
|---|---|---|---|
| N-1 | **projc 全域 17 表 0 主键、0 外键**（pg_constraint 实测；pc 侧 79 FK+全 PK，0001 ALTER TABLE 段声明）；projc 仅 10 个部分唯一索引+0026/0028 两约束。0025 建表文本与总账均未登记该差异 | 服务层零 UPDATE/DELETE 现状下无直接覆盖风险（同 GAP-37 论证）；DB 层身份/引用兜底缺位属防御纵深议题 | 总账维护者核对既有 GAP；若无需负责人决定是否设立（同 §4.4 流程） |
| N-2 | Step3《Schema-Dependency-Analysis V1.0》「pc/projc 全 schema 外键=0（0001 grep REFERENCES=0）」**对 pc 不成立**（pc 实有 79 FK，位于 0001 ALTER TABLE 段 5300+ 行；取证 grep 误限于 CREATE TABLE 段 3021-3470） | 仅涉锁分析取证口径（FK 级联锁实际在位），不影响 Step3 冻结结论的约束/触发器面 | 文档勘误（总账维护者登记即可） |
| N-3 | 同文 §214 活库快照「triggers pc=22/projc=12」与 2026-11-17 当日实测（dev 库、aipm_mig_full、迁移链三方一致 pc=17/projc=10）不符 | 历史文档快照失真（不影响本批：本批以当日实测为据） | 文档勘误（同上） |
| E-1 | dev 库 aipm 空壳重建（0001→0028 恢复） | 环境操作，非本批实施 | 已登记 §一 |

## 八、回滚就绪声明（§七，未触发）

- 回滚前对象检查基线已留存：两域 `cognition_revision_object_id_revision_no_key` 的 pg_constraint 存在且定义=逐字；两触发器 pg_get_triggerdef 与 §3.2 逐字一致（§四 目录断言实测）。
- 本批未发生迁移失败/测试失败，未触发 §7.1/§7.2/§7.3 流程。

## 九、交付物与 Git 状态

| 交付物 | 位置 |
|---|---|
| Migration 0029/0030 | `migrations/0029_cognition_revision_no_unique.sql`、`migrations/0030_projc_mirror_guards.sql` |
| 新增测试 | `crates/pm-application/tests/projc_mirror_guards.rs`（T-1/T-2/T-3） |
| 本报告 | `docs/audit/ia-2026-11-16/P3-D04-02-Step5-Batch1-实施与验证报告-V1.0.md`（含 V1.3 修订记录） |

Git：commit & push 至 origin/master（实施 commit 与报告同件归档）；commit 后 HEAD=origin/master 同步。执行纪律八项自检：127.0.0.1 连接 ✔／RUSTC_WRAPPER="" ✔／纯增量无回填 ✔／泄漏检查 0 ✔／编号顺延 ✔／不跨批 ✔／GAP-37-C 未触碰 ✔／不自行宣告 GAP 状态变化（总账 V1.3 GAP-04/GAP-37 状态行的更新由总账维护者落账）✔。

## 十、停止点

Batch 1 实施与验证完成，**停止等验收**。Batch 2（账本批）未开始，等负责人指令。
