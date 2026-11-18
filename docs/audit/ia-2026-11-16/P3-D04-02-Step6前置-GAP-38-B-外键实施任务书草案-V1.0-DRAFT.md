# P3-D04-02 Step 6 前置 · GAP-38-B 外键实施任务书草案 V1.0（DRAFT）

- 编制日期：2026-11-18
- 性质：**草案（零实施）**——《38-B 外键关系与锁序专项分析 V1.0》的落地预案；**本草案存在≠实施授权**：38-B 实施须负责人基于独立分析明确裁决后本草案方生效
- 依据：《38-B 外键关系与锁序专项分析 V1.0》（79 pc 镜像候选边/六簇/NO ACTION 口径/逐写路径锁序未见反转）+ 负责人裁决口径（38-B=独立分析与后续裁决子项；外键实施当前不授权）

---

## 一、触发条件

负责人就《38-B 专项分析 V1.0》作出实施裁决（全部/部分簇/暂缓）后，本草案按裁决修订为正式任务书生效；未裁决前零执行。

## 二、实施方案（按簇分批；拟迁移编号 0037 起，逐簇一文件或单文件分段事务）

| 批次簇 | FK 边（pc 镜像） | 前置预检 |
|---|---|---|
| 簇1 core | 各表 (tenant_id,project_id)→core.project、created_by/decided_by/confirmed_by/resolved_by 等→core.principal | 孤儿值扫描=0 |
| 簇2 身份/内容 | cognition_object.current_revision_id→revision；cognition_revision→object/document/document_revision；document_revision→document | 同上 |
| 簇3 提案 | proposal→source；proposal_member→proposal/object/relationship/revision/relationship_revision；approval→proposal | 同上 |
| 簇4 版本 | cognition_version→proposal；version_member→version/object/relationship/revision/relationship_revision | 同上 |
| 簇5 冲突 | conflict_object→conflict/object/relationship | 同上 |
| 簇6 字典 | cognition_object.object_type→object_type(type_key)；relationship.*→relationship_type(type_key) | 同上 |

全部边：**NO ACTION（pc 真源默认逐字镜像）**；引用列与被引用列（0036 PK/既有 UNIQUE）对齐校验先行。

## 三、并发实测背书义务（分析 §三 结论的强制闭环）

- 并发交错用例：publish×draft upsert×decide 三方并发压测（多连接），断言无死锁/无 PoolTimedOut/结果语义不变；
- 锁等待观测：pg_locks 采样确认 FK SHARE 检查不与既有 FOR UPDATE 形成反向边。

## 四、回滚

逐簇反向依赖序 DROP CONSTRAINT；每簇独立事务；失败即停报告。

## 五、零实施声明

本草案未执行任何 SQL/代码修改；未预占迁移编号（0037 起空闲）；生效以负责人裁决为准。
