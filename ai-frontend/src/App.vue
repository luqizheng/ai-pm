<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";

import { WorkspaceShell } from "@/components/shell";

/**
 * 应用根 —— 按路由 meta 切换外壳。
 *
 *   meta.layout = "auth"  —— 认证页（自带无导航的认证页外壳 AuthLayout）
 *   meta.layout = "bare"  —— 整屏参照页（认证状态参照板）
 *   其余                   —— 应用壳（壳内含 RouterView，见 WorkspaceShell）
 *
 * 壳的五区域与折叠开关见 components/shell/WorkspaceShell.vue；
 * 路由表见 router/index.ts（由 router/navigation.ts 派生 + router/auth.ts 认证路由组）。
 */
const route = useRoute();

/** 认证页 / 参照页：不套应用壳，直接渲染页面组件 */
const isStandalone = computed(
  () => route.meta.layout === "auth" || route.meta.layout === "bare"
);
</script>

<template>
  <!-- 认证页/参照页直接渲染（各自内部已含 AuthLayout 外壳）；
       其余走应用壳（壳内含自己的 RouterView）。
       两支互斥渲染，避免根级 RouterView 套壳内 RouterView 造成 depth+1 空渲染。 -->
  <RouterView v-if="isStandalone" />
  <WorkspaceShell v-else />
</template>
