# frontend 前端服务规范

PRISM 控制台 —— 公司自有的 admin 前端。**独立工程**，不基于任何后台模板框架。

## 必需文件

- [x] Dockerfile
- [x] nginx.conf
- [x] package.json
- [x] README.md
- [x] 国际化配置（locales/）
- [x] 入口（index.html → src/shell/entry.ts）

## 配置选项

- VITE_PORT: 本地端口，默认 `8848`
- VITE_API_BASE: API 基础路径，默认 `/api`
- VITE_PUBLIC_PATH: 公共路径，默认 `/`
- VITE_ROUTER_HISTORY: 路由模式（hash）

## 项目结构

| 目录 | 说明 |
|------|------|
| `src/styles/prism/` | **PRISM 设计系统 token 层**（唯一真源，见下） |
| `src/components/ui/` | 组件基座（reka-ui + scoped CSS + token） |
| `src/lib/` | 通用工具（cn 等） |
| `src/shell/api/` | 后端接口封装（基于 `@digua/http`） |
| `src/shell/store/` | Pinia store（auth / menu） |
| `src/shell/router/` | 路由与鉴权守卫（DB 动态菜单装载） |
| `src/shell/components/` | 壳布局（侧栏 / 顶栏 / 标签栏 / 命令面板 / 页面形态） |
| `src/shell/views/` | 框架系统页 |
| `locales/` | i18n 资源（`shell:` 与 `system:` 两个命名空间） |

## 硬性约束

1. **视觉数值唯一来源** = `src/styles/prism/tokens.css`。
   组件源码禁止出现裸 hex / rgba / 裸 px / 裸 ms。
   门禁：`node ../scripts/check-prism-tokens.js`（CI 阻断）。
2. **颜色不得写死**。语义色用 `--success/--warning/--danger/--info`；
   tint 底上的文字用强调档（`--accent-deep` / `--*-strong`）。
3. **样式一律 scoped CSS**，不走 Tailwind utility 拼装颜色（Tailwind 未引入）。
4. **图标用 `lucide-vue-next`**，不引入图标字体或 SVG 雪碧图。
5. **文案不得硬编码中文**，统一走 `locales/*.yaml`。
   门禁：`node ../scripts/check-i18n-hardcode.js` 与 `node ../scripts/check-i18n-namespace.js`。
6. **每个视图 ≥1 条 vitest 用例**，断言行为而非类名。
   门禁：`node ../scripts/check-vitest-coverage.js`。
7. **写操作按钮必须带权限校验**：`v-if="auth.hasPermission('<权限码>')"`。
8. **组件必须自带焦点环**，不依赖全局样式；`prefers-reduced-motion` 逐项降级。

## lmms 基础服务 SDK（必须用上游包，禁止自建）

上游 lmms（coder-micro-services）以 npm 包提供微服务客户端，发布到私有 registry
`http://192.168.2.12:14873/`（域名别名 `npm.kudi.repos`），随上游 tag 发版：
`iam-services`（登录/用户管理/token 管理）、`audit-services`（日志/合规导出/保留策略）、
`config-services`（配置中心）、`file-services`（multipart 上传/下载/预签名）、
`notification-services`（公告/未读数，不含 WS）、`prue-services`（动态路由/菜单配置版本）。

**硬性规则**：
1. 凡调用上述基础服务的 HTTP API，一律 `import` 对应 `*-services` 包——DTO、路径、错误归一以上游 SDK 为准；**禁止手写 fetch/axios 客户端、手抄 DTO/路径、复制上游实现**。
2. 升级走 registry 版本依赖（`pnpm add <svc>-services@<ver>` / `pnpm up`），**不做 fork**；SDK 缺能力到上游 lmms 仓提需求（上游改 → 发布 → 下游升包）。
3. 机器 `~/.npmrc` 需含 `//192.168.2.12:14873/:_authToken=<令牌>`（与 `npm.kudi.repos` 令牌通用）。

> 现状（2026-10-31）：壳的 IAM 调用已走 `iam-services`（`src/shell/api/iam.ts`，`/gw` 代理到 api-gateway）；`/v1/pure/*` 菜单调用为存量手写，待换 `prue-services`。`@digua/contracts`/`@digua/http` 同出自 lmms（现为 `packages/` 下 vendor 副本，升级走 registry）。

## 常用命令

```bash
pnpm dev                 # 开发
pnpm build               # 构建（会先构建 @digua/* 基础包）
pnpm test:unit           # 单测
pnpm typecheck           # 类型检查
pnpm lint:eslint         # 规范
```
