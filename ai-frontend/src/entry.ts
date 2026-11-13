import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { i18n } from "./lib/i18n";
import { installVirtualScrollbars } from "./lib/virtual-scrollbar";
import { designRoutesReady, router } from "./router";
import { installSessionExpiryRedirect } from "./router/session-guard";
import { onUnauthorized } from "./api/base/session";
import { useAuthStore } from "./stores/auth";

/**
 * ai-frontend 入口。
 *
 * 装配顺序：token 层 → pinia → i18n → router → 应用根。
 * 壳的五区域布局见 src/styles/index.css（.workspace-shell grid）。
 */
import "./styles/index.css";

const app = createApp(App);
app.use(createPinia()).use(i18n);

/**
 * router 必须在设计目录路由注册完成后才装：router.install() 会立刻触发首航，
 * 首航若早于注册，/design 只匹配 404 兜底并被认证守卫踢去登录页——而设计目录
 * 对匿名访客始终开放（见 src/router/index.ts 的 designRoutesReady 注释）。
 * 设计目录仅开发期存在，生产构建里该 promise 立即兑现，不产生额外等待。
 */
void designRoutesReady.then(() => {
  // 会话彻底失效（401 + 刷新失败）时：先清本地登录态，再统一跳登录页。
  // 两条请求通道（@digua/http 与 lmms SDK 的 sessionFetch）共用这一份订阅。
  onUnauthorized(() => useAuthStore().resetSession());
  installSessionExpiryRedirect(router);

  app.use(router);
  app.mount("#app");

  // 全系统虚拟滚动条（Figma 式 overlay，原生条已在 index.css 全局隐藏）
  installVirtualScrollbars();
});
