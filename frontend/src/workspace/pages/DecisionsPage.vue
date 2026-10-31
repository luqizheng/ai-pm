<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { AlertTriangle } from "lucide-vue-next";
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
      <!-- V1.2：需求变化触发的重新评估提示（系统主动提示） -->
      <div
        v-if="decision.reassess"
        class="ws-reassess"
        data-test="decision-reassess"
        role="alert"
      >
        <AlertTriangle :size="14" aria-hidden="true" />
        <div>
          <strong>{{
            t("workspace.pages.decisionsExtra.reassessTitle")
          }}</strong>
          <span>{{ decision.reassess.reason }}</span>
          <span class="ws-reassess-src">
            {{ decision.reassess.triggeredBy }}
          </span>
        </div>
      </div>

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

      <!-- V1.2：问题背景 -->
      <div class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisionsExtra.background") }}
        </span>
        <p class="ws-decision-impact">{{ decision.background }}</p>
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

      <!-- V1.2：比较维度（双方案对照） -->
      <div class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisionsExtra.dimensions") }}
        </span>
        <div class="ws-dims" data-test="decision-dimensions">
          <div class="ws-dims-head">
            <span class="ws-dims-name"></span>
            <span
              v-for="option in decision.options"
              :key="option.name"
              class="ws-dims-opt"
            >
              {{ option.name }}
            </span>
          </div>
          <div
            v-for="dim in decision.dimensions"
            :key="dim.name"
            class="ws-dims-row"
          >
            <span class="ws-dims-name">{{ dim.name }}</span>
            <span
              v-for="(value, i) in dim.values"
              :key="i"
              class="ws-dims-value"
            >
              {{ value }}
            </span>
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

      <!-- V1.2：AI 推荐理由 + 风险 -->
      <div class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisionsExtra.recommendation") }}
        </span>
        <p class="ws-decision-impact">{{ decision.recommendation }}</p>
      </div>
      <div class="ws-decision-block">
        <span class="ws-decision-label is-risk">
          {{ t("workspace.pages.decisionsExtra.risks") }}
        </span>
        <ul class="ws-decision-list">
          <li v-for="(item, i) in decision.risks" :key="i">{{ item }}</li>
        </ul>
      </div>

      <div class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisions.impact") }}
        </span>
        <p class="ws-decision-impact">{{ decision.impact }}</p>
      </div>

      <!-- V1.2：决策人 / 批准时间 / 后续验证 -->
      <div v-if="decision.status === 'decided'" class="ws-decision-meta">
        <span>
          {{ t("workspace.pages.decisionsExtra.decidedBy") }}：{{
            decision.decidedBy
          }}
        </span>
        <span>
          {{ t("workspace.pages.decisionsExtra.approvedAt") }}：{{
            decision.approvedAt
          }}
        </span>
      </div>
      <div v-if="decision.conclusion" class="ws-decision-block">
        <span class="ws-decision-label is-conclusion">
          {{ t("workspace.pages.decisions.conclusion") }}
        </span>
        <p class="ws-decision-impact">{{ decision.conclusion }}</p>
      </div>
      <div v-if="decision.verification?.length" class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisionsExtra.verification") }}
        </span>
        <ul class="ws-decision-list">
          <li v-for="(item, i) in decision.verification" :key="i">
            {{ item }}
          </li>
        </ul>
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

.ws-reassess {
  display: flex;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid currentcolor;
  border-radius: var(--ws-radius-card);
  color: var(--ws-color-warning-text);
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}

.ws-reassess strong {
  margin-right: 6px;
}

.ws-reassess-src {
  margin-left: 8px;
  color: var(--ws-text-secondary);
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

/* 比较维度对照表 */
.ws-dims {
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow: hidden;
}

.ws-dims-head,
.ws-dims-row {
  display: grid;
  grid-template-columns: 120px repeat(auto-fit, minmax(0, 1fr));
}

.ws-dims-head {
  background: var(--ws-elevated);
}

.ws-dims-head > span,
.ws-dims-row > span {
  padding: 6px 10px;
  font-size: var(--ws-font-size-caption);
  border-bottom: 1px solid var(--ws-border);
}

.ws-dims-row:last-child > span {
  border-bottom: none;
}

.ws-dims-name {
  font-weight: 600;
}

.ws-dims-value {
  color: var(--ws-text-primary);
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

.ws-decision-label.is-risk {
  color: var(--ws-color-danger-text);
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

.ws-decision-meta {
  display: flex;
  gap: 16px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
