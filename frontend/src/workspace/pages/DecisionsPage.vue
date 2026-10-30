<script setup lang="ts">
import { useI18n } from "vue-i18n";
import PageHeader from "./PageHeader.vue";
import { decisions } from "../fixtures";

const { t } = useI18n();
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.decisions.title')"
      :subtitle="t('workspace.pages.decisions.subtitle')"
    />
    <section
      v-for="decision in decisions"
      :key="decision.id"
      class="ws-card ws-decision"
      data-test="decision-card"
    >
      <div class="ws-decision-head">
        <h2 class="ws-decision-title">{{ decision.title }}</h2>
        <span
          class="ws-badge"
          :class="decision.status === 'pending' ? 'is-pending' : 'is-decided'"
        >
          {{
            t(
              decision.status === "pending"
                ? "workspace.pages.decisions.pending"
                : "workspace.pages.decisions.decided"
            )
          }}
        </span>
      </div>
      <p class="ws-decision-question">{{ decision.question }}</p>

      <div class="ws-options">
        <div
          v-for="option in decision.options"
          :key="option.name"
          class="ws-option"
          data-test="decision-option"
        >
          <h3 class="ws-option-name">{{ option.name }}</h3>
          <div class="ws-option-group">
            <span class="ws-option-label is-pros">
              {{ t("workspace.pages.decisions.pros") }}
            </span>
            <ul class="ws-option-list">
              <li v-for="(item, i) in option.pros" :key="i">{{ item }}</li>
            </ul>
          </div>
          <div class="ws-option-group">
            <span class="ws-option-label is-cons">
              {{ t("workspace.pages.decisions.cons") }}
            </span>
            <ul class="ws-option-list">
              <li v-for="(item, i) in option.cons" :key="i">{{ item }}</li>
            </ul>
          </div>
        </div>
      </div>

      <div class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisions.evidence") }}
        </span>
        <ul class="ws-decision-list">
          <li v-for="(item, i) in decision.evidence" :key="i">{{ item }}</li>
        </ul>
      </div>
      <div class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisions.impact") }}
        </span>
        <p class="ws-decision-impact">{{ decision.impact }}</p>
      </div>
      <div v-if="decision.conclusion" class="ws-decision-block">
        <span class="ws-decision-label is-conclusion">
          {{ t("workspace.pages.decisions.conclusion") }}
        </span>
        <p class="ws-decision-impact">{{ decision.conclusion }}</p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px 24px 32px;
  max-width: 960px;
}

.ws-decision {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
}

.ws-decision-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.ws-decision-title {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 紫色克制：状态用语义色 */
.ws-badge.is-pending {
  color: var(--ws-color-warning-text);
}

.ws-badge.is-decided {
  color: var(--ws-color-success-text);
}

.ws-decision-question {
  margin: 0;
  font-size: var(--ws-font-size-body);
}

.ws-options {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.ws-option {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-option-name {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: 600;
}

.ws-option-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ws-option-label {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
}

.ws-option-label.is-pros {
  color: var(--ws-color-success-text);
}

.ws-option-label.is-cons {
  color: var(--ws-color-danger-text);
}

.ws-option-list {
  margin: 0;
  padding-left: 16px;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}

.ws-decision-block {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ws-decision-label {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-decision-label.is-conclusion {
  color: var(--ws-color-success-text);
}

.ws-decision-list {
  margin: 0;
  padding-left: 16px;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}

.ws-decision-impact {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}
</style>
