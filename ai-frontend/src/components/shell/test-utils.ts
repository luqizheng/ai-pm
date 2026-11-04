import { mount, type ComponentMountingOptions } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import type { Component } from "vue";

import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

/**
 * 壳组件的测试挂载：统一装上 pinia、i18n 与 router。
 *
 * 壳组件用到 RouterLink / useRoute，必须带 router；
 * 用 memory history，避免污染 jsdom 的 window.location。
 */
export function mountShell<T extends Component>(
  component: T,
  options: ComponentMountingOptions<T> = {} as ComponentMountingOptions<T>,
  router?: Router
) {
  const testRouter =
    router ??
    createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });

  const plugins = options.global?.plugins ?? [];
  return {
    router: testRouter,
    wrapper: mount(component, {
      ...options,
      global: {
        ...options.global,
        plugins: [createPinia(), i18n, testRouter, ...plugins]
      }
    } as ComponentMountingOptions<T>)
  };
}
