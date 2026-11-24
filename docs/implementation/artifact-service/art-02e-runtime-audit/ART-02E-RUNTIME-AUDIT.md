# ART-02E — Artifact Service 目标环境运行态核验报告

| 元信息 | 值 |
|---|---|
| 任务 | ART-02E（前置核验链 ART-02A~D 之后） |
| 审计性质 | 只读（仅 SELECT / SHOW / inspect；无写操作、无迁移执行、无部署变更） |
| 执行时间 | 2026-11-24 06:57–07:04 **UTC**（数据库时区 Etc/UTC；本地时区 +08:00） |
| 环境类型 | **development —— 本机 full-dev 全栈部署（Compose 项目 `superpowers-full`）**，非生产 |
| 数据库 | 库名 `postgres`，用户 `postgres`，PostgreSQL 16.16（容器 `superpowers-full-postgres-1`，无宿主端口，经 docker exec 只读访问） |
| AI-PM 部署版本 | 镜像 `superpowers-full-pm-api` 构建 2026-11-24T02:43:56Z；源码状态 ≈ `remediation/integration` @ `9a1adb3d`（BEST-EFFORT 推断，详见 [ART-02E-ENVIRONMENT.md](ART-02E-ENVIRONMENT.md) §5） |
| Migration 最大版本 | **59（discovery round error），59/59 全部 success** |
| 文档基线 | `docs/artifact-service-baseline-v1` @ `fca8e279`（PR #6） |
| 配套文档 | [ART-02E-ENVIRONMENT.md](ART-02E-ENVIRONMENT.md) · [ART-02E-SCHEMA-CHECK.md](ART-02E-SCHEMA-CHECK.md) · [ART-02E-ROWCOUNTS.tsv](ART-02E-ROWCOUNTS.tsv) |

## 1. 核验矩阵（总览）

| 核验项 | 判定 | 说明 |
|---|---|---|
| 目标环境身份 | **VERIFIED** | full-dev 部署库（库名/用户/容器/Compose 项目均实证；多 PG 实例已全量识别并排除） |
| 数据库身份查询 | **VERIFIED** | current_database/user/version 实时取得（§2） |
| `_sqlx_migrations` 状态 | **VERIFIED** | 59 条全部 success，0 条失败（§3） |
| 四表存在性 | **VERIFIED** | viz schema 恰好 4 张表，无多余表 |
| 四表实时行数 | **VERIFIED** | 全部为 0（§4） |
| 表结构/约束/索引/触发器 vs 0020 | **VERIFIED（一致）** | 全项吻合，schema drift = 无（见 SCHEMA-CHECK） |
| 部署版本 | **VERIFIED（BEST-EFFORT 口径）** | 镜像无 git 元数据，以构建时间线 + 0059 存在性交叉推断为 `9a1adb3d` |
| 与 PR #6 master 基线一致性 | **VERIFIED** | merge-base = `3a50c7cd` = 当前 master；部署领先 2 个 UI 提交，migrations/ 范围内完全一致 |
| 生产环境实时状态 | **NOT VERIFIED** | 本机无生产实例；本报告不得被引用为生产结论 |
| 仓库外部消费者 | **NOT VERIFIED** | 本轮未核验（继承 ART-02 开放项） |
| tenant/project 复合约束、跨聚合 FK | **NOT FOUND** | 与 0020 设计口径一致，非漂移（SCHEMA-CHECK §3） |

## 2. 数据库身份（实时查询原文）

```text
database_name = postgres
database_user = postgres
checked_at    = 2026-11-24 06:59:05.526154+00
postgres_version = PostgreSQL 16.16 (Debian 16.16-1.pgdg13+1) on x86_64-pc-linux-gnu
timezone      = Etc/UTC
```

环境身份结论：**可确认**。该库为 full-dev 部署（Compose 项目 `superpowers-full`）内的 PostgreSQL 实例，与 ART-02 §13 记录的 2026-11-15 历史快照（同容器名 `superpowers-full-postgres-1`、同库名、同用户）为同一部署环境的后继状态。本机不存在生产实例。

## 3. Migration 状态（VERIFIED）

| 项 | 值 |
|---|---|
| 总条数 | 59 |
| 失败条数 | 0（success=false 查询为空） |
| 最大成功版本 | **59 — discovery round error**（installed 2026-11-24 02:54:56Z） |
| 安装时间线 | 全库于 2026-11-23 16:03:49Z 起初始化（1~58 在 16:03:49–54Z 内批量应用）；59 号于 2026-11-24 02:54:56Z 由新镜像首次启动时应用 |
| Top 10 | 59 discovery round error / 58 external commit adoption / 57 workspace reclamation totals / 56 workspace reclamation bytes / 55 workspace reclamation ledger / 54 task run cleanup claim fencing / 53 device single active run / 52 issue risk blocker / 51 pa05 work item / 50 task code —— 均与仓库 migrations/ 文件名逐一对应 |

版本号与部署源码吻合：`9a1adb3d` 已含 `0059_discovery_round_error.sql`，且库内最大版本恰为 59 —— 交叉印证部署版本推断。

## 4. 四表实时行数（VERIFIED）

| 表 | 行数 | 判定 |
|---|---|---|
| viz.artifact | **0** | VERIFIED |
| viz.approval | **0** | VERIFIED |
| viz.render_output | **0** | VERIFIED |
| viz.slot_config | **0** | VERIFIED |

