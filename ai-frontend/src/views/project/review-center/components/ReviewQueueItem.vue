<script setup lang="ts">
import Badge from "@/components/ui/badge/Badge.vue";
import Card from "@/components/ui/card/Card.vue";

/**
 * 队列条目卡片（V1.1 §2.1/R03：474px 队列只承担识别与排序，不承担审核阅读）。
 *
 * 容器 = DS 组件 Card（interactive 视觉口径：hover 升描边）。
 * 纯展示件：文案由调用方翻译后经 props 传入（D8）。
 * 字段取舍（V1.1 §2.1）：kind 标签 / title / detail 两行 / domain / 相对时间 /
 * 紧凑状态标签；action_hint 不进卡片（与 kind 语义重复，Drawer 内完整展示）。
 * 不出现确认、拒绝、验收按钮（D4：先看上下文，再决策）。
 * 选中语义保留在内部 button（aria-current），测试锚点 .review-item 不变。
 */
interface Props {
  kindLabel: string;
  kindTone: "neutral" | "accent" | "warning" | "danger" | "success" | "info";
  stateLabel: string;
  title: string;
  detail: string;
  domainLabel: string;
  timeLabel: string;
  selected: boolean;
}

defineProps<Props>();
defineEmits<{ select: [] }>();
</script>

<template>
  <Card interactive class="review-item-card" :data-selected="selected">
    <button
      type="button"
      class="review-item"
      :class="{ 'is-selected': selected }"
      :aria-current="selected ? 'true' : undefined"
      @click="$emit('select')"
    >
      <div class="review-item__tags">
        <Badge :variant="kindTone" size="sm" outline>{{ kindLabel }}</Badge>
        <Badge variant="neutral" size="sm">{{ stateLabel }}</Badge>
      </div>
      <p class="review-item__title">{{ title }}</p>
      <p v-if="detail" class="review-item__detail">{{ detail }}</p>
      <div class="review-item__meta">
        <span>{{ domainLabel }}</span>
        <span>{{ timeLabel }}</span>
      </div>
    </button>
  </Card>
</template>

<style scoped>
/* 队列卡是紧凑条目：清掉 Card 默认 body 大留白，由按钮自己承担内距 */
.review-item-card :deep(.ui-card__body) {
  padding: 0;
}

.review-item-card[data-selected="true"] {
  border-color: var(--accent);
}

.review-item {
  display: block;
  width: 100%;
  padding: var(--space-3) var(--space-4);
  border: 0;
  border-radius: inherit;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-decelerate);
}

.review-item:hover {
  background: var(--surface-hover-veil);
}

.review-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(-1 * var(--size-focus-ring));
  border-radius: inherit;
}

.review-item.is-selected {
  background: var(--accent-surface);
}

.review-item__tags {
  display: flex;
  gap: var(--space-2);
  align-items: center;
  margin-bottom: var(--space-2);
}

.review-item__title {
  margin: 0;
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.review-item__detail {
  margin: var(--space-1) 0 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.review-item__meta {
  display: flex;
  justify-content: space-between;
  gap: var(--space-2);
  margin-top: var(--space-2);
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}
</style>
