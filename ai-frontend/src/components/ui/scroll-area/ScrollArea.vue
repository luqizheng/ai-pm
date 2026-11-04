<script setup lang="ts">
import { ScrollAreaRoot, ScrollAreaViewport } from "reka-ui";
</script>

<template>
  <ScrollAreaRoot class="ui-scroll-area" type="auto">
    <ScrollAreaViewport class="ui-scroll-area__viewport">
      <slot />
    </ScrollAreaViewport>
  </ScrollAreaRoot>
</template>

<style scoped>
.ui-scroll-area {
  position: relative;
  overflow: hidden;
}

/* reka-ui 的视口在组件内部渲染，需 :deep() 命中 */
:deep(.ui-scroll-area__viewport) {
  width: 100%;
  height: 100%;
  overflow: auto;
}

/* 滚动条指示统一交给全局虚拟滚动条（src/lib/virtual-scrollbar.ts）：
 * 视口是普通 overflow:auto 容器，会被自动跟踪并在 hover/滚动时浮出
 * thumb（双轴自动），本组件不再渲染自带的 reka 滚动条，也不再有
 * orientation 取向 API（2026-11-04 全局虚拟滚动条裁决）。 */
</style>
