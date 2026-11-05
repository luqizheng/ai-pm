<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import Button from "@/components/ui/button/Button.vue";
import ReasonInputControl from "./ReasonInputControl.vue";

/**
 * Accept / reject pair with an inline reason capture on reject.
 *
 * `rejectReasonEnabled` (default) requires a reason before the rejection is
 * emitted; set it to false for flows that reject in one click.
 */
const props = withDefaults(
  defineProps<{
    acceptLabel?: string;
    rejectLabel?: string;
    submitting?: boolean;
    disabled?: boolean;
    rejectReasonEnabled?: boolean;
  }>(),
  {
    acceptLabel: undefined,
    rejectLabel: undefined,
    submitting: false,
    disabled: false,
    rejectReasonEnabled: true
  }
);

const emit = defineEmits<{
  accept: [];
  reject: [reason: string];
}>();

const { t } = useI18n();

const reasonOpen = ref(false);
const reason = ref("");

const busy = computed(() => props.disabled || props.submitting);

/** A reason is mandatory while the reason step is enabled. */
const reasonRequired = computed(() => props.rejectReasonEnabled);

const acceptText = computed(
  () => props.acceptLabel ?? t("ai.controls.acceptReject.accept")
);

const rejectText = computed(
  () => props.rejectLabel ?? t("ai.controls.acceptReject.reject")
);

const canConfirmReject = computed(
  () => !busy.value && (!reasonRequired.value || reason.value.trim().length > 0)
);

function accept() {
  if (busy.value) return;
  emit("accept");
}

function onReject() {
  if (busy.value) return;
  reasonOpen.value = true;
}

function confirmReject() {
  if (!canConfirmReject.value) return;
  emit("reject", reason.value.trim());
  reasonOpen.value = false;
  reason.value = "";
}

function cancelReject() {
  reasonOpen.value = false;
  reason.value = "";
}
</script>

<template>
  <div class="ai-accept-reject">
    <div class="ai-accept-reject__actions">
      <Button
        variant="primary"
        size="sm"
        :loading="submitting"
        :disabled="busy"
        @click="accept"
      >
        {{ acceptText }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        :disabled="busy"
        :aria-expanded="reasonOpen"
        @click="onReject"
      >
        {{ rejectText }}
      </Button>
    </div>

    <div v-if="reasonOpen" class="ai-accept-reject__reason">
      <ReasonInputControl
        :model-value="reason"
        :label="t('ai.controls.acceptReject.reasonLabel')"
        :placeholder="t('ai.controls.acceptReject.reasonPlaceholder')"
        :required="reasonRequired"
        :disabled="submitting"
        auto-focus
        @update:model-value="reason = $event"
      />

      <div class="ai-accept-reject__reason-actions">
        <Button
          variant="danger"
          size="sm"
          :loading="submitting"
          :disabled="!canConfirmReject"
          @click="confirmReject"
        >
          {{ t("ai.controls.acceptReject.confirmReject") }}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          :disabled="submitting"
          @click="cancelReject"
        >
          {{ t("ai.controls.acceptReject.cancel") }}
        </Button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ai-accept-reject {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-accept-reject__actions,
.ai-accept-reject__reason-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.ai-accept-reject__reason {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}
</style>
