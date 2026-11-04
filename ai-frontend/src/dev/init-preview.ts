import { createApp, h } from "vue";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";

import { i18n } from "@/lib/i18n";
import "@/styles/index.css";
import InitializationOverviewView from "@/views/project/initialization/InitializationOverviewView.vue";

/**
 * S01-01 初始化总览的**临时预览入口**（不进产品包）。
 *
 * 用途：在导航树被其他会话改动、整个应用起不来时，仍能单独预览本页。
 * 自建最小路由，不引入 @/router 的导航树。
 */

const router = createRouter({
  history: createMemoryHistory(),
  routes: [
    { path: "/", redirect: "/project/initialization" },
    { path: "/project/initialization", component: InitializationOverviewView },
    { path: "/:pathMatch(.*)*", component: { render: () => h("div") } }
  ]
});

const app = createApp({
  render: () =>
    h("div", { class: "preview-canvas" }, [h(InitializationOverviewView)])
});

app.use(createPinia());
app.use(i18n);
app.use(router);
app.mount("#app");
