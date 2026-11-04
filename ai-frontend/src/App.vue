<script setup lang="ts">
/**
 * 应用根 —— 按路由 meta.layout 切换外壳。
 *
 *   有 meta.layout（如 "auth" / "design"）→ 裸 RouterView（全屏页，无导航外壳）
 *   无 meta.layout                      → 应用壳 WorkspaceShell
 *
 * 两种全屏页各自的骨架由页面自己承担：
 *   - 认证页 components/auth/AuthLayout.vue
 *   - 设计系统目录页 views/design/DesignLayout.vue
 *
 * 判据用「有 layout 即无壳」而非枚举值，保证新增全屏页类型不必改这里。
 */
import { computed } from "vue";
import { useRoute } from "vue-router";

import { WorkspaceShell } from "@/components/shell";

const route = useRoute();

/** 是否为无导航外壳的全屏页 */
const isBareLayout = computed(() => Boolean(route.meta.layout));
</script>

<template>
  <RouterView v-if="isBareLayout" />
  <WorkspaceShell v-else />
</template>
