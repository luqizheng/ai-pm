<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { briefing } from "../fixtures";

const { t } = useI18n();
</script>

<template>
  <section class="ws-card ws-briefing" aria-label="AI briefing">
    <div class="ws-briefing-meta">
      <span class="ws-briefing-author">{{ briefing.author }}</span>
      <span class="ws-briefing-time"
        >· {{ t("workspace.briefing.today") }}</span
      >
    </div>
    <p class="ws-briefing-body">{{ briefing.body }}</p>
    <p class="ws-briefing-suggestion">
      <span aria-hidden="true">💡</span>
      {{ briefing.suggestion }}
    </p>
  </section>
</template>

<style scoped>
.ws-briefing {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
}

.ws-briefing-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-briefing-author {
  font-weight: 600;
  color: var(--ws-accent);
}

.ws-briefing-body {
  margin: 0;
  font-size: var(--ws-font-size-body);
  line-height: 1.6;
}

.ws-briefing-suggestion {
  margin: 0;
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-secondary);
}
</style>
