# P3-D04-02 Step 5 Batch 1 实施任务书 V1.0

- 编制日期：2026-11-17
- 性质：**实施任务书（草案，供负责人审阅）**——本文件为文档交付，本身不执行任何实施
- 规划基线：《P3-D04-02 Step 5 实施批次规划与授权建议 V1.0》（已审阅，批次划分暂定认可；第一批候选范围=GAP-04+GAP-37-A/B，**当前不授予实施授权**）
- Git 基点：`ca198c8`
- ⚠ **执行门（硬性）**：本任务书获批 ≠ 实施开始。实施动工同时满足两个前置：①《Step 3 设计冻结批准令》已正式签发并 commit 入库归档；②负责人对本 Batch 1 下达明确实施授权（范围=本任务书 §三，或其裁剪版）。两门任一未发生，不得动工。

---

## 一、任务目标

Batch 1 = **DB 保护兜底批**：为正式事实补齐数据库层防御纵深，纯增量、零行为变化。

| GAP | 内容 | 总账 V1.3 状态锚 |
|---|---|---|
| GAP-04 | 两域 cognition_revision 补 (object_id, revision_no) UNIQUE 约束 | 活跃实施差距（C 级）；行锁推号在位，DB 兜底缺位 |
| GAP-37-A | projc.cognition_revision 补 UPDATE/DELETE immutable 保护 | 已设立〔D-4〕未授权实施 |
| GAP-37-B | projc.version_member 补 UPDATE/DELETE immutable 保护 | 同上 |

**不在本批**：任何其他 GAP；任何服务层/行为变更；任何数据回填；GAP-04 仅 pc+projc 两域（ai/kb 域已有同款约束，不在范围内）。

## 二、权威输入与验收锚

1. 验收对照=《GAP 编号唯一总账及统计复算表 V1.3》GAP-04 行与 GAP-37 行（含 D-4 验收三类+追加维度）；
2. 设计依据=总账 GAP-04 行引 §D5「(object_id,revision_no) 唯一约束缺位=C-1 后置兜底」；GAP-37=裁决归档记录 V1.0（D-4）；
3. 镜像真源=pc 侧既有触发器 DDL（0001:7043/:7071，逐字见 §四.3）；
4. 契约纪律：不修改 V0.6/R36~R38/冻结批准令/任何已裁决事项；AGENTS.md 全部规则生效。

## 三、实施范围（Migration 与测试面草案——获批后执行）

### 3.1 迁移 0029：GAP-04（文件名 `migrations/0029_cognition_revision_no_unique.sql`）

```sql
-- 0029_cognition_revision_no_unique.sql
--
-- P3-D04-02 Step5 Batch1 GAP-04：两域 cognition_revision 补 (object_id, revision_no)
-- 唯一约束（DB 层 revision_no 兜底；行锁推号在位不变）。
-- 同款先例：ai.document_revision / kb.document_revision / kb.memory_revision
--   （0001:3722/5210 UNIQUE(document_id, revision_no) 同构）。
-- 迁移前检查：两库两域 (object_id,revision_no) 重复分组=0（2026-11-17 只读预检，
--   实施日须复跑，见 §五.1）。
-- AGENTS.md 规则 1：无数据迁移、无回填、无兼容分支。

ALTER TABLE pc.cognition_revision
    ADD CONSTRAINT cognition_revision_object_id_revision_no_key
    UNIQUE (object_id, revision_no);

ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_object_id_revision_no_key
    UNIQUE (object_id, revision_no);
```

### 3.2 迁移 0030：GAP-37-A/B（文件名 `migrations/0030_projc_mirror_guards.sql`）

```sql
-- 0030_projc_mirror_guards.sql
--
-- P3-D04-02 Step5 Batch1 GAP-37-A/B：ProjC 镜像保护面补齐（D-4 裁决）。
-- A=projc.cognition_revision / B=projc.version_member 补 UPDATE/DELETE immutable
-- 触发器——与 pc 真源逐字同款（0001:7043 trg_pc_revision_immutable /
-- 0001:7071 trg_pc_vm_immutable 同构，域内改前缀）。
-- 函数 projc.forbid_row_update 已在位（0025:329-334，与 pc 0001 同款：
--   无条件 RAISE EXCEPTION→BEFORE DELETE OR UPDATE 下 UPDATE/DELETE 双面拒绝）；
--   本迁移零新函数。
-- 实施设计核实记录：函数实际行为/触发时机/异常语义已于 2026-11-17 只读核实
--   （《Step5 批次规划 V1.0》§五.1）；实测探针为 §六 验收项，不以核实替代验收。
-- AGENTS.md 规则 1：纯增量，无数据迁移。

CREATE TRIGGER trg_projc_revision_immutable
    BEFORE DELETE OR UPDATE ON projc.cognition_revision
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();

CREATE TRIGGER trg_projc_vm_immutable
    BEFORE DELETE OR UPDATE ON projc.version_member
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();
```

