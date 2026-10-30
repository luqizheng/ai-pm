<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { dynamics } from "../fixtures";

const { t } = useI18n();

const kindClass: Record<string, string> = {
  change: "is-change",
  failure: "is-failure",
  decision: "is-decision"
};
</script>

<template>
  <section class="ws-dynamics" aria-label="dynamics">
    <h2 class="ws-section-title">{{ t("workspace.console.dynamics") }}</h2>
    <ul class="ws-dynamics-list">
      <li
        v-for="entry in dynamics"
        :key="entry.id"
        class="ws-dynamic"
        data-test="dynamic-item"
      >
        <span class="ws-dynamic-kind" :class="kindClass[entry.kind]">
          {{ t(`workspace.dynamics.kind.${entry.kind}`) }}
        </span>
        <span class="ws-dynamic-text">{{ entry.text }}</span>
        <span class="ws-dynamic-time">{{ entry.time }}</span>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.ws-section-title {
  margin: 0 0 8px;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-dynamics-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.ws-dynamic {
  display: flex;
  align-items: center;
  gap: 10px;
  height: var(--ws-row-height);
  border-bottom: 1px solid var(--ws-border);
}

.ws-dynamic:last-child {
  border-bottom: none;
}

.ws-dynamic-kind {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

.ws-dynamic-kind.is-change {
  color: var(--ws-color-info-text);
}

.ws-dynamic-kind.is-failure {
  color: var(--ws-color-danger-text);
}

.ws-dynamic-kind.is-decision {
  color: var(--ws-color-warning-text);
}

.ws-dynamic-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ws-dynamic-time {
  flex: none;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
