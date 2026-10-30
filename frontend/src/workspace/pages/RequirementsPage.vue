<script setup lang="ts">
import { useI18n } from "vue-i18n";
import PageHeader from "./PageHeader.vue";
import { requirements, type RequirementStatus } from "../fixtures";

const { t } = useI18n();

const statusLabel: Record<RequirementStatus, string> = {
  draft: "workspace.pages.requirements.draft",
  reviewing: "workspace.pages.requirements.reviewing",
  confirmed: "workspace.pages.requirements.confirmed",
  implementing: "workspace.pages.requirements.implementing",
  accepted: "workspace.pages.requirements.accepted"
};

const priorityClass: Record<string, string> = {
  P0: "is-p0",
  P1: "is-p1",
  P2: "is-p2"
};
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.requirements.title')"
      :subtitle="t('workspace.pages.requirements.subtitle')"
    />
    <section
      v-for="req in requirements"
      :key="req.id"
      class="ws-card ws-req"
      data-test="req-item"
    >
      <div class="ws-req-head">
        <span class="ws-req-id">{{ req.id }}</span>
        <span class="ws-req-title">{{ req.title }}</span>
        <span class="ws-badge" :class="priorityClass[req.priority]">
          {{ req.priority }}
        </span>
        <span class="ws-badge is-status">
          {{ t(statusLabel[req.status]) }}
        </span>
      </div>
      <p class="ws-req-desc">{{ req.description }}</p>
      <div class="ws-req-accept">
        <span class="ws-req-label">
          {{ t("workspace.pages.requirements.acceptance") }}
        </span>
        <ul class="ws-req-list">
          <li v-for="(item, i) in req.acceptance" :key="i">{{ item }}</li>
        </ul>
      </div>
      <div class="ws-req-source">
        {{ t("workspace.pages.requirements.source") }}：{{ req.source }}
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

.ws-req {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px 16px;
}

.ws-req-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.ws-req-id {
  flex: none;
  font-family: var(--font-mono, monospace);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-req-title {
  flex: 1;
  min-width: 0;
  font-weight: 600;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 紫色克制：优先级/状态用语义色 */
.ws-badge.is-p0 {
  color: var(--ws-color-danger-text);
}

.ws-badge.is-p1 {
  color: var(--ws-color-warning-text);
}

.ws-badge.is-p2 {
  color: var(--ws-text-secondary);
}

.ws-badge.is-status {
  color: var(--ws-color-info-text);
}

.ws-req-desc {
  margin: 0;
  font-size: var(--ws-font-size-body);
  line-height: 1.6;
}

.ws-req-accept {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ws-req-label {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-req-list {
  margin: 0;
  padding-left: 18px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-primary);
}

.ws-req-list li {
  margin: 2px 0;
}

.ws-req-source {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
