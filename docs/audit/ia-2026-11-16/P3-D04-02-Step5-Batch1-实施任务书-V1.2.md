# P3-D04-02 Step 5 Batch 1 实施任务书 V1.2

- 编制日期：2026-11-17
- 性质：**实施任务书（草案）**——V1.1 技术审阅通过后按两处细节意见修订；**本任务书获批≠实施授权**
- 版本：V1.0→V1.1（三项修订）→**V1.2（两处细节+§4.4 处置登记）**；V1.0/V1.1 保留为历史
- Git 基点：`ca198c8`
- ⚠ **执行门（不变）**：①《Step 3 设计冻结批准令》正式签发并 commit 入库归档 ②负责人明确下达 Batch 1 实施授权——两门齐备方可动工；不得提前创建 Migration、修改测试或代码。

---

## 〇、版本修订记录

**V1.0→V1.1**（负责人审阅意见三项）：R-1=GAP-04 NOT NULL 双证核实（约束范围不变）；R-2=镜像清单测试完善（六维+closure/immutable 分类+预期清单 A~D 组）；R-3=回滚三段纪律化；R-4=§4.4 新发现登记（trg_pc_version_delete projc 缺位）。

**V1.1→V1.2**（技术审阅通过后两处细节+处置登记）：

| # | 修订 | 内容 |
|---|---|---|
| R-5 | T-2 测试边界提醒 | 清单测试避免把「触发器存在」误当成「保护语义完全一致」——不同函数≠语义等价；本批重点=新增的 revision/version_member 保护面与 pc 真源一致；其余既有保护面按清单验证**结构**，不扩大为对所有函数业务语义的重新审计（§3.3） |
| R-6 | §7.3 第 4 项状态修正 | 回滚≠回到「未授权原点」——实施状态标记为失败并完成回滚；保留原实施授权及执行审计记录；是否重新实施由负责人重新决定，不自动恢复实施资格（§7.3） |
| R-7 | §4.4 处置登记 | 审阅确认：暂不扩 Batch 1；保留事实与例外登记；由总账维护者核对是否已有对应 GAP；若无需负责人决定是否创建 GAP-37-C；**裁决前不增加 Migration、不改变本批验收范围**（§4.4） |

## 一、任务目标（不变）

Batch 1 = **DB 保护兜底批**：正式事实 DB 层防御纵深补齐；纯增量、零行为变化。

| GAP | 内容 | 总账 V1.3 状态锚 |
|---|---|---|
| GAP-04 | 两域 cognition_revision 补 (object_id, revision_no) UNIQUE | 活跃实施差距（C 级） |
| GAP-37-A | projc.cognition_revision 补 UPDATE/DELETE immutable | 已设立〔D-4〕未授权 |
| GAP-37-B | projc.version_member 补 UPDATE/DELETE immutable | 同上 |

**不在本批**：其他任何 GAP；服务层/行为变更；数据回填；ai/kb 域；§4.4 新发现项（待总账核对+负责人裁决）。

## 二、权威输入与验收锚（不变）

总账 V1.3 GAP-04/GAP-37 行（D-4 验收三类+追加维度）；设计方案 §D5；裁决归档记录 V1.0（D-4）；pc 镜像真源 DDL；AGENTS.md 全部规则。

## 三、实施范围（Migration 与测试面草案——获批后执行）

### 3.1 迁移 0029：GAP-04（`migrations/0029_cognition_revision_no_unique.sql`）

前置核实（V1.1 R-1 双证）：两域 object_id/revision_no 均 NOT NULL（0001/0025 建表文本＋活库 information_schema `nullable=NO` ×4）→ 约束范围维持不变，无需 NOT NULL 增补；预检重复分组=0/0（两度只读预检，实施日复跑）。

```sql
-- 0029_cognition_revision_no_unique.sql
--
-- P3-D04-02 Step5 Batch1 GAP-04：两域 cognition_revision 补 (object_id, revision_no)
-- 唯一约束（DB 层 revision_no 兜底；行锁推号在位不变）。
-- 同款先例：ai/kb document_revision、kb.memory_revision
--   （0001:3722/5210 UNIQUE(document_id, revision_no) 同构）。
-- NOT NULL 核实（V1.1）：两域 object_id/revision_no 均 NOT NULL
--   （0001/0025 建表文本＋活库 information_schema 双证）——无需增补列约束。
-- 迁移前检查：两库两域 (object_id,revision_no) 重复分组=0
--   （2026-11-17 两度只读预检；实施日须复跑，非 0 即停止并报告）。
-- AGENTS.md 规则 1：无数据迁移、无回填、无兼容分支。

ALTER TABLE pc.cognition_revision
    ADD CONSTRAINT cognition_revision_object_id_revision_no_key
    UNIQUE (object_id, revision_no);

ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_object_id_revision_no_key
    UNIQUE (object_id, revision_no);
```

