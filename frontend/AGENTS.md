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
| `src/components/ui/markdown-editor/` | **Markdown / 富文本编辑器底座**（Vditor 封装，只认端口） |
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
9. **长文本 / 富文本录入一律用 `MarkdownField`**，禁止自己用 `<textarea>` 拼 Markdown 编辑器。
   详见下节「富文本 / Markdown 录入」。

## 富文本 / Markdown 录入（强制使用公共组件）

**结论：只要页面要"写一段内容"，就用 `MarkdownField`。不要新建编辑器、不要引第三方编辑器。**

```ts
import { MarkdownField } from "@/shell/components";
```

| 场景 | 用法 |
|------|------|
| 新建 / 编辑表单里的正文 | `<MarkdownField v-model="form.content" label="需求描述" />` |
| 详情页只读展示 | `<MarkdownField v-model="detail.content" mode="view" />` |
| 表单校验报错 | 传 `error="..."`；有 error 时不再显示 `description` |
| 纯编辑区（不带 label / 错误位） | 直接用底座 `@/components/ui/markdown-editor` 的 `MarkdownEditor` |

最小可用示例：

```vue
<MarkdownField
  v-model="form.content"
  label="需求描述"
  required
  height="520"
  :error="errors.content"
/>
```

### 硬性规则

1. **禁止引入其他编辑器**（tiptap / milkdown / quill / bytemd / wangEditor …）。
   需要新能力先改底座 `src/components/ui/markdown-editor/`，不要在业务页里另起一套。
2. **正文只存 Markdown 字符串**，`v-model` 就是唯一的真源。
   附件一律写成 `/v1/file/{id}`；**禁止把预签名 URL 写进正文**（15 分钟就失效，文档等于损坏）。
3. **不要自己写上传**。`MarkdownField` 已把 `uploader` / `resolveUrl` 默认接到
   `src/shell/api/attachments.ts`（走 `file-services`，预签名带缓存）。
   只有测试或要换存储后端时才显式传这两个 prop。
4. **单文件上限默认 100MiB**（multipart 上限）。确实要更大就别在组件里放大，
   改走预签名分片上传——RustFS 不支持 POST Object 表单直传。
5. **提示文案走 i18n**（`markdown.*` 命名空间），不要在组件里硬编码中文。

### 底座分层的分工（改代码前先看）

- `components/ui/markdown-editor/MarkdownEditor.vue`：编辑态，**只认端口**，不 import shell 层。
- `.../MarkdownViewer.vue`：只读渲染，附件链接自动换成可下载卡片。
- `.../file-ref.ts`：`/v1/file/{id}` 引用约定 + 就地把引用换成临时地址（只改 DOM，不回写正文）。
- `.../upload-handler.ts`：粘贴 / 拖拽 / 工具栏三条路径的统一上传回调。
- `shell/components/MarkdownField.vue`：业务字段，默认接好上面所有东西。
- `shell/api/attachments.ts`：`file-services` 适配器，含 15 分钟签名缓存。

### Vditor 三个已踩过的坑（改底座前必读）

1. **`upload.handler` 的返回值和官方类型不一致**：返回 string 会被当成提示条且**不插入内容**；
   只有返回 `null` 才算成功，插入要自己调 `insertValue`。按文档写会静默失效。
2. **解析引擎 Lute 默认从 unpkg 加载**，内网必失败且编辑器起不来。
   已由 `../scripts/sync-vditor-assets.js` 同步到 `public/vditor/`，`cdn` 指向同源，
   `dev` / `build` 前自动执行。**首次进编辑页要加载约 3.6MB（gzip 后约 1MB），需在 nginx 开 gzip。**
3. **`mode`（`wysiwyg` / `ir` / `sv`）构造时固定，不能运行时切换**，
   要切换只能销毁重建，撤销栈与光标会丢。当前产品口径是统一富文本界面、底层存 Markdown。

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
pnpm check:i18n          # i18n 门禁：命名空间对齐 + 硬编码中文（存量基线在 scripts/i18n-hardcode-baseline.json）
```
