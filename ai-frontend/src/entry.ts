import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { i18n } from "./lib/i18n";
import { installVirtualScrollbars } from "./lib/virtual-scrollbar";
import { designRoutesReady, router } from "./router";

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
  app.use(router);
  app.mount("#app");

  // 全系统虚拟滚动条（Figma 式 overlay，原生条已在 index.css 全局隐藏）
  installVirtualScrollbars();
});
