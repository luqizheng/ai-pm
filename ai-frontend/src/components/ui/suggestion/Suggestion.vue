<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/AI/Suggestion" node=13:34757 date=2026-11-04 -->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { Sparkles } from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * Suggestion —— Figma DS/AI/Suggestion（13:34757）。
 * AI 建议卡片：卡片（surface 底 border 描边 radius 8）+ sparkles 图标 +
 * 标题（accent 色以示 AI 建议，AGENTS.md「AI 建议与人工确认视觉可辨」）+
 * 正文（次文字）+ 动作区插槽。
 * accepted / rejected 态分别走 success / danger 语义色。
 */

export type SuggestionState = "default" | "accepted" | "rejected";

interface Props {
  title: string;
  body?: string;
  icon?: Component;
  state?: SuggestionState;
}

const props = withDefaults(defineProps<Props>(), {
  body: undefined,
  icon: undefined,
  state: "default"
});

const emit = defineEmits<{ action: [] }>();

const hasBody = computed(() => Boolean(props.body));
</script>

<template>
  <div
    :class="cn('ui-suggestion', `is-${state}`)"
    role="note"
    :aria-label="`AI suggestion: ${title}`"
  >
    <span class="ui-suggestion__icon-ring" aria-hidden="true">
      <Sparkles v-if="!icon" class="ui-suggestion__icon" />
      <component :is="icon" v-else class="ui-suggestion__icon" />
    </span>
    <div class="ui-suggestion__content">
      <p class="ui-suggestion__title">{{ title }}</p>
      <p v-if="hasBody" class="ui-suggestion__body">{{ body }}</p>
    </div>
    <div
      v-if="$slots.actions"
      class="ui-suggestion__actions"
      @click.capture="emit('action')"
    >
      <slot name="actions" />
    </div>
  </div>
</template>

<style scoped>
.ui-suggestion {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-3);
  background-color: var(--surface-base);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
}

.ui-suggestion.is-accepted {
  border-color: var(--ws-color-success);
}

.ui-suggestion.is-rejected {
  border-color: var(--ws-color-danger);
}

.ui-suggestion__icon-ring {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  flex: none;
  border-radius: var(--radius-full);
  background-color: var(--accent-surface);
  color: var(--accent);
}

.ui-suggestion__icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
}

.ui-suggestion__content {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: var(--space-1);
}

/* AI 建议标题用 accent 色（视觉可辨：非人工确认） */
.ui-suggestion__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  color: var(--accent);
}

.ui-suggestion.is-accepted .ui-suggestion__title {
  color: var(--ws-color-success-text);
}

.ui-suggestion.is-rejected .ui-suggestion__title {
  color: var(--danger-title);
}

.ui-suggestion__body {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.ui-suggestion__actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}
</style>
