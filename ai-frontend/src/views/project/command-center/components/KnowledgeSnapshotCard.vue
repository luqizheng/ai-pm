<script setup lang="ts">
import { FileText } from "lucide-vue-next";
import Card from "@/components/ui/card/Card.vue";
import SectionHead from "./SectionHead.vue";
import type { KnowledgeStat } from "../knowledge";

/**
 * 知识快照卡（Pixso「Project Command Center」3:7814）。
 *
 * 只读摘要：四类知识计数 pill（file-text 图标 + 类别 + 计数）+ 同步时间脚注。
 * 只读 —— 不提供写入/管理动作，仅「打开 Knowledge」下钻。
 * 文案由调用方翻译后传入。
 */
interface Props {
  title: string;
  meta?: string;
  actionLabel?: string;
  stats: KnowledgeStat[];
  /** 同步时间脚注；不传不渲染 */
  footnote?: string;
}

withDefaults(defineProps<Props>(), {
  meta: "",
  actionLabel: "",
  footnote: ""
});

const emit = defineEmits<{ (e: "action"): void }>();
</script>

<template>
  <Card class="pm-knowledge">
    <template #header>
      <SectionHead
        :title="title"
        :meta="meta"
        :action-label="actionLabel"
        @action="emit('action')"
      />
    </template>

    <div class="pm-knowledge__stats">
      <span v-for="stat in stats" :key="stat.label" class="pm-knowledge__pill">
        <FileText class="pm-knowledge__icon" aria-hidden="true" />
        <span class="pm-knowledge__label">{{ stat.label }}</span>
        <span class="pm-knowledge__count">{{ stat.count }}</span>
      </span>
    </div>

    <p v-if="footnote" class="pm-knowledge__footnote">{{ footnote }}</p>
  </Card>
</template>

<style scoped>
.pm-knowledge :deep(.ui-card__header) {
  padding: var(--space-3) var(--space-3) 0;
  border-bottom: 0;
}

.pm-knowledge :deep(.ui-card__body) {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: 0 var(--space-3) var(--space-3);
}

.pm-knowledge__stats {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.pm-knowledge__pill {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: transparent;
}

.pm-knowledge__icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  flex: none;
  color: var(--text-secondary);
}

.pm-knowledge__label {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.pm-knowledge__count {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.pm-knowledge__footnote {
  margin: 0;
  font-size: var(--font-size-nano);
  line-height: 1.4;
  color: var(--text-disabled);
}
</style>
