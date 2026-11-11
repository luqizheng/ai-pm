# 测试耗时画像与提速（2026-11-11 实测）

**结论先行：全量 1499 用例从 421s 降到 255s（-40%），零代码改动——只调测试库
durability 配置。** 剩余时间的 63% 集中在 925 个「1~3s」的数据库用例（每个
用例都新建库 + 跑全量迁移），13% 集中在 7 个 git 重负载用例。

## 1. 怎么测

```bash
bash scripts/setup-test-db.sh          # 一次性：测试库 durability 调优（4x）
bash scripts/test-timing.sh            # 全量：编译 + 执行分阶段计时
bash scripts/test-timing.sh -p pm-application --test docproc_pipeline
```

产出：`target/test-timing/{nextest.log,summary.txt}`（per-test 计时、套件聚合、
耗时分布）。计时来自 nextest 每条用例的输出行，不是估算。

> 为什么不用 `cargo test`：libtest 的 `--report-time` 是 nightly-only；且
> `cargo test` 串行跑测试二进制，nextest 是进程级并行 + 原生耗时统计。

## 2. 慢在哪（调优前 / 调优后）

| 指标 | 调优前 | 调优后 |
|---|---|---|
| 执行墙钟 | 421s | **255s** |
| 用例 CPU 之和 | 4309s | **2391s** |
| 并行效率 | 10.2x / 20 核 | 9.4x / 20 核 |
| 单例均值 | 2.87s | 1.60s |
| sqlx 用例建库+迁移 | 525ms | **130ms**（迁移 380→65ms） |

固定成本解剖（`cargo run -p pm-application --example testdb_bench`）：

| 阶段 | 默认 durability | fsync=off |
|---|---|---|
| CREATE DATABASE | 55ms | 33ms |
| 连接池 | 22ms | 22ms |
| **17 个迁移 / 363 表** | **380~410ms** | **62~71ms** |
| DROP DATABASE | 50ms | 12ms |
| 合计 | **≈525ms** | **≈130ms** |

**根因**：`#[sqlx::test]` 每个用例都 `CREATE DATABASE` + 重跑 17 个迁移（363 张
表、索引、触发器、种子）。近 1400 个 sqlx 用例 ⇒ 每人 0.4~0.5s 纯 DDL；且
20 路并行时 DDL 争用把单例耗时再放大 2 倍（串行 1.19s → 并行 2.5s）。

**为什么调 fsync 就够**：测试库每次用例新建、结束即删，没有需要保护的数据；
DDL 密集负载下 WAL fsync 是主要等待点。

## 3. 剩余热点（调优后 CPU 时间）

| 套件/用例 | 用例数 | 均值 | CPU 合计 |
|---|---|---|---|
| `pm-application::knowledge_index_v2` | 29 | 4.56s | 132s |
| `pm-application::delivery` | 24 | 4.06s | 97s |
| `pm-application::repo_facts` | 5 | **19.2s** | 96s |
| `pm-ai`（库内联单测） | 125 | 0.63s | 79s |
| `pm-infrastructure::kb_pc_repo_flows` | 7 | 9.43s | 66s |
| `pm-api::e2e_flow` | 7 | 7.98s | 56s |

单例前 3 名（合计 157s CPU，占全量 6.6%）：

1. `repo_facts::collect_truncates_at_three_hundred_commits` — **85.7s**（造 300 个
   git commit，每个都是一次进程 spawn）
2. `e2e_flow::flow_04_cold_start_import_from_real_repo` — 36.6s（真仓库导入）
3. `knowledge_index_v2::k29_two_pools_serialize_the_same_project...` — 34.7s
   （并发语义用例，慢是设计意图）

耗时分布：`0–0.5s` 384 例（89s）｜`0.5–1s` 49 例（38s）｜`1–3s` **925 例
（1501s）**｜`3–10s` 133 例（560s）｜`≥10s` 7 例（203s）。

## 4. 还可以做的（未实施，按 ROI 排序）

| 措施 | 预期收益 | 代价/风险 |
|---|---|---|
| 把 300-commit 用例降到 50 commit 或复用夹具仓库 | -60~80s CPU | 需确认用例语义（截断阈值断言） |
| `git commit` 批量造历史改用 `git fast-import` | -50s CPU | 单点改造 |
| nextest `-j` 调优（实测 j=13 时争用仍轻，j=20 效率 9.4x） | 墙钟 -10~20% | 需扫一遍最优值 |
| release 档跑测试 | 解析/git 重负载显著变快 | 编译时间翻倍，CI 需权衡 |
| 模板库（`CREATE DATABASE ... TEMPLATE`）替代逐用例迁移 | 再省 60ms/用例 | sqlx::test 不支持，需自建 fixture，改动面大 |

## 5. 复现命令

```bash
# 固定成本探针
DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' \
  cargo run -p pm-application --example testdb_bench

# 并行争用曲线
for J in 1 4 13; do cargo nextest run -p pm-application --test docproc_pipeline -j $J; done
```

> DATABASE_URL 必须是 `127.0.0.1`（AGENTS.md 规则 3）；测试库容器
> `aipm-pg-gate:16433`。
