<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";

import { ToastHost } from "@/components/ui/toast";
import { WorkspaceShell } from "@/components/shell";

/**
 * 应用根 —— 按路由 meta.layout 切换外壳。
 *
 *   有 meta.layout（"auth" 认证页 / "bare" 参照板 / "design" 设计系统目录）
 *     → 裸 RouterView（全屏页，无导航外壳）；
 *       各全屏页骨架由页面自己承担（AuthLayout / DesignLayout 等）
 *   无 meta.layout → 应用壳 WorkspaceShell（壳内含自己的 RouterView）
 *
 * 判据用「有 layout 即无壳」而非枚举值，保证新增全屏页类型不必改这里。
 */
const route = useRoute();

/** 是否为无导航外壳的全屏页 */
const isBareLayout = computed(() => Boolean(route.meta.layout));
</script>

<template>
  <!-- 认证页/参照页/设计目录页直接渲染（各自内部已含全屏外壳）；
       其余走应用壳（壳内含自己的 RouterView）。
       两支互斥渲染，避免根级 RouterView 套壳内 RouterView 造成 depth+1 空渲染。 -->
  <RouterView v-if="isBareLayout" />
  <WorkspaceShell v-else />
  <!-- 全局 Toast 出口（G-UI-1）：viewport 为 fixed 定位，
       挂在根上同时覆盖 bare 页与壳页；状态是应用级单例，见 useToast.ts -->
  <ToastHost />
</template>
