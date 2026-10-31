<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{ reason: string; dataRef?: string }>();
const { t } = useI18n();
const label = computed(() => t("workspace.spec.fallback", { reason: props.reason }));
</script>

<template>
  <section class="ws-fallback" data-test="fallback-panel">
    <p class="ws-fallback__label">{{ label }}</p>
    <p v-if="dataRef" class="ws-fallback__ref">{{ dataRef }}</p>
  </section>
</template>

<style scoped>
.ws-fallback {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px;
  border: 1px dashed var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-fallback__label {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
  color: var(--ws-text-secondary);
}

.ws-fallback__ref {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-primary);
  font-variant-numeric: tabular-nums;
}
</style>