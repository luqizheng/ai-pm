# 并行开发协作纪律（ai-pm 仓）

> **为什么有这份文档**：并行会话（多 worktree / 多分支 / 同机多 agent）在本仓已多次互相踩坑——迁移撞号、测试大面积假红、格式化波及同 crate 文件。本文把**已实测的坑与处置**固定下来，避免重复排查。
> **适用范围**：任何在本仓并行改动的会话（人工多分支 / 多 agent 会话）。

---

## 一、迁移编号纪律

### 1.1 开工必查号

```bash
ls migrations/          # 每个批次开工当日必查——并行会话占号是常态
```

已被占用的历史事实：0003~0005（对话锚点/llm 元数据/交互）、0010（design 域）、0011（task_plan 域）均为并行批次占用。

### 1.2 撞号处置

- 若你与他人**各自**选了同一号 → **一方顺移**（顺延到当前最大号 +1），并在提交信息与迁移台账中记录顺移事实。
- 若**文件已存在**同名号但内容不同 → 立即改号，不要"覆盖"。

### 1.3 ⚠️ 新增迁移文件后，测试不会自动看到它（最隐蔽的坑）

`sqlx::migrate!` 宏与 `#[sqlx::test(migrations = "...")]` **都在编译期固化迁移文件列表**——新增 `.sql` **不触发 cargo 重编**。

**症状（极具误导性）**：

```
failed to apply migrations: VersionMissing(N)
```

- 看起来像迁移号撞车 / template1 脏了 / 迁移链断了；
- **实际上 template1 账本完全正确**（`SELECT count(*) FROM _sqlx_migrations` 会对得上）；
- 真实原因：测试二进制里嵌入的迁移列表还是旧的（少最后一个）。

**额外症状**：用例时长从正常的 ~130s 涨到 ~500s+（每个用例都在重放整条迁移链）。

**处置**（任选其一）：

```bash
# A. 强制重编（推荐）
cargo build --workspace --tests

# B. touch 各 crate 入口，让 cargo 感知
for f in crates/*/src/lib.rs apps/pm-api/src/main.rs; do touch "$f"; done
cargo build --workspace --tests
```

**不要**被 template1 误导去反复重烙模板库——先看 `ls migrations/` 与 `VersionMissing(N)` 的 N 是否等于目录里的最大号。

### 1.4 泄漏测试库清理

批量失败后残留的 `_sqlx_test_*` 库会拖慢后续建库（每个用例建库变慢）：

```bash
docker exec <pg> psql -U aipm -d postgres -t -c \
  "SELECT format('DROP DATABASE IF EXISTS %I;', datname) FROM pg_database WHERE datname LIKE '_sqlx_test_%';" \
| docker exec -i <pg> psql -U aipm -d postgres -q
```

---

## 二、并行会话下的提交纪律

| 规则 | 原因 |
| --- | --- |
| **只 add 自己的文件**（显式路径），不用 `git add -A` | 会把他人未完成的半成品（编译不过的 `design.rs` 等）卷进提交 |
| **热点文件谨慎 add**：`apps/pm-api/src/{lib,routes}.rs`、`crates/pm-application/src/lib.rs` | 他人常在同处改动；若 diff 里含他人物理顺序调整（无语义）可一并提交，含语义改动则不提交 |
| **工作区出现非我改动的文件属正常** | 并行会话的产物；不代改、不代提交、不 revert |
| **他人在编译不过的中间态** | 此时 `cargo clippy --all` 会失败——**不等于我的代码有问题**，用 `-p <我的 crate>` 局部验 |

---

## 三、格式化纪律

- **单文件 rustfmt**：`rustfmt --edition 2021 <file>`；**不要** `cargo fmt -p <crate>`（会波及同 crate 非本批文件——本仓已有 11 个存量 rustfmt 漂移文件）。
- 前端用 `npx prettier --write <显式文件>`。

---

## 四、联合测试（多批次合并后）

并行批次各自绿 ≠ 合并后绿。合并后必须跑一次**全量**，且注意：

1. **强制重编**（见 §1.3）——合并进来的新迁移文件对旧测试二进制不可见。
2. **清泄漏库**（见 §1.4）。
3. **重烙 template1**：`cargo run -p pm-infrastructure --example seed_test_template`（工具用 fingerprint 判断是否需要重建；fingerprint 与实际账本脱节时手工 `DROP DATABASE template1` 后重跑）。
4. **跑全量**：`bash scripts/cargo-test.sh`（后端）+ `npx vitest run`（前端）。
   - 前端 3 个 `e2e/*.spec.ts` 失败是**存量已知**（vitest 误收集 e2e），不算回归；判据看 `Tests` 总数而非 `Test Files`。
5. **门禁清单**：后端 `cargo-test.sh` 全绿 + `cargo clippy --all --all-targets` 零新增告警 + 前端 `vue-tsc --noEmit` 0 + `npm run check:i18n` 对齐 + `vitest` 单测全绿。

---

## 五、数据库与代理陷阱（复用时直接查）

| 坑 | 症状 | 处置 |
| --- | --- | --- |
| IPv6 回环黑洞 | 本机连 PG 超时 | `DATABASE_URL` 必须用 **127.0.0.1**（不是 localhost） |
| 死代理劫持 loopback | wiremock/本地端点 `error sending request` | 导出 `NO_PROXY=localhost,127.0.0.1,::1`（`cargo-test.sh` 已内置） |
| 死代理继承到子进程 | `ollama pull` DNS 失败但 curl 通 | 重启该进程（代理环境在进程启动时继承） |
| 端口冲突 | 服务起不来 | `netstat -ano \| grep <port>` 查占用 |
