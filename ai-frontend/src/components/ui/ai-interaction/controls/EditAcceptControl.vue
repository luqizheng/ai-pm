<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Check, CheckCircle2 } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";

export type EditAcceptState = "editing" | "dirty" | "saving" | "saved";

export interface EditAcceptLabels {
  accept?: string;
  revert?: string;
}

/**
 * Accept / revert pair for an inline edit session.
 *   editing -> nothing to accept yet, revert cancels the session
 *   dirty   -> both enabled
 *   saving  -> both blocked, accept shows a spinner
 *   saved   -> both blocked, accept reads as done with a check
 */
const props = withDefaults(
  defineProps<{
    state: EditAcceptState;
    labels?: EditAcceptLabels;
  }>(),
  {
    labels: () => ({})
  }
);

const emit = defineEmits<{ accept: []; revert: [] }>();

const { t } = useI18n();

const saved = computed(() => props.state === "saved");
const saving = computed(() => props.state === "saving");

const acceptLabel = computed(() => {
  if (props.labels.accept) return props.labels.accept;
  if (saving.value) return t("ai.controls.editAccept.saving");
  if (saved.value) return t("ai.controls.editAccept.saved");
  return t("ai.controls.editAccept.accept");
});

const revertLabel = computed(
  () => props.labels.revert ?? t("ai.controls.editAccept.revert")
);

const acceptDisabled = computed(
  () => props.state === "editing" || saving.value || saved.value
);

const revertDisabled = computed(() => saving.value || saved.value);

const statusText = computed(() => t(`ai.controls.editAccept.${props.state}`));

function accept() {
  if (acceptDisabled.value) return;
  emit("accept");
}

function revert() {
  if (revertDisabled.value) return;
  emit("revert");
}
</script>

<template>
  <div class="ai-edit-accept" :data-state="state">
    <span class="ai-edit-accept__status" :class="`is-${state}`" role="status">
      <CheckCircle2 v-if="saved" :size="14" aria-hidden="true" />
      {{ statusText }}
    </span>

    <Button
      variant="secondary"
      size="sm"
      :disabled="revertDisabled"
      @click="revert"
    >
      {{ revertLabel }}
    </Button>

    <Button
      variant="primary"
      size="sm"
      :loading="saving"
      :disabled="acceptDisabled"
      @click="accept"
    >
      <Check v-if="saved" :size="14" aria-hidden="true" />
      {{ acceptLabel }}
    </Button>
  </div>
</template>

<style scoped>
.ai-edit-accept {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-edit-accept__status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin-inline-end: auto;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-edit-accept__status.is-dirty {
  color: var(--warning-strong);
}

.ai-edit-accept__status.is-saving {
  color: var(--text-secondary);
}

.ai-edit-accept__status.is-saved {
  color: var(--success-strong);
}
</style>
