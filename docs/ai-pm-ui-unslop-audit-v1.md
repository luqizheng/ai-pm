# ai-frontend 全站 UI 视觉一致性排查（unslop 审计）台账 v1

> 日期：2026-11-12 · 范围：`ai-frontend/src` 唯一活跃工程（旧 `frontend/` 冻结，未纳入）
> **性质：只读审计产物。本轮未修改任何源文件、token、共享组件、门禁脚本或基线文件。**
> 状态：**台账已交，批次与范围待用户确认；不得据此直接开工修改。**

---

## 0. 依据链与采信口径（先读依据，后开口）

| 优先级 | 依据 | 采信结论 |
|---|---|---|
| 1 | `docs/design/pm-ai-ui-design-system-v2.md` | 上位视觉规范，已确认**只调色彩，不改布局/组件尺寸/字体/间距/圆角/交互结构**；禁紫色品牌、大面积渐变、霓虹/Glow、彩色发光阴影；`info/success/warning/danger` 语义色独立表达业务状态 |
| 2 | `ai-frontend/src/styles/tokens.css` | token 唯一真源。`--radius-sm`/`--radius-md` 均 8px、`--radius-lg` 12px、`--radius-full` 999px；`--space-*` 4px 基数；`--duration-*`；`--font-size-*`；`--size-icon-*`；@media 断点白名单 `{1440,1240,1100,880,640}` |
| 3 | `docs/s01-项目初始化/ui/` 四份已冻结 UX 规范 | **口径修正**：用户口述的 `docs/ui/s01/` 目录不存在，冻结规范实际位于 `docs/s01-项目初始化/ui/`。其中 `s01-01-project-initialization-overview-ux-v1.md` 的 **R-S1「状态徽标必须颜色+文字+图标共同区分，不能只靠颜色」** 用作语义色项上位判据 |
| 3 | `docs/ai-pm-ui-design-system-v1.md` | 文档自身已标注「2026-11-06 已废弃、紫色品牌强调色废止」，仅作历史，不作为现行判据 |
| 4 | `unslop` 技能 `references/signs.md` | 本机已安装，按 9 组 signs 审计（表面与色彩 / 字体 / 图标 / 间距 / 文案 / 组件 / 首屏 hero / 装饰视觉 / 动效 + 模板感） |
| 5 | `ai-frontend/locales/{zh-CN,en}/*.yaml` | 双语文案 |
| 6 | `scripts/check-prism-tokens.js`（5 规则）、`scripts/check-i18n-hardcode.js`、`scripts/i18n-hardcode-baseline.json` | 门禁口径；本轮仅以 `--report` 只读模式取证 |

**硬边界（执行时同样生效）**
- 不得把「符合 V2 设计系统 / Figma 惯例」的既有设计当成 slop 来改：token 化 spinner/shimmer、veil 渐变、`mask-image` 渐隐、图表 `repeating-linear-gradient` 斜纹、avatar/dot/switch/radio/progress/chip 用 `--radius-full`、进行态「正在加载…」均判合规。
- 不得改共享组件（`src/components/shell`、`src/components/ui`）与全站 token；此类问题单列「转待裁」，只说明波及面，不给页面级补丁。
- 不得重跑或扩容 `i18n-hardcode-baseline.json` 的 fingerprints。
- 不做浏览器实渲染（本机无 Playwright / Chromium / ffmpeg）；视频生成、演示视频导出一律不做。

**已处理基线（不得重复改、不得重复列为发现）**：`views/project/list/components/OverviewMetrics.vue`、`ProjectToolbar.vue`、`locales/{zh-CN,en}/pm.yaml` 共 4 个文件——前两个指标状态点已改 neutral、仅「需要关注」保留 warning；subtitle 去句号、searchPlaceholder 去省略号；指标按钮与 attention chip 圆角已改 `--radius-sm`。

**已知全站级技术债（转待裁，非本轮修复，不重复上报）**：图标 16/18px 两档混用；`--radius-md` 与 `--radius-sm` 同为 8px 导致嵌套半径规则不成立；`shell/PageHeader.vue` 21px 标题 `-0.005em` 负字距。

---

## 1. 一句话总判

