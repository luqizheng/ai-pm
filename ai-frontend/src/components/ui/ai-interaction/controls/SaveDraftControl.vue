<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Save } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";

/**
 * Draft save affordance: only meaningful while there are unsaved edits, and it
 * switches to a "saved" read-out once the caller reports a `savedAt` stamp.
 */
const props = withDefaults(
  defineProps<{
    dirty?: boolean;
    saving?: boolean;
    savedAt?: string;
    disabled?: boolean;
  }>(),
  {
    dirty: false,
    saving: false,
    savedAt: undefined,
    disabled: false
  }
);

const emit = defineEmits<{ save: [] }>();

const { t } = useI18n();

const isDisabled = computed(
  () => props.disabled || props.saving || !props.dirty
);

const text = computed(() => {
  if (props.saving) return t("ai.controls.saveDraft.saving");
  if (!props.dirty && props.savedAt) {
    return t("ai.controls.saveDraft.savedAt", { time: props.savedAt });
  }
  return t("ai.controls.saveDraft.label");
});

function save() {
  if (isDisabled.value) return;
  emit("save");
}
</script>

<template>
  <Button
    class="ai-save-draft"
    :class="{ 'is-dirty': dirty && !saving }"
    :variant="dirty && !saving ? 'primary' : 'secondary'"
    size="sm"
    :loading="saving"
    :disabled="isDisabled"
    @click="save"
  >
    <Save :size="14" aria-hidden="true" />
    {{ text }}
  </Button>
</template>

<style scoped>
.ai-save-draft {
  flex: none;
}
</style>
