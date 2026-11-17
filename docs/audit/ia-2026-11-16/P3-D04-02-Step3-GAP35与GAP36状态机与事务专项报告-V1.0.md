# P3-D04-02 Step 3 — GAP-35/GAP-36 状态机与事务专项报告 V1.0

- 日期：2026-11-17｜性质：实施前技术讨论（零实施）
- 裁决边界：Q-02a/Q-02b/Q-03/Q-11/Q-12 已裁决不得重开；`approved→withdrawn` 非法=**设计规则推导**（封闭转换表未列；E-46 负证据），不得表述为 Q-11 裁决内容

---

## 一、GAP-35：跨态转换矩阵与写入路径

### 1.1 现状跨态转换矩阵（CODE/TEST FACT，本轮实测）

| 当前态\动作 | approve | reject | 实测依据 |
|---|---|---|---|
| submitted | ✓→approved | ✓→rejected | 合法（T-03/T-04） |
| approved | 同态幂等✓ | **✗仍放行→rejected（非法，GAP-35）** | pc:972 守卫含 approved+`:1064` 无目标态限制；projc:1062 镜像 |
| rejected | **✗仍放行→approved（非法，GAP-35）** | 同态幂等✓ | 同上；TEST FACT=decide_is_provisional（pc_proposal.rs:1432）断言互放行 |
| draft/withdrawn/applied/failed/expired | 409 | 409 | :972 守卫拒绝（现状即合规） |

### 1.2 目标矩阵（Q-02b+Q-11 收敛后）

| 当前态\动作 | approve | reject |
|---|---|---|
| submitted | ✓→approved | ✓→rejected |
| approved | 同态幂等（GAP-14 语义，返回原 receipt） | **409（Q-11）** |
| rejected | **409（Q-02b）** | 同态幂等 |
| 其余四态+expired | 409 | 409 |

### 1.3 实际写入路径清单（E-51 全量 12 UPDATE 复核）

| 写点 | 能否产生跨态翻转 | 结论 |
|---|---|---|
| submit（pc:810/projc:808） | 否（draft-only 守卫） | 合规 |
| **decide（pc:1064/projc:1062）** | **是——唯一跨态源** | GAP-35 唯一实施点：守卫收敛 |
| withdraw（pc:1181/projc:1180） | 否（draft/submitted 守卫） | 合规 |
| publish→failed（pc/publish.rs:278/279） | 否（approved+CAS 失配内置） | 合规 |
| publish→applied（:598） | 否（approved 守卫+行数断言） | 合规 |
| close 批量 withdraw（analysis_report.rs:622/682） | 否（draft/submitted 清单） | 合规 |

- HTTP/MCP/handlers/infra 零旁路（E-51 §三+本轮抽查）；批处理/内部调用无绕行；直接 SQL 无运行时路径；
- 数据库触发器现状=纯拒绝式（forbid_approval_mutation 等锚列保护），**无状态推进逻辑**——符合「服务层推进/DB 拒绝」边界；GAP-06（C 级）未来补 proposal 转换守卫触发器时须维持纯拒绝式。

### 1.4 测试缺口清单

| # | 缺口 | 归属 |
|---|---|---|
| 1 | decide_is_provisional 断言互放行——**与新规则相反，须改写**（跨态 8 组合断言 409） | GAP-35 实施批 |
| 2 | approved→reject、rejected→approve 两个方向现无 409 用例（现状测试证明的是旧行为） | 同上 |
| 3 | 同态幂等重放保持用例（防回归） | 同上 |
| 4 | projc 镜像侧同 1~3 | 同上 |

### 1.5 错误语义

非法跨态请求=**409 Conflict**（`PcError::Conflict` 同族，与并发裁决「decided concurrently」同状态码不同消息）；与幂等账本无冲突（decide 无账本行/指纹语义——Q-12 明示 expire 不产生指纹，decide 同理按设计无账本登记）；审计不受影响（被拒请求零写入，无审计行=现状纪律维持）。

## 二、GAP-36：expire 完整契约（任务书 §二.4 十二问逐项）

| # | 问题 | 答案（依据类型） |
|---|---|---|
| 1 | 调用主体与项目作用域？ | 服务命令 `expire_proposal(pool, project_id, actor, proposal_id, reason)`，project_id 显式入参（与 decide 同型）→DESIGN REQUIREMENT |
| 2 | 复用现有真人+A3 机制？ | **是**：复用 `ensure_formal_authorized`（pc/mod.rs:152：human 检查→`ensure_governance_authorized` approver.rs:56-79）——CODE FACT 机制在位，零新权限模型 |
| 3 | AI/普通成员绕过可能？ | 无：主体类型层 kind≠human→403 fail-closed（CODE FACT :162-166）；无 A3→Forbidden（:76-78）；无继承/代办（三入口独立调用纪律） |
| 4 | reason 拒绝 null/空/纯空白？ | 是（Q-12 必填+D2.2 输入契约）→DESIGN REQUIREMENT |
| 5 | trim+存规范化值？ | 是（trim 后非空校验+存 trim 值；decision_reason 现行同族 CODE FACT pc:1041-1045） |
| 6 | expired_at/expired_reason 字段？ | 是：proposal 新列（现无，0001:3240-3264 证实）；expired_at=决策事实时间（R38 语义，≠updated_at）——GAP-33 同批 |
| 7 | 四写同事务？ | 是：status+expired_at+expired_reason+审计单事务（D4.10 保证 6） |
| 8 | 与 decide 并发行锁+获锁重检？ | 是：同款 `FOR UPDATE` +获锁后重读 status（D4.10 保证 1/2；decide 半边 CODE FACT :938-946/:947-976） |
| 9 | 失败全回滚？ | 是（单事务原子性，D4.10 保证 7） |
| 10 | 只有一个终态成功？ | 是：守卫 `WHERE status='submitted'`+rows_affected==1（D4.10 保证 5/8；先提交者定终局） |
| 11 | 明确不增加自动 Worker/巡检？ | 是：Q-12.5 显式禁止，V1 零自动到期面 |
| 12 | 审计同事务？ | 是：`{domain}_proposal.expire` 同事务审计行，expired_reason 入 after_data，四身份列全写（GAP-36④；无 Outbox 事件=设计裁定，通知面后置不阻塞） |

