# ai-frontend 前端规范

**AI R&D Workspace 前端基础工程**（独立工程，2026-11-04 自 `frontend/` 复制重组：
旧 PRISM 壳 `src/shell/` 与 AI Workspace `src/workspace/` 已整体移除，页面按本文规范**全部重建**）。

**本文档是唯一权威规范：后续所有前端设计与实现工作，都必须以 §一「Frontend Foundation Design」为基准。**
配套：高保真设计系统 V1.0 详见 `../docs/ui/ai-pm-ui-design-system-v1.md`。

> **当前状态（2026-11-04）**：UI 冻结已解除；**应用壳骨架已落地**——
> 规范源 = Figma 组件库 `hNlGqLEMQpOVCZ0o5u3DkT`（node 13:37296，7 族 41 组）
>
> - Figma「03 — Application Shell」两帧（7:3454 展开侧栏 / 7:3724 折叠侧栏）。
>   组件同步进度见 `src/components/ui/FIGMA-SYNC.md`；已同步组件带 `@figma-sync` 头标记。
>   业务页面（登录页等）仍待组件同步完成后实施。

- [x] 入口：`index.html` → `src/entry.ts`（pinia + i18n + router）
- [x] 视觉真源：`src/styles/tokens.css`（设计系统 V1.0 token，`--ws-*` 变量）
- [x] 组件基座：`src/components/ui/`（reka-ui + scoped CSS + token）
- [x] **应用壳**：`src/components/shell/`（顶栏 / 左导航 / 右上下文栏 / AI 指令栏 / 页面标题区）
- [x] 导航与路由：`src/router/navigation.ts`（单源）+ `src/router/index.ts`（派生）
- [x] Dockerfile / nginx.conf / `locales/` / `packages/`（`@digua/contracts`、`@digua/http`）

---

## 一、AI R&D Workspace — Frontend Foundation Design（规范原文，必须遵守）

### 1. Project Background

Design the global frontend foundation for an AI-powered software R&D management platform.

This is not a conventional project management dashboard.

The platform enables human project managers and AI agents to collaborate throughout the software development lifecycle.

The system includes:

- Project Management
- Repository Management
- Code Discovery
- Requirement Management
- AI Execution
- Knowledge Management
- Development Environment
- Personal Workspace

The human remains responsible for project decisions. AI agents assist with analysis, planning, execution, and recommendations.

### 2. Design Objective

Before designing any business-specific pages, establish a complete and reusable frontend foundation.

Focus on:

1. Authentication and login experience.
2. Global application shell.
3. Global navigation and menus.
4. Workspace and project switching.
5. User profile and account menu.
6. Global search and command interface.
7. Notifications and activity center.
8. Global AI assistant entry point.
9. Global settings.
10. Reusable UI components and interaction states.

Do **NOT** design detailed business workflows at this stage.

### 3. Existing Design System — Mandatory

Follow the established AI R&D Workspace High-Fidelity Design System V1.0.

Visual direction:

- Professional developer IDE experience.
- Linear-like clarity and simplicity.
- Restrained, sophisticated, information-dense interface.
- Dark theme by default.
- Light theme supported.
- Low-saturation blue-purple accent.
- Avoid excessive gradients, oversized cards, decorative illustrations, and consumer-app aesthetics.

Core colors:

| Token            | Value     |
| ---------------- | --------- |
| Canvas           | `#111318` |
| Navigation       | `#171A21` |
| Surface          | `#1D212A` |
| Elevated Surface | `#252A35` |
| Border           | `#353B48` |
| Primary Text     | `#E9ECF3` |
| Secondary Text   | `#99A2B2` |
| AI Accent        | `#8B83F6` |

Layout specifications:

- Global top bar: 52px
- Left navigation: 232px
- Collapsed navigation: 60px
- Right context panel: 320px
- Bottom AI command bar: 72–120px
- Spacing system: 4px base unit
- Body text: 13px
- Page title: 20–22px
- Standard row height: 36px
- Standard border radius: 8px

Keep the same design language across all screens.

### 4. Design Principles

