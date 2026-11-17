# P3-D04-02 Step 3 — Operation 身份与 Fingerprint 方案对比 V1.0

- 日期：2026-11-17
- 任务书条目：§二 P0（操作身份及指纹模型）
- 性质：设计分析与建议；**推荐方案不构成已批准裁决**

## 0. 维度澄清（防止与 Q-01 混淆）

本对比的「方案 A/B」=**操作切分维度**（起草+提交是一体还是两个独立操作）。
未决清单的「Q-01 方案 A/B/C」=**锚载体维度**（账本扩展/advisory 现状/幂等列）。
两维正交：一体切分×账本扩展=V1.0.2 推荐组合（Q-01 方案 A 的具体化）；独立切分可与任一锚载体组合但均不推荐（§5）。

## 1. 方案 A：起草与提交作为一体操作（完整规格）

| 任务书要求 | 规格 |
|---|---|
| 一个 operation_id 覆盖完整链路 | `{domain}-analyze:{task_id}`（分析链）/`{domain}-identity:{round_id}:{slot}`（身份链，slot=profile/vision）——单键贯穿账本登记→起草→提交→事件写入全链 |
| 一个稳定 fingerprint 覆盖完整请求意图 | `proposal_content_hash`（规范化哈希覆盖 title/rationale/origin/governance/expected_version_no/成员集全集）——**完成时一次性锚定**；成员在创建时即锚定→起草段与提交段的逻辑内容恒等→单指纹天然稳定；启动时列占位、完成回填（一次性写入，非阶段变更） |
| 中间状态如何记录 | **无持久中间态**——单事务 all-or-nothing；账本 pending 行是事务内占位（随事务成败），不构成跨事务中间状态；阶段事实（draft 态提案+pending 审批行）仅提交后可见且不可变锚定 |
| 中途失败如何恢复 | 全回滚（账本无痕）→调用方同 operation_id 重放→L1 内容去重（content_hash/object_key）保证不产生重复草稿；无孤儿状态需处理 |
| 重复请求如何返回正确结果 | 账本 completed 短路→返回既有 receipt（artifact_id 回填的 proposal_id）——`replayed=true` 语义（发布段同款推广） |
| write_operation 结构调整 | **需要**（=GAP-27，实施批）：①command CHECK 扩展（propose/submit 复合命令或单 propose 命令，wire 名 Q-06）；②artifact_id 列（存 proposal_id）；③指纹存储（复用 expected_content_hash 列） |

## 2. 方案 B：起草与提交作为两个独立操作（完整规格）

| 任务书要求 | 规格 |
|---|---|
| 两个 operation_id 生成规则 | `{op}:draft` 与 `{op}:submit`（同上层锚前缀，两段键） |
| 两个 fingerprint 计算规则 | draft fp=成员集规范化哈希；submit fp=(proposal_id, proposal_content_hash, expected_version_no) |
| 两条账本记录事务边界 | draft 账本行+提案创建=事务 1；submit 账本行+状态翻转+pending 审批行=事务 2（独立提交） |
| submit 如何绑定原始 proposal | submit 段携带 proposal_id + draft 段 artifact_id 反查 |
| 起草成功但提交失败恢复 | 草稿已留存（L1 去重安全）→submit op_id 单独重试；draft 账本已 completed 不重放 |
| 三类风险 | ①重复起草：L1 内容去重可挡；②重复提交：pending 唯一约束可挡；③**孤儿账本**：draft completed 但提案永停留 draft（提交永不发生）→账本终态与领域状态脱节，重放必须状态感知（「提案已 submitted/已存在→返回而不重建」），语义复杂度显著上升 |

## 3. 对比与推荐

| 维度 | 方案 A（一体） | 方案 B（独立） | 判定 |
|---|---|---|---|
| 正确性 | 单事务无中间态→无状态失配面 | 中间态（draft 持久化）→账本态与领域态可能脱节（孤儿） | A 优 |
| 复杂度 | 1 行 1 事务 1 指纹 | 2 行 2 事务 2 指纹+状态感知重放 | A 优 |
| 兼容性 | 与现状分析链单事务结构一致（create+submit 同 tx 已是现实，proposal.rs:1466-1491） | 需把现状同事务拆为两事务=**行为变更** | A 优 |
| 恢复能力 | 重放即恢复（无孤儿） | 需孤儿处理+状态感知 | A 优 |
| 与 Q-01 选项关系 | 即 Q-01 方案 A 的具体化 | 不改变锚载体选择，但需 Q-01 重新裁定切分语义 | —— |

**唯一推荐：方案 A（一体操作）。**
理由（全部来自实证）：①现状分析链即同事务一体——方案 A 是「定稿现状」，方案 B 是「改行为」，违背最小变更原则；②成员集在创建时锚定→单一指纹天然稳定，方案 B 的两指纹设计要处理「draft fp 与最终内容不一致」的失效场景，复杂而无效益；③无孤儿账本面，重放语义与发布段（completed 短路）完全同构。

## 4. 五场景时序（方案 A 定稿语义）

```
① 首次调用：账本 INSERT(op,pending) → create_proposal_in → submit_proposal_in
   → outbox → 账本 completed(fp=proposal_content_hash, artifact_id=proposal_id) → COMMIT → 返回 receipt
② 并发调用（同 op_id 双事务）：T2 的 INSERT 在唯一索引上等待 T1 结论
   → T1 commit completed → T2 命中冲突 DO NOTHING → 读得 completed → 幂等返回同 receipt
③ 重复投递（首次成功后再次投递）：账本命中 completed → 短路返回（不新建提案/不发新事件）
④ 事务回滚（业务校验失败，如 L3 冻结/基线迁移）：全回滚（账本无痕）→ 409/403 → 修复后同 op_id 重试
⑤ 进程崩溃：commit 前=等同回滚（无痕）→ 重放重执行；commit 后=账本 completed+产物完整 → 重放幂等返回
```

## 5. 三组合行为（任务书 §二.4）

| 组合 | 行为 |
|---|---|
| 同 operation_id + 同 fingerprint | 幂等重放：返回既有 receipt（proposal_id） |
| 同 operation_id + 不同 fingerprint | **拒绝 409 mismatch**（键复用错配防护；分析链消费者无独立指纹→由 op_id 所有权规则约束） |
| 不同 operation_id + 相同 content_hash | **不同逻辑操作**：允许并存（各自独立审批、不合并）；content_hash 仅作 L1 内容去重与指纹材料 |

## 6. Q-01 裁决选项影响

**不需要修改 Q-01 的裁决选项**：一体模型已内嵌于 Q-01 方案 A 的设计定义（V1.0.2 §D6.6/D6.7），本对比是对该定义的完整论证而非变更。若人工裁决改选独立切分（方案 B），Q-01 须重新裁定（条件性影响，登记于未决清单 Q-01 备注）。核心设计问题已在本方案内闭合，**未推迟为 Q-06 实施细节**（Q-06 仅余 wire 命名）。
