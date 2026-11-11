# 测试耗时画像与提速（2026-11-11 实测）

**结论：全量墙钟 421s → 172s（-59%），零产品代码改动。** 三个杠杆：
测试库 durability 配置、**测试模板库（template1 预装迁移）**、git 夹具
`fast-import`。

## 1. 怎么用

```bash
bash scripts/test.sh              # 内环：fast profile（排除真远端 SSH 套件）
bash scripts/test.sh --full       # 全套：含 e2e_flow 真远端（需网络 + SSH key）
bash scripts/test.sh --timing     # 附带耗时画像
bash scripts/test.sh -p pm-application --test docproc_pipeline

bash scripts/setup-test-db.sh          # 一次性：测试库调优（幂等）
bash scripts/seed-test-template.sh     # 可选：模板库加速（-46%，读脚本头的风险说明）
bash scripts/seed-test-template.sh --reset   # 切分支前还原（迁移集不同会 VersionMissing）
```

画像产出：`target/test-timing/{nextest.log,summary.txt}`（per-test 计时、套件聚合、
分布）。计时取自 nextest 每条用例输出行，非估算。

> 不用 `cargo test`：libtest 的 `--report-time` 是 nightly-only，且 `cargo test`
> 按二进制串行（本仓 173 个测试二进制）。nextest 是进程级并行 + 原生耗时输出。

## 2. 实测阶梯（1491 用例，fast profile，20 核）

| 配置 | 墙钟 | CPU 之和 | 并行度 | 失败 |
|---|---|---|---|---|
| 原始（fsync on，干净模板库） | 421s | 4309s | 10.2x | 2 |
| + 测试库 fsync/页写 off | 319s | 4761s | 14.9x | 0 |
| + git 夹具改 fast-import | — | — | — | — |
| **+ 模板库播种** | **172s** | **1802s** | 10.5x | **0** |

> 注意 CPU 之和会随并行度变化（争用导致同一条用例在拥挤时更慢），所以看墙钟
> 与失败数，不要拿 CPU 和横向比。

单例建库固定成本（`cargo run -p pm-application --example testdb_bench`）：

| 阶段 | 默认 durability | fsync=off | + 模板库 |
|---|---|---|---|
| CREATE DATABASE | 55ms | 30ms | 30ms |
| 连接池 | 22ms | 20ms | 20ms |
| **17 个迁移 / 362 表** | **380~410ms** | **300~350ms** | **5ms** |
| DROP DATABASE | 50ms | 10ms | 8ms |
| 合计 | ≈525ms | ≈340ms | **≈62ms** |

**根因**：`#[sqlx::test]` 每用例 `CREATE DATABASE` + 重放 17 个迁移（362 表）。
近 1400 个 sqlx 用例 ⇒ 每人 0.3~0.4s 纯 DDL；20 路并行时 DDL 争用还会把单例放大
2 倍。两条对策：

1. **测试库 durability**：`fsync/full_page_writes/synchronous_commit=off`
   （测试数据可弃）+ `max_locks_per_transaction=1024`。
   > 锁参数不是调优而是**修抖动**：0001 迁移单事务建 362 表，20 路并行 × 每事务
   > 数百把锁会把共享锁表打满，报 `out of shared memory, increase
   > max_locks_per_transaction`——表现为随机用例失败（实测命中过一次）。默认 64
   > 正在临界点。
2. **模板库**：`CREATE DATABASE` 无 `TEMPLATE` 时克隆 template1
   （sqlx-postgres/migrate.rs:49）。预装迁移后迁移 run 只剩校验查询。
   **风险**：schema 钉在当前分支迁移集上；切到迁移更少的分支会 `VersionMissing`
   → `scripts/seed-test-template.sh --reset`。`scripts/test.sh` 检测到该错误会打印
   恢复命令。

**根因 2**：`repo_facts::collect_truncates_at_three_hundred_commits` 逐条
`git add`+`git commit` 造 301 个提交 = 602 次进程 spawn；Windows 下单次 ≈70ms
⇒ 单例 45~86s，曾是全量最大单例。改用一次 `git fast-import`（`Fixture::
commit_many`）⇒ **1.18s（72x）**。
> 坑：fast-import 的 `from` 必须放在提交信息 `data` 之后，且要显式接 HEAD，
> 否则报 `expected committer` 或新链被判非快进而丢 ref。

## 3. 当前剩余热点（播种模板库 + fast profile）

| 套件 | 用例数 | 均值 | CPU |
|---|---|---|---|
| `pm-application::knowledge_index_v2` | 29 | ≈4.6s | ≈130s |
| `pm-application::delivery` | 24 | ≈4.0s | ≈95s |
| `pm-ai`（库内单测） | 125 | ≈0.7s | ≈85s |
| `pm-application`（lib 单测） | 141 | ≈0.4s | ≈60s |
| `pm-infrastructure::kb_pc_repo_flows` | 7 | ≈7.4s | ≈52s |

这些是**真实工作量**：大文档索引（220 章节夹具）、并发语义用例（k29 单例 ≈35s）、
真 git 提交链 + outbox 跑批。

## 4. 已验证但否掉的方案

| 方案 | 结果 |
|---|---|
| release 档跑测试 | 增量构建 6m42s；执行侧数据被机器噪声淹没（dev 8.4/10.5/12.8s vs release 17.8/22.4/7.0s），无稳定收益 |
| 提高并行度 / 换 `-j` | 默认已 20 线程；争用已由模板库化解，再压并行收益不明确 |

**测量噪声警告**：本机（Windows + Docker + 杀软）单个数据的方差可达 ±50%，且会
出现 2~3 倍的整体慢窗口（例：某轮全量 538s，同期单套件 CPU 从 8s 涨到 30s）。
结论只取「同一天、相邻时间、多次重复」的对比。

## 5. 复现命令

```bash
# 单例固定成本探针（三种配置对比）
DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' \
  cargo run -p pm-application --example testdb_bench

# 模板库播种 / 还原
bash scripts/seed-test-template.sh
bash scripts/seed-test-template.sh --reset

# 画像
bash scripts/test.sh --timing
```

> DATABASE_URL 必须用 `127.0.0.1`（AGENTS.md 规则 3：localhost 走 `::1` 黑洞）。
> 测试库容器 `aipm-pg-gate:16433`。
