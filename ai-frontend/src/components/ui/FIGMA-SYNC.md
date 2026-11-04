# Figma 组件同步台账

**权威来源**：Figma `hNlGqLEMQpOVCZ0o5u3DkT`「13 — Component Library」
（<https://www.figma.com/design/hNlGqLEMQpOVCZ0o5u3DkT?node-id=13-37296>），共 7 族 41 个组件组，Dark/Light 双主题。

**同步约定**：

1. 已同步的组件在 `.vue` 文件头部带 `<!-- @figma-sync ... -->` 标记——**再同步前先看标记，避免重复同步**。
2. 同步 = 以 Figma 为准修改组件样式；数值落 `src/styles/tokens.css`（禁止裸色值/裸尺寸散落组件层）。
3. 每完成一组，在本表登记（状态 / 日期 / node id）。

## 已同步（✅ 全量 / 🟡 部分）

| 组件                             | Figma 组（node）                                          | 状态 | 日期       | 备注                                                                                                                                                                                           |
| -------------------------------- | --------------------------------------------------------- | ---- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Button                           | DS/Buttons/Button（13:32697）                             | ✅   | 2026-11-04 | 新增 tertiary/danger 变体；五态齐全；sm28/md36/lg44                                                                                                                                            |
| Input（TextInput/PasswordInput） | DS/Forms/TextInput（13:32788）+ PasswordInput（13:32847） | ✅   | 2026-11-04 | 外壳结构 + 内嵌 loading/error 图标 + 密码 mono；SearchInput（13:32905）待并入。同日（认证页）Input 增 `trailing` 插槽、独立 PasswordInput 组件补 eye 明文切换（带「显示/隐藏口令」可访问名称） |
| Checkbox                         | DS/Forms/Checkbox（13:33050）                             | ✅   | 2026-11-04 | 16/14/20 三档 + focus 2px accent + indeterminate；disabled 0.4 整体不透明度                                                                                                                    |
| FormField / Label                | DS/Forms/TextInput 结构（13:32788）                       | ✅   | 2026-11-04 | Label 13 Medium；帮助/错误 11px（micro）                                                                                                                                                       |
| Alert                            | DS/Feedback/Alert（13:34149）                             | ✅   | 2026-11-04 | info/success/warning=中性面（Figma 口径）；error=danger 面+#E69A9F 描边；size 三档 + dismissible                                                                                               |
| Separator                        | —（登录页 Rule 1px）                                      | ✅   | 2026-11-04 | 无需改样式（hairline token 命中）                                                                                                                                                              |
| Switch                           | DS/Forms/Switch（13:33107）                               | ✅   | 2026-11-04 | sm/md 32×20（thumb14）/lg 40×24（thumb18）；Off thumb=次文字色；focus 2px accent；禁用整体 0.4                                                                                                 |
| Badge                            | DS/Feedback/Badge（13:34167）                             | 🟡   | 2026-11-04 | neutral/accent/count 全量（accent 底 #242238 token）；success/warning/danger/info 为本地语义延伸未在 Figma 定义                                                                                |
| Toast                            | DS/Feedback/Toast（13:34097）                             | ✅   | 2026-11-04 | 图标式 + 中性面；error=danger 面；Action 态动作链接（11px accent）+ 新增 action 事件；sm/md/lg                                                                                                 |
| Tooltip                          | DS/Overlays/Tooltip（13:33673）                           | ✅   | 2026-11-04 | 浮层 elevated 面/radius 8/h36/主文字 13；新增 shortcut 快捷键 chip（mono 10px=nano token）                                                                                                     |
| Skeleton                         | DS/Feedback/Skeleton（13:34281）                          | ✅   | 2026-11-04 | 双色占位线（strong=border 色/base=elevated 色，tone prop）；radius 4；shimmer 保留                                                                                                             |
| Avatar                           | DS/Data/Avatar（13:34344）                                | ✅   | 2026-11-04 | 三档 28/36/44（复用控件 token）；elevated 底+描边；首字母两位 10/12/14 Medium；Fallback 图标；新增 size/online                                                                                 |
| Textarea                         | DS/Forms/Textarea（13:32944）                             | ✅   | 2026-11-04 | 外壳结构 + 内嵌 error 图标；三档高 72/88/112（--size-textarea\*）；交互态同 TextInput                                                                                                          |
| Select                           | DS/Forms/Select（13:33018）                               | ✅   | 2026-11-04 | 触发器同 Input 口径（canvas 底/三档/0.45 禁用）；面板 elevated+radius 8+内边 4；选项 h36/radius 8，选中与悬停底=accent 着色面；禁用项 0.35                                                     |
| Button（认证页扩展）             | 02 — Authentication / Patterns and states（5:2810）       | ✅   | 2026-11-04 | secondary 增 `surface="elevated"` 表面档（面板内 SSO/返回按钮 = elevated 底 #252A35，hover/active 叠主题薄纱）；主按钮六态与 5:2810 状态板一致                                                 |

