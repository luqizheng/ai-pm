<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Clock } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";

const props = withDefaults(
  defineProps<{
    label?: string;
    disabled?: boolean;
  }>(),
  {
    label: undefined,
    disabled: false
  }
);

const emit = defineEmits<{ defer: [] }>();

const { t } = useI18n();

const text = computed(() => props.label ?? t("ai.controls.defer.label"));

function defer() {
  if (props.disabled) return;
  emit("defer");
}
</script>

<template>
  <Button
    class="ai-defer-control"
    variant="tertiary"
    size="sm"
    :disabled="disabled"
    @click="defer"
  >
    <Clock :size="14" aria-hidden="true" />
    {{ text }}
  </Button>
</template>

<style scoped>
.ai-defer-control {
  flex: none;
}
</style>
