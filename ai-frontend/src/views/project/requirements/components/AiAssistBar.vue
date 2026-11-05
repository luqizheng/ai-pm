<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowUp, Sparkles } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import type { AiActionKind } from "../requirement-entry";

/**
 * AI 协作入口（S02-UI-01 §10）。
 *
 * 主内容区底部紧凑入口：输入框 + 六个建议动作。
 * AI 输出一律作为提案（需人工评审），入口文案与提交动作都不得直接改需求状态。
 */
const emit = defineEmits<{
  (e: "ask", text: string): void;
  (e: "action", kind: AiActionKind): void;
}>();

const { t } = useI18n();

const draft = ref("");

const actions: Array<{ kind: AiActionKind; key: string }> = [
  { kind: "completeness", key: "aiActionCompleteness" },
  { kind: "missing", key: "aiActionMissing" },
  { kind: "duplication", key: "aiActionDuplication" },
  { kind: "conflict", key: "aiActionConflict" },
  { kind: "acceptance", key: "aiActionAcceptance" },
  { kind: "summary", key: "aiActionSummary" }
];

function submit() {
  const text = draft.value.trim();
  if (!text) return;
  emit("ask", text);
  draft.value = "";
}
</script>

<template>
  <section class="req-ai" :aria-label="t('pm.requirements.aiBarAria')">
    <div class="req-ai__head">
      <Sparkles class="req-ai__icon" aria-hidden="true" />
      <span class="req-ai__title">{{ t("pm.requirements.aiBarTitle") }}</span>
      <Badge variant="accent" size="sm" outline>
        {{ t("pm.requirements.aiProposalBadge") }}
      </Badge>
      <span class="req-ai__note">{{ t("pm.requirements.aiBarNote") }}</span>
    </div>
    <form class="req-ai__composer" @submit.prevent="submit">
      <input
        v-model="draft"
        type="text"
        class="req-ai__input"
        :placeholder="t('pm.requirements.aiBarPlaceholder')"
        :aria-label="t('pm.requirements.aiBarPlaceholder')"
      />
      <button
        type="submit"
        class="req-ai__send"
        :disabled="draft.trim().length === 0"
        :aria-label="t('pm.requirements.aiBarSend')"
      >
        <ArrowUp aria-hidden="true" />
      </button>
    </form>
    <div
      class="req-ai__actions"
      role="group"
      :aria-label="t('pm.requirements.aiActionsAria')"
    >
      <button
        v-for="action in actions"
        :key="action.kind"
        type="button"
        class="req-ai__action"
        @click="emit('action', action.kind)"
      >
        {{ t(`pm.requirements.${action.key}`) }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.req-ai {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-md);
  background: var(--ws-color-accent-surface);
  padding: var(--space-3) var(--space-4);
}

.req-ai__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.req-ai__icon {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  color: var(--accent);
}

.req-ai__title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.req-ai__note {
  margin-left: auto;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-ai__composer {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.req-ai__input {
  flex: 1 1 0;
  min-width: 0;
  height: var(--size-control-sm);
  padding: 0 var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.req-ai__input::placeholder {
  color: var(--text-disabled);
}

.req-ai__input:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.req-ai__send {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  flex: none;
  border: 0;
  border-radius: var(--radius-sm);
  background: var(--accent);
  color: var(--accent-contrast-text);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-standard);
}

.req-ai__send:hover:not(:disabled) {
  background: var(--accent-hover);
}

.req-ai__send:disabled {
  opacity: var(--opacity-disabled-button);
  cursor: not-allowed;
}

.req-ai__send:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.req-ai__send svg {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

.req-ai__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.req-ai__action {
  padding: var(--space-1) var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-full);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

.req-ai__action:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.req-ai__action:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}
</style>