**全站无系统性 slop。** V2 色彩纪律（雾蓝青 + 语义色独立）、token 化、lucide 单集、`tabular-nums`、4px 间距在 187 view / 171 component 中基本贯彻，未见紫蓝渐变、发光、装饰球、hype 话术或模板化堆叠。按 unslop 分级属 **light slop（局部 polish）**：问题集中在「少数页面级语义色 / 图标 / 状态态的局部失配」，**无高严重度项**。本轮 13 条发现（中 6 / 低 7），其中 ✅ 页面级可改 7 条。

---

## 2. 批次划分表

| 批次 | 目录（`views/` 下） | 文件数（实测 / 给定） | 覆盖方式 | 结论 |
|---|---|---|---|---|
| B1 | `design/settings` | 20 / 20 | 逐文件全文精读 | 通过（1 低） |
| B2 | `project/command-center` | 16（入口 1 + components 15）/ 15 | 逐文件全文精读 | 6 条，主问题区 |
| B3 | `design/ai-interaction` | 13（12 + `_shared` 1）/ 12 | 逐文件全文精读 | **通过，零发现** |
| B4 | `settings` | 11（9 + `team` 2）/ 9 | 逐文件全文精读 | 2 条 |
| B5 | `auth` | 9 / 9 | 逐文件全文精读 | 1 条（落在 locales） |
| B6 | `project/requirements` | 9（入口 1 + components 8）/ 8 | 逐文件全文精读 | 并入 A06 / A12 / A13 |
| B7 | `project/requirement-detail` | 9（入口 1 + components 8）/ 8 | 逐文件全文精读 | 并入 A06 / A12 / A13 |
| B8 | `project/execution` | 8 / 8 | 逐文件全文精读 | 并入 A05 / A06 / A12 |
| B9 | 其余目录 | 92（推算） | 高信号 Grep 全量 + 8 文件精读 | 2 条 |
| — | `src/components/shell`、`src/components/ui` | 共享组件 | 按硬边界不逐审、只记波及面 | 转待裁 |

实测规模：`views` 187 个 `.vue`、`components` 171 个 `.vue`。B2/B3/B4/B6/B7 实测比给定数多 1–2 个（入口 view、`team/`、`_shared/`），已全部纳入，未遗漏。`design/` 展示页自带画框与样板断点（门禁豁免），刻意并排展示多种变体/色板不计为 slop。

**明确「通过」的维度**：B3 全批 13 个文件零发现；B1/B2/B4/B5/B6/B7/B8/B9 在「表面与色彩、字体、图标集、间距、组件复用、首屏 hero、装饰视觉、动效、模板感、断点白名单」均通过。

---

## 3. 发现台账（按严重度排序）

