# 测试耗时画像与提速（2026-11-11 实测）

**结论：全量墙钟 421s → 211s（-50%），零产品代码改动。** 两处杠杆：测试库
durability 配置（-166s）与 git 夹具改用 `fast-import`（-41s）。

## 1. 怎么用

```bash
bash scripts/setup-test-db.sh              # 一次性：测试库调优（迁移 380ms→65ms）
bash scripts/test-timing.sh                # 全量画像（编译+执行分阶段，per-test 计时）
bash scripts/test-timing.sh -p pm-application --test docproc_pipeline

cargo nextest run --profile fast           # 本地内环：排除真远端 SSH 套件
cargo nextest run                          # 全套（含 e2e_flow 真远端）
```

产出：`target/test-timing/{nextest.log,summary.txt}`（per-test 计时、套件聚合、
分布）。计时取自 nextest 每条用例输出行，非估算。

> 为什么不用 `cargo test`：libtest 的 `--report-time` 是 nightly-only；`cargo test`
> 还按二进制串行。nextest 是进程级并行 + 原生耗时输出。

## 2. 三阶段实测

| 阶段 | 用例数 | 执行墙钟 | CPU 之和 | 并行度 | 失败 |
|---|---|---|---|---|---|
| 基线（默认 durability） | 1499 | 421s | 4309s | 10.2x | 2（含 1 回归） |
| ① 测试库 fsync=off | 1498 | 255s | 2391s | 9.4x | 0 |
| ② + git fast-import 夹具 | 1499 | 214s | 2339s | 10.9x | 0 |
| ③ + `--profile fast`（排除真远端） | 1491 | **211s** | 2298s | 10.9x | 0 |

单例固定成本解剖（`cargo run -p pm-application --example testdb_bench`）：

| 阶段 | 默认 durability | fsync=off |
|---|---|---|
| CREATE DATABASE | 55ms | 33ms |
| 连接池 | 22ms | 22ms |
| **17 个迁移 / 363 表** | **380~410ms** | **62~71ms** |
| DROP DATABASE | 50ms | 12ms |
| 合计 | **≈525ms** | **≈130ms** |

**根因 1**：`#[sqlx::test]` 每用例 `CREATE DATABASE` + 重跑 17 个迁移（363 表）。
近 1400 个 sqlx 用例 ⇒ 每人 0.4s 纯 DDL；20 路并行时 DDL 争用再把单例放大 2 倍
（串行 1.19s → 并行 2.5s）。测试库数据可弃，`fsync/full_page_writes/
synchronous_commit=off` 即可（`scripts/setup-test-db.sh`，仅限测试容器）。

**根因 2**：`repo_facts::collect_truncates_at_three_hundred_commits` 逐条
`git add`+`git commit` 造 301 个提交 = 602 次进程 spawn；Windows 下单次 ≈70ms
⇒ 单例 45~86s，是当时全量最大单例。改用一次 `git fast-import`（`Fixture::
commit_many`）⇒ **1.18s（72x）**，套件 CPU 96s → 1.4s。

## 3. 当前剩余热点（fast profile，CPU 时间）

| 套件 | 用例数 | 均值 | CPU |
|---|---|---|---|
| `pm-application::knowledge_index_v2` | 29 | 4.91s | 142s |
| `pm-application::delivery` | 24 | 4.38s | 105s |
| `pm-ai`（库内单测） | 125 | 0.67s | 83s |
| `pm-application`（lib 单测） | 141 | 0.42s | 60s |
| `pm-infrastructure::kb_pc_repo_flows` | 7 | 7.43s | 52s |
| `pm-application::review` | 14 | 3.69s | 52s |

这些是**真实工作量**（大文档索引 220 章节、并发语义用例 k29 单例 35s、真 git
仓库提交链 + outbox 跑批），不是无谓开销。

## 4. 还能做什么（按 ROI，未实施）

| 措施 | 预期 | 代价 |
|---|---|---|
| nextest `-j` 扫最优（当前 10.9x/20 核） | 墙钟 -10~20% | 需多轮全量测量 |
| release 档跑测试（解析/git 重负载明显变快） | 执行 -30%+ | 编译时间翻倍 |
| 模板库（`CREATE DATABASE ... TEMPLATE`）替代逐用例迁移 | 再省 65ms/用例 | sqlx::test 不支持，需自建 fixture，改动面大 |
| `knowledge_index_v2` 的 220 章节夹具降到最低有效规模 | -30s CPU | 需逐用例确认断言阈值 |
| 拆分 heavy 套件到独立 CI 分片（`--partition`） | 墙钟随分片数线性下降 | CI 侧配置 |

## 5. 复现命令

```bash
# 固定成本探针
DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' \
  cargo run -p pm-application --example testdb_bench

# 并行争用曲线（改 -j 看墙钟与 CPU 和）
cargo nextest run -p pm-application --test docproc_pipeline -j 1
cargo nextest run -p pm-application --test docproc_pipeline -j 13
```

> DATABASE_URL 必须用 `127.0.0.1`（AGENTS.md 规则 3：localhost 走 `::1` 黑洞）。
> 测试库容器 `aipm-pg-gate:16433`。
