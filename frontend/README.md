# ai-pm frontend

AI-PM 平台前端 = **PRISM 壳**（上游 `low-memory-micro-services/frontend`，shadcn 风格设计系统 + 管理壳）+ **pm 业务视图**（`src/shell/views/pm/` + `src/api/`）。

壳真源在上游仓库；本目录是按 `.agents/skills/scaffold`（上游 repo-local skill）于 2026-10-29 从上游基线 scaffold 出来的下游实例。后续壳升级走上游的 **update-shell** skill（共同祖先判定法），不要手改壳基线。

## 命令

```bash
pnpm install            # workspace：根 + packages/contracts + packages/http
pnpm dev                # http://localhost:8848（默认带 mock，见下）
pnpm test:unit          # vitest（279 用例）
pnpm lint               # eslint --max-warnings 0 + prettier --check
pnpm typecheck          # vue-tsc --noEmit --skipLibCheck
pnpm build              # 先 build:packages 再 vite build
pnpm check:prism-tokens # token 数量门禁（../scripts/check-prism-tokens.js）
```

## 数据模式（mock / 真后端）

pm-api 已实现壳认证与 `/v1/pm/*` 读端点，当前 `.env.development` 为 `VITE_USE_MOCK = 0`（真实数据）。
mock 层保留作两种用途：后端不可用时的本地预览（置回 `1`），以及 **pm-api 的接口活契约**
（`src/api/mock/fixtures.ts`，尚未实现的写端点以它为准）。

- 真模式：登录走 `core.auth_session` 本地会话（任意口令可登录，IAM 上游接管前过渡）；
  菜单由 pm-api 下发（当前为与 mock 相同的静态树，菜单表落地后改查库）。
- 写操作（审批通过/驳回、需求编辑等）前后端都尚未实现——契约先落 fixtures，再两端并行。

## 业务层结构（非壳基线，可自由改动）

```
src/api/            pm 契约层：types.ts（领域类型）/ pm.ts（端点）/ status.ts / format.ts / mock/
src/shell/views/pm/ 业务页：overview / projects / requirements / tasks / approvals / audit / settings
locales/*/pm.yaml   业务 i18n 命名空间（pm.*）
build/mock-plugin.ts dev mock 层
```

壳基线（同步范围，勿随手改）：`src/styles/prism/`、`src/components/ui/`、`src/lib/`、
`src/shell/`（业务 views 除外）、`locales/`（pm.yaml 除外）。

## 对上游基线的已知漂移（update-shell 时会按"下游已改"裁决）

- 品牌化：`index.html` / `src/shell/entry.ts` / `locales/*/shell.yaml` 的产品名 → **AI-PM 平台**；
  `LoginPage.test.ts` 品牌断言同步改名。
- `pnpm lint --fix` 对 8 个壳基线文件做了纯 prettier 格式修复（上游自身 lint 门禁当前同样红）。
- prettier 钉在 `~3.6.2`（上游声明 ^3.6.2，3.9 格式有漂移）。
- `vite.config.ts`：代理仅保留 `/api → pm-api`，移除上游 probe 端点；新增 mock 插件；
  相对导入补 `.ts` 扩展（vite 8 要求）+ tsconfig `allowImportingTsExtensions`。
- `src/components/ui/command/Command.vue` 的遮罩修复与上游
  `fix/command-overlay-opacity`（efffc2dd）同补丁，上游合入 main 后此处即自然收敛。