## 待同步（按族）

- **Buttons**：IconButton（13:32734）——本地无独立组件（Button size="icon" 承接，需核对 28/36/44 方钮态色）
- **Forms**：Radio（13:33076，本地无）、DatePicker（13:33274，本地无）；~~SearchInput（13:32905）~~ 2026-11-04 已随 Global/Search/SearchInput.vue 落地（暂居 global/search/，归位 ui/ 待后续）
- **Navigation**：SidebarItem（13:33329）、NavigationGroup（13:33391）、Breadcrumb（13:33452）、Tab（13:33477）、TopBarItem（13:33520）
- **Overlays**：Dropdown（13:33607）、Popover（13:33652）、Modal（13:33764）、ConfirmationDialog（13:33877）；~~CommandPalette（13:34028）~~ 2026-11-04 已由 Global/Search/SearchPalette.vue 全量承接（含 Query/Results 状态机）
- **Feedback**：StatusIndicator（13:34203）、ProgressIndicator（13:34246）、EmptyState（13:34322）
- **Data**：AvatarGroup（13:34379）、ListItem（13:34428）、MenuItem（13:34522）；~~NotificationItem（13:34472）~~ 2026-11-04 已随 Global/Notification/NotificationItem.vue 落地（暂居 global/notification/）
- **AI**：EntryButton（13:34559）、CommandInput（13:34680）、Suggestion（13:34757）、AI StatusIndicator（13:34798）；~~HumanApprovalIndicator（13:34834）~~ 2026-11-04 已随 Global/Notification/HumanApprovalIndicator.vue 落地

## 已落 token 的 Figma 修正值（tokens.css）

- accent hover `#9B94FF` / pressed `#7971DE`（深色）；浅色 accent 基准 `#6257CE`
- accent 着色面 `#242238`（Badge Accent 底、Select 选中/悬停项；浅色候选 `#EEEBFF`）
- 禁用文字 `#697384`；禁用不透明度四档 0.38（Button）/ 0.4（Checkbox·Switch）/ 0.45（输入类）/ 0.35（菜单禁用项）
- 控件三档 28/36/44；spinner 16px；checkbox 16px；Switch 轨道 32/40 thumb 14/18；Badge 16/20/24；Textarea 72/88/112
- danger 反馈面三色 `#2C2028 / #66424D / #E69A9F`（浅色候选 `#FBECEE / #EDB6BA / #8F2430`）
- Input hover 描边 = 次文字色；字阶 body 13px、micro 11px（帮助/Toast 动作/Tooltip sm 等元文案）、nano 10px（快捷键 chip）

## 应用壳同步（Figma 页 03 — Application Shell）

**权威来源**：Figma `hNlGqLEMQpOVCZ0o5u3DkT`「03 — Application Shell」
（<https://www.figma.com/design/hNlGqLEMQpOVCZ0o5u3DkT?node-id=7-3454> 展开侧栏 /
<https://www.figma.com/design/hNlGqLEMQpOVCZ0o5u3DkT?node-id=7-3724> 折叠侧栏），
配合 Application Shell V1.1 文字定义（三种导航上下文、指令栏两态、右栏可关）。

