<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, ChevronUp, FileText, Link2 } from "lucide-vue-next";

import type { AiEvidence } from "../types";

/**
 * 证据区 —— 分析摘要 + 来源引用 + 关联业务对象。
 * 默认折叠，仅显示「证据与来源 N」，展开后逐条列出。
 */
const props = withDefaults(
  defineProps<{
    evidence?: AiEvidence;
    defaultOpen?: boolean;
  }>(),
  { evidence: undefined, defaultOpen: false }
);

const { t } = useI18n();

const open = ref(props.defaultOpen || (props.evidence?.defaultOpen ?? false));

const hasContent = computed(() => {
  const evidence = props.evidence;
  if (!evidence) return false;
  return Boolean(
    evidence.summary || evidence.sources?.length || evidence.related?.length
  );
});

const sourceCount = computed(() => props.evidence?.sources?.length ?? 0);
const relatedCount = computed(() => props.evidence?.related?.length ?? 0);

const countLabel = computed(() => {
  const parts: string[] = [];
  if (sourceCount.value) {
    parts.push(
      t("ai.card.evidence.sourcesCount", { count: sourceCount.value })
    );
  }
  if (relatedCount.value) {
    parts.push(
      t("ai.card.evidence.relatedCount", { count: relatedCount.value })
    );
  }
  return parts.join(" · ");
});
</script>

<template>
  <section v-if="hasContent || $slots.default" class="ai-evidence">
    <button
      type="button"
      class="ai-evidence__toggle"
      :aria-expanded="open"
      @click="open = !open"
    >
      <span class="ai-evidence__toggle-label">{{
        t("ai.card.evidence.title")
      }}</span>
      <span v-if="countLabel" class="ai-evidence__count">{{ countLabel }}</span>
      <component
        :is="open ? ChevronUp : ChevronDown"
        :size="14"
        aria-hidden="true"
      />
    </button>

    <div v-if="open" class="ai-evidence__body">
      <p v-if="evidence?.summary" class="ai-evidence__summary">
        <span class="ai-evidence__summary-label">{{
          t("ai.card.evidence.analysis")
        }}</span>
        {{ evidence.summary }}
      </p>

      <ul v-if="evidence?.sources?.length" class="ai-evidence__sources">
        <li
          v-for="source in evidence.sources"
          :key="source.id"
          class="ai-evidence__source"
        >
          <FileText :size="14" aria-hidden="true" />
          <span class="ai-evidence__source-label">{{ source.label }}</span>
          <span v-if="source.meta" class="ai-evidence__source-meta">{{
            source.meta
          }}</span>
        </li>
      </ul>

      <ul v-if="evidence?.related?.length" class="ai-evidence__related">
        <li
          v-for="item in evidence.related"
          :key="item.id"
          class="ai-evidence__related-item"
        >
          <Link2 :size="12" aria-hidden="true" />
          <span class="ai-evidence__related-kind">{{ item.kind }}</span>
          <span class="ai-evidence__related-label">{{ item.label }}</span>
        </li>
      </ul>

      <slot />
    </div>
  </section>
</template>

<style scoped>
.ai-evidence {
  padding: 0 var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
}

.ai-evidence__toggle {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: var(--size-control-sm);
  padding: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  cursor: pointer;
  background: transparent;
  border: none;
}

.ai-evidence__toggle:hover {
  color: var(--ws-text-primary);
}

.ai-evidence__toggle:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.ai-evidence__toggle-label {
  font-weight: var(--weight-medium);
}

.ai-evidence__count {
  margin-right: auto;
  color: var(--ws-text-disabled);
}

.ai-evidence__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: 0 0 var(--space-3);
  max-height: var(--size-ai-evidence-max-height);
  overflow-y: auto;
}

.ai-evidence__summary {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-evidence__summary-label {
  margin-right: var(--space-2);
  color: var(--ws-text-primary);
  font-weight: var(--weight-medium);
}

.ai-evidence__sources,
.ai-evidence__related {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-evidence__source {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--size-control-sm);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

.ai-evidence__source-label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-evidence__source-meta {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  white-space: nowrap;
}

.ai-evidence__related {
  flex-direction: row;
  flex-wrap: wrap;
}

.ai-evidence__related-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-badge);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
  font-size: var(--font-size-micro);
}

.ai-evidence__related-kind {
  color: var(--ws-text-secondary);
}

.ai-evidence__related-label {
  color: var(--ws-text-primary);
}
</style>
