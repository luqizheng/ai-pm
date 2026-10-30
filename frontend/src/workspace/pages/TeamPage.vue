<script setup lang="ts">
import { useI18n } from "vue-i18n";
import PageHeader from "./PageHeader.vue";
import { agents, type AgentStatus } from "../fixtures";

const { t } = useI18n();

const statusLabel: Record<AgentStatus, string> = {
  working: "workspace.pages.team.working",
  "waiting-approval": "workspace.pages.team.waitingApproval",
  idle: "workspace.pages.team.idle"
};

const statusClass: Record<AgentStatus, string> = {
  working: "is-working",
  "waiting-approval": "is-waiting",
  idle: "is-idle"
};
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.team.title')"
      :subtitle="t('workspace.pages.team.subtitle')"
    />
    <div class="ws-agent-grid">
      <section
        v-for="agent in agents"
        :key="agent.id"
        class="ws-card ws-agent"
        data-test="agent-card"
      >
        <div class="ws-agent-head">
          <span class="ws-agent-avatar" aria-hidden="true">
            {{ agent.name.slice(0, 1) }}
          </span>
          <div class="ws-agent-id">
            <span class="ws-agent-name">{{ agent.name }}</span>
            <span class="ws-agent-role">{{ agent.role }}</span>
          </div>
          <span class="ws-badge" :class="statusClass[agent.status]">
            {{ t(statusLabel[agent.status]) }}
          </span>
        </div>
        <div class="ws-agent-row">
          <span class="ws-agent-label">
            {{ t("workspace.pages.team.currentTask") }}
          </span>
          <span class="ws-agent-value">{{ agent.currentTask }}</span>
        </div>
        <div class="ws-agent-row">
          <span class="ws-agent-label">
            {{ t("workspace.pages.team.completed") }}
          </span>
          <span class="ws-agent-value">{{ agent.completed }}</span>
        </div>
        <div class="ws-agent-row">
          <span class="ws-agent-label">
            {{ t("workspace.pages.team.lastActive") }}
          </span>
          <span class="ws-agent-value">{{ agent.lastActive }}</span>
        </div>
      </section>
    </div>
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

.ws-agent-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.ws-agent {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px 16px;
}

.ws-agent-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.ws-agent-avatar {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: 1px solid var(--ws-border);
  background: var(--ws-elevated);
  font-weight: 600;
}

.ws-agent-id {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.ws-agent-name {
  font-weight: 600;
}

.ws-agent-role {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 紫色克制：状态用语义色，不用 AI 紫 */
.ws-badge.is-working {
  color: var(--ws-color-info-text);
}

.ws-badge.is-waiting {
  color: var(--ws-color-warning-text);
}

.ws-badge.is-idle {
  color: var(--ws-text-secondary);
}

.ws-agent-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: var(--ws-font-size-caption);
}

.ws-agent-label {
  flex: none;
  color: var(--ws-text-secondary);
}

.ws-agent-value {
  flex: 1;
  min-width: 0;
  color: var(--ws-text-primary);
}
</style>