- Desktop-first, optimized for 1440px and 1920px screens.
- Consistent global navigation.
- Predictable interaction patterns.
- Reusable components instead of page-specific styling.
- Clear visual hierarchy.
- Compact but readable information density.
- Keyboard-friendly interactions.
- All interactive elements must have hover, focus, active, disabled and loading states where appropriate.
- Support dark and light themes.

### 5. Expected Deliverables

Create a complete Frontend Foundation design package.

Organize the Figma pages into:

1. 01 — Design Tokens
2. 02 — Authentication
3. 03 — Application Shell
4. 04 — Global Navigation
5. 05 — User & Workspace Menus
6. 06 — Global Search & Commands
7. 07 — Notifications
8. 08 — Global AI Entry
9. 09 — Global Settings
10. 10 — Component Library
11. 11 — Responsive & Interaction States

Use reusable Figma components and variants.

Do not independently redesign the existing V1.0 design system.

The objective is to create the visual and interaction foundation that every subsequent business module will reuse.

---

## 二、工程现状（2026-11-04 重组后）

| 路径                           | 说明                                                                                                                                                                                                                                                     |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/entry.ts` + `src/App.vue` | 入口：装配 pinia + i18n + router，`App.vue` 只挂应用壳                                                                                                                                                                                                   |
| `src/styles/tokens.css`        | **视觉数值唯一真源**：`--ws-*` token + 组件消费的尺寸/字阶/动效原语 + PRISM 同名变量别名（全部落到 `--ws-*`）                                                                                                                                            |
| `src/styles/index.css`         | 三栏布局基类（workspace-shell 等，设计系统 V1.0 §二）+ 全局 border-box / body margin 归零                                                                                                                                                                |
| `src/components/ui/`           | 组件基座（reka-ui + scoped CSS + token）；**同步状态见 `FIGMA-SYNC.md`**                                                                                                                                                                                 |
| `src/components/shell/`        | **应用壳**（Figma「03 — Application Shell」两帧）：`WorkspaceShell` 壳根 / `TopBar` 顶栏 / `SideNav` 左导航（展开 232px、折叠 60px）/ `ContextPanel` 右上下文栏 / `CommandBar` AI 指令栏 / `PageHeader` 页面标题区 / `IconTooltip` 壳内 asChild 提示浮层 |
| `src/router/navigation.ts`     | **导航树 = 菜单与路由的单源**（`src/router/index.ts` 的路由表由它派生，杜绝漂移）                                                                                                                                                                        |
| `src/stores/shell.ts`          | 壳状态：左导航折叠（持久化 `aipm.sidebar-collapsed`）/ 右栏开关 / 指令栏展开态                                                                                                                                                                           |
| `src/views/placeholder/`       | 占位视图（业务模块落地时按路由替换）                                                                                                                                                                                                                     |
| `src/api/`                     | pm 业务 API 封装（保留复用；`mock/fixtures.ts` 是未实现写端点的活契约）                                                                                                                                                                                  |
| `src/api/base/`                | **基础接口层**（自旧 `shell/api/` 打捞：`client.ts` http 实例 + iam / prue / audit / config / file / notification / users 适配 + `token-storage.ts`；登录/菜单基础重建时直接复用）                                                                       |
| `src/lib/`                     | 通用工具（cn 等）+ `i18n.ts`（vue-i18n 组装，`../../locales` glob 加载）+ `test-utils.ts`（测试挂载辅助）                                                                                                                                                |
| `locales/`                     | i18n 资源（`shell.yaml` 承载 `shell.app.*` 壳文案）                                                                                                                                                                                                      |

已删除：`src/shell/`（传统管理壳）、`src/workspace/`（AI 工作台页面）、`src/styles/prism/`
（旧 PRISM 主题层，2026-11-04 用户裁决放弃，原语已并入 `styles/tokens.css`）——
**不要从旧目录恢复或参照旧页面实现**。

### 壳的约定（Application Shell V1.1）

- **三种导航上下文不得混成一张扁平菜单**：个人 / 工作区 / 项目模块（仅选中项目时出现）/ 管理。
  分组定义在 `src/router/navigation.ts`，路由由它派生。
- **不引入 Workspace Switcher**：顶栏的组织芯片只是上下文指示（静态展示，无切换实体）；
  **项目可切换**——`components/shell/ProjectSwitcher.vue`（左导航项目头 + 顶栏芯片双变体），
  切换状态机在 `stores/shell.ts`（两阶段：switching 期间不显示旧项目数据，Figma「07」）。
- 折叠态经壳根元素 `data-sidebar` / `data-context` 属性切换，由 `index.css` 的 grid 模板消费。
- 快捷键：⌘/Ctrl+K 命令面板，⌘/Ctrl+J 切换指令栏展开态。
- **AI 建议与人工确认的动作必须视觉可辨**：建议用 accent 色，人工确认提示用次文字色。
- 共享 `ui/tooltip` 的触发器固定是 `<button>`；壳内图标按钮/链接用
  `components/shell/IconTooltip.vue`（`asChild`，触发器即真实元素），避免嵌套交互元素。
- 右上下文栏**可选且可独立关闭**，不是每页常驻。

## 三、硬性约束（沿用并适配新结构）

1. **视觉数值唯一来源** = `src/styles/tokens.css`。
   组件源码禁止出现裸 hex / rgba / 裸 px / 裸 ms。
   门禁：`node ../scripts/check-prism-tokens.js`（CI 阻断）。
2. **Figma 为组件样式权威**：以组件库 `13:37296` 为准；同步约定与进度见
   `src/components/ui/FIGMA-SYNC.md`，已同步组件带 `@figma-sync` 头标记，勿重复同步。
3. **颜色不得写死**。语义色用 `--ws-color-success/--ws-color-warning/--ws-color-danger/--ws-color-info`。
4. **样式一律 scoped CSS**，不引入 Tailwind。
5. **图标用 `lucide-vue-next`**，不引入图标字体或 SVG 雪碧图。
6. **文案不得硬编码中文**，统一走 `locales/*.yaml`。
   门禁：`node ../scripts/check-i18n-hardcode.js` 与 `node ../scripts/check-i18n-namespace.js`。
7. **每个视图 ≥1 条 vitest 用例**，断言行为而非类名（`pnpm test:unit`）。
8. **组件必须自带焦点样式**（Figma 口径：2px accent 描边），不依赖全局样式；`prefers-reduced-motion` 逐项降级。
9. **富文本 / Markdown 录入**：底座 `src/components/ui/markdown-editor/`（Vditor 封装）保留；
   业务包装层（原 `shell/components/MarkdownField`）随基础重建恢复；禁止引入其他编辑器
   （tiptap / milkdown / quill / bytemd / wangEditor …）。正文只存 Markdown 字符串，
   附件一律 `/v1/file/{id}`，禁止把预签名 URL 写进正文。
10. **lmms 基础服务 SDK（必须用上游包，禁止自建）**：凡调用 iam / audit / config / file /
    notification / prue 基础服务，一律 import 对应 `*-services` 包（私有 registry
    `http://192.168.2.12:14873/`）；DTO、路径、错误归一以上游 SDK 为准；禁止手写 fetch/axios
    客户端、手抄 DTO/路径、复制上游实现；SDK 缺能力到上游 lmms 仓提需求，不做 fork。

## 四、常用命令

```bash
pnpm install            # workspace：根 + packages/contracts + packages/http
pnpm dev                # http://localhost:8848
pnpm build              # 先 build:packages 再 vite build
pnpm test:unit          # vitest（先 build:packages）
pnpm typecheck          # vue-tsc --noEmit --skipLibCheck（先 build:packages）
pnpm lint               # eslint --max-warnings 0 + prettier --check
pnpm check:prism-tokens # token 门禁（../scripts/check-prism-tokens.js）
pnpm check:i18n         # i18n 门禁（命名空间对齐 + 硬编码中文，基线 scripts/i18n-hardcode-baseline.json）
```

dev 代理：`/api` → pm-api（`PM_API_URL`，默认 `127.0.0.1:8090`）；`/gw` → lmms api-gateway（`LMMS_GATEWAY_URL`，默认 `127.0.0.1:3000`）。