| 编号 | 批次/文件:行号 | 问题描述 | 严重度 | 处置 | 建议动作 | 回退方式 |
|---|---|---|---|---|---|---|
| A01 | B2 `views/project/command-center/components/InitializationCard.vue:91-100` | 状态点 tone 三元只映射 `success/danger/info`，**遗漏 `warning`**；`real-data.ts:225` 的 `waiting_for_human`（待人工处理）被降级为灰色 neutral，最需被看见的状态失去语义色，违反 S01 `R-S1` | 中 | ✅ 页面级可改 | 第 91-100 行整段改为 `:status="item.tone"`（`InitItemView.tone` 枚举与 `StatusIndicatorTone` 已对齐，公共组件支持 `ui-status--warning`）；保守则至少补 `item.tone === 'warning' ? 'warning'` 分支。`size="sm"` 不动 | 还原原三元表达式 |
| A02 | B4 `views/settings/TeamProviderCard.vue:264-270` | 测试连接结果 `.is-ok` 用品牌青 `--ws-accent`、`.is-bad` 用中性 `--ws-text-secondary`，均未用语义色；失败态甚至无颜色区分，违反 V2 §1.5/§3/§4（同文件 `statusVariant()` 已正确用 success/danger Badge） | 中 | ✅ 页面级可改 | 第 265 行 → `var(--ws-color-success-text)`；第 269 行 → `var(--ws-color-danger-text)` | 还原两行 CSS 声明 |
| A03 | B9 `views/project/tasks/readiness/ExecutionReadinessView.vue:224-230` | 状态徽标用 `✓`/`⛔`/`⚠` Unicode 符号充当图标，破 lucide 单集；同文件 `:125`/`:207` 已用 `<ShieldCheck>`、`<ClipboardCheck>`；`⛔` 为 emoji，跨平台基线不一 | 中 | ✅ 页面级可改 | 第 224-230 行改为 `<component :is>` 映射 `CircleCheck` / `CircleX`（或 `Ban`）/ `TriangleAlert`，`:size="14"` 沿用同文件；`:543-560` 的 `.rd-item-state--pass/--warn/--block` 语义色类保持不动 | 还原三元文本（或保留文本并补 `aria-hidden` + `role="img"`，但不满足 lucide 单集） |
| A04 | B5 `ai-frontend/locales/{zh-CN,en}/auth.yaml:196`（渲染点 `views/auth/AuthPatternsView.vue:227`） | 把 V1 **已废弃紫 `#8B83F6`** 标为现行「AI 强调色」，与 `tokens.css` 的 `--ws-accent`（`#75a9a4`/`#397c78`）矛盾；V2 实施前检查第 2/3 条明确要求清理旧紫色并检查 Authentication。仅参照板底注文案，CSS 实际未渲染紫色，属规范一致性/误导风险 | 中 | 🟡 需权衡 | 两个 yaml 第 196 行改为「AI 强调色 `--ws-accent`（雾蓝青）」/「AI accent `--ws-accent` (muted teal)」，或加「V1 历史稿」限定词；`AuthPatternsView.vue` 无需改（仅渲染 key）。需设计裁决「Figma 参照页历史标注是否随 V2 同步」 | 还原两个 yaml 行 |
| A05 | B2 `.../command-center/components/AttentionPanel.vue:47-48,69-74`；B8 `views/project/execution/ExecutionFrame.vue:88,112,118,124,253-267` | **聚合计数数字本身**被 inline style / 四条 CSS 覆盖染成 `--danger-strong`/`--warning-strong`/`--color-*-text`；同屏 7 项计数颜色不统一，「活跃执行数」是中性运行计数却用 `info`，接近 V2「以颜色代替信息层级」。**与已处理基线的区分**：基线改的是指标卡旁「状态点」，此处是计数数字本身、且为 B2/B8 新批次文件，机制与文件均不同 | 中 | 🟡 需权衡 | 去掉 `AttentionPanel.vue:71` 的 `:style` 与 `ExecutionFrame.vue:253-267` 四条颜色覆盖，`.exec-overview__value` / `.pm-attention__count-num` 固定 `var(--ws-text-primary)`；语义交给已有 label / `variant="warning"` chip / 前置状态点；`ExecutionFrame.vue:88` 的 `info` 改 `neutral`。若裁决保留，至少改走 class 而非 inline style | 恢复 `:style`、`countColor` 常量、四条 CSS 覆盖与 88 行三元式 |
| A06 | B2 `RunHistoryCard.vue:48-63`、`DeliverablesCard.vue:49-90`、`ActivityTimelineCard.vue:57-89`、`AiAgentsOverviewCard.vue:82-106`、`KnowledgeSnapshotCard.vue:43-49`、`MembersCard.vue:28-38`（均在 `command-center/components/`）；B8 S05 七页 `HumanInterventionView.vue:36`、`ExecutionHistoryView.vue:138`、`CollaborationView.vue:114`、`ExecutionCenterView.vue:358`、`RunDetailView.vue:106` 等 | `loading`/`empty` 状态不齐：六张卡在数据为空时只剩列头 + 空 `<ul>`，而 `real-data.ts:393-404` 明确发布构建下交付物/知识快照/成员返回空数组；S05 加载中直接显示「暂无…」空态或空表格，被误读为「无数据」。`s05/execution.yaml:33` 已定义 `common.loading` 却仅 ExecutionCenter 使用。维度 g | 中 | 🟡 需权衡 | 各卡 `<ul>` 后补 `v-if="!rows.length"` 空文案（`--text-disabled` / `--font-size-nano`），需新增 `emptyLabel?: string` prop 并在 `CommandCenterView.vue` 传 `t('pm.overview.empty')`；S05 各页在 `v-else` 前统一 `<EmptyState>` / `<Skeleton>`，`ExecutionCenterView.vue:358` 把 loading 行上移到内容区顶部并包裹五个 Card。**波及 13 文件，非页面级单行补丁** | 移除新增 `v-if` 空文案行、prop 透传与 S05 loading 块 |
| A07 | B4 `views/settings/SettingsTeamAiView.vue:409` | 刷新按钮以字符 `↻` 当图标，无 `aria-hidden`、无尺寸 token，破 lucide 单集（本批其余图标统一 `lucide-vue-next`） | 低 | ✅ 页面级可改 | script 增 `import { RefreshCw } from "lucide-vue-next"`；第 409 行 → `<RefreshCw :size="16" aria-hidden="true" />`；保留 `data-test="refresh-models"` 与 `:title` | 还原 `↻` 文本节点并移除 import |
| A08 | B2 `.../command-center/components/AttentionPanel.vue:281-302` | 「直达」图标按钮只有 `:focus-visible`，缺 `:hover`；同面板 `.pm-attention__inbox:hover`、`SectionHead.vue:87`、`ActivityTimelineCard.vue:216` 均有 hover，同屏交互反馈不齐。维度 g | 低 | ✅ 页面级可改 | 第 293 行后补 `.pm-attention__open:hover { color: var(--accent-hover); }`，并在第 292 行补 `transition: color var(--duration-fast) var(--ease-standard);`（与 inbox 对齐） | 移除新增 `:hover` 规则与 transition |
| A09 | B9 `views/project/tasks/components/TaskStatusChip.vue:38`、`TaskPriorityChip.vue:32` | `border-radius: var(--radius-full, var(--radius-sm))` 回退写法：`--radius-full` 已在 `tokens.css:116` 定义，回退值永不生效，实际渲染为 pill；且塞入语义不同的 8px 方角 token，未来 token 缺失会降级成方角。同控件组 `TaskDetailView.vue:317/:517`、`DependencyGraphView.vue:1018` 均为纯 `var(--radius-full)`。**无视觉差异，仅写法/防混淆项** | 低 | 🟡 需权衡 | 两行改为 `border-radius: var(--radius-full);`，与同组一致 | 保留原样即零视觉差异、零风险 |
| A10 | B1 `views/design/settings/SettingsProjectMembersView.vue:163-165` | 状态徽标硬编码 `variant="success"` + 固定「已加入」，未读 `member.status`；同目录 `SettingsTenantMembersView.vue:127-136` 已按 status 推导。fixture 三条均 `active`，**当前渲染结果正确、视觉无差错**，属防御性对齐；另复用租户域 i18n key `settings.tenant.members.*` | 低 | ✅ 页面级可改 | 第 163-165 行改为 `:variant="member.status === 'active' ? 'success' : 'info'"` + 按 status 取 `statusActive/statusInvited`，与租户页一致。彻底消除跨域 key 复用需增补 `settings.project.members.*`（触及全站 locales，另议） | 还原 `variant="success"` + 固定 `statusActive` |
| A11 | B2 `.../components/KnowledgeSnapshotCard.vue:80` 与 `MembersCard.vue:67`（同屏 `CommandCenterView.vue:316-335` `.cc-rail`） | 右栏上下相邻、类名同为 `__pill` 的两个容器圆角不一致：数据 pill 用 `--radius-sm`(8px)、身份 pill 用 `--radius-full`(999px)。单看两处均合规（成员 pill 内含 Avatar，属排除项），但**同屏同类 pill 口径不统一**；V2 明确「不改圆角」，需设计裁决 | 低 | 🟡 需权衡 | 二选一：走数据 pill 口径则 `MembersCard.vue:67` 改 `var(--radius-sm)`；走身份 pill 口径则 `KnowledgeSnapshotCard.vue:80` 改 `var(--radius-full)`。**勿两处都改** | 还原对应一行 |
| A12 | B6 `.../requirements/components/RequirementDetail.vue:669-675`；B7 `.../requirement-detail/components/OverviewCard.vue:139-145`、`DetailContextPanel.vue:518-522`；B8 `execution/ExecutionHistoryView.vue:339`、`RunDetailView.vue:471-479` | 同组件内一部分日期时间字段有 `font-variant-numeric: tabular-nums`、另一部分没有（如 `.eh-muted` 承载 `formatDateTime(started_at)` 无，`.eh-mono` 有），数字列宽不齐。聚合计数侧已合规 | 低 | ✅ 页面级可改 | 上述 5 处 CSS 规则各补一行 `font-variant-numeric: tabular-nums;`（约在 340 / 478 / 144 / 674 / 521 行） | 删除补的声明 |
| A13 | B6 `views/project/requirements/RequirementsView.vue:85,393-397`；B7 `RequirementDetailView.vue:70,253`、`.../components/DetailHeader.vue:108-123` | `acting` 仅有 JS 闸 `if (acting.value) return`，页头「新建/编辑/提交确认/确认/拒绝」按钮无 `:disabled`，命令在途仍可点击且无视觉反馈；同页 `AcceptanceCard.vue:140` 已 `:disabled="acting"`。维度 g | 低 | 🟡 需权衡 | 第 393/395 行补 `:disabled="acting \|\| !projectId"`，并把 `:acting` 透传至 `RequirementDetail` / `DetailHeader` 各自 Button。**注意**：`RequirementsView.test.ts:243/271/296` 有点击流程用例，须先确认 `flushPromises` 顺序，避免 acting 未复位时被拦 | 移除新增 `:disabled` 与 prop 透传 |

