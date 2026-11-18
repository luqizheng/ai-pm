# P3-D04-02 Step 6 前置 · GAP-38-B 外键实施与验证报告 V1.0

- 编制日期：2026-11-18
- 授权：负责人「3 按推荐实施」裁决（2026-11-18）——《38-B 实施任务书草案 V1.0-DRAFT》随之生效
- 依据：《38-B 外键关系与锁序专项分析 V1.0》（实施范围与并发背书义务）+负责人验收确认（37-C/38-A 同轮落账）
- 范围遵守：仅 GAP-38-B 实施；Q-06/07/10 与 09①③ 未触碰

## 一、实施内容（Migration 0037，单文件 91 约束）

| 段 | 内容 |
|---|---|
| 1. FK 引用面 UNIQUE ×12 | pc 同名镜像（如 `proposal_tenant_id_project_id_id_key`）；引用列组合此前未在 projc 建——38-A「业务身份必要」口径下的 **FK 引用面必需项**（被引用侧须有唯一背书），非冗余身份族复制；object_type/relationship_type 的 type_key 组合由 0036 PK 背书故不在其列 |
| 2. FK ×79 | pc 真源约束名/列序/DEFERRABLE 逐字镜像（pc→projc 换 schema；core.* 不变）；**全部 NO ACTION**（pc 默认逐字，与 append-only/终态不可变领域语义一致）；含 `cognition_object_current_revision_fkey` DEFERRABLE 特例 |

实施前核对（脚本化逐边实测）：79 边 projc 列存在性 ✓、被引用列存在性 ✓、**孤儿预检=79 边全部 0**。

## 二、测试面与实际结果（tests/gap38b_foreign_keys.rs，3/3 绿）

| 用例 | 断言 | 结果 |
|---|---|---|
| `fk_catalog_matches_pc_mirror` | 79 FK 全集在位；`cognition_object_current_revision_fkey` 定义逐字（含 DEFERRABLE）；引用面 UNIQUE ×12 在位（含 63 字符 PG 截断名如实断言） | ✅ |
| `orphan_member_insert_rejected_by_fk` | 孤儿成员 INSERT → **23503** 拒绝（NO ACTION 语义实测） | ✅ |
| `concurrent_publish_upsert_decide_no_deadlock` | publish×draft upsert×decide 三方并发（分析 §三 强制闭环）：三方全部成功、无死锁、版本/提案终态语义不变 | ✅ |

并发腿基线取 receipts 自洽值（publish 腿与 create/submit/approve 腿并发先后不定，CAS 语义不变）。

## 三、全量回归

真·全量 `--no-fail-fast`（后台落盘 target/final-regression-38b.log；结果随最终回报登记）。

## 四、GAP 逐项验收对照（草案任务书）

| 项 | 结果 |
|---|---|
| 79 边落列（pc 逐字镜像） | ✅ 目录断言 |
| 12 引用面 UNIQUE | ✅ |
| 孤儿预检 0 + 23503 拒绝 | ✅ |
| 并发交错背书（无死锁/语义不变） | ✅ |
| NO ACTION 全量（无 CASCADE/SET NULL） | ✅ 生成器逐字镜像保证 |
| 回滚=反向 DROP CONSTRAINT | 预案在位（未触发） |

## 五、边界自检

dev 库 0037 已应用（projc_fks=79 实测）；GAP-38-A 验收满足与 38-B 实施同轮落账（总账 V1.8）；Q-06/07/10、09①③ 未触碰；泄漏检查随回归收尾。

**状态：GAP-38-B 已实施待验收。Step6 前置批三项（37-C/38-A 已满足、38-B 待验收）全部落地。停止等验收。**
