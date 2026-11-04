import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { i18n } from "./lib/i18n";
import { installVirtualScrollbars } from "./lib/virtual-scrollbar";
import { router } from "./router";

/**
 * ai-frontend 入口。
 *
 * 装配顺序：token 层 → pinia → i18n → router → 应用根。
 * 壳的五区域布局见 src/styles/index.css（.workspace-shell grid）。
 */
import "./styles/index.css";

createApp(App).use(createPinia()).use(i18n).use(router).mount("#app");

// 全系统虚拟滚动条（Figma 式 overlay，原生条已在 index.css 全局隐藏）
installVirtualScrollbars();