### 3.1 ✅ 项测试断言核查明细（执行前必读）

7 条 ✅ 项经逐一检查同名/同目录 `*.test.ts`，**全部为运行时渲染断言**（`mount` + `wrapper.text().toContain(...)` / `data-testid` / `trigger("click")` / `emitted(...)`），**无 `readFileSync` + `source(...).toContain(...)` 源码静态断言、无 `classes()` 锁 CSS/圆角/颜色**。该类源码静态断言仅存在于 `src/components/ui/`（`ui.contract.test.ts`、`Avatar.test.ts:138`、`Dialog.test.ts:55`、`Tabs/Toast.test.ts`）与 `src/styles/*`（`tokens.test.ts` 等），**均不在本轮改动范围**。结论：**A01/A02/A03/A07/A08/A10/A12 均不影响任何现有测试断言**。

| 编号 | 关联测试文件 | 断言性质 | 影响 |
|---|---|---|---|
| A01 | `InitializationCard.test.ts:38,70-76,90-91` | 已有 `tone:"warning"` 数据，但仅文本断言，无 status/class/颜色断言 | 不影响；建议**顺带补** `expect(wrapper.find('.ui-status--warning').exists()).toBe(true)` 固化回归（建议，非本轮改动） |
| A02 | `SettingsTeamAiView.test.ts:169` | 断言 `[data-test='test-result']` 文本含 `42` | 不影响（改 `color`） |
| A03 | `ExecutionReadinessView.test.ts:117-123,129-134,189-190` | `wrapper.text()` / `[data-testid]`，无 `✓/⚠/⛔` 文本断言 | 不影响 |
| A07 | `SettingsTeamAiView.test.ts` | 未断言该按钮文本/字符（仅 `[data-test='create-model']`） | 不影响 |
| A08 | `AttentionPanel.test.ts:93` | 仅 `trigger("click")` 断言 `emitted("open")` | 不影响 |
| A10 | `settings-views.test.ts` | 成员页仅断言「开发者」与 `.ui-select__trigger` 数量 | 不影响 |
| A12 | 三目录同名 test | 运行时文案 `toContain`，纯 CSS 改动 | 不影响 |

**另需留意（非本轮改动）**：`settings-views.test.ts` 用 `toContain` 绑定了 `settings.yaml` 的 `personal.ai.boundary`、`common.readOnlyMsg`、`common.permissionDeniedMsg`，后续改文案须核对；`pages-controls.test.ts:67-71` 绑定 `AiInteractionControlsView.vue` 的硬编码英文 `DesignFrame label` 与 `INT-2041`，若后续英译中/改画框标签须同步改测试。

---

## 4. 只上报不改（本轮不动，含转待裁全站级）

| 项 | 位置 | 判定 | 波及面 |
|---|---|---|---|
| 装饰性彩色轨道点 | B2 `ActivityTimelineCard.vue:60,124-153` | 组件注释自认「装饰性分类标记（aria-hidden）」，按 `item.tone` 上语义色，且第 66-71 行 StatusIndicator chip 用同一 tone 重复表达；但设计稿 `3:7722` 明确画了彩色轨道点，属 **Figma 既有设计**，按硬边界不当 slop | 若裁决收敛：六变体合并为单一 `--text-secondary`，语义交给类别 chip；波及 command-center 动态区 |
| uppercase + 宽字距 | B7 `OverviewCard.vue:121-123`、`SpecificationCard.vue:99-101`、`AcceptanceCard.vue:221-223`、`TraceabilityCard.vue:256-258`；B6 `RequirementDetail.vue:630-632` | unslop Type 组命中，但 **V2 明确「不改字体/字距」**，`--tracking-caption` 是既有 token → 转待裁，本轮不建议擅自改 | 详情页 5 张卡 + 全站 `--tracking-caption` 消费方 |
| locales placeholder 尾省略号 | `settings.yaml:205/260`（中英各 2）、`shell.yaml:4,7,184,234,237`、`global.yaml:4`、`s04/tasks.yaml:32` 等约 30 处 | 命中维度 e「省略号 placeholder」，但属**全站文案惯例**；单改 4 行会造成同站风格分裂，且落在共享 locales 而非 `views/` → 转待裁 | 全站 locales 双语 + shell/global/s04 消费方；已处理基线 `pm.yaml` 已有专项处理，勿重复 |
| `pm.requirementReview.entryLabel` / `pm.requirementImpact.entryLabel` 未定义 | 引用点 `RequirementsView.vue:382`、`DetailHeader.vue:122` | `locales/` 全目录无定义，属 **i18n 数据完整性债**（同类 `requirementReview.pageTitle/pageDesc` 亦缺）；疑似被其它 i18n 门禁覆盖，按「门禁输出违规不重复上报」仅登记，非视觉项 | `locales/{zh-CN,en}/pm.yaml` 的 requirementReview / requirementImpact 段 |
| 认证页演示账号 | `ResetPasswordView.vue:34`、`SessionExpiredView.vue:31,33`、`UnauthorizedView.vue:31,33`、`AuthPatternsView.vue:69,91` | `alex.morgan@acme.dev` / `Acme Research` 与 Figma 认证九帧及全站 mock persona（`api/mock/auth.ts`、`stores/auth.test.ts`）一致，属既有演示数据约定，不判假数据 | 若裁决清理生产回显，波及 `api/mock/auth.ts`、`stores/auth.test.ts`、`LoginView.test.ts`、`ForgotPasswordView.test.ts` |
| 共享组件门禁债 | `components/ui/combobox/Combobox.vue:427,432`（裸色值 `rgba(` + 裸时长 `1.4s`）、`components/shell/AssistantColumn.vue:235`（裸色值 `#f07e88`） | 落在硬边界内，**不得给页面级补丁**；只登记 | `components/ui/` 全套基础控件消费方 + `shell` 三栏壳，改动会让 187 个 view 的渲染基线整体漂移 |
| 图标尺寸字面量 | 如 `ExecutionCenterView.vue:131` 等 `:size="14"`、`SettingsTeamMembersView.vue` `:size="15"/"13"` | 已知全站债「图标 16/18px 两档混用」的具体实例，不重复 | `views/` + `components/shell/` + `components/ui/` 图标调用口径 |
| 已知全站技术债 | `tokens.css:112-113`、`shell/PageHeader.vue` | 用户已点名、`转待裁、非本轮` | 全站 token 层；本轮不给页面补丁 |

**核实后判「通过」的高信号线索（不计发现）**：`ExecutionReadinessView.vue:538/:658`、`DependencyGraphView.vue:702/:1018`、`TaskDetailView.vue:317/:517`、`AssistantView.vue:553` 的 `--radius-full` 均为 dot/chip/pill，属排除项；`TimelineView.vue:1009` 的 `repeating-linear-gradient` 是 `.tl-bar.is-blocked` 45° 斜纹，色值全为 `var(--ws-color-danger-surface)` + `transparent`，token 化、小面积、表达 blocked 语义；`BootstrapView.vue:349-357` 的 `✓`/`—` 位于 `<dd>` 数据单元格而非图标位；`views/project/design/use-design-project.ts:13` 的 `⚠` 在注释里。B3 的 `AiInteractionDecisionView.vue:149-166` 三色 swatch 为刻意并排展示色板。

---

## 5. 既有门禁债（两个门禁本就红，单列，不与本轮发现混，不顺手修）

| 门禁 | 实测违规数 | 按文件聚合 |
|---|---|---|
| `node scripts/check-prism-tokens.js --report` | **146 条** | `views/project/knowledge/KnowledgeView.vue` **97**、`views/admin/AdminSettingsView.vue` **46**、`components/ui/combobox/Combobox.vue` 2、`components/shell/AssistantColumn.vue` 1 |
| `node scripts/check-i18n-hardcode.js --report` | **39 处硬编码中文** | `views/settings/team-llm-types.ts` 16、`views/settings/model-source.ts` 8、`views/settings/SettingsTeamAiView.vue` 7、`lib/viz/specAdapter.ts` 7、`router/index.ts` 1 |