### 3.3 测试面（随本批交付）

| # | 测试 | 性质 | 验收锚 |
|---|---|---|---|
| T-1 | GAP-37 探针：UPDATE projc.cognition_revision（普通字段）/DELETE 同表 → 均被 DB 拒绝；UPDATE/DELETE projc.version_member → 均被拒绝（A/B 各自独立断言） | 新增用例（位置随现有测试布局，建议 projc_formal_chain.rs 或独立 tests/projc_mirror_guards.rs） | 总账 GAP-37 验收① |
| T-2 | **镜像保护清单测试**：静态断言 ProjC 守卫集合与 pc 真源清单一致（触发器名/表/事件面逐项对照）——防保护面再次出现无记录差异 | 新增用例 | 总账 GAP-37 验收②+D-4 追加维度 |
| T-3 | GAP-04 探针：同 (object_id, revision_no) 第二次 INSERT 被拒（两域各一） | 新增用例 | 总账 GAP-04 |
| T-4 | 回归：projc_formal_chain 全量（6 用例）+ pc_model + pc_proposal 全量；sqlx::test 全新临时库按序应用 0001~0030 成功 | 既有回归 | D-4 验收③+迁移全序 |
| T-5 | 追加维度断言：INSERT 与既有发布流程不受影响（T-4 回归内覆盖）；无绕过路径（测试边界说明）；PC/ProjC 保护语义一致（T-2 承载） | 融合于 T-1/T-2/T-4 | D-4 追加维度 |

### 3.4 明确不做

- 不改 pc/projc 服务层任何 .rs 行为代码（测试新增除外）；
- 不动 0001~0028 既有迁移文件；
- 不做 ai/kb 域变更；不做数据回填/清理；
- 不处理其他 GAP（35/31-A/36 等属后续批次，另行授权）。

## 四、GAP-37-A/B 目标表、现有触发器挂载与新增内容（审阅要求②）

### 4.1 目标表 A：projc.cognition_revision

| 项 | 现状 |
|---|---|
| 建表 | 0025:112-129（CHECK 仅 analysis_mode/revision_no≥1） |
| 现有触发器挂载 | **零**（活库 pg_trigger 清点 B-13：projc.cognition_revision 无任何触发器） |
| pc 真源参照 | trg_pc_revision_immutable（0001:7043）BEFORE DELETE OR UPDATE→pc.forbid_row_update |
| 本次新增 | **trg_projc_revision_immutable**（§3.2 第一条）——域内唯一新增挂载，无调整 |

### 4.2 目标表 B：projc.version_member

| 项 | 现状 |
|---|---|
| 建表 | 0025:148-162 |
| 现有触发器挂载 | **仅 1 个**：trg_projc_vm_closure（0026:203，AFTER INSERT DEFERRABLE INITIALLY DEFERRED 约束触发器，成员端点闭合=W1 批准项，探针 P4 已验）——**与 pc 同款在位，本批不动** |
| pc 真源参照 | trg_pc_vm_closure（0001:7064）+ **trg_pc_vm_immutable（0001:7071）**——pc 有 closure+immutable 两个，projc 缺 immutable |
| 本次新增 | **trg_projc_vm_immutable**（§3.2 第二条）——新增挂载，不调整既有 closure |

### 4.3 需要新增/调整的内容汇总

| 表 | 新增 | 调整 | 依据 |
|---|---|---|---|
| projc.cognition_revision | trg_projc_revision_immutable | 无 | pc 0001:7043 同款镜像 |
| projc.version_member | trg_projc_vm_immutable | **无**（既有 closure 保持原样） | pc 0001:7071 同款镜像 |
| 两域 cognition_revision | UNIQUE 约束（0029） | 无 | §D5 + document_revision 先例 |
| 函数 | **零新增**（projc.forbid_row_update 0025:329 在位） | 无 | — |

