<script setup lang="ts">
/**
 * 应用根 —— 只挂应用壳。
 *
 * 壳的五区域与折叠开关见 components/shell/WorkspaceShell.vue；
 * 路由表见 router/index.ts（由 router/navigation.ts 派生）。
 */
import { WorkspaceShell } from "@/components/shell";
</script>

<template>
  <WorkspaceShell />
</template>
