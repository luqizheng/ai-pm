<script setup lang="ts">
import { useI18n } from "vue-i18n";
import PageHeader from "./PageHeader.vue";
import { dynamics } from "../fixtures";

const { t } = useI18n();

const kindLabel: Record<string, string> = {
  change: "workspace.pages.activity.kindChange",
  failure: "workspace.pages.activity.kindFailure",
  decision: "workspace.pages.activity.kindDecision"
};

const kindClass: Record<string, string> = {
  change: "is-change",
  failure: "is-failure",
  decision: "is-decision"
};
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.activity.title')"
      :subtitle="t('workspace.pages.activity.subtitle')"
    />
    <section class="ws-card ws-feed">
      <div
        v-for="entry in dynamics"
        :key="entry.id"
        class="ws-activity"
        data-test="activity-item"
      >
        <span class="ws-badge" :class="kindClass[entry.kind]">
          {{ t(kindLabel[entry.kind]) }}
        </span>
        <span class="ws-activity-text">{{ entry.text }}</span>
        <span class="ws-activity-time">{{ entry.time }}</span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 960px;
}

.ws-feed {
  display: flex;
  flex-direction: column;
  padding: 4px 16px;
}

.ws-activity {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: var(--ws-row-height);
  border-bottom: 1px solid var(--ws-border);
}

.ws-activity:last-child {
  border-bottom: none;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

.ws-badge.is-change {
  color: var(--ws-color-info-text);
}

.ws-badge.is-failure {
  color: var(--ws-color-danger-text);
}

.ws-badge.is-decision {
  color: var(--ws-color-warning-text);
}

.ws-activity-text {
  flex: 1;
  min-width: 0;
}

.ws-activity-time {
  flex: none;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
