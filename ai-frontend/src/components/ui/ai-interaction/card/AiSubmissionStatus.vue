<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { CheckCircle2, LoaderCircle, TriangleAlert } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import type { SubmissionPhase, SubmitResult } from "../types";

/**
 * 提交结果条 —— 提交中 / 已记录 / 失败可重试。
 *
 * 语义红线：成功文案只写「已记录你的选择」+ 记录号 + 「正式业务变更以审批结果为准」，
 * 绝不显示「已生效 / 已发布」。
 */
const props = withDefaults(
  defineProps<{
    phase: SubmissionPhase;
    result?: SubmitResult | null;
    errorMessage?: string | null;
    /** 失败时展示的重试按钮文案 */
    retryLabel?: string;
    showRetry?: boolean;
  }>(),
  {
    result: null,
    errorMessage: null,
    retryLabel: "",
    showRetry: true
  }
);

const emit = defineEmits<{ retry: [] }>();

const { t } = useI18n();

const successText = computed(() => {
  const key = props.result?.messageKey ?? "ai.card.submission.recorded";
  return t(key);
});

const reference = computed(() => props.result?.reference ?? "");

const errorText = computed(
  () => props.errorMessage || t("ai.card.submission.failed")
);
</script>

<template>
  <div
    v-if="phase !== 'idle'"
    class="ai-submission"
    :class="`is-${phase}`"
    role="status"
    aria-live="polite"
  >
    <template v-if="phase === 'submitting'">
      <LoaderCircle
        class="ai-submission__spinner"
        :size="14"
        aria-hidden="true"
      />
      <span class="ai-submission__text">{{
        t("ai.card.submission.submitting")
      }}</span>
    </template>

    <template v-else-if="phase === 'success'">
      <CheckCircle2 :size="14" aria-hidden="true" />
      <span class="ai-submission__text">
        {{ successText }}
        <span v-if="reference" class="ai-submission__reference">
          {{ t("ai.card.submission.reference", { reference }) }}
        </span>
      </span>
      <span class="ai-submission__note">{{
        t("ai.card.submission.notEffective")
      }}</span>
    </template>

    <template v-else>
      <TriangleAlert :size="14" aria-hidden="true" />
      <span class="ai-submission__text">{{ errorText }}</span>
      <Button
        v-if="showRetry"
        class="ai-submission__retry"
        variant="secondary"
        size="sm"
        @click="emit('retry')"
      >
        {{ retryLabel || t("ai.card.submission.retry") }}
      </Button>
    </template>
  </div>
</template>

<style scoped>
.ai-submission {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0 var(--space-4) var(--space-3);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-submission.is-submitting {
  color: var(--ws-text-secondary);
  background: var(--ws-elevated);
}

.ai-submission.is-success {
  color: var(--ws-color-success-text);
  background: var(--ws-elevated);
}

.ai-submission.is-error {
  color: var(--ws-color-danger-text);
  background: var(--ws-color-danger-surface);
  border: var(--border-1) solid var(--ws-color-danger-border);
}

.ai-submission__text {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.ai-submission__reference {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.ai-submission__note {
  margin-left: auto;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

.ai-submission__retry {
  margin-left: auto;
  flex: none;
}

.ai-submission__spinner {
  animation: ai-submission-spin var(--duration-spinner) linear infinite;
}

@keyframes ai-submission-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ai-submission__spinner {
    animation: none;
  }
}
</style>
