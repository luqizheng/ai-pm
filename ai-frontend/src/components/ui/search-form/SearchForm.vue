<script setup lang="ts">
/**
 * 统一搜索过滤行（SearchForm）。
 *
 * 解决的共性问题：ui-select 的触发器根样式 `width: 100%`（且 SelectRoot
 * 不渲染 DOM，触发器即 flex 子项），放进 `flex-wrap` 工具行时每个筛选
 * 控件假设主轴尺寸 = 容器整宽 → 各自换行、独占一整行（竖排缺陷）。
 *
 * 契约：
 *   - `#search` 具名插槽：搜索框（SearchInput），限宽
 *     `--size-list-search-width`，不参与换行计算；
 *   - 默认插槽：Select / 按钮等筛选控件，按内容取宽（覆盖 width:100%），
 *     与搜索框横排、窄屏自然换行；
 *   - `#append` 具名插槽：右对齐区（视图切换等，margin-left auto）。
 *
 * 用法：
 *   <SearchForm>
 *     <template #search><SearchInput … /></template>
 *     <Select … /><Select … />
 *     <template #append><ViewToggle … /></template>
 *   </SearchForm>
 */
</script>

<template>
  <div class="search-form">
    <div v-if="$slots.search" class="search-form__search">
      <slot name="search" />
    </div>
    <slot />
    <div v-if="$slots.append" class="search-form__append">
      <slot name="append" />
    </div>
  </div>
</template>

<style scoped>
.search-form {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

/* 搜索框槽：固定宽（窄主区下也不参与换行拉伸） */
.search-form__search {
  flex: none;
  width: var(--size-list-search-width);
  min-width: 0;
}

/* 筛选控件按内容取宽：覆盖 ui-select 触发器根样式的 width:100%
 * （SelectRoot 无 DOM，触发器是本容器的直接 flex 子项，width:100%
 * 会令其独占一整行）。双 :not 提高特异性，稳定压过组件自身样式。 */
.search-form > :not(.search-form__search):not(.search-form__append) {
  flex: 0 1 auto;
  width: max-content;
  min-width: 0;
  max-width: 100%;
}

/* 右对齐区（视图切换等） */
.search-form__append {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin-left: auto;
}
</style>