### 3.2 迁移 0030：GAP-37-A/B（`migrations/0030_projc_mirror_guards.sql`）

```sql
-- 0030_projc_mirror_guards.sql
--
-- P3-D04-02 Step5 Batch1 GAP-37-A/B：ProjC 镜像保护面补齐（D-4 裁决）。
-- 与 pc 真源逐字同款（0001:7043 trg_pc_revision_immutable /
--   0001:7071 trg_pc_vm_immutable 同构，域内改前缀）。
-- 函数 projc.forbid_row_update 已在位（0025:329-334，无条件 RAISE→
--   BEFORE DELETE OR UPDATE 下 UPDATE/DELETE 双面拒绝）；本迁移零新函数。
-- 实施设计核实记录：函数实际行为/触发时机/异常语义 2026-11-17 只读核实
--   （《Step5 批次规划 V1.0》§五.1）；实测探针为 §六 验收项。
-- AGENTS.md 规则 1：纯增量，无数据迁移。

CREATE TRIGGER trg_projc_revision_immutable
    BEFORE DELETE OR UPDATE ON projc.cognition_revision
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();

CREATE TRIGGER trg_projc_vm_immutable
    BEFORE DELETE OR UPDATE ON projc.version_member
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();
```

### 3.3 测试面

| # | 测试 | 性质 | 验收锚 |
|---|---|---|---|
| T-1 | GAP-37 探针：UPDATE projc.cognition_revision（普通字段）/DELETE 同表 → 均被 DB 拒；UPDATE/DELETE projc.version_member → 均被拒（A/B 各自独立断言；事务内 ROLLBACK 零残留） | 新增用例 | 总账 GAP-37 验收① |
| T-2 | 镜像保护清单测试（V1.1 六维规格＋**V1.2 边界提醒**，基准表=§四.5） | 新增用例（建议独立 tests/projc_mirror_guards.rs） | 总账 GAP-37 验收②+D-4 追加维度 |
| T-3 | GAP-04 探针：同 (object_id, revision_no) 二次 INSERT 被拒（两域各一） | 新增用例 | 总账 GAP-04 |
| T-4 | 回归：projc_formal_chain 全量+pc_model+pc_proposal 全量；sqlx::test 全新临时库 0001~0030 全序应用 | 既有回归 | D-4 验收③+迁移全序 |
| T-5 | 追加维度：INSERT 与发布流程不受影响（T-4 覆盖）；无绕过路径（测试边界说明）；PC/ProjC 保护语义一致（T-2 承载） | 融合 | D-4 追加维度 |

**T-2 断言维度（六维）**：①触发器名 ②目标表（schema.table）③事件（INSERT/UPDATE/DELETE 组合）④触发时机（BEFORE/AFTER＋CONSTRAINT＋DEFERRABLE INITIALLY）⑤行级属性（FOR EACH ROW）⑥执行函数（schema.function 全名）。实现可对 pg_trigger＋pg_get_triggerdef/pg_proc 联查整串或逐字段断言。

**closure/immutable 分类区分**：immutable 类=BEFORE DELETE/UPDATE、非约束触发器、FOR EACH ROW、拒绝型函数（=本批保护面）；closure 类=CONSTRAINT TRIGGER、AFTER、DEFERRABLE（一致性校验面，登记不测拒绝行为）——两类分列，禁止混同。

**T-2 边界提醒（V1.2 R-5，测试实现时遵守）**：
1. **不同函数≠语义等价**——六维核对的是结构一致性；函数同名仅保证同款实现起点，不自动证明业务语义在两域完全等价；
2. **本批语义验证重点=新增保护面**：revision（cognition_revision）与 version_member 的 immutable 保护面须与 pc 真源语义一致（T-1 探针+T-4 回归承载实测）；
3. **其余既有保护面按清单验证结构即可**（A/C 组存在性与六维属性），**不因清单测试扩大为对所有触发器函数业务语义的重新审计**——既有函数语义已被各自验收与既有测试覆盖；
4. 函数有意分歧处（B 组 document_revision）按注记断言差异本身，不作语义裁断。

