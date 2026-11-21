<script setup lang="ts">
/**
 * PA-06 三段式布局：Main（结构化项目工作）+ 页内右栏（证据与 AI 推理）。
 *
 * 画布口径：右栏是**页面内容的一部分**（320px 白卡，屏专属），与全局壳的
 * ContextPanel（288px 全局唯一实例）无关——不写 data-context，不影响壳层
 * grid 模板（use-shell-chrome 单源）。
 */
</script>

<template>
  <div class="collab-layout">
    <div class="collab-layout__main">
      <slot />
    </div>
    <aside v-if="$slots.context" class="collab-layout__aside">
      <slot name="context" />
    </aside>
  </div>
</template>

<style scoped>
.collab-layout {
  display: flex;
  align-items: flex-start;
  gap: var(--space-5);
}

.collab-layout__main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.collab-layout__aside {
  flex: 0 0 320px;
  min-width: 0;
}

@media (max-width: 1100px) {
  .collab-layout {
    flex-direction: column;
  }

  .collab-layout__aside {
    flex-basis: auto;
    width: 100%;
  }
}
</style>
