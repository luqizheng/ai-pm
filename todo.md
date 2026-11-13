1. 仓库分析按钮 —— 已交付（2026-11-13，分支 `feat/todo-2026-11-13`；验收遗留见第 3 条）

2. SSH 私钥独立设置页（2026-11-13 提出）

   背景：SSH 私钥目前挂在仓库设置页（`/project/settings/repositories`）内，需移出为与【仓库】同级的独立导航项，并支持为每把私钥起名。

   - S1 后端：SSH 私钥独立实体（增 name 字段，CRUD + 重命名端点）；与仓库的引用关系改为按 id 关联，仓库设置页只保留选择器。
   - S2 前端：`/project/settings` 导航新增「SSH 私钥」项（与【仓库】同级）；新视图 `SettingsProjectSshKeysView.vue`（列表/新增/改名/删除）；仓库设置页移除内嵌私钥管理，改下拉选择。
   - 验收：迁移 + cargo test 覆盖重命名/删除被引用拒绝；前端 vitest + `pnpm typecheck` / `pnpm lint` / `check:prism-tokens` / `check:i18n` 全绿。

3. 仓库分析按钮验收遗留（2026-11-13，均非阻塞）

   - L1「行内进度」落地为批次级进度（导入批次是项目级，无法派生行内）→ 在 S2 视图注释或接口文档补一句说明，避免后续按字面验收打回。
   - L2 活跃检查只匹配 `repo_analyze:` 键族：verify/confirm 联动产生的活跃 intake 不阻塞手动分析，同 commit 可并行两条 intake（内容寻址去重保证零新增修订）。是否改为跨族阻塞待产品确认。
   - L3 `pnpm check:i18n` 存量 39 处硬编码（5 个文件与 `e105e9b9` 逐字节相同）：补 baseline 或另立清理任务。
   - L4 `RepositoryError::AnalyzeIntake` 的 422 只带 `message`、无稳定 `error` 码 → 前端只能透出英文原文；建议补 `analysis_intake_rejected` + i18n。
   - L5 `analyze` 端点对 `idempotency_key` 无长度/字符集约束 → 建议加长度上限并禁止 `:`，避免污染键分段语义。