## 四、GAP-37 目标表、挂载现状与新增内容

### 4.1 目标表 A：projc.cognition_revision
建表 0025:112-129；现有挂载=零；pc 真源=trg_pc_revision_immutable（0001:7043）；本次新增 trg_projc_revision_immutable，无调整。

### 4.2 目标表 B：projc.version_member
建表 0025:148-162；现有挂载=仅 trg_projc_vm_closure（0026:203，CONSTRAINT/AFTER INSERT/DEFERRABLE INITIALLY DEFERRED，W1 批准项，本批不动）；pc 真源=trg_pc_vm_closure＋trg_pc_vm_immutable（0001:7064/7071）；本次新增 trg_projc_vm_immutable，既有 closure 零调整。

### 4.3 新增/调整汇总（不变）
两触发器新增＋0029 两约束；函数零新增；实施后 projc 触发器 11→13；差余量=D-3 relationship 四件套（未来强制前置）＋§4.4 待裁决项。

### 4.4 新发现登记与处置（V1.2 R-7：审阅处置确认）

**事实**：`trg_pc_version_delete`（BEFORE DELETE ON pc.cognition_version FOR EACH ROW→pc.forbid_row_update，0001:7050）在 projc 无对应物——已发布版本行 DELETE 守卫 projc 缺位；与 GAP-37 同族；projc 现状纯 INSERT（publish.rs:473/487）无违规，属防御纵深缺位。

**审阅确认处置（2026-11-17）**：
1. **暂不扩 Batch 1**（第一批范围保持不变）；
2. 保留本节事实与 §四.5 D 组例外登记；
3. **由总账维护者单独核对是否已有相应 GAP**；若无需**负责人决定**是否创建 GAP-37-C；
4. **裁决前：不增加 Migration、不改变本批验收范围**——「发现问题≠获得实施授权」原则适用。

### 4.5 镜像保护清单预期表（T-2 断言基准；Batch 1 完成后目标状态）

**A 组·两域必须一致（immutable/identity 保护面）**：

| 目标表 | 事件 | 时机/属性 | 函数 | pc | projc |
|---|---|---|---|---|---|
| approval | UPDATE | BEFORE/ROW | forbid_approval_mutation | trg_pc_approval_immutable | trg_projc_approval_immutable |
| cognition_object | DELETE | BEFORE/ROW | forbid_identity_delete | trg_pc_object_delete | trg_projc_object_delete |
| cognition_object | UPDATE | BEFORE/ROW | enforce_object_lifecycle | trg_pc_object_lifecycle | trg_projc_object_lifecycle |
| cognition_revision | DELETE,UPDATE | BEFORE/ROW | forbid_row_update | trg_pc_revision_immutable | **trg_projc_revision_immutable（本批新增）** |
| version_member | DELETE,UPDATE | BEFORE/ROW | forbid_row_update | trg_pc_vm_immutable | **trg_projc_vm_immutable（本批新增）** |
| proposal | UPDATE | BEFORE/ROW | forbid_proposal_content_mutation | trg_pc_proposal_anchors | trg_projc_proposal_anchors |
| write_operation | UPDATE | BEFORE/ROW | enforce_write_op_state | trg_pc_write_op_state | trg_projc_write_op_state |

**B 组·两域一致但函数有意分歧（注记行）**：

| 目标表 | 事件 | 时机/属性 | pc 函数 | projc 函数 | 注记 |
|---|---|---|---|---|---|
| document_revision | DELETE,UPDATE | BEFORE/ROW | guard_document_revision | forbid_row_update | 0009 FTS 演进：pc 允许 FTS 列更新，projc 无 FTS 全拒——设计内分歧，非缺位 |

**C 组·closure/anchored 校验面（CONSTRAINT 触发器，非保护面，两侧一致）**：

| 目标表 | 事件 | 属性 | 函数 |
|---|---|---|---|
| version_member | INSERT | CONSTRAINT/DEFERRABLE INITIALLY DEFERRED | enforce_member_endpoint_closure |
| proposal_member | INSERT | CONSTRAINT/DEFERRABLE INITIALLY IMMEDIATE | enforce_proposal_member_anchored |
| proposal_member | DELETE,UPDATE | 同上 | 同上 |

