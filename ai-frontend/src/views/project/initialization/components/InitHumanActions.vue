<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowRight } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import type { HumanAction } from "@/api/initialization-overview";

/**
 * Human Actions —— 待人工处理（设计稿 §6）。
 *
 * 只收「需要人做决定或授权」的事项；普通通知、Agent 建议、信息同步一律不进。
 *
 * 视觉纪律（沿用 S01-05 §D-a）：
 *   - 类型用**中性 chip**，不承载颜色；
 *   - 颜色只承载紧急度（左侧 2px 轨）。
 */

interface Props {
  actions: HumanAction[];
}

const props = defineProps<Props>();

const emit = defineEmits<{ open: [action: HumanAction] }>();

const { t } = useI18n();

const kindLabel = (kind: string) =>
  t(`s01.initialization-overview.humanActions.kind.${kind}`, kind);

const domainLabel = (domain: string) =>
  t(`s01.initialization-overview.humanActions.domain.${domain}`, domain);

const formatTime = (value?: string) => {
  if (!value) return "";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return parsed.toLocaleString();
};
</script>

<template>
  <section class="init-actions" data-testid="init-human-actions">
    <header class="init-actions__header">
      <h2 class="init-actions__heading">
        {{ t("s01.initialization-overview.humanActions.title") }}
      </h2>
      <Badge variant="count" size="sm">{{ props.actions.length }}</Badge>
    </header>

    <p v-if="props.actions.length === 0" class="init-actions__empty">
      {{ t("s01.initialization-overview.humanActions.empty") }}
    </p>

    <ul v-else class="init-actions__list">
      <li
        v-for="action in props.actions"
        :key="action.id"
        class="init-actions__item"
        :class="`is-${action.urgency}`"
      >
        <div class="init-actions__body">
          <div class="init-actions__line">
            <Badge variant="neutral" size="sm">
              {{ kindLabel(action.kind) }}
            </Badge>
            <span class="init-actions__title">{{ action.title }}</span>
          </div>
          <p class="init-actions__meta">
            <span>{{ domainLabel(action.domain) }}</span>
            <code class="init-actions__state">{{ action.source_state }}</code>
            <span v-if="action.updated_at" class="init-actions__time">
              {{ formatTime(action.updated_at) }}
            </span>
          </p>
          <p v-if="action.detail" class="init-actions__detail">
            {{ action.detail }}
          </p>
        </div>
        <Button variant="ghost" size="sm" @click="emit('open', action)">
          {{ t("s01.initialization-overview.humanActions.open") }}
          <ArrowRight :size="14" aria-hidden="true" />
        </Button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.init-actions {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.init-actions__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.init-actions__heading {
  flex: 1;
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.init-actions__empty {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

.init-actions__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.init-actions__item {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-3) var(--space-3) var(--space-4);
  background: var(--ws-elevated);
  border-radius: var(--radius-sm);
}

/* 紧急度轨：颜色只承载紧急度，类型仍是中性 chip */
.init-actions__item.is-warning {
  box-shadow: inset var(--size-indicator) 0 0 var(--ws-color-warning);
}

.init-actions__item.is-danger {
  box-shadow: inset var(--size-indicator) 0 0 var(--ws-color-danger);
}

.init-actions__body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.init-actions__line {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.init-actions__title {
  overflow: hidden;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.init-actions__meta {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.init-actions__state {
  font-family: var(--font-mono);
}

.init-actions__time {
  color: var(--ws-text-disabled);
}

.init-actions__detail {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}
</style>
