# 派发器 ↔ Runner 竞争缓解（D-1=b）验收记录 V1.0

> 归档 2026-11-08。本记录是 `ai-pm-v05-dispatch-competition-adjudication-record-v1.md`
> 的实施验收（对齐 R4 体例）。裁决 D-1=b / D-2=c 原文见裁决记录 §一。
> 实施 commit：`a1bd7f4`（feat）+ `f90e649`（docs）。

## 一、排除机制最终形态

**开关面**（读 env 不硬编码，`crates/pm-application/src/run_dispatch/mod.rs`）：

| env | 语义 | 缺省 |
|---|---|---|
| `AI_PM_RUN_DISPATCH` | `off/probe/on`，门控 worker 无人值守领取 | off |
| `AI_PM_RUN_DISPATCH_EXCLUDE_PROJECTS` | 逗号分隔 project code，命中项 worker 静默跳过 | 空（行为不变） |

**接线面**（本批关键，纠正 R4 记录「默认 On」的失实归因）：
- `run_executor.rs` 的 `claim_and_start` 此前**硬编码 `DispatchMode::On`**，worker 主循环
  读到的 `mode` 只打日志不参与门控 → 运行时恒派发。本批把 `mode` 透传进
  `claim_and_start`（`apps/ai-pm-worker/src/main.rs` → `claim_and_start(..., mode)`）。
- 排除项在 `claim_core` 阶段 A 候选发现用 SQL 过滤
  （`claim.rs`：`NOT EXISTS (SELECT 1 FROM core.project p WHERE p.id = r.project_id
  AND p.code = ANY($2::text[]))`）——命中项**不领、不审计、不推退避**（与 R4 演练期
  的 36 次 `task_not_ready` 审计噪声形成对照）。空列表走原查询，零行为漂移。
- **A-1 不变**：Model A（`pm_claim_task` / `claim_task_run`）经 `task_filter` 直入
  阶段 B，不经阶段 A 过滤，永不受开关与排除列表约束；`uq_task_run_active`、
  CodexExecutor spawn 通道均未动。

## 二、本机验证证据

| 项 | 结果 |
|---|---|
| 单测 `DispatchMode::parse` 矩阵（off/probe/on/缺省/未知） | 4/4 ✓ |
| 单测 `parse_exclude_projects`（逗号/空白/空项） + `excludes_project`（命中/未命中/精确/大小写） | ✓ |
| 集成 `excluded_project_run_is_not_claimed_by_worker`（排除→零领取、run 保持 queued、**零审计行**） | ✓ |
| 集成 `exclude_list_miss_and_empty_claim_normally`（未命中/空列表=现状一致） | ✓ |
| 集成 `model_a_explicit_claim_ignores_exclude_list`（A-1） | ✓ |
| `run_dispatch` 全套件 | 27/27 ✓ |
| `run_executor` 全套件（claim_and_start 签名改动回归） | 11/11 ✓ |
| pm-application 全量（lib 69 + 全部集成二进制） | **0 failed** ✓ |
| ai-pm-worker（main_loop_switch） | 3/3 ✓ |
| rustfmt（单文件，`--edition 2021 --check`） | 干净 ✓ |
| clippy（pm-application / ai-pm-worker `--no-deps`） | 我改动的文件零新告警（`claim.rs:954` 与 `devenv/mod.rs:194` 为存量告警，非本批引入）✓ |
| 零迁移核实 | 未新增 `migrations/` 文件，无迁移号占用 ✓ |

> 本机测试 `DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm`（aipm-pg-dev），
> 遵守 127.0.0.1 禁 localhost 硬规则。

## 三、本机栈演练说明

「同一 queued run + Runner 与 worker 并存」的竞争语义，在 claim 层由 §二的三个集成
测试直接钉死：排除命中时 worker 零领取零审计、Runner（Model A）独占领取；空列表时
`concurrent_claims_yield_exactly_one_claimed_run`（存量 27 项之一）保证先到先得互斥。
**完整 Docker 栈演练未执行**：见 §四阻塞（master 半提交，无法从干净树构建镜像）。

## 四、.10 应用 —— 暂缓（用户裁决 2026-11-08）

**阻塞**：干净 worktree（a1bd7f4）构建 `ai-pm-worker` 镜像失败：
`error[E0433]: cannot find snapshot_model in pm_ai`（`apps/ai-pm-worker/src/main.rs:81`）。
根因：已提交的 main.rs 引用未提交的 `crates/pm-ai/src/snapshot_model.rs` /
`crates/pm-application/src/snapshot_analysis.rs` / `migrations/0062_*`（并行会话
P2-13/PC-I6 WIP 半提交）。用户裁决：**暂缓部署，等并行会话先提交**。

**就绪配方**（master 可干净编译后照做，本批已备好 compose 透传与 `.env.prod` 位置）：
1. 干净 worktree（master）`docker build --target runtime-ai-pm-worker -t aipm/ai-pm-worker:0.2.0 .`
   （rsproxy DNS 抖动时 `--add-host lf3-static.rsproxy.cn:112.50.108.40 --add-host rsproxy.cn:111.63.62.234`）；
2. `docker save` / gzip / SFTP 至 `.10` → `docker load` → 覆盖 `aipm/ai-pm-worker:0.2.0`（同版本号重发，R3 体例）；
3. 同步 `infrastructure/docker-compose.ai-pm.yml`（本批已含透传）至 `.10:~/ai-pm/infrastructure/`；
4. `.10` 的 `.env.prod` 增补 `AI_PM_RUN_DISPATCH_EXCLUDE_PROJECTS=e2e-acp09`（`AI_PM_RUN_DISPATCH=off` 已在第 74 行）；
5. `docker compose up -d --force-recreate pm-worker ai-pm-worker`（compose 项目 `infrastructure`）；
6. 验证：`docker exec infrastructure-ai-pm-worker-1 env | grep AI_PM_RUN` 应见 off + exclude 列表；
   日志应无 e2e-acp09 的 `claim_task_run`/`skip_task_run`；lmms 八服务 healthy 冒烟。

## 五、残余风险与已知限制

1. **D-2=c 未实施**（本批只登记方向）：排除是静态负向白名单，无法表达「Runner 此刻在线才可派」
   的正向可用性；正向语义须等 D-2=c 把 Runner 心跳接入 claim 判定（可用性真源统一），
   并入 roadmap §9「调度入口」设计闭合。
2. **master 半提交态**：`5b212c6` 起 main.rs 引用未提交模块，干净构建不可行；属并行会话
   P2-13/PC-I6 在途工作，非本批引入。
3. **.10 未应用**：D-1=b 当前只存在于代码/测试/docs，`.10` 上 ai-pm-worker 仍是
   硬编码 `On` 的旧镜像（会继续对 e2e-acp09 派发并退避）；待 master 修复后按 §四配方上栈。
4. 排除列表**大小写敏感、精确匹配**（`core.project.code` 全文比对）；若 project code
   后续引入别名/改名，需同步维护该 env。