**D 组·例外登记行（断言 pc 有/projc 无＋注记）**：

| pc 触发器 | projc 对应 | 例外依据 |
|---|---|---|
| trg_pc_relationship_anchors / trg_pc_relationship_delete / trg_pc_rel_revision_endpoint_match / trg_pc_rel_revision_immutable | 无 | **D-3 已裁**：暂不补齐；未来开放关系修订/修改/删除路径前强制前置；届时重验四件套+镜像一致 |
| trg_pc_version_delete（BEFORE DELETE ON cognition_version） | 无 | **§4.4 待裁决**（总账维护者核对→负责人决定 GAP-37-C 与否） |

测试断言规则：A 组两侧均须存在且六维逐项相等；B/C 组两侧存在＋注记差异字段按表断言；D 组 pc 存在＋projc 不存在＋注记引用。清单外任何一侧新增/删减触发器未同步清单 → 测试失败（防无记录差异）。**R-5 边界：A 组结构一致性通过≠对各函数业务语义的重新审计（见 §3.3 边界提醒）。**

## 五、实施前置检查（动工日，全部只读）

1. GAP-04 重复预检复跑（强制；非 0 即停并报告）；
2. 迁移编号 0029/0030 占用检查；
3. Git：批准令已入库＋工作树干净＋HEAD=origin/master；
4. §四.5 清单基准快照重导核对（防基线漂移）。

## 六、验收标准（不变，8 项）

迁移应用/两域 GAP-04 探针/GAP-37 探针/镜像清单测试（T-2，含 R-5 边界）/回归/泄漏检查/边界自检/验证报告（含 §4.4 处置状态记录）。

## 七、回滚方案

### 7.1 事务失败处理
每迁移文件整体单事务（`psql -v ON_ERROR_STOP=1 -1 -f`；sqlx 按序应用，任一文件失败中止该库全部应用）——文件内任一语句失败→整体回滚→**无部分对象状态**。失败即停止：记录失败语句与错误原文→回滚确认→不重试不跳过→报告。

### 7.2 回滚前对象检查（先查后删）

| 将回滚对象 | 回滚前必须确认 |
|---|---|
| cognition_revision_object_id_revision_no_key（pc/projc） | pg_constraint 存在且 pg_get_constraintdef=UNIQUE (object_id, revision_no)；测试探针已随代码 revert |
| trg_projc_revision_immutable / trg_projc_vm_immutable | pg_trigger 存在且 pg_get_triggerdef 与 §3.2 逐字一致；无其他对象依赖 |

「对象不存在/定义不符」→ 停止机械回滚，先报告差异（防误删非本批对象）。

### 7.3 回滚后验证与状态登记

1. 清点复验：pg_constraint/pg_trigger 复查——本批对象全部不存在、零残留（两库）；
2. 数据完整性：两域目标表行数与回滚前一致；
3. 应用面：读路径冒烟＋回归套件任选一套复跑通过；
4. **状态登记（V1.2 R-6 修正）**：回滚后，本批实施状态标记为**失败并已完成回滚**；**保留原实施授权及执行审计记录**（授权事实与实施失败事实均已发生，不因回滚抹除）；是否重新实施由**负责人重新决定**，**不自动恢复实施资格**。

## 八、执行纪律（不变）

127.0.0.1:16433／RUSTC_WRAPPER=""／纯增量无回填／_sqlx_test% 泄漏检查清理／编号顺延／完成即停等验收／不跨批／不自行宣告状态变化。

## 九、交付物与停止点

本任务书（V1.2 草案）→ 负责人批准为 Batch 1 基线【**当前停止点**】→ 批准令签发入库＋明确实施授权 → 0029/0030+T-1~T-5+验证报告 → 停止等验收 → commit&push 与批 2 由负责人决定。

## 十、零实施声明（不变）

本任务书为文档交付：未修改代码/Schema/Migration/API/测试/数据库（仅只读核实与 SELECT 预检）；未提前实施 §三 任何内容；未修改 GAP 状态、未创建 GAP（§4.4 处置权在总账维护者/负责人）、未裁决 Q 级问题。**§三 迁移 SQL 为获批后执行的草案，现在不以任何形式执行。**
