import { createApp, watch } from "vue";
import { createPinia } from "pinia";
import { i18n, useI18n } from "./i18n";
import { documentTitle } from "./i18n-helpers";
import { router } from "./router";
import ShellApp from "./ShellApp.vue";
import { onUnauthorized } from "./api/client";
import { useAuthStore } from "./store/auth";
import "@/styles/prism/index.css";
import "@/styles/workspace/index.css";

/**
 * PRISM 壳的应用入口。
 *
 * 这个前端是独立工程：没有 legacy 入口，没有模板框架的运行时。
 */
const app = createApp(ShellApp);
const pinia = createPinia();

app.use(pinia);
useI18n(app);
app.use(router);

// token 彻底失效的全局兜底：必须在这里注册，
// 不能挂在登录页上 —— 用户带着 localStorage 会话直接进业务页时从未挂载过登录页。
onUnauthorized(() => {
  useAuthStore(pinia).reset();
  if (router.currentRoute.value.name !== "login") {
    router.replace({
      name: "login",
      query: { redirect: router.currentRoute.value.fullPath }
    });
  }
});

// 页面标题随路由切换（Phase 2 §5.3）；标题走 i18n-helpers，切语言也随当前路由刷新
router.afterEach(to => {
  document.title = documentTitle(to);
});

// 切语言后标题里的页面名要跟着翻（afterEach 只在导航时触发）
watch(i18n.global.locale, () => {
  document.title = documentTitle(router.currentRoute.value);
});

router.isReady().then(() => {
  app.mount("#app");
});