逐表独立查询，四表均存在且可查询；未读取任何业务行。明细见 [ART-02E-ROWCOUNTS.tsv](ART-02E-ROWCOUNTS.tsv)。

## 5. Schema 一致性（VERIFIED，无 drift）

库内 viz schema（4 表 57 列、4 PK、19 CHECK、2 FK（均 convalidated）、4 unique 索引、2 辅助索引、2 守卫触发器 + 2 函数）与 `migrations/0020_visualization_artifact.sql` **全项一致**；0021~0059 无一触碰 viz.*。逐表明细与六项重点核验见 [ART-02E-SCHEMA-CHECK.md](ART-02E-SCHEMA-CHECK.md)。

**Schema drift：未发现（INCONSISTENT = 0）。**

## 6. 与 2026-11-15 full-dev 历史快照对比

| 维度 | 2026-11-15 快照（ART-02 §13/§14.4，VERIFIED-HISTORICAL） | 本次实测（VERIFIED） |
|---|---|---|
| 容器 / 库 / 用户 | superpowers-full-postgres-1 / postgres / postgres | 相同标识 |
| migrations | 1~24 已执行 | 1~59 全部执行（**库已重建**：2026-11-23 16:03Z 重置后批量重放） |
| viz.* 四表行数 | 全部 0 | 全部 0 |
| 代码版本 | `05ce33d8`（2026-11-15） | ≈ `9a1adb3d`（2026-11-24） |

解释边界：
1. 两次盘点均为 **full-dev 开发环境**数据，均不能外推为生产状态；生产实时行数仍为 **NOT VERIFIED**。
2. 本库于 2026-11-23 16:03Z 重建（DB 卷删除重置），2026-11-15 快照与本次实测之间**不存在连续数据谱系**；快照记录当时即为 0 行，故「重置丢失 viz 数据」不成立，但该库此前若有过数据也已随卷销毁——任何「历史归档」主张都必须指向卷外备份，本轮未核验到任何 viz.* 专用备份（NOT VERIFIED）。

## 7. 迁移风险判断（任务 §九，可多选）

| 选项 | 判定 | 依据 |
|---|---|---|
| **A. 可进入 ART-03 独立仓库创建** | **✅ 成立** | 环境身份可确认、schema 与迁移全绿、无 drift、无阻塞项 |
| **B. 可开始 Artifact Service 开发，暂不迁移旧数据** | **✅ 成立** | 四表 0 行，本环境无存量数据可迁；开发不依赖任何数据迁移 |
| C. 需先修复数据库结构或历史数据问题 | ✗ 不成立 | 结构无 drift、迁移 59/59 成功、无失败迁移记录 |
| **D. 须先确认生产环境或外部消费者** | **⚠ 作为前置条件保持开放** | 生产实时状态 NOT VERIFIED；仓库外部消费者 NOT VERIFIED。凡涉及旧表退役/生产迁移动作，必须先关闭本项 |
| E. 环境无法确认 | ✗ 不成立 | 环境身份已实证（§2） |

**综合结论：A + B 同时成立 —— ART-03 可以启动，Artifact Service 开发可以开始；真实数据迁移与旧表退役继续冻结，直至 D 条件（生产环境确认 + 仓库外消费者确认）关闭。**

### 任务 §九「特别注意」四项逐条回覆（即使四表 0 行也不得直接删表）

| 确认项 | 状态 | 说明 |
|---|---|---|
| 仓库外部消费者 | **NOT VERIFIED** | 仓库内消费者已确认存在（`pm-application/src/visualization/*`：artifact_service、queries、render_job、adapter_contract + pm-domain + 3 个测试模块）；仓库外消费者本轮未核验 |
| 备份 / 历史归档 | **NOT VERIFIED** | 本轮未发现 viz.* 专用备份；已知的唯一历史记录是 ART-02 引用的 2026-11-15 盘点快照（0 行）；2026-11-23 卷重置前的任何数据已随卷销毁且无归档证据 |
| JSONB 内部引用 | **不适用（当前）** | spec / provenance 为 jsonb 全文快照、可内嵌任意引用，设计上存在内部引用可能性；当前 0 行，无实际引用；ART-03 数据模型设计须显式处理该载荷 |
| 正在运行的旧版本服务 | **VERIFIED（本机无）** | 本部署为单套当前版本（pm-api/worker 同批构建），无并行旧版本服务；外部系统的旧版本服务 NOT VERIFIED（归入 D 条件） |

## 8. 局限性声明

1. 本报告全部结论仅适用于 **full-dev 开发环境**，不得描述或引用为生产环境结果。
2. 部署版本为 BEST-EFFORT 推断（镜像不含 git 元数据），误差窗口 ≤ 5 分钟且 migrations/ 范围内无歧义。
3. 行数为时点快照（2026-11-24 06:59 UTC），不构成持续承诺。
4. 生产环境、仓库外消费者、备份归档三项维持 NOT VERIFIED，是 ART-03 之后任何迁移/退役动作的硬前置。

## 9. 交付物

| 文件 | 内容 |
|---|---|
| `ART-02E-RUNTIME-AUDIT.md` | 本主报告（核验矩阵 + 迁移风险判断） |
| `ART-02E-ENVIRONMENT.md` | 环境识别（Compose/容器/多 PG 实例/部署版本/基线一致性） |
| `ART-02E-SCHEMA-CHECK.md` | 库内实时 schema vs 0020 逐项比对 |
| `ART-02E-ROWCOUNTS.tsv` | 四表行数机读证据 |
