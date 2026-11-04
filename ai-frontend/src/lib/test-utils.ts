import {
  flushPromises,
  mount,
  type ComponentMountingOptions
} from "@vue/test-utils";
import { createPinia } from "pinia";
import type { Component } from "vue";
import { i18n } from "./i18n";

/**
 * 壳组件的测试挂载：统一装上 pinia 与 i18n。
 *
 * 壳的组件用 `useI18n()` 取文案，没有 i18n 插件会直接抛
 * "Need to install with `app.use` function"。
 */
export function mountShell<T extends Component>(
  component: T,
  options: ComponentMountingOptions<T> = {} as ComponentMountingOptions<T>
) {
  const plugins = options.global?.plugins ?? [];
  return mount(component, {
    ...options,
    global: {
      ...options.global,
      plugins: [createPinia(), i18n, ...plugins]
    }
  } as ComponentMountingOptions<T>);
}

/**
 * 等待异步动作彻底落地。
 *
 * vue-router 的跳转要经过异步组件解析与多级 promise，一轮 flushPromises
 * 往往不够（currentRoute 仍是旧值）；这里补一个宏任务节拍再 flush 一次。
 */
export async function settle() {
  await flushPromises();
  await new Promise(resolve => setTimeout(resolve, 0));
  await flushPromises();
}

/**
 * 轮询等待路由到达目标路径。
 *
 * 懒加载路由组件（`() => import(...)`）首次导航时需经 Vite 转换加载，
 * 耗时可能超过单个宏任务节拍，因此用轮询而非固定等待。
 */
export async function waitForRoute(
  router: { currentRoute: { value: { path: string } } },
  path: string,
  timeout = 5000
) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (router.currentRoute.value.path === path) {
      await flushPromises();
      return;
    }
    await new Promise(resolve => setTimeout(resolve, 25));
    await flushPromises();
  }
  throw new Error(
    // i18n-ignore（测试辅助的断言消息，非界面文案）
    `waitForRoute timeout: expected ${path}, actual ${router.currentRoute.value.path}`
  );
}
