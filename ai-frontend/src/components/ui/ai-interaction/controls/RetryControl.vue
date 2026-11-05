<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RotateCcw } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";

/**
 * Retry with attempt bookkeeping: the button locks once `maxAttempts` is
 * reached so the caller never has to duplicate the guard.
 */
const props = withDefaults(
  defineProps<{
    attempts?: number;
    maxAttempts?: number;
    retrying?: boolean;
    disabled?: boolean;
    label?: string;
  }>(),
  {
    attempts: 0,
    maxAttempts: undefined,
    retrying: false,
    disabled: false,
    label: undefined
  }
);

const emit = defineEmits<{ retry: [] }>();

const { t } = useI18n();

const atLimit = computed(
  () => props.maxAttempts !== undefined && props.attempts >= props.maxAttempts
);

const isDisabled = computed(
  () => props.disabled || props.retrying || atLimit.value
);

const text = computed(() => props.label ?? t("ai.controls.retry.label"));

const attemptText = computed(() => {
  if (props.maxAttempts !== undefined) {
    return t("ai.controls.retry.attempt", {
      attempt: props.attempts,
      max: props.maxAttempts
    });
  }
  if (props.attempts <= 0) return "";
  return t("ai.controls.retry.attempts", { count: props.attempts });
});

function retry() {
  if (isDisabled.value) return;
  emit("retry");
}
</script>

<template>
  <div class="ai-retry-control">
    <Button
      class="ai-retry-control__button"
      variant="secondary"
      size="sm"
      :loading="retrying"
      :disabled="isDisabled"
      @click="retry"
    >
      <RotateCcw :size="14" aria-hidden="true" />
      {{ text }}
    </Button>
    <span v-if="attemptText" class="ai-retry-control__attempts">{{
      attemptText
    }}</span>
  </div>
</template>

<style scoped>
.ai-retry-control {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-retry-control__button {
  flex: none;
}

.ai-retry-control__attempts {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  white-space: nowrap;
}
</style>