| 区域               | 组件                     | Figma 帧（node）               | 状态 | 日期       | 备注                                                                                                                         |
| ------------------ | ------------------------ | ------------------------------ | ---- | ---------- | ---------------------------------------------------------------------------------------------------------------------------- |
| 顶栏 52px          | `shell/TopBar.vue`       | 7:3454 / 7:3724                | ✅   | 2026-11-04 | 产品标识 + 组织/项目上下文指示（**无 Workspace Switcher**）+ 360px 搜索入口（⌘K）+ 通知/助手/帮助/账户                       |
| 左导航 232/60px    | `shell/SideNav.vue`      | 7:3454 / 7:3724                | ✅   | 2026-11-04 | 四组上下文（个人/工作区/项目模块/管理）+ 项目标记行 + Repository 二级子菜单 + 激活指示条；折叠态纯图标 + 组分隔线 + 未读圆点 |
| 右上下文栏 320px   | `shell/ContextPanel.vue` | 7:3454                         | ✅   | 2026-11-04 | 当前项目 / 仓库详情 / 相关文档 / 助手可用上下文；**可独立关闭**                                                              |
| AI 指令栏 72–120px | `shell/CommandBar.vue`   | 7:3454（展开）/ 7:3724（收起） | ✅   | 2026-11-04 | 停靠式通栏（border-top，不压右栏）；AI 建议用 accent 色，人工确认提示用次文字色                                              |
| 页面标题区         | `shell/PageHeader.vue`   | 7:3454 / 7:3724                | ✅   | 2026-11-04 | 面包屑 + 21px 标题 + 描述 + 操作插槽                                                                                         |
| 提示浮层           | `shell/IconTooltip.vue`  | —                              | ✅   | 2026-11-04 | 壳内 asChild 版（触发器即真实元素，避免与 ui/tooltip 的固定 button 触发器嵌套）                                              |

### 壳相关的 token 修正（tokens.css）

- 壳级紧凑控件 32px（`--size-control-compact`）：顶栏搜索框 / 上下文芯片 / 子菜单行 / 键盘提示行
- 搜索入口宽 360px（`--size-search-width`）
- accent 着色面浅色 `#EEEBFF`（激活导航项底色；深色 `#242238` 与 Badge Accent 底同值）
- 全局 `box-sizing: border-box` + body margin 归零（否则 100dvh 的壳溢出视口 16px）

### 项目切换器（Figma 页 04 — Global Navigation / 07 — Project Switching）

**权威来源**：Project Context Header 三态（7:16866 Selected / 7:16874 Open / 7:16892 Empty）+
Project switcher menu（7:16466）+ 切换序列（7:16226，Before → Switching → After）。

| 组件                         | Figma 节点                            | 状态 | 日期       | 备注                                                                                                                          |
| ---------------------------- | ------------------------------------- | ---- | ---------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `shell/ProjectSwitcher.vue`  | 7:16866 / 7:16874 / 7:16892 + 7:16466 | ✅   | 2026-11-04 | 三态（Selected/Open/Empty）+ 双变体（header 40px 导航头 / chip 32px 顶栏芯片）+ 菜单（搜索行 / 当前项 Active / 切换中状态行） |
| `stores/shell.ts` 项目上下文 | 7:16226 切换序列                      | ✅   | 2026-11-04 | 两阶段切换：switching 期间不暴露旧项目数据（No stale project data），消费方=左导航/顶栏/右栏/指令栏                           |

### 壳相关的 token 修正（tokens.css）—— 本批新增

- 项目切换菜单宽 330px（`--size-project-menu-width`，Figma Project switcher menu）
- 就绪状态色 `--ws-color-status-ready`（深色 #8DC5AF = Figma Navigation Badge Type=Status 绿点；浅色 #3F8F70 保 UI ≥3:1）

### 本批新增 token（tokens.css）

