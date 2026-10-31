<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <section class="ws-focus-timeline" data-test="focus-timeline">
    <p class="ws-focus-kicker">{{ t("workspace.focus.timeline") }}</p>
    <p class="ws-focus-empty">{{ t("workspace.focus.timelineEmpty") }}</p>
  </section>
</template>

<style scoped>
.ws-focus-timeline {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ws-focus-kicker {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-focus-empty {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>