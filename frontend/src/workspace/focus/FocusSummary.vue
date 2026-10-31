<script setup lang="ts">
import { useI18n } from "vue-i18n";

const props = defineProps<{ objectType?: string; objectId?: string }>();
const { t } = useI18n();
</script>

<template>
  <section class="ws-focus-summary" data-test="focus-summary">
    <p class="ws-focus-kicker">{{ t("workspace.focus.subject") }}</p>
    <p class="ws-focus-title">{{ objectType }} · {{ objectId }}</p>
  </section>
</template>

<style scoped>
.ws-focus-summary {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-focus-kicker {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-focus-title {
  margin: 0;
  font-weight: 600;
}
</style>