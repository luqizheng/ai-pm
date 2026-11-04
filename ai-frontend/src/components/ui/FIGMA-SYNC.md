# Figma 组件同步台账

**权威来源**：Figma `hNlGqLEMQpOVCZ0o5u3DkT`「13 — Component Library」
（<https://www.figma.com/design/hNlGqLEMQpOVCZ0o5u3DkT?node-id=13-37296>），共 7 族 41 个组件组，Dark/Light 双主题。

**同步约定**：

1. 已同步的组件在 `.vue` 文件头部带 `<!-- @figma-sync ... -->` 标记——**再同步前先看标记，避免重复同步**。
2. 同步 = 以 Figma 为准修改组件样式；数值落 `src/styles/tokens.css`（禁止裸色值/裸尺寸散落组件层）。
3. 每完成一组，在本表登记（状态 / 日期 / node id）。

## 已同步（✅ 全量 / 🟡 部分）

| 组件                             | Figma 组（node）                                          | 状态 | 日期       | 备注                                                                                                            |
| -------------------------------- | --------------------------------------------------------- | ---- | ---------- | --------------------------------------------------------------------------------------------------------------- |
| Button                           | DS/Buttons/Button（13:32697）                             | ✅   | 2026-11-04 | 新增 tertiary/danger 变体；五态齐全；sm28/md36/lg44                                                             |
| Input（TextInput/PasswordInput） | DS/Forms/TextInput（13:32788）+ PasswordInput（13:32847） | ✅   | 2026-11-04 | 外壳结构 + 内嵌 loading/error 图标 + 密码 mono；SearchInput（13:32905）待并入                                   |
| Checkbox                         | DS/Forms/Checkbox（13:33050）                             | ✅   | 2026-11-04 | 16/14/20 三档 + focus 2px accent + indeterminate；disabled 0.4 整体不透明度                                     |
| FormField / Label                | DS/Forms/TextInput 结构（13:32788）                       | ✅   | 2026-11-04 | Label 13 Medium；帮助/错误 11px（micro）                                                                        |
| Alert                            | DS/Feedback/Alert（13:34149）                             | ✅   | 2026-11-04 | info/success/warning=中性面（Figma 口径）；error=danger 面+#E69A9F 描边；size 三档 + dismissible                |
| Separator                        | —（登录页 Rule 1px）                                      | ✅   | 2026-11-04 | 无需改样式（hairline token 命中）                                                                               |
| Switch                           | DS/Forms/Switch（13:33107）                               | ✅   | 2026-11-04 | sm/md 32×20（thumb14）/lg 40×24（thumb18）；Off thumb=次文字色；focus 2px accent；禁用整体 0.4                  |
| Badge                            | DS/Feedback/Badge（13:34167）                             | 🟡   | 2026-11-04 | neutral/accent/count 全量（accent 底 #242238 token）；success/warning/danger/info 为本地语义延伸未在 Figma 定义 |
| Toast                            | DS/Feedback/Toast（13:34097）                             | ✅   | 2026-11-04 | 图标式 + 中性面；error=danger 面；Action 态动作链接（11px accent）+ 新增 action 事件；sm/md/lg                  |
| Tooltip                          | DS/Overlays/Tooltip（13:33673）                           | ✅   | 2026-11-04 | 浮层 elevated 面/radius 8/h36/主文字 13；新增 shortcut 快捷键 chip（mono 10px=nano token）                     |
| Skeleton                         | DS/Feedback/Skeleton（13:34281）                          | ✅   | 2026-11-04 | 双色占位线（strong=border 色/base=elevated 色，tone prop）；radius 4；shimmer 保留                              |
| Avatar                           | DS/Data/Avatar（13:34344）                                | ✅   | 2026-11-04 | 三档 28/36/44（复用控件 token）；elevated 底+描边；首字母两位 10/12/14 Medium；Fallback 图标；新增 size/online  |
| Textarea                         | DS/Forms/Textarea（13:32944）                             | ✅   | 2026-11-04 | 外壳结构 + 内嵌 error 图标；三档高 72/88/112（--size-textarea*）；交互态同 TextInput                            |
| Select                           | DS/Forms/Select（13:33018）                               | ✅   | 2026-11-04 | 触发器同 Input 口径（canvas 底/三档/0.45 禁用）；面板 elevated+radius 8+内边 4；选项 h36/radius 8，选中与悬停底=accent 着色面；禁用项 0.35 |

## 待同步（按族）

- **Buttons**：IconButton（13:32734）——本地无独立组件（Button size="icon" 承接，需核对 28/36/44 方钮态色）
- **Forms**：SearchInput（13:32905）、Radio（13:33076，本地无）、DatePicker（13:33274，本地无）
- **Navigation**：SidebarItem（13:33329）、NavigationGroup（13:33391）、Breadcrumb（13:33452）、Tab（13:33477）、TopBarItem（13:33520）
- **Overlays**：Dropdown（13:33607）、Popover（13:33652）、Modal（13:33764）、ConfirmationDialog（13:33877）、CommandPalette（13:34028）
- **Feedback**：StatusIndicator（13:34203）、ProgressIndicator（13:34246）、EmptyState（13:34322）
- **Data**：AvatarGroup（13:34379）、ListItem（13:34428）、NotificationItem（13:34472）、MenuItem（13:34522）
- **AI**：EntryButton（13:34559）、CommandInput（13:34680）、Suggestion（13:34757）、AI StatusIndicator（13:34798）、HumanApprovalIndicator（13:34834）

## 已落 token 的 Figma 修正值（tokens.css）

- accent hover `#9B94FF` / pressed `#7971DE`（深色）；浅色 accent 基准 `#6257CE`
- accent 着色面 `#242238`（Badge Accent 底、Select 选中/悬停项；浅色候选 `#EEEBFF`）
- 禁用文字 `#697384`；禁用不透明度四档 0.38（Button）/ 0.4（Checkbox·Switch）/ 0.45（输入类）/ 0.35（菜单禁用项）
- 控件三档 28/36/44；spinner 16px；checkbox 16px；Switch 轨道 32/40 thumb 14/18；Badge 16/20/24；Textarea 72/88/112
- danger 反馈面三色 `#2C2028 / #66424D / #E69A9F`（浅色候选 `#FBECEE / #EDB6BA / #8F2430`）
- Input hover 描边 = 次文字色；字阶 body 13px、micro 11px（帮助/Toast 动作/Tooltip sm 等元文案）、nano 10px（快捷键 chip）
