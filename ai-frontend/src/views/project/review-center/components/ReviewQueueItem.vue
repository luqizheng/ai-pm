<script setup lang="ts">
import Badge from "@/components/ui/badge/Badge.vue";

/**
 * 队列条目卡片（V1.1 §2.1/R03：474px 队列只承担识别与排序，不承担审核阅读）。
 *
 * 纯展示件：文案由调用方翻译后经 props 传入（D8）。
 * 字段取舍（V1.1 §2.1）：kind 标签 / title / detail 两行 / domain / 相对时间 /
 * 紧凑状态标签；action_hint 不进卡片（与 kind 语义重复，Drawer 内完整展示）。
 * 不出现确认、拒绝、验收按钮（D4：先看上下文，再决策）。
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
</template>

<style scoped>
.review-item {
  display: block;
  width: 100%;
  padding: 12px 16px;
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition:
    border-color var(--duration-fast, 120ms) var(--ease-out, ease-out),
    background var(--duration-fast, 120ms) var(--ease-out, ease-out);
}

.review-item:hover {
  background: var(--surface-hover-veil);
}

.review-item.is-selected {
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  border-color: var(--accent);
}

.review-item__tags {
  display: flex;
  gap: 6px;
  align-items: center;
  margin-bottom: 8px;
}

.review-item__title {
  margin: 0;
  color: var(--text-primary);
  font-size: 14px;
  font-weight: 600;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.review-item__detail {
  margin: 4px 0 0;
  color: var(--text-secondary);
  font-size: 13px;
  line-height: 1.45;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.review-item__meta {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  margin-top: 8px;
  color: var(--text-secondary);
  font-size: 12px;
}
</style>
