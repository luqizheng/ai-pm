# PA-08 Linux CI 性能基线与优化计划

日期：2026-11-22  
目标：在不减少测试覆盖、不跳过 Q10 / e2e 测试、保留 Windows 平台检查的前提下，缩短 PA-08 Linux Hosted CI 墙钟时间。策略：4 个并行 Linux 测试分片，平衡等待时间与 Actions 资源消耗。

## 1. Hosted 基线

来源：PA-08 Registry Portability Run [#45012463835](https://github.com/luqizheng/ai-pm/actions/runs/45012463835)，Job：`public-registry-build`。

从 Job 日志时间戳提取：

| 阶段 | UTC 时间 | 墙钟耗时 |
|---|---:|---:|
| Job 启动 | 10:26:43.58 | — |
| PostgreSQL healthy | 10:27:04.89 | 约 21 秒（含服务启动） |
| `cargo check --workspace --locked` | 10:27:16.15–10:27:49.47 | 约 33 秒 |
| `cargo clippy --workspace --all-targets --locked` | 10:27:49.47–10:29:26.50 | 约 97 秒 |
| ADR guard test | 10:29:26.52–10:29:33.86 | 约 7 秒 |
| audit actor coverage | 10:29:33.87–10:30:10.69 | 约 37 秒 |
| `cargo test --workspace --locked` | 10:30:10.70–10:49:00.92 | 约 18 分 50 秒 |
| Job 结束 | 10:49:00.92 | 总计约 22 分 17 秒 |

**重要限制**：该次全量测试步骤以 1 个测试失败结束，因此 18 分 50 秒是失败运行的耗时，不是完整成功运行的基准；后续专门测试、promtool 和 fmt 步骤因前序失败而未执行。优化后的验收必须以所有分片、doctests、静态检查和 Windows 检查均通过为准。

## 2. 已存在的性能基础

- workflow 已使用 `Swatinem/rust-cache@v2`，不重复叠加相同用途的缓存动作。
- 仓库已有 `.config/nextest.toml`、`scripts/test.sh` 和 `scripts/test-timing.sh`，说明 Nextest 已是本地测试工具链的一部分；Hosted workflow 尚未使用它。
- `scripts/setup-test-db.sh` 已定义仅供可丢弃测试集群使用的 PostgreSQL durability 调优，以及 `max_locks_per_transaction` 调整；该脚本绑定本地容器名，因此 Hosted workflow 直接对每个临时 service container 执行等价的 `ALTER SYSTEM` 配置并重启。
- 仓库另有 template1 迁移播种工具，但第一轮 Hosted 优化不启用它：避免每个 shard 为播种程序重复编译；先用独立数据库 + 低并发验证稳定性，后续根据耗时数据再决定是否把播种二进制加入 archive。
- 仓库既有本地性能记录（20 核机器、fast profile）为：默认测试库 421 秒；durability 调优后 319 秒；再使用模板库后 172 秒。此数据不是 GitHub Hosted Linux 基线，只用于确认数据库初始化是值得优化的热点。

## 3. 优化设计

1. 保留 Windows `windows-mirror-lock` Job，不改变其锁语义、Runner / Worker 编译检查。
2. 保留 Linux crates.io metadata、公共 registry 来源、workspace check、Clippy、migration 唯一性、Prometheus promtool 和 changed-file rustfmt 门禁。
3. 将 Linux 测试二进制构建一次，生成 Nextest archive；四个 Linux shard 下载同一 archive，避免每个 shard 独立重新编译全部测试二进制。
4. 使用 Nextest `hash:1/4` 至 `hash:4/4` 分片。每个测试用例由且仅由一个 shard 执行；不使用 `fast` profile，不排除 Q10、e2e 或其他测试。
5. doctests 单独执行一次，因为 Nextest 不执行 Rust doctests。
6. 每个 shard 使用独立 PostgreSQL 16 临时容器；只对临时容器启用 fsync/full_page_writes/synchronous_commit 调优并提高 max_locks_per_transaction，不触碰持久化或共享数据库。
7. 移除完整 workspace 测试之后重复执行的定向测试命令；这些测试目标仍由完整 Nextest workspace archive 覆盖。保留静态门禁和 Windows 检查。
8. 对比优化前后的同类 Hosted run：总墙钟、archive 构建、doctests、四个 shard 中最慢者、数据库准备、Actions 分钟数，以及所有测试是否通过。

## 4. 完整覆盖验收

- `cargo nextest archive --workspace --locked` 构建 workspace 全部测试目标。
- 四个 shard 的 hash 分区合计覆盖完整测试集，全部 shard 均必须成功。
- `cargo test --workspace --doc --locked` 单独通过。
- 不使用 `--profile fast`，不跳过 Q10 或 e2e 测试。
- PA-08 Linux 静态检查全部通过。
- Windows mirror lock、Runner / Worker Windows 编译继续通过。
- 不把 cancelled、skipped、旧 SHA 的结果当作当前 SHA 的通过证据。

## 5. 资源策略

使用 4 个 Linux test shard，每个 shard 限制为 2 个测试线程，避免单个 PostgreSQL 实例被过度并发压垮。四个 shard 各自拥有隔离的临时 PostgreSQL 容器。通过单次 archive 构建减少重复编译；首轮暂不启用 template1 播种，避免为每个 shard 重复编译播种工具。接受四个并行 runner 的短时资源占用，以换取更短的 PR 等待时间。
