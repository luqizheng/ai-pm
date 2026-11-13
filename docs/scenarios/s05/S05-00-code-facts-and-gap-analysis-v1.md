# S05 现状对照与缺口分析 V1.0 —— AI Agent 工程执行与协作

| 项目 | 内容 |
|---|---|
| 文档性质 | S05 后台服务开工前的代码事实对照（禁重建区红线 + 真实缺口清单 + 排期映射） |
| 日期 | 2026-11-10 |
| 代码基线 | ai-pm master @ 89c3db63（worktree feat/s05-execution-backend） |
| 输入 | S05 蓝图（S05-01~08 工作包 + P1~P4 阶段）· `docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md` · `ai-pm-v2-code-alignment-verification-v1.md` · `docs/scenarios/s03/ui/acp/`（V1.3.1 冻结基线）· `docs/reference/ai-pm-migration-ledger.md` |
| 迁移基线 | 0001_baseline（squash 后）+ 0002~0011 已应用；0012（system embedding config）为 master 在途未提交 |

---

## 一、总结论

**S05 不是绿地。** 蓝图中八个工作包的主干在仓库里已实施并通过真实栈 E2E（V0.5 WP-3/4/5 ✅、ACP-01~07 全链落库）。S05 后台服务的正确姿势是 **收敛 + 补齐**，不是新建体系：

1. Task/Run 分离、13 态任务机、Run 7 态 + 租约/fencing/deadline、双通道领取（MCP Model A / Runner spawn Model B）、Worker 命令通道、Continuation 恢复链——全部已存在。
2. **禁重建区**（`ai-pm-v2-code-alignment-verification-v1.md` §四，直接约束 S05 设计）：
   不建 `task_execution`/`execution_result` 新表（用 `delivery.task_run` + `task_repository_commit` + manifest 视图）；不改任务 13 态；不动 MCP 四工具契约；`WAITING_FOR_*` 不做成状态（用 `awaiting_decision` + `task_waiting.reason`）；不新造第二套 Client Identity（V-05-1 已落库）。
3. 蓝图 S05-01~08 中的「Waiting 状态」「执行实例管理」等表述须归并进既有语义（对照表见 §二），不新造平行状态机。

## 二、S05-01~08 逐包对照

| 工作包 | 现状（既有实现，file 级） | 真实缺口 |
|---|---|---|
| S05-01 Agent 接入与注册 | Client Registry V-05-1 已落库（0049→baseline：`devenv.device` 凭据四列/cpk_ 凭据/is_runner + `task_run.lease_client_id`）；凭据签发/撤销 `pm-application/src/device_credential.rs`；Runner `apps/ai-pm-runner/src/profiles.rs` 已含 **zcode/codex/dsh/custom** 四 adapter 模板；MCP 通道天然多客户端（`crates/pm-mcp`） | ① Claude Code / Trae 的 spawn profile 扩展（模板机制现成）；② spawn 适配器族扩展 = V2.0-F 增量 |
| S05-02 执行上下文注入 | S04→S05 交接已实现：`pm-application/src/task_plan.rs::prepare_handoff`（五重资格检查 + 标准执行上下文载荷 + 追溯链）；Run 契约 `run_context.rs`（CONTRACT_VERSION=1，契约字段回指库表，不拼装）；MCP `pm_get_task_context` / `pm_get_run_context`；任务级知识引用 `delivery.task_knowledge_ref`；kb retrieval 四通道降级链完整 | 执行侧知识注入编排（kb retrieval 通道 × `task_knowledge_ref` → run 上下文载荷）；Vector 通道仍在 G-3 门控（`platform.embedding_config` 0012 已配置先行） |
| S05-03 执行实例管理 | Run 7 态（queued/claimed/running/succeeded/failed/timed_out/cancelled）+ 租约 13 列 + generation fencing + 绝对 deadline + 双心跳；调度 `run_dispatch/{claim,lease,policy,transition}.rs`；命令通道 `worker_command.rs`（issued→delivered→acked\|failed，同 id 幂等重投）；恢复 `recovery.rs` / `run_cleanup.rs`；REST `/v1/pm/task-runs` | 基本齐。缺口=库内过程事件无承载（→ 本批 0013 落地，见 §三） |
| S05-04 多 Agent 协作 | 并行执行：`uq_task_run_active` 每 Task 一活跃 Run + SKIP LOCKED 双 worker + review 分轨（0026）；竞争缓解 D-2=c 已裁（Runner 心跳=派发可用性真源，`ai-pm-v05-d2c-runner-availability-design-v1.md`）；多 Client 隔离：跨项目 NotFound 防探测 + Run 授权快照 ∩ 实时 verified 交集（`repo_access.rs::execution_allows`） | ① 调度策略层 V2.0-E（可用性过滤 + 按 Role/Capability 自动匹配执行者，现 assignee 人工指定）——**Q-D2C-1~6 待裁后方可实施**；② 多 Run 同仓冲突的 Workspace 锁语义 |
| S05-05 执行过程监控 | `GET /v1/pm/task-runs`（列表+失败摘要）与 `/{run_id}`（租约/执行器/时间线/仓基线）已有；但库内无 run 级事件表，过程时刻散落 audit_log / outbox / 执行端磁盘 jsonl | **本批落地**：`delivery.task_run_event` 最小事件账（0013）+ 打点接线 + `/{run_id}/events` 与 `/summary` 查询面（roadmap §9 最小方案，不做大型事件溯源） |
| S05-06 工程成果管理 | 逐仓 commit 事实 `delivery.task_repository_commit` + Run 集成状态/integration_status + `v_task_delivery_manifest` 对账视图 + `task_run_repository` 仓基线快照；`task_repository` 生产写入口（原 P0 空洞）已由 `task_repository_binding.rs` + `task_plan.rs`（正式化写入）+ REST `task_repositories.rs` 补齐（WP-C 完成） | 基本齐。增量=交付清单与 Role Acceptance Criteria 挂接（V2.0-H） |
| S05-07 人工介入与恢复 | ACP-01~07 全链落库（baseline：`task_interaction` / `interaction_response` / `interaction_response_delivery`（0050 一回复一账）/ `continuation`（0051 open→started→consumed/closed/frozen）/ `task_checkpoint`（0054 ACP-06）/ `task_run_execution_authorization`（0054 ACP-07 Deadline 重授权）+ Recovery Foundation（0052）；等待语义=`awaiting_decision` + `task_waiting.reason`（不做 WAITING_FOR_* 状态）；REST 回复入口 ACP-03 已实施 | ACP-09 完整回归（真实栈：离线/重启/超时/迟到 ACK/多次介入）未跑——P2 收口项 |
| S05-08 交付验收与知识回写 | 蓝图三态「Agent 声称完成 / AI-PM 检查通过 / 人工验收」= 现有 `submitted`（E1 守卫）→ review 分轨 verdict 三值（approved/changes_requested/inconclusive）→ `completed`（E2 验收证据守卫）——**已区分，无需新状态**；验收判据逐条 `delivery.acceptance_criterion_verdict`；知识红线 propose/confirm/veto + 质量门已跑（`pm-mcp` tools） | 增量=验收条件与 Role 的 Acceptance Criteria 挂接（V2.0-H）；执行结果回写知识的编排入口 |

