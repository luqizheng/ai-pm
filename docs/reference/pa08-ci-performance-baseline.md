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

## 6. 首轮完整 Hosted 验收结果

验收提交：`643231111624aa170d90f5bf4a467720302cb07a`  
PA-08 Run [#45044216787](https://github.com/luqizheng/ai-pm/actions/runs/45044216787)（Run #292），2026-11-22 16:30:29–16:44:32 UTC，端到端约 **14 分 03 秒**。8/8 Jobs 成功。

### 6.1 Nextest 四分片结果

来自四个 shard 的 Nextest 最终 Summary（测试执行时间为 Nextest 报告值，不含 Job 的容器初始化与 archive 下载）：

| 分片 | 测试数 | 通过 | 失败 | Nextest 执行耗时 |
|---|---:|---:|---:|---:|
| 1/4 | 528 | 528 | 0 | 136.127 秒 |
| 2/4 | 507 | 507 | 0 | 209.952 秒 |
| 3/4 | 476 | 476 | 0 | 156.088 秒 |
| 4/4 | 489 | 489 | 0 | 195.640 秒 |
| **合计** | **2,000** | **2,000** | **0** | — |

所有 shard 成功。每个 shard Summary 中的 skipped 是 hash 分片未分配给该 shard 的用例，不代表 workflow 排除了测试；四个 shard 的实际执行计数合计 2,000。Doctests 单独执行并通过。没有使用 fast profile，也没有排除 Q10 / e2e。

关键回归用例均有实际 PASS 记录：
- `tenant_removal_racing_grant_has_single_final_state`（shard 1/4）。
- `grant_rejects_agent_principal`（shard 4/4）。

### 6.2 Archive 与墙钟

- `cargo nextest archive --workspace --locked --archive-file nextest-archive.tar.zst --zstd-level 3`：16:30:41–16:35:09 UTC，约 **4 分 28 秒**（Job step）。
- 上传步骤：16:35:09–16:36:05 UTC，约 **56 秒**。
- 上传后的 artifact 大小：**3,503,762,456 bytes**（约 3.26 GiB / 3.50 GB 十进制）。
- 四个 shard 并行执行；Job 总耗时分别约 8:22、6:23、6:17、5:52。最慢 Job 主要受 shard 1 下载大 archive 的时间影响；Nextest 本身最慢分片为 shard 2（209.952 秒）。
- PA-08 其余门禁：Linux doctests、public-registry build / workspace Clippy / migration uniqueness / promtool / changed-file rustfmt、Windows mirror lock、Windows Runner/Worker 编译全部通过。

### 6.3 PA-07 回归真实执行

PA-07 Run [#45044216776](https://github.com/luqizheng/ai-pm/actions/runs/45044216776)（Run #296）在同一 SHA 上 2/2 Jobs 成功。Integration 使用 ephemeral PostgreSQL，执行命令 `cargo test --locked -p pm-ai --test orchestrator batch_create_tasks`，结果 **2 passed; 0 failed; 0 ignored; 6 filtered out**。`batch_create_tasks_with_10_items_auto_applies_at_the_threshold` 与 `batch_create_tasks_with_11_items_escalates_on_item_count_not_proposal_count` 均实际执行成功，不再依赖未配置的外部数据库 secret 而跳过。

### 6.4 结论与后续观察

- 当前实现已达到“完整测试覆盖 + 四 shard + doctests + Linux 静态门禁 + Windows 检查”全通过的验收条件。
- 本轮完整 PA-08 workflow 墙钟约 14 分钟；这与原始失败基线的 22 分 17 秒相比，方向上明显改善，但两者门禁编排和测试成功状态并非完全同类，不能把差值当作严格的性能提升百分比。
- 3.50 GB archive 的构建、上传和 shard 下载已成为下一轮主要优化观察点。当前不应通过减少测试或降低覆盖来缩小 archive；可在后续独立评估 archive 组成、缓存/传输策略和 runner 资源成本。
- PA-08 与 PA-07 结果均对应上述同一 SHA。PR #1 继续保持 Open / Draft；本次验收不代表合并或部署授权。
