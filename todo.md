1. 仓库分析按钮（2026-11-13 裁决：落点两处都放 / 多选不预选 / 增量分析一并做）

   背景：现后端冷启动分析 = `POST /v1/pm/projects/{id}/imports`（项目必须为空，仓库集合创建时固化）；failed/cancelled 重发同一 API 即再分析；已 apply 项目永久 409；增量分析目前只有 verify/update-version 自动联动快照 intake，无手动入口。前端 imports API 封装齐全但零调用。

   - S1 后端「手动增量分析」：新增 `POST /v1/pm/repositories/{id}/analyze` 手动触发快照 intake（幂等键锚 commit + 触发时刻，与 `repo_verify:{repo}:{commit}` 区分；复用 `submit_repository_snapshot_intake`，`crates/pm-application/src/repository/repo.rs:670`）。落点：repo.rs + `apps/pm-api/src/pm_repositories.rs`（路由表 §2.1 补一条）。语义：HEAD 未前进也允许重触发（区别于 verify 联动）；进行中/限流语义对齐 intake task retry 规则（非 FAILED/NEEDS_HUMAN_REVIEW 409 同族）。
   - S2 前端-仓库设置页（`SettingsProjectRepositoriesView.vue`）：列表行加 checkbox（仅 `status === 'verified'` 可勾，**不预选**）+「开始分析 (N)」按钮 → 确认弹层 → `startImport(projectId, ids, uuid 幂等键)`；`listImports` 过滤 queued/cloning/reading/analyzing → 有进行中导入则按钮禁用 + 行内进度；failed/cancelled 重发即「再次分析」，不设单独 retry 按钮；已 apply 项目调用 409 → 展示后端错误码兜底文案。
   - S3 前端-总控台（`InitializationCard.vue`）：「代码分析」事项旁放动作按钮（接入 listImports 状态，可发起/重发），事项本体加跳转 `/project/settings/repositories` 链接；保持七态状态展示语义不变。
   - S4 前端-仓库详情面板：加「重新分析」按钮（单仓库手动增量，调 S1 新端点；与 verify/confirm-update 并列，人工确认动作次文字色）。
   - 风险与边界：① 前端无 requirement count 端点，「项目非空」只能靠后端 409 兜底文案；② 幂等键用 ref 持有防双击双发，成功后清除；③ scope=`__all__`（无项目）整段隐藏按钮；④ S1 不得改动既有 `repo_verify:`/`repo_confirm:` 幂等键语义。
   - 验收：S1 cargo test 覆盖幂等重放/409/422/失败释放名额；S2-S4 每视图 ≥1 条 vitest（断言行为）；`pnpm typecheck` / `pnpm lint` / `check:prism-tokens` / `check:i18n` 全绿。

2. SSH 私钥独立设置页（2026-11-13 提出）

   背景：SSH 私钥目前挂在仓库设置页（`/project/settings/repositories`）内，需移出为与【仓库】同级的独立导航项，并支持为每把私钥起名。

   - S1 后端：SSH 私钥独立实体（增 name 字段，CRUD + 重命名端点）；与仓库的引用关系改为按 id 关联，仓库设置页只保留选择器。
   - S2 前端：`/project/settings` 导航新增「SSH 私钥」项（与【仓库】同级）；新视图 `SettingsProjectSshKeysView.vue`（列表/新增/改名/删除）；仓库设置页移除内嵌私钥管理，改下拉选择。
   - 验收：迁移 + cargo test 覆盖重命名/删除被引用拒绝；前端 vitest + `pnpm typecheck` / `pnpm lint` / `check:prism-tokens` / `check:i18n` 全绿。