## 三、本批已落地（S05-05 监控事件面）

- `migrations/0013_task_run_event.sql`：追加式事件账（幂等键 `UNIQUE (run_id, event_type, lease_generation, dedupe_tag)`；只记无专表承载的生命周期时刻；interaction/continuation/delivery 有专表不重复落账；review Run 不落账——D5 平台内确定性检查无外部 Agent）。
- 打点接线（与主操作同事务，`pm-application/src/run_events.rs` 唯一入口）：claim / 续领（recovery）/ MCP 首报 started / 阶段推进 / spawn 启动 / 启动失败退避 / 基础设施耗尽 / record_termination / finish_run / lease 失联隔离 / cancel_run / 归档批量取消 / worker_command 四态。
- REST：`GET /v1/pm/task-runs/{run_id}/events`（分页+type 过滤）、`GET /v1/pm/task-runs/summary`（项目级聚合）。
- 详见 `docs/scenarios/s05/S05-run-events-monitoring-report-v1.md`。

## 四、P1~P4 排期映射（蓝图 ↔ 既有规划体系）

蓝图 P1~P4 与仓库既有排期载体（`ai-pm-v05-execution-roadmap-v1.md` 五阶段关键路径 + ACP 切片 + WP-A~D + V-05-* 裁决）的映射——**不新造平行排期体系，后续 S05 批次在 roadmap 上顺延**：

| 蓝图阶段 | 对应既有载体 | 状态 | S05 残余工作 |
|---|---|---|---|
| P1 单 Agent 完整执行闭环 | V0.5 WP-3（Runner）/ WP-4（平台调度）/ WP-5（介入恢复）+ ACP-01~05 E2E PASS | **已达成** | 监控数据面（本批 0013）收口 |
| P2 执行管理与人工介入 | ACP-03/04/05（已实施）+ ACP-06/07（0054 已落库） | 骨架已落地 | ① ACP-09 完整回归（真实栈）；② 监控查询面消费验证（本批） |
| P3 工程成果与验收闭环 | D4 delivery manifest（0022）/ D5 review（0026）+ 验收判据 verdict（0002/0026） | 主体已落地 | 验收条件 ↔ Role 挂接；执行结果知识回写编排（与 S05-02 注入同链路） |
| P4 多 Agent 协作 | V2.0-E 调度策略层 + D-2=c 竞争缓解设计（`ai-pm-v05-d2c-runner-availability-design-v1.md`） | 设计在途 | **前置：裁决 Q-D2C-1~6** → 可用性过滤 + 能力匹配 + Workspace 并发语义 → Claude/Trae profile（S05-01 增量） |

### 后续批次建议顺序

1. **S05-B2**：ACP-09 完整回归（P2 收口）+ Claude Code/Trae spawn profile（低风险模板扩展，需真实 CLI 联调）。
2. **S05-B3**：执行侧知识注入编排（kb retrieval × task_knowledge_ref → run 上下文），承接 DK-C 验收口径。
3. **S05-B4**：Q-D2C-1~6 裁决清单整理 → 调度策略层实施（P4 起步）。

## 五、红线重申（S05 全批次适用）

1. 不建 `task_execution` / `execution_result` 类新表；执行事实只认 `delivery.task_run` 系。
2. 不改任务 13 态 / Run 7 态 / MCP 四工具契约；事件面、监控面是**新增旁路**，不改变任何状态机语义。
3. `WAITING_FOR_*` 需求一律进 `task_waiting.reason` 枚举值，不做成状态。
4. Agent 身份一律走 V-05-1 Client Registry，不新造身份层。
5. 「事件账 ≠ 执行记录重建」：`task_run_event` 只做监控观测面，不参与任何状态判定/守卫逻辑。

---

**文档结束**
