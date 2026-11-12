<script setup lang="ts">
import PageHeader from "./PageHeader.vue";

/**
 * 统一页框（AGENTS.md §三.12：应用壳内页面强制使用的页面骨架）。
 *
 * 页框拥有：
 *   - 页根留白 `padding: var(--space-5) var(--space-8)`
 *     （`.workspace-main-scroll` 无 padding，页面不得依赖壳留白、不得另设页边距）；
 *   - 根纵列布局（区块间距 `--space-6`）；
 *   - 可选 `PageHeader` 标题区（面包屑 / 标题 / 描述 / #actions）。
 *
 * 页面通过默认插槽填充内容：
 *   - 纵列布局的业务页把内容直接放进插槽（区块间距即页框 gap）；
 *   - 根布局不是纵列的工作台（如通知中心双栏）在插槽内自带容器，
 *     并省略 `title`（标题区由其内部组件承担）。
 *
 * `narrow` 变体：管理 / 设置等表单型页面内容限宽居中——这类页面主区
 * 独占全宽（无 AI 面板），拉满可读性差；列表 / 工作台页保持默认全宽。
 *
 * `PlaceholderView` = PageFrame + 「建设中」空态的特例（未落地路由的默认页）。
 */
interface Props {
  /** 面包屑条目（透传 PageHeader） */
  breadcrumbs?: Array<{ label: string; to?: string }>;
  /** 页面标题；不传则不渲染标题区 */
  title?: string;
  /** 页面描述（透传 PageHeader） */
  description?: string;
  /** 窄页变体：内容限宽居中（管理 / 设置等表单页） */
  narrow?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  breadcrumbs: () => [],
  description: undefined,
  narrow: false
});
</script>

<template>
  <div
    class="shell-page-frame"
    :class="{ 'shell-page-frame--narrow': props.narrow }"
  >
    <PageHeader
      v-if="props.title"
      :breadcrumbs="props.breadcrumbs"
      :title="props.title"
      :description="props.description"
    >
      <template v-if="$slots.actions" #actions>
        <slot name="actions" />
      </template>
    </PageHeader>

    <slot />
  </div>
</template>

<style scoped>
.shell-page-frame {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  padding: var(--space-5) var(--space-8);
}

/* 窄页变体：内容限宽居中。宽度取搜索入口 / 表单卡两列的舒适上限，
 * 媒体查询断点之下自然随主区收缩。 */
.shell-page-frame--narrow {
  width: 100%;
  max-width: var(--ws-page-narrow-width, 60rem);
  margin-inline: auto;
}
</style>
