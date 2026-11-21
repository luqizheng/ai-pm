<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";

import type { ScreenFeedbackState } from "./screen-feedback";

/**
 * PA-06 屏幕反馈三态（画布组件集 PA-06 / Screen feedback + 行为规范）：
 *   - loading：骨架条**保留上下文**（不是执行状态）；
 *   - empty：明确下一步（空态不抹历史）；
 *   - error：**不伪成功** + Retry safely。
 * 文案由调用方传入（各屏语义不同）；默认键取画布规范句。
 */
const props = withDefaults(
  defineProps<{
    state: ScreenFeedbackState;
    title?: string;
    description?: string;
  }>(),
  {
    title: undefined,
    description: undefined
  }
);

const emit = defineEmits<{ retry: [] }>();

const { t } = useI18n();

const resolvedTitle = () =>
  props.title ??
  (props.state === "error"
    ? t("pa06.feedback.errorTitle")
    : props.state === "empty"
      ? t("pa06.feedback.emptyTitle")
      : t("pa06.feedback.loadingTitle"));

const resolvedDescription = () =>
  props.description ??
  (props.state === "error"
    ? t("pa06.feedback.errorHint")
    : props.state === "empty"
      ? t("pa06.feedback.emptyHint")
      : t("pa06.feedback.loadingHint"));
</script>

<template>
  <div
    v-if="state === 'loading'"
    class="sf sf--loading"
    data-testid="screen-feedback-loading"
    aria-busy="true"
  >
    <span class="sf__eyebrow">{{ resolvedTitle() }}</span>
    <span class="sf__bar sf__bar--lg" />
    <span class="sf__bar sf__bar--md" />
    <span class="sf__block" />
  </div>

  <div
    v-else-if="state === 'empty'"
    class="sf sf--empty"
    data-testid="screen-feedback-empty"
  >
    <span class="sf__eyebrow">{{ resolvedTitle() }}</span>
    <p class="sf__title">{{ t("pa06.feedback.emptyHeadline") }}</p>
    <p class="sf__hint">{{ resolvedDescription() }}</p>
    <slot />
  </div>

  <div
    v-else
    class="sf sf--error"
    data-testid="screen-feedback-error"
    role="alert"
  >
    <span class="sf__eyebrow sf__eyebrow--danger">{{ resolvedTitle() }}</span>
    <p class="sf__hint">{{ resolvedDescription() }}</p>
    <Button
      variant="secondary"
      size="sm"
      data-testid="screen-feedback-retry"
      @click="emit('retry')"
    >
      {{ t("pa06.feedback.retry") }}
    </Button>
  </div>
</template>

<style scoped>
.sf {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.sf__eyebrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.sf__eyebrow--danger {
  color: var(--ws-color-danger-text);
}

.sf__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.sf__hint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-secondary);
}

.sf__bar {
  display: block;
  height: 0.75rem;
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
}

.sf__bar--lg {
  width: 60%;
}

.sf__bar--md {
  width: 30%;
  height: 0.625rem;
}

.sf__block {
  display: block;
  height: 3rem;
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
}
</style>
