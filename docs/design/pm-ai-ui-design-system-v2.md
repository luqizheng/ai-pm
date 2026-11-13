# PM-AI UI Design System V2 — Slate & Muted Teal

> 状态：视觉方向已确认；本稿基于当前提供的全局 CSS Token 文件整理。  
> 品牌方向：石墨蓝灰 + 雾蓝青。深色主题稳重克制，浅色主题明亮清爽。  
> 本次只调整色彩系统，不改变现有布局、组件尺寸、字体、间距、圆角和交互结构。

## 1. 设计原则

1. **专业、克制、低刺激**：服务于研发管理、项目协作和复杂信息阅读。
2. **品牌色少量使用**：雾蓝青用于主要操作、当前选中、焦点和 AI 身份提示，不作为大面积背景。
3. **日夜同一品牌**：Dark / Light 是同一套色彩语义的两种明度表达，不是两套品牌。
4. **表面层级优先**：通过背景、卡片、嵌套表面和细边框区分层级，不依赖强阴影。
5. **语义色独立**：成功、警告、危险、信息色继续表达业务状态，不被品牌青色替代。
6. **禁止装饰性效果**：不使用紫色品牌色、紫蓝渐变、霓虹、Glow 或彩色发光阴影。

## 2. 核心色彩 Token

### 2.1 Dark Theme

| Token | 色值 | 用途 |
|---|---|---|
| Canvas | `#171C24` | 全局页面背景 |
| Sidebar | `#1D2631` | 侧栏与导航区域 |
| Surface | `#222A34` | 卡片与主要内容表面 |
| Elevated | `#2B3541` | 嵌套表面、悬浮层次 |
| Border | `#374452` | 分隔线与控件边框 |
| Accent | `#75A9A4` | 主操作、激活、AI 标识 |
| Accent Hover | `#8AB9B4` | 主色悬停 |
| Accent Pressed | `#63948F` | 主色按下 |
| Primary Text | `#E7EDF2` | 主文字 |
| Secondary Text | `#A8BBC9` | 次级文字 |
| Disabled Text | `#71818D` | 禁用文字 |

### 2.2 Light Theme

| Token | 色值 | 用途 |
|---|---|---|
| Canvas | `#F5F8FA` | 明亮冷白页面背景 |
| Sidebar | `#EDF2F5` | 浅蓝灰导航区域 |
| Surface | `#FFFFFF` | 卡片与主要内容表面 |
| Elevated | `#EAF0F3` | 嵌套表面、禁用与弱强调面 |
| Border | `#DCE4E9` | 轻量分隔线与控件边框 |
| Accent | `#397C78` | 主操作、激活、AI 标识 |
| Accent Hover | `#468B86` | 主色悬停 |
| Accent Pressed | `#306C68` | 主色按下 |
| Accent Surface | `#E5F1EF` | 激活导航、轻量品牌底色 |
| Primary Text | `#263A45` | 主文字 |
| Secondary Text | `#71818D` | 次级文字 |
| Disabled Text | `#A5B2BA` | 禁用文字 |

## 3. 使用规则

- 主按钮：使用主题对应的 `--ws-accent`，文字使用高对比度前景色。
- 导航选中：使用轻量 Accent Surface 或低透明度强调，不铺满高饱和品牌色。
- AI 标识：使用雾蓝青图标、细线或小面积底色；不为 AI 面板整体铺色。
- 卡片：Dark 使用 `--ws-surface`；Light 使用白色 `--ws-surface`。
- 重要信息：优先使用文字层级、图标和状态标签，不依赖彩色大卡片。
- 状态反馈：继续使用 info / success / warning / danger 语义 Token。
- 焦点态：继续使用 `--ws-focus-ring`，其颜色随主题 Accent 自动变化。

## 4. 明确禁止

- 紫色及紫蓝色作为品牌强调色。
- 大面积渐变背景或渐变按钮。
- 霓虹、Glow、彩色发光边框。
- 以颜色代替信息层级或业务状态。
- 在业务组件中直接写裸色值；必须引用全局 Token。

## 5. 本次变更范围

- 更新 `:root` 深色主题的基础表面、文字、品牌强调色和品牌标识色。
- 更新 `[data-theme="light"]` 浅色主题，使日间模式更明亮、清爽。
- 保留原有尺寸、字阶、字体、间距、圆角、动效和布局 Token。
- 保留现有 info / success / warning / danger 语义色，后续仍需通过自动化对比度测试验证各组件实际组合。

## 6. 实施前检查

1. 同步更新 `tokens.test.ts` 中旧色值断言和对比度测试基准。
2. 全局搜索并清理组件内硬编码的旧紫色色值。
3. 检查 Button、Badge、Navigation、Input Focus、AI Avatar、Authentication、状态提示等组件。
4. 在 Figma 中同步 Dark / Light 两套 Color Styles，并检查真实页面密度下的视觉效果。
5. 确认 Light 模式正文、禁用文字、状态色及按钮文字的可读性。

## 7. 交付边界

当前提供的文件是全局 CSS Token 文件。原始 `ai-pm/docs/ui/ai-pm-ui-design-system-v1.md` 与 `tokens.test.ts` 未包含在本次附件中，因此本稿是 V2 视觉基线及 CSS Token 更新稿，不代表已修改仓库中的原始 Markdown 文档或测试文件。
