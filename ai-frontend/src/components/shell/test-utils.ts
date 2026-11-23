import {
  flushPromises,
  mount,
  type ComponentMountingOptions
} from "@vue/test-utils";
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

/**
 * 等待一次导航真正落定后再读取 `currentRoute`。
 *
 * 业务路由组件多为懒加载（`() => import(...)`）；在 vitest 里该动态导入首次
 * 要跨多个宏任务（vite-node 模块拉取是异步 I/O，Windows 上单视图可到秒级），
 * `flushPromises()` 只等一个宏任务——导航仍在途，`currentRoute` 还是旧值。
 * 组件内触发的跳转（表单提交、行点击等）拿不到 push 的 promise，测试侧无法
 * `await router.push(...)`，统一经本助手等 `afterEach` 确认，而不是多刷几次
 * flush 或放宽断言。导航被守卫重定向到别处时报错并附当前路径，便于定位。
 */
export async function waitForRoute(
  router: Router,
  path: string
): Promise<void> {
  await flushPromises();
  if (router.currentRoute.value.path === path) return;

  await new Promise<void>((resolve, reject) => {
    const stopAfterEach = router.afterEach(to => {
      if (to.path === path) done();
    });
    const stopOnError = router.onError(error => fail(error));
    // 兜底而非等待手段：正常路径由 afterEach 落定；超过（远高于实测冷导入
    // 秒级、低于 vitest 20s 用例超时）就带着当前路径报错，避免无限挂起
    const timer = setTimeout(
      () =>
        fail(
          new Error(
            `navigation to "${path}" did not settle (current: ${router.currentRoute.value.fullPath})`
          )
        ),
      15_000
    );

    function cleanup() {
      stopAfterEach();
      stopOnError();
      clearTimeout(timer);
    }
    function done() {
      cleanup();
      resolve();
    }
    function fail(error: unknown) {
      cleanup();
      reject(error);
    }
  });
  await flushPromises();
}