- 全局搜索面板宽 640px（`--size-search-palette-width`，Figma「01」Initial palette 构成宽）
- 通知下拉宽 380px（`--size-notification-dropdown-width`，**实现推断值**：Figma 样本容器 520 内未给面板定宽，锚定右缘取 380，待设计侧确认）
- 通知中心分类过滤宽 368px（`--size-notification-filter-width`，Figma Category Select 宽）

### 全局组件 Global/Notification（Figma 帧编号 03 — Notification Dropdown / 04 — Notification Center）

**权威来源**：Figma `hNlGqLEMQpOVCZ0o5u3DkT`「03 — Notification Dropdown」
（<https://www.figma.com/design/hNlGqLEMQpOVCZ0o5u3DkT?node-id=33-81887>）+
「04 — Notification Center」
（<https://www.figma.com/design/hNlGqLEMQpOVCZ0o5u3DkT?node-id=33-82196>），
组件族 `Global/Notification/Bell`（33:87783）/ `Dropdown`（33:87790）/
`Record`（33:87784）/ `Center`（33:87792）/ `Detail`（33:87791）/
`Preferences`（33:85983）+ `TopBarSlice`（33:85795），Dark/Light 双主题。

| 组件                               | Figma 组（node）                       | 状态 | 日期       | 备注                                                                                                                                                                                    |
| ---------------------------------- | -------------------------------------- | ---- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `global/notification/NotificationBell.vue`    | Global/Notification/Bell（33:87783）   | ✅   | 2026-11-04 | 36×36 六态；**未读计数=文本徽标（Badge Count）非色点**（Figma 帧注记）；Active=accent 着色面（下拉展开中）；Disabled 0.4                                                                   |
| `global/notification/NotificationItem.vue`    | DS/Data/NotificationItem（13:34472）   | ✅   | 2026-11-04 | Unread=accent 面+13 SB 标题+6px 未读点；Read=surface 面+Regular；图标 message-square；元数据行「状态 · 时间 · 对象」状态词走 i18n                                                          |
| `global/notification/NotificationRecord.vue`  | Global/Notification/Record（33:87784） | ✅   | 2026-11-04 | 分类行（mono 11）+ Human attention 徽标（Badge Accent accent 描边）+ Item + 操作行（打开对象/标记已读·未读）；Selected=accent 面+accent 描边（中心）                                       |
| `global/notification/NotificationDropdown.vue` | Global/Notification/Dropdown（33:87790） | ✅ | 2026-11-04 | 铃铛锚定浮层（reka-ui Popover）；头部全部已读（ghost sm）+ All/Unread·N 页签（底 2px accent）+ 分类分组记录 + 「查看全部通知」；五态（含 Loading 骨架/Error 反馈框/Empty 已全部处理）；状态机与中心同 store |
| `global/notification/NotificationCenter.vue`   | Global/Notification/Center（33:87792） | ✅   | 2026-11-04 | 分类 Select + 仅看未读 Checkbox 双过滤 + 记录数行（mono 11）+ 记录列表（selected）；`/notifications` 路由落点（不在左导航树）                                                              |
| `global/notification/NotificationDetail.vue`  | Global/Notification/Detail（33:87791） | ✅   | 2026-11-04 | 标题/分类/元数据/分隔线/主题/上下文 + 审批指示器（可选）+ 操作行（处理请求仅待决 primary、标记已读仅未读）；中心页选中记录的详情面板                                                       |
| `global/notification/HumanApprovalIndicator.vue` | DS/AI/HumanApprovalIndicator（13:34834） | ✅ | 2026-11-04 | Pending accent 描边 shield-question / Approved shield-check / Rejected shield-x；**Approved 显式「非 AI 批准」、Rejected 显式「未授权任何变更」**（设计红线落地）                           |
| `global/notification/NotificationPreferences.vue` | Global/Notification/Preferences（33:85983） | ✅ | 2026-11-04 | 六类送达 Switch（复用 ui/switch）；**不含通道与路由配置**（Figma 帧注记）；独立组件（挂账户设置域，暂未接线）                                                                              |

