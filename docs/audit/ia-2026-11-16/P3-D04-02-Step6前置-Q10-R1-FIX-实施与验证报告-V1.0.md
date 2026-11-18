# P3-D04-02 Step 6 前置 · Q10-R1-FIX — 实施与验证报告 V1.0

- 编制日期：2026-11-18
- 性质：**实施交付报告**——审批对象=四件套合并语义（V1.2 c562a20 + V1.2.1 531088e + V1.2.2 b0c86c4 + 审批附件 7d8909f）；实施批准=负责人实施指令（2026-11-18）
- 状态：**已实施待验收**（GAP-32 维持 OPEN 至负责人验收；本报告+总账 V1.10+关闭申请=验收输入）

---

## 〇、实施摘要

| 项 | 结果 |
|---|---|
| 修改面 | 产品 2 文件+测试 2 文件（§二）；**零越界**（publish.rs×2/migrations/触发器/close 外层结构/隔离级别/连接池全局配置全零改动，`git diff --stat` 证实） |
| 定向测试 | T1-T8e 矩阵 14/14 + q10_race_closure 2/2 + q10_r3_snapshot_check 2/2（**原样未动**，契约 6）；矩阵 3 轮重复全绿（10.2s/10.3s/10.4s，barrier 确定性实证） |
| 前端 | CloseReportOutcome +2 字段；vue-tsc 全量通过（exit 0）+AnalysisReportDetailView 26/26 |
| 全量回归 | workspace `--no-fail-fast`：208 目标 ok；5 FAILED 全部定性（1=指令豁免 c84_c85；4=干净 HEAD stash 复现的存量断裂；另 viz_seecode flaky 两轮一绿一红如实登记）（§六） |
| 泄漏 | `_sqlx_test_%` 清理后复测=**0**（§七） |

## 一、环境事实（与指令 §四.0 的偏差声明）

- docker daemon 2026-11-18 晚曾停止；本次启动后 **aipm-dev-postgres 容器幸存**（Exited 255 → start 即复，**非重建**）：6 schema（core/pc/projc/kb/integration/governance）在位、迁移链 0001→0037 无需重放、`max_connections=400`+`max_locks_per_transaction=512` 已在位（SHOW 实测）、启动时 `_sqlx_test_%`=0。
- 全程 `127.0.0.1:16433`（DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm'；零 localhost）。

## 二、实施内容（对照四件套逐项）

### 2.1 D1-D4 + R1 分类事务（crates/pm-application/src/analysis_report.rs，+224 行）

| 项 | 落点 |
|---|---|
| CloseOutcome II-A doc + `replayed_proposals`/`projc_replayed` | §三 doc comment=V1.2 §三→V1.2.1 R4.3 双来源归类→V1.2.2 §四 failed 口径定稿的合并文本；两字段插入 applied/projc_applied 之后 |
| `ClassifyOutcome{Replay,Failed,FailedTech{kind,error,cleanup_error}}` | doc 注明不变式 I-1/I-2/I-3；cleanup_error=V1.2.2 §1.1 双故障保留 |
| `classify_after_publish_err`（核心，私有） | BEGIN→advisory→单条 JOIN→COMMIT；F1-F5 五失败点=V1.2.1 R1.2 逐项落位（F3 显式 rollback+双故障 kind=`join_and_rollback`；F2 drop 清理；F4 行未采信） |
| `classify_pc_after_publish_err`/`classify_projc_after_publish_err`（pub，T8 L2 直调封装边界=V1.2.2 §三 T8-a-1） | 两域同构包装 |
| advisory 语句 | **逐字复制** pc/publish.rs:117 / projc/publish.rs:118（含 `::bigint` 括号形态零差；常量 `PC/PROJC_PUBLISH_ADVISORY_SQL`） |
| 分类 JOIN | V1.2 §2.1 形态（与 q10_r3_snapshot_check.rs 契约文本同构；operation_id='pc/projc-decision:'‖p.id::text） |
| pc 段/projc 段 Err 分支 | 统一入口=分类事务；FailedTech → `tracing::warn!(kind, proposal_id, source_publish_error, error, cleanup_error, "R-1 classify tx technical failure")`+计数 failed（技术故障绝不产生 Replay） |
| Ok 分支拆分（R4） | `receipt.replayed → replayed_proposals+=1 否则 applied_proposals+=1`（两域） |

### 2.2 前端（ai-frontend/src/api/analysis-reports.ts，+4 行）

`CloseReportOutcome` 增 `replayed_proposals`/`projc_replayed` 两行（注释=II-A 双来源口径）。

