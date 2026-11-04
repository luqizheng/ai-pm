import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { i18n } from "./lib/i18n";
import { router } from "./router";

/**
 * ai-frontend 入口。
 *
 * 装配顺序：token 层 → pinia → i18n → router → 应用根。
 * 壳的五区域布局见 src/styles/index.css（.workspace-shell grid）。
 */
import "./styles/index.css";

createApp(App).use(createPinia()).use(i18n).use(router).mount("#app");