**接线记录**：TopBar 旧铃铛 stub（IconTooltip+色点）替换为 NotificationDropdown
（Tooltip 略去——TopBarSlice 帧无 tooltip，且避免与 PopoverTrigger 双层 asChild 嵌套）；
`demoUnreadNotifications` 删除，未读数改由 `stores/notifications.ts`（三处同源）驱动；
新增 `/notifications` 路由（NotificationsView = Center + Detail）；
`global.notification.*` i18n 命名空间；Badge/Select/Checkbox/Switch/Skeleton/Alert/Button
全部复用已同步 ui 组件。

### 全局组件 Global/Search（Figma 帧编号 01 — Global Search / 02 — Search Results）

**权威来源**：Figma `hNlGqLEMQpOVCZ0o5u3DkT`「01 — Global Search」
（<https://www.figma.com/design/hNlGqLEMQpOVCZ0o5u3DkT?node-id=33-81343>）+
「02 — Search Results」
（<https://www.figma.com/design/hNlGqLEMQpOVCZ0o5u3DkT?node-id=33-81649>），
组件族 `Global/Search/CommandPalette`（33:87787）/ `Query`（33:87788）/
`Results`（33:87789）/ `ResultRow`（33:87782）/ `RecentSearches`（33:85727）/
`Input`（33:87811），Dark/Light 双主题。

| 组件                        | Figma 组（node）                                        | 状态 | 日期       | 备注                                                                                                                                     |
| --------------------------- | ------------------------------------------------------- | ---- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `global/search/SearchPalette.vue` | Global/Search/CommandPalette + Query + Results（33:87787/88/89） | ✅   | 2026-11-04 | ⌘K 唯一权限感知入口；五态状态机（idle/searching/results/no-results/error）；命令分 navigate·create（主面板）+ action（附加面板）；键盘 ↑↓/Enter/Esc；选中（accent 着色面）与键盘焦点（2px accent 描边）互相独立 |
| `global/search/SearchResultRow.vue` | Global/Search/ResultRow（33:87782）                     | ✅   | 2026-11-04 | 六类对象（Project/Repository/Requirement/Task/AI Run/Document）× 三态；行下 11px meta（不参与分类）；图标按对象类型区分（Figma 样本统一 file-text，实现细分） |
| `global/search/SearchRecentPanel.vue` | Global/Search/RecentSearches（33:85727）               | ✅   | 2026-11-04 | surface 面板 + mono 群组标签 + ListItem 行；点击行 = 回填查询重搜；空态由 Palette 内联 EmptyState（无动作按钮，DS/Feedback/EmptyState 待入库） |
| `global/search/SearchInput.vue` | DS/Forms/SearchInput（13:32905）+ Global/Search/Input（33:87811） | ✅   | 2026-11-04 | 四态（Default canvas 底/Hover elevated/Focus 2px accent/Disabled 0.45）+ 内嵌 loading spinner + Esc 芯片 + 11px helper；落地在 global/search（ui/SearchInput 独立组件待后续归位） |
| 组合级键盘提示条             | 两帧共有 Search keyboard guidance                        | ✅   | 2026-11-04 | sidebar 底 / mono 11（↑↓ 选择 · Enter 打开 · Esc 关闭）；DS 核心面板内嵌 footer 略去（避免重复提示，实现取舍已记录）                     |

**替换记录**：原 `ui/command/Command.vue`（旧 ⌘K 命令面板，仅 WorkspaceShell 消费）已删除，
`WorkspaceShell` 改接 `global/search/SearchPalette`（命令 + 最近搜索 + 演示搜索函数）；
`shell.command.*` i18n 键随之移除，新增 `global.search.*` 命名空间
（`locales/{zh-CN,en}/global.yaml`）。搜索进度条的 ProgressIndicator（13:34246）
与骨架行复用现有 ui/skeleton（DS/Feedback/ProgressIndicator 仍待入库）。

### 本批新增 token（tokens.css）

- 全局搜索面板宽 640px（`--size-search-palette-width`，Figma「01」Initial palette 构成宽）