### 2.3 测试

- **新增 `tests/q10_classify_matrix.rs`**（14 用例=T1-T8e 矩阵载体，§四）。
- **`tests/q10_race_closure.rs` 场景 C 断言升级**（V1.2 §六：`applied+replayed+failed ≤ 1` 单提案至多一个计数事件；T2 语义重述入注释）。
- **`tests/q10_r3_snapshot_check.rs` 原样保留**（`git status` 证实未改动）。

### 2.4 零改动面核实

`git status`：仅上述 4 文件。pc/projc publish.rs、migrations/*、触发器、close 外层事务结构、隔离级别、连接池全局配置——全部未触碰。**审计 payload（`analysis_report.closed` 动作）未变更**——不在四件套批准 diff 范围（V1.2 §六）内，未顺带扩展（遗留观察：审计轨迹现不含 replayed 计数，如验收方认为需要，作为独立微批另行授权）。

## 三、与审批对象的两处**实施期测试设计发现**（不触碰产品代码与硬边界，登记待验收方确认）

### 3.1 T5 注入机制替换：pre-advisory Forbidden（非 L3-Forbidden）+ 不可能性证明

指令与 V1.2 §五 T5 名义注入=L3-Forbidden。实施期证明：**L3-Forbidden 端到端下无法确定性产出 classify-Replay**——

1. L3 检查发生于 publish 事务**内部**（已持 advisory，publish.rs:356-376）；竞争者完成发布必须获取同一 advisory；
2. 竞争者要在「close 的 publish Err 之后、分类 JOIN 之前」完成，只能经 advisory FIFO 排队在 close 的 publish 之前入队；而竞争者入队期间其自身 L3 检查与 close 的 L3 检查读到**同一已提交冲突状态**（两次检查之间无合法写入者——resolve 不经 advisory 但其提交时点无法约束在 close 的检查之后且竞争者检查之前）→ 两检查同判 → close 若 Err 则竞争者亦 Err；
3. 竞争者「resolve→publish 重试」与 classify 对 advisory 的竞争 = 跨任务时序竞态（V1.2.2 §三明确放弃该类时序猜测）。

故 T5 采用 **pre-advisory Forbidden**（close 主体=无治理授权真人 STRANGER，`ensure_formal_authorized` 在 `pool.begin()`/advisory 之前拒绝，publish.rs:112 代码序保证）：Err 与竞争者发布的先后由「测试持 advisory → 竞争腿入队（观测 barrier）→ close 入队 → 放行」的 FIFO 因果序唯一确定，分类 JOIN 必见 (applied,completed)→Replay。**II-A 语义覆盖等价**（Err→分类→Replay 全链路真实执行）；**L3-Forbidden 的 Err/分类 Failed 路径端到端由 T4 覆盖**。

### 3.2 T7 字面 (1,1,1) 不可满足证明（CAS 耦合墙）+ 可达组合承载

close 列出的提案必须创建于 close 之前（approved_ids 按 source_id 清点），其 `expected_version_no`=创建时项目 current 版本号 C。单 close 内「首发（applied）」与「竞争者完成→短路 replay」各需**一个真实发布**过 CAS（current==expected），而首个真实发布使 current=C+1，另一提案 CAS（expected=C）必败——**同代两真实发布互斥**；replayed 仅可能来自同一提案的短路（=T2 形态，applied=0）。故 V1.2.1 R4.3 的「三提案=1 首发+1 短路 replay+1 CAS→(1,1,1)」在产品 CAS 语义下**数学不可满足**（该条为方案层预期，未核对 CAS 耦合）。

T7 承载（同测试两项目两 close，三计数路径全覆盖、各 close 计数 II-A 正确）：

| close | 构成 | 实测 |
|---|---|---|
| 项目 A（close 先行） | 载体发布 v1 → first_a（expected=1，发布时 current=1 过闸）applied；cas_a（基线 0，CAS 1≠0 败→failed,failed→分类 Failed） | **(1,0,1)** |
| 项目 B（T2 机制） | 竞争者完成 replay_b（0==0→v1）→ close 对 first_b 真实发布 CAS 败（耦合墙）→分类 Failed；对 replay_b 短路→replayed | **(0,1,1)** |

（文件头 doc comment 已登记本发现；两 close 合计 (1,1,2)。）

### 3.3 其余 T 项与矩阵逐字对应

T1/T3/T4/T6 与 V1.2 §五 一致；T2 预期=applied=0,replayed=1（R4 修订后）；T8-a-1（L2 F1 直调）/T8-a-2（L3 小池行为级）/T8-b/c/d（L1 三语句边界 terminate）/T8-e（B.2 五步）按 V1.2.1 R3+V1.2.2 §三 落位。

## 四、测试矩阵与实测结果

| # | 场景（层） | 确定性机制 | 断言核心 | 结果 |
|---|---|---|---|---|
| T1 | 正常发布（L3） | 单线程 | (1,0,0)；v1；applied+completed；closed | ✓ |
| T2 | 时序 1 短路 replay（L3） | 持 advisory+竞争腿先入队（观测 barrier ≥1）→close 入队（≥2）→放行 FIFO | (0,1,0)；v1；closed | ✓ |
| T3 | CAS 提交型 Err（L3） | 载体 v1+目标基线 0 | (0,0,1)；(failed,failed)；提案 failed；closed | ✓ |
| T4 | 回滚型 Err L3-Forbidden（L3） | 预置未解决冲突 | (0,0,1)；提案保持 approved；无账本行；closed | ✓ |
| T5 | 时序 2 端到端→分类 Replay（L3） | pre-advisory Forbidden（§3.1）+FIFO barrier | (0,1,0)；v1；closed | ✓ |
| T6 | 分类持锁互斥（协议级） | BEGIN+P+同款 JOIN 保持打开 | 持锁期发布者未完成；释放后 Ok(replayed=true)；v1 | ✓ |
| T7 | 混合计数（L3×2） | §3.2 两项目组合 | (1,0,1)+(0,1,1) | ✓ |
| T8-a-1 | F1 直调（L2） | 1 连接池占位+300ms 超时 | FailedTech{kind:"begin"} | ✓ |
| T8-a-2 | 小池耗尽容错（L3） | 1 连接池+300ms 跑完整 close | (0,0,1)；closed；不 panic；提案态未变 | ✓ |
| T8-b/c/d | L1 三语句边界 terminate | 协议同款事务+同线程边界+自终止 | 各边界语句报错；会话回滚（pid 消失）；**advisory 服务端释放**（lock_timeout 探针）；F5 数据库形态（死连接 rollback 亦报错） | ✓ |
| T8-e | Pool 归还路径剔除实测（附件 B.2 五步） | 被测 Pool：取连接 C→独立连接 terminate→C 语句失败+drop 归还→循环重取 | 10 次重取全部健康且 PID≠已终止 PID——**sqlx 0.8.6 连接淘汰由实测背书**（V1.2.2 §1.2 降级项闭合） | ✓ |
| L2-F2/F3 | 错误映射（L2） | statement_timeout 注入+持锁/ACCESS EXCLUSIVE 阻塞 | kind="advisory"（drop 清理）/ kind="join"+cleanup_error=None（显式回滚成功）；**I-3 释放探针**=F3 回滚后 advisory 空闲 | ✓ |
| L2 映射 | pc/projc 同构 | 未发布→Failed；发布后→Replay | I-1 唯一来源实证 | ✓ |
| T1-projc | projc close 首发（L3） | projc-only 报告 | projc_applied=1/projc_replayed=0；applied=0 | ✓ |

**稳定性**：`q10_classify_matrix` 连续 3 轮全绿（10.19s/10.31s/10.38s）——barrier 无 sleep、无跨线程时序假设，确定性成立。

## 五、登记事项（按审批附件）

1. **T8-e Pool 恢复实测证据**（附件 B.2 要求）：五步实测全过（§四 T8-e 行）——「sqlx 0.8.6 连接淘汰」自本报告起**有实测背书**；正确性仍不依赖之（F1→failed+warn 兜底）。
2. **F2 清理限制声明**（补充 A，如实沿用）：F2（advisory 语句失败）走 drop 清理——sqlx 0.8.6 的 Transaction drop 触发回滚发送但不 await 完成，清理阶段错误**不可同步捕获、不进入 FailedTech**；本实施不声称已确认回滚成功；锁最终释放由 PostgreSQL 服务端会话中断兜底（L1 T8-b/c/d 实测佐证）；计数口径不变（failed+warn）。维持 drop+服务端兜底组合，未扩大。
3. **DDL 运行约束登记**（附件 B.3）：分类事务持表级 ACCESS SHARE（pc.proposal/pc.write_operation 及 projc 同名两表）期间，取得 ACCESS EXCLUSIVE 的 DDL（ALTER/DROP 等）与之互斥——**运行约束：迁移仅在应用实例停止或维护窗口执行**（现有部署惯例即如此）。请随 GAP-32 关闭申请在运维文档同步登记。
4. **failed 口径**（V1.2.2 §四定稿）已按原文落于 CloseOutcome doc comment：failed=①业务失败+②分类技术故障（无法确认≠已观察未发布）；技术故障经结构化 warn（kind/error/cleanup_error）区分。

## 六、全量回归

- 命令：`RUSTC_WRAPPER="" DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' cargo test --no-fail-fast --workspace`（workspace 实际 crate=kb-pc-contracts/pm-domain/pm-ai/pm-application/pm-infrastructure/pm-mcp；指令所列 pm-api 为部署容器名，仓库无该 crate——回归面取 workspace 全量=覆盖并超出指令清单）。完整日志两轮留证（第 1 轮 tail 截断致 4 个目标名丢失，第 2 轮完整落盘复跑）。
- **结果（第 2 轮，完整日志 /tmp/full_reg.log）**：**208 个目标 ok；5 个目标 FAILED，逐一分类如下**——

| 失败目标 | 定性 | 与本次实施的因果判定 |
|---|---|---|
| `-p pm-mcp --test contract`（c84_c85_submit_result_lands_the_delivery_and_replays） | 指令 §四.3 明示豁免（已知 git 环境依赖） | 无关 |
| `-p kb-pc-contracts --test guard_ai_container_users`（ADR-06a 静态扫描：`reference_targets.rs` 命中 `ai.project_document` 不在白名单） | **存量断裂（干净 HEAD 复现）** | 无关：该文件与守卫白名单均久未变更（bb33ea6/d62710b 时代）；本次 diff 四文件零命中两表名 |
| `-p pm-ai --test context`（start_run_creates_analyzing_row_from_config：unresolved≠ctx-fake-model） | **存量断裂（干净 HEAD 复现）** | 无关：pm-ai 域，本次 diff 零触碰 |
| `-p pm-ai --test discovery_writeback`（2 用例 Forbidden "require a human principal"） | **存量断裂（干净 HEAD 复现）** | 无关：同上 |
| `-p pm-application --test projc_mirror_guards`（3 用例 FK 23503：cognition_revision_document_fkey / cognition_version_confirmed_by_fkey） | **存量断裂（干净 HEAD 复现）** | 无关：测试夹具明言「projc 零外键：document 侧引用可为任意 uuid」（写于 0037 之前）——**GAP-38-B 的 0037 加 79 条 FK 后夹具未同步更新**；随机 UUID 播种现必违 FK |

- **存量断裂的决定性验证协议**：`git stash push -u`（摘除本次全部改动）→ 干净 HEAD=7d8909f 上复跑 4 个目标 → **同 panic 同行号逐字复现**（guard/context/discovery_writeback/projc_mirror_guards 全部 FAILED）→ `git stash pop` 恢复。即：4 个失败在本次实施**之前**已存在于 HEAD，与 Q10-R1-FIX 零因果。
- **本次实施所在 crate 面零失败**：pm-application 全套除上述 3 个存量用例外全绿（含 q10_classify_matrix 14/14、q10_race_closure 2/2、q10_r3_snapshot_check 2/2、既有 analysis/acp/pc/projc 全族）。
- **viz_seecode_adapter（真 Node+真 SeeCode 出图，外部渲染管线）**：flaky 确定性断言（两次渲染 SVG 不一致）——第 1 轮 FAILED（878s）、单跑 FAILED（776s）、**第 2 轮 PASSED**；失败点=SeeCode 渲染输出非确定性（外部 Node 管线），与本次改动（close 计数路径）无代码交集；登记为环境类 flaky，不在指令豁免清单内、如实报告待验收方定性（非 DB/锁/事务面）。
- flow_01：第 2 轮全绿（未触发并行网络抖动豁免）。

## 七、泄漏复测

`SELECT count(*) FROM pg_database WHERE datname LIKE '_sqlx_test_%'` 清理后复测=**0**（启动时=0；两轮回归后各泄漏 7/9 个，均按无活动连接清单 DROP WITH (FORCE) 清空）。

## 八、零越界声明

未改 publish.rs 两域、migrations/*、触发器、close 外层事务结构、隔离级别、连接池全局配置、q10_r3_snapshot_check.rs；未新增锁机制/键（advisory 键=既有 pc/projc_publish 两键，获取语句逐字同 :117/:118）；外层 close 任何时刻不持有 advisory（分类事务=短命独立事务）；未顺带重构；GAP-32 由总账 V1.10 登记为**已实施待验收**（OPEN 维持至验收）。
