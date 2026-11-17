# P3-D04-02 Step 5 Batch 4b 实施与验证报告 V1.0

- 编制日期：2026-11-18
- 授权：负责人《剩余批次一次性实施授权与执行指令 V1.0》（2026-11-18）
- 依据：总账 V1.4 GAP-02/23 行+《19 项 GAP 实施责任矩阵 V1.0》GAP-02/23 行+D6.6 V1.1 §3/§4（身份链键形态与 slot 三规则）
- 范围遵守：仅 GAP-02+GAP-23（经 02 承载）；ProjC 无身份综合链（域差异第 7 项，不镜像）；slot 键不进指纹（D6.6 §2 行 6）

## 一、实施内容

### 1.1 Migration 0033（`0033_pc_propose_identity_command.sql`）

pc.write_operation command CHECK 增列 `pc_propose_identity`（0032 已铺 request_fingerprint 列；projc 不涉及=身份链 PC 独有）。

### 1.2 GAP-02 身份链幂等（identity.rs）

- **apply_identity_drafts 增 `round_id: Uuid` 透传参数**（矩阵行：round_id 调用方生成透传）；调用方 pm_cognition.rs 处理器接收可选 `?round_id=`（缺省服务端生成=新轮；非法 uuid→400）；
- **op_id=D6.6 §4 身份链形态** `{pc}-identity:{round_id}:{slot}`，slot∈{profile,vision}（1 round→≤2 PC 提案）；
- **slot 三规则落地（D6.6 §3）**：规则 1 键空间所有权=服务端派生（调用方不得构造键）结构性满足；规则 2 结构校验=slot 枚举守卫（非 profile/vision→fail-closed Failed）；规则 3 匹配校验=认领/查询同键派生（无错配输入面）结构性满足；
- **fp=请求意图**（LLM 草案正文即意图：title|rationale|origin|source_id|object_key|markdown 规范化串；成员维度由 object_key+正文承载）；
- **认领先于一切副作用**（upsert 每次产生新修订）：completed+同 fp→幂等返回原提案（场景 1/9）；同轮异草案→Conflict(fingerprint mismatch) 409（场景 2）；pending 行+活提案在=崩溃恢复（补完成锚定返回，矩阵行「中途失败账本留痕可续」）；无行→认领后执行；
- **Goal 腿 operation_id 确定性化**：`identity-synthesis:{round_id}`（原随机 uuid 尾段废止）——同轮重试经知识链既有 operation_id 幂等去重；
- 账本完成锚定（command=pc_propose_identity，status pending→completed）。

### 1.3 GAP-23 事件幂等

身份链事件幂等经 02 承载（矩阵口径「23 无独立实施条目；实施对象唯一=02」）：同轮重跑不再产生第二提案→无重复 outbox 事件；Goal 腿经确定性 operation_id 去重。分析链半边=既有已满足（E-22/E-28）。

## 二、测试与实际结果

既有 identity_synthesis.rs 2 用例补 round_id 实参（行为不变）；新增 2 用例：

```
identity_synthesis  ok. 4 passed; 0 failed（20.92s）
  same_round_replay_is_idempotent       —— 同轮重跑：profile/vision 幂等返回原提案；提案总数仍 2；账本 2 行 completed（slot 异键各自成立）
  same_round_different_drafts_rejected  —— 同轮异草案：Conflict(fingerprint mismatch) 409
  apply_requires_open_report / apply_attaches_proposals_to_open_report —— 既有回归绿
```

## 三、全量回归

并入终态全量回归（pm-application+pm-api+pm-infrastructure 三 crate 全量，落盘 target/final-regression.log；结果见《Step5 统一实施最终交付报告》§回归）。

## 四、GAP 逐项验收对照

| GAP | 验收项（矩阵行） | 结果 |
|---|---|---|
| GAP-02 | 同轮重跑零重复提案 | ✅ same_round_replay 实测（count=2） |
| GAP-02 | 异 slot 异键各自成立 | ✅ 账本 2 行（profile/vision）completed |
| GAP-02 | round_id 调用方生成透传 | ✅ apply 签名+HTTP ?round_id= |
| GAP-02 | slot 校验随 27 校验器 | ✅ 枚举守卫+结构性满足（§3 三规则） |
| GAP-02 | 中途失败账本留痕可续 | ✅ pending+活提案恢复路径 |
| GAP-23 | 身份链事件幂等 | ✅ 经 02 承载（矩阵口径） |

## 五、边界自检

Q-06 未触碰（未动发布键面）；slot 不进指纹（D6.6 §2 行 6）；fp 失配 409=Q-04 严判（LLM 重 synthesis 产出不同正文=异意图，fail-closed 防静默替换）；dev 库 0033 已应用；无数据回填。

**状态：Batch 4b 实施与验证完成；独立验收证据齐备；按统一授权自动进入 Batch 5。**
