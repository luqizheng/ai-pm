# P3-D04-02 Step 5 Batch 4a 实施与验证报告 V1.0

- 编制日期：2026-11-18（与 Batch 3 同轮回归；独立验收证据）
- 授权：负责人《剩余批次一次性实施授权与执行指令 V1.0》（2026-11-18）
- 依据：《D6.6 Request Fingerprint 与 Operation Identity 完整性核验 V1.1》（实施规范）+设计方案 V1.0.7 §D6.6/D6.7+总账 V1.4 GAP-01/27/30 行+《Q04发布段幂等证据补充表 V1.0》
- 范围遵守：仅 GAP-01+GAP-27+GAP-30；slot 键结构/结构匹配校验归 Batch 4b（GAP-02）；31-B wire 命名（Q-06）不触碰；22/32 未触碰

## 一、实施内容

### 1.1 Migration 0032（`0032_write_operation_request_fingerprint.sql`，两域镜像）

- write_operation ADD `request_fingerprint text`（账本行=单一权威指纹的物理载体，D6.6 §1）；
- command CHECK 收敛增列起草段命令：pc=`pc_propose_from_analysis`、projc=`projc_propose_from_analysis`（既有 `{domain}_{snake_fn}` 约定；发布键 wire 命名=Q-06 域不涉本列）。

### 1.2 GAP-01 起草段账本（两域 propose_from_analysis，pc=ai_agent/projc=repo_analysis）

- **op_id=D6.6 §4 分析链形态** `{pc|projc}-analyze:{task_id}`（服务端派生，调用方无键入参）；
- **请求意图指纹**=D6.6 §2 九要素规范化投影（E-43 同族：title/rationale trim 原文、origin_kind 枚举原文、source_id、执行可知维度空值入规范化串；分析链请求不携带成员→pins 空段；slot/任务锚由 op_id 承载不进指纹）；
- **认领即锚定**：INSERT（command=起草段命令、status=pending、fp 落列）唯一键等待互斥（D5.1 补充 1）；完成时 UPDATE status=completed（同事务、无部分状态=D6.7）；
- **场景 1/9 幂等短路**：候选重推导之前先查账本——completed → 严格指纹判定 → 幂等返回原提案（source_id+origin_kind 锚定，零重复起草）；
- **场景 2 失配拒绝**：同 op 异意图 → `Conflict(fingerprint mismatch)` 409（不得误判重放）；
- pending 可见面防御拒绝（advisory 串行下不可见）。

### 1.3 GAP-27 发布段严格指纹（两域 publish.rs）

- completed 短路分支：SELECT 增 request_fingerprint → 取版本 source_proposal_id → 提案内容锚比对——同 op 同 fp → replayed=true；**同 op 异 fp → 409（publish ledger fingerprint mismatch）**（§五 T4 两行严判定落地；fp=NULL 视为失配=fail-closed，无兼容分支〔AGENTS 规则 1〕）；
- 完成 UPDATE：`request_fingerprint = proposal_hash`（发布确认时已三方锚验证的内容锚）——账本行完成即携带权威指纹；
- expected_content_hash 占位串维持不动（其 CAS 职责由 GAP-15/31/35 承载，指纹权威=新列）。

### 1.4 GAP-30 起草段幂等用例（新文件 tests/analysis_ledger_idempotency.rs，4 用例）

| 用例 | 场景矩阵 |
|---|---|
| `analyze_replay_is_idempotent_via_ledger`（pc） | 场景 1/9 幂等重放返回原提案+场景 8 账本行承载（command/status/fp） |
| `analyze_same_op_different_intent_rejected`（pc） | 场景 2 同 op 异意图 409+不产生第二提案 |
| `publish_completed_short_circuit_compares_fingerprint`（pc） | 发布段同 op 同 fp replayed / 合成异 fp 409 / 恢复后重放 |
| `projc_analyze_replay_is_idempotent_via_ledger`（projc） | ProjC 镜像（场景 1/2+账本承载） |

配套小变更：`AnalysisProposalInput` 增 `#[derive(Clone)]`（重放用例构造需要；DTO 纯增量）。

## 二、实际执行结果

```
analysis_ledger_idempotency  ok. 4 passed; 0 failed（10.02s）
```

全量回归（两批合并终态，见 §三）：pm-application 76 测试二进制 `test result: ok` 全过+pm-api 全套件全过（backgrounded 落盘 target/b3b4a-regression.log）。

## 三、全量回归

- `cargo test -p pm-application`：76/76 二进制 ok（含新增 analysis_ledger_idempotency）。
- `cargo test -p pm-api`：全部 ok（analysis_reports close 链 E2E 11/11+其余套件）。
- 中途发现并修复：实施序中 0032 迁移曾遗漏落盘导致 pm-api close 链 `42703 request_fingerprint does not exist`×3——补 0032 后全绿（如实验证记录，最终态=全过）。
- `_sqlx_test%` 泄漏检查：0。

## 四、GAP 逐项验收对照

| GAP | 验收项 | 结果 |
|---|---|---|
| GAP-01 | 起草段账本行（起草+提交一体同事务；op_id=D6.6 §4 分析链形态） | ✅ pc/projc 实测 |
| GAP-01 | 唯一键等待互斥+同事务无孤儿 | ✅ 机制镜像 publish 段 |
| GAP-27 | request_fingerprint 列+认领锚定 | ✅ 账本承载断言 |
| GAP-27 | 起草段场景 1/2（同 op 同/异意图） | ✅ 幂等/409 实测 |
| GAP-27 | 发布段严格指纹（同 op 同 fp 确认+异 fp 409） | ✅ 合成失配实测 |
| GAP-30 | 起草段幂等用例集 | ✅ 4 用例 |
| slot 结构/匹配校验 | **不在本批**（GAP-02，Batch 4b） | —— |

## 五、边界自检

Q-01 方案 A 落地（账本与产物/审计/Outbox 同事务；advisory 仅并发协调）；未自行裁决 Q-06（未动发布键 wire 命名与校验位置面）；slot 校验未混入（4b）；无数据回填（fp=NULL 既有行 fail-closed 而非兼容分支）；dev 库 0032 已应用。

**状态：Batch 4a 实施与验证完成；独立验收证据齐备；按统一授权自动进入 Batch 4b。**