**与给定口径的两处偏差（供校正）**：
1. i18n 39 处中 **8 处在 `views/settings/` 之外**（`specAdapter.ts` 7 + `router/index.ts` 1），并非全部集中在 settings。
2. PRISM 裸色值除 `KnowledgeView.vue` 外，**`AdminSettingsView.vue` 另有 46 条**（占总量 31.5%）。

**基线状态**：`scripts/i18n-hardcode-baseline.json` 当前 `fingerprints` 为**空数组 `[]`**，即 39 处均非基线豁免。本轮**未重跑 `--update-baseline`、未扩容 fingerprints、未修改该文件**。

---

## 6. 能力边界与未核实项

1. **本阶段严格只读**：未创建/修改/删除任何源文件、token、共享组件、门禁脚本或基线；门禁仅以 `--report` 只读模式取证，未跑构建、未跑测试。
2. **渲染态对齐与密度未经核实**：本机无 Playwright / Chromium / ffmpeg，未做浏览器实渲染。深色/浅色两套主题的实际对比度、hover/focus 视觉反馈、1440×900 下横向溢出、卡片密度与首屏留白**均未经视觉验证**；本报告全部结论基于源码静态判读。
3. B9 的 92 个 view 为**高信号 Grep 全量扫描 + 8 个文件精读**（`ExecutionReadinessView`、`TaskStatusChip`、`TaskPriorityChip`、`TaskDetailView`、`DependencyGraphView`、`TimelineView`、`AssistantView`、`BootstrapView`），非逐文件 100% 全文精读；`components/shell`、`components/ui` 依硬边界未逐审。
4. 双语 locales 仅核了批次相关 yaml，**未做 1431 key 全量对齐复核**。
5. 视频生成、演示视频导出按硬边界未做；unslop 的方向确认 / 九遍修复 / redline 故事板均未进入（本轮仅审计）。

---

## 7. 交接说明（给新 session）

- **本台账不等于开工许可**。批次与范围仍待用户确认；执行前须按项目 `todo.md` §0.1 走开工门禁，并重新读取本文件 §0 依据链与硬边界。
- 建议的批次顺序沿用 §2 表；每批只改该批文件 + 它引用的文案，**不改共享组件与全站 token**。
- 🟡 项（A04/A05/A06/A09/A11/A13）需设计裁决或波及多文件，**不得与 ✅ 项混在同一提交**；A06 波及 13 文件、A13 可能影响点击流程用例时序，须单独验证。
- 转待裁项（§4）与既有门禁债（§5）**本轮一律不动**，尤其不得顺手修 `KnowledgeView.vue` / `AdminSettingsView.vue` 的裸色值，不得扩容 `i18n-hardcode-baseline.json`。
- 修改完成后按项目既有验收基线回归：`vue-tsc`、`eslint`、`vitest`、`check:prism-tokens`、`check:i18n`；注意两个门禁**修改前本来就是红的**，须以「不新增违规」为判据，而非要求全绿。
- 若要做渲染态对齐与密度核实，需先补齐 Playwright / Chromium，再按 §6 第 2 条补验。