实施后触发器面：projc 由 10 → 12 个；与 pc 17 个的差异余量=relationship 守卫四件套+rel_revision/rel_revision_immutable（D-3 已裁：暂不补齐，未来修订链开放前强制前置）+cognition_revision FTS 面读侧项——不在本批范围。

## 五、实施前置检查（动工日执行，全部只读）

1. **GAP-04 重复预检复跑（强制）**：两库（aipm_mig_full/aipm_mig_s1）两域 `(object_id,revision_no)` 重复分组必须=0（本任务书编制时已重跑=0/0，证据 2026-11-17；**实施日以复跑结果为准**，非 0 即停止并报告）；
2. GAP-37 函数体核实记录引用（《Step5 批次规划 V1.0》§五.1）——已过，实施时不再重复函数级核实，直接进入探针验收；
3. 迁移编号占用检查：0029/0030 未被占用（当前 migrations 目录止于 0028）；
4. Git 状态：批准令已入库、工作树干净、HEAD 与 origin/master 同步。

## 六、验收标准（逐项对照总账 V1.3）

| # | 验收项 | 通过标准 |
|---|---|---|
| 1 | 迁移应用 | docker exec + ON_ERROR_STOP 单事务执行 0029/0030 于两验证库成功；sqlx::test 全新临时库 0001~0030 全序通过 |
| 2 | GAP-04 探针 | 两域同 (object_id,revision_no) 二次 INSERT → duplicate key 违反新约束（事务内 ROLLBACK 零残留） |
| 3 | GAP-37 探针 | A/B 两表 UPDATE（普通字段）与 DELETE 各自被拒（事务内 ROLLBACK）；INSERT 与发布链路不受影响 |
| 4 | 镜像清单测试 | T-2 通过——ProjC 守卫集合与 pc 真源一致 |
| 5 | 回归 | projc_formal_chain 6/6 + pc_model + pc_proposal 全绿 |
| 6 | 泄漏检查 | `pg_database WHERE datname LIKE '_sqlx_test%'` = 空 |
| 7 | 边界自检 | 未触碰 pc Schema/冻结契约/服务层行为/其他表（逐项列入验证报告自检表，同 A-1 报告格式） |
| 8 | 交付报告 | 《Batch 1 实施与验证报告 V1.0》：执行记录+验证单回填+边界自检+Git 变化清单 |

## 七、回滚方案

| 项 | 回滚 |
|---|---|
| 0029 | `ALTER TABLE pc.cognition_revision DROP CONSTRAINT cognition_revision_object_id_revision_no_key;` + projc 同款——单语句无痕，无数据变更 |
| 0030 | `DROP TRIGGER trg_projc_revision_immutable ON projc.cognition_revision;` + `DROP TRIGGER trg_projc_vm_immutable ON projc.version_member;` |
| 测试 | revert 对应 commit |
| 失败预案 | 任一验收项失败 → 回滚该迁移+revert+报告中如实登记失败现象；不带着失败项进入后续批次 |

## 八、执行纪律

1. AGENTS.md 全部规则：`DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/…'`（禁 localhost）；cargo 跑前 `RUSTC_WRAPPER=""`；迁移纯增量无回填无兼容分支；测试进程被杀后检查并清理 `_sqlx_test%` 泄漏库；
2. 迁移编号顺延（0029/0030），不重排既有编号；
3. 每步先只读检查后执行；验证库操作事务内 ROLLBACK 收尾（探针类）；
4. 完成即停：交付验证报告后等负责人验收；**不跨批**（不触碰 35/31-A/36 等后续批次内容）、不自行扩大范围、不自行宣告任何冻结/授权状态变化。

## 九、交付物与停止点

| 阶段 | 交付 | 停止点 |
|---|---|---|
| 本任务书 | 本文件（草案） | **交负责人审阅**——当前停止 |
| 获批+授权后 | 0029/0030+T-1~T-5+验证报告 | 报告提交后停止，等验收 |
| 验收通过后 | 由负责人决定是否 commit&push 及启动批 2 规划 | —— |

## 十、零实施声明

本任务书为文档交付：未修改代码/Schema/Migration/API/测试/数据库；对数据库仅执行 SELECT 级只读预检（§四/§五 证据）；未提前实施任何 §三 内容；未修改 GAP 状态、未裁决 Q 级问题、未改变冻结设计与裁决。**§三 迁移 SQL 为获批后执行的草案，现在不以任何形式执行。**