### 2.1 事务时序图（目标）

```
expire_proposal:
  ensure_formal_authorized（事务外只读，H-11 窗口维持现状登记）
  BEGIN
    SELECT status FROM proposal WHERE id=$p FOR UPDATE   ← 与 decide 同行锁竞争面
    重检：status≠submitted → 409（含已 expired 同请求=非同态命令，409）
    reason 校验（null/空/空白→Invalid；trim）
    UPDATE proposal SET status='expired', expired_at=now(), expired_reason=$r
      WHERE id=$p AND status='submitted'  ← rows_affected==1 双保险
    append_audit('{domain}_proposal.expire', after_data 含 reason)
  COMMIT                                        ← 四写原子；失败全回滚
```

### 2.2 权限矩阵（expire 动作）

| 主体 | expire |
|---|---|
| AI Agent/服务主体 | ✗ 403（kind 层拒绝） |
| 普通用户（human 无 A3） | ✗ 403 |
| A3 授权人（Lead 或 approver_grant 生效行，**project 绑定**） | ✓ |
| 未认证/跨租户跨项目 | ✗ 401/404 |

### 2.3 并发测试矩阵（GAP-36 实施批验收）

| # | 场景 | 期望 |
|---|---|---|
| 1 | expire‖decide(approve) 同提案并发 | 一胜一 409；终局唯一 |
| 2 | expire‖decide(reject) 并发 | 同上 |
| 3 | 双 expire 并发 | 先者成、后者 409 |
| 4 | expire 事务内失败（如审计写失败注入） | 全回滚：无 status/时间/reason/审计残留 |
| 5 | 非 submitted（draft/approved/终态）expire | 409 |
| 6 | reason null/""/"  " | Invalid 400（fail-closed，零写入） |
| 7 | AI 主体 expire | 403 |
| 8 | 跨项目 A3 | 403/404（project 绑定查询不命中） |
| 9 | 成功后审计行 | 同事务可见，after_data 含 reason |

## 三、Q-04 端到端覆盖矩阵（任务书 §二.5）

| Q-04 条件 | 起草段 | 发布段〔V1.2 勘误后口径〕 |
|---|---|---|
| 同 op 重放→原 receipt（短路机制） | B→GAP-01 | A：completed 短路 CODE FACT（publish.rs:110-179；TEST FACT pc_model.rs:1096-1116）——仅机制面 |
| 同 op+同完整 fp→原 receipt（严格判定） | B→GAP-01/27 | **未实现→GAP-27**（短路不比对指纹；账本无指纹列，expected_content_hash 仅占位串——证据补充表 V1.0） |
| 同 op+异 fp→409 | B→GAP-27 | **未实现→GAP-27**（短路无条件返回原 receipt；总账 V1.2 §五 T4 已标实施差距，不新增编号） |
| 异 op+同内容并存 | A：无内容唯一约束（SCHEMA FACT） | A：同左（SCHEMA FACT；发布段专门用例缺位登记随 GAP-29/30） |
| 内容/基线变化→新身份 | B（指纹含 expected_version_no+九要素） | A：CAS 失配→failed→409→re-propose（CODE FACT publish.rs:264-317） |

- 九要素完整性=冻结（D6.6 V1.1 §4：title/rationale/origin_kind/source_id/governance_level/risk_level/confidence/expected_version_no/成员 pins）；现状 canonical hash 缺 source_id/risk/confidence（E-42）→GAP-27；
- proposal_content_hash 与 request_fingerprint 分离=冻结（职责分离，不可互代）；slot=op_id 槽位段不进指纹（键承载+强制校验补偿，场景 10/11）→校验器随 GAP-27；
- malformed op_id fail-closed 不落账本=场景 11（GAP-27）；CAS 失败/事务回滚/并发重试无重复执行=同事务模型+唯一键互斥+completed 短路（发布段 CODE FACT；起草段随 GAP-01）；
- 无内容唯一约束导致意外合并=不存在（SCHEMA FACT）。

## 四、零实施声明

本报告为文档交付：未修改代码/Schema/Migration/API/测试/数据库；未重开 Q-02a/b、Q-03、Q-11、Q-12、Q-04；未将 `approved→withdrawn` 表述为人工裁决。
