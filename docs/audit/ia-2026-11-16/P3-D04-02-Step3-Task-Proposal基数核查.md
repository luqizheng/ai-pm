# P3-D04-02 Step 3 — Task-Proposal 基数核查 V1.0

- 日期：2026-11-17
- 任务书条目：§三 P0（逻辑提案身份与业务基数核查）
- 方法：实际代码+数据库约束核实；结论标注【代码证实】/【设计建议】/【待人工裁决】

## 1. 六问逐项核查

| # | 问题 | 结论 | 证据 |
|---|---|---|---|
| 1 | 一个 task 能否生成多个 proposal | **现状=可能**（task 生命周期内多次调用 propose_from_analysis，候选排除只挡「已在开放提案/已入版本」的修订，时间维重跑遇到新候选即产生新提案）；**方案 A 后=不可能**（账本 `pc-analyze:{task_id}` 一 task 一结论，硬保证） | pc/proposal.rs:1401-1541（单 create_proposal_in 多候选成员集）；:1432-1438（候选排除 SQL）【代码证实+设计建议】 |
| 2 | 一个 identity round 能否生成多个 proposal | **是：至多 2 个 PC 提案**（profile/vision 各一，propose_pc_object×2，identity.rs:228-247）+ 1 个 Goal 提案经 `knowledge_proposal::submit_proposal` 8 态管线（**非 pc.proposal**，identity.rs:256-262） | E-35【代码证实】 |
| 3 | 同一 task 多次分析产生不同 proposal | 现状可能（同上）；方案 A 下同 task_id 重放被账本/指纹拦截→**不同分析产生不同提案须使用不同 task_id**（运营模型：每次分析运行=独立任务） | 同 1【设计建议】 |
| 4 | proposal 是否有稳定身份字段 | **无**——proposal 表无 slot/idempotency/logical-identity 字段；source_id 可空且为追溯链入口非身份；身份链提案仅可经成员 object_key（product-profile/product-vision）事后区分 | 0001:3240-3269（建表列清单）【代码证实】→ 补锚=GAP-01/27（账本 artifact_id+slot 键）【设计建议】 |
| 5 | `pc-analyze:{task_id}` 能否唯一标识逻辑提案 | 方案 A 下=**能**（账本唯一约束+一 task 一结论）；现状=软成立（无账本，靠候选排除趋零） | E-13/E-16【代码证实+设计建议】 |
| 6 | ProjC 是否存在不同于 PC 的提案产生方式 | **否**——ProjC 提案产生=分析链单一路径（derive_projc_cognition→propose_from_analysis，kb_pc_handlers.rs:304；origin_kind=repo_analysis）；identity 综合链为 PC 独有（projc 模块无 identity 引用，仅对象「identity 占位态」注释） | E-22/E-35 |【代码证实】 |

## 2. 四情形区分（任务书要求的身份语义矩阵）

| 情形 | 判定依据 | op_id 行为 | 结果 |
|---|---|---|---|
| 同一逻辑提案的**重放** | 同 op_id + 同指纹（proposal_content_hash 一致） | 账本 completed 命中 | 幂等返回既有 receipt（无新提案） |
| 同一逻辑提案的**新版本** | 同对象新 Revision → 新成员集 → 新 content_hash | 新 op_id（新 task/新轮次） | 新提案，与旧提案各自独立审批；旧提案按其状态正常流转 |
| 同一来源产生的**不同逻辑提案** | 同 source_id、不同 (task_id/round_id,slot) | 不同 op_id | 并存，各自独立审批 |
| 内容相同但**身份不同**的两个提案 | 不同 op_id + 同 content_hash | 不合并（V1.0.1 已裁） | 并存；折叠提示=可选产品优化不占编号 |

## 3. 稳定身份方案（一对多关系的补锚设计）

| 链路 | 关系 | 稳定身份方案 |
|---|---|---|
| 分析链 | 1 task → ≤1 proposal（方案 A 后硬保证） | op_id=`{domain}-analyze:{task_id}`，账本 artifact_id 回填 proposal_id |
| 身份链 | 1 round → ≤2 proposal | op_id=`{domain}-identity:{round_id}:{slot}`，slot=对象键（product-profile/product-vision），轮内稳定；**round_id 单键不可用（基数>1）** |
| Goal 提案 | knowledge_proposal 管线独立身份 | 不占 pc/projc 账本键空间 |

【均为设计建议，随 Q-01 方案 A 裁决生效；实施=GAP-27】

## 4. 登记项

- **ProjC 产生路径差异**（现状事实登记，非差距）：PC 双产生路径（分析链+身份链）vs ProjC 单一路径（分析链）——建议列入设计方案 §D1.6 领域差异清单第 7 项（下一版自然并入，本轮仅登记）。
- 基数语义变更（方案 A 账本强制 1:1）不构成对 V0.6 的改动（提案形态概念不受影响，状态值域/基数均为实现层）。
