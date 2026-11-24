# AGENTS.md — 给 AI 的硬规则

1. **这是新项目，没有存量数据，不考虑数据迁移与兼容。**
   任何一个设计都不要为「旧数据 / 旧格式 / 旧行为」写迁移脚本、双写、兼容分支、回退开关或过渡层。
   需要改契约就直接改契约本身；需要重来就重建表，不做数据搬运。
   （判据：如果一段代码的存在理由只有「以前是这样」，就不要写。）

2. **你是高效助手。规则：**
   1. 只回答必要信息，不寒暄、不重复问题。
   2. 默认用要点/表格/代码，不写长段落。
   3. 不确定先问1个关键问题；信息够就直接做。
   4. 输出不超过我要求的字数/条数；未指定则尽量≤200字。
   5. 不复述上下文，不解释显而易见的内容。

3. **本机连 Docker 端口映射的服务，URL/地址一律写 `127.0.0.1`，禁止 `localhost`。**
   本机 Docker Desktop 的 IPv6 回环代理是黑洞（`::1:<port>` SYN 永无应答），而 Windows 把
   `localhost` 优先解析为 `::1`：每次建连先在 `::1` 上耗尽 OS 超时才回落，sqlx::test 每用例
   建连 4~5 次，309 个用例即把全量测试拖成小时级（2026-11-07 实测：localhost 挂死 >4min，
   127.0.0.1 同用例 1.28s）。写法：`DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm'`。
   走远程 IP（如 192.168.2.10）不受影响。测试进程被杀会泄漏 `_sqlx_test_%` 库，记得清理。

4. **发布前端必须禁止 mock 数据。**
   `production` / `staging` 构建不得设置 `VITE_USE_MOCK=1`；产物（dist）不得包含
   `fixtures`、`demo` 或 `src/api/mock` 模块。无真实接口的页面在发布态给空态/占位，不得用演示数据填充。
   门禁：`ai-frontend/build/release-guard.ts`（构建期硬失败）。fixtures / demo 只能在本地 dev
   （`VITE_USE_MOCK=1`）与单测（`.env.test`）动态加载。

5. **禁止 AI 助手的页面，只改一处路径前缀清单。**
   助手可见性单源 = `ai-frontend/src/components/shell/use-shell-chrome.ts`；
   路径判定 = `ai-frontend/src/router/navigation.ts` 的 `isAssistantHiddenPath()`
   （页面清单 `ASSISTANT_HIDDEN_RULES` 驱动：含 `/project/settings` 前缀、admin 遗留两项
   `/admin/settings` 与 `/admin/announcements`、团队管理页 members/ai-providers；
   C4 起显式清单替代 admin 组动态派生，禁止凭页面名推断禁用范围）。
   新增禁用页只追加路径前缀，**禁止**在业务页里 `v-if` 打补丁；四处必须同步收列：
   顶栏「助手」按钮、AI Chat 列、右上下文栏、壳根 `data-ai-panel` / `data-context`。
   详细步骤与验收见 `ai-frontend/AGENTS.md` §壳的约定。

6. **跑测试：Q10 系列用例必须从全量中排除，单独跑。**
   `q10_*` 测试（`crates/pm-application/tests/q10_race_closure.rs` / `q10_r3_snapshot_check.rs` /
   `q10_classify_matrix.rs`，P3-D04 历史链竞态类）在多包并行全量时会因 DB 资源竞争轮询挂死
   （2026-11-21 实测：四包全量挂死 >10min 两次；同用例单独跑 18/18 约 9s 全绿）。
   写法：
   - 全量：`cargo test -p pm-api -p pm-application -p pm-infrastructure -p pm-mcp --no-fail-fast -- --skip q10`
   - 单独：`cargo test -p pm-application --test q10_race_closure --test q10_r3_snapshot_check --test q10_classify_matrix`
   （注意 `--skip q10` 传给 libtest 须放在 `--` 之后。）
   附：全量遇测试 exe 链接 `permission denied` = 残留测试进程占用产物，先
   `Get-CimInstance Win32_Process | Where-Object {$_.Path -like '*target*debug*'}` 杀进程再编。

<!-- 后续规则按序号追加到这里 -->

7. **项目生命周期与 Greenfield 工程政策。**
   全仓架构、数据库、API、重构和实现工作必须遵循
   `docs/architecture/engineering-lifecycle-policy.md`。
   当前项目无历史生产数据、无客户、无正式发布版本；不要为从未发布的旧行为
   引入不必要的迁移、兼容层或过渡机制。删除或替换现有结构前仍须检查实际引用与依赖；
   保留事务一致性、并发安全、执行权唯一性、凭据安全、审计、测试及空库初始化能力。
   本政策本身不构成实施或破坏性操作授权，具体任务仍以明确授权范围为准。
