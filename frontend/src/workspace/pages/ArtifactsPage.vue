<script setup lang="ts">
import { useI18n } from "vue-i18n";
import PageHeader from "./PageHeader.vue";
import { artifacts, type ApprovalStatus, type TestStatus } from "../fixtures";

const { t } = useI18n();

const testLabel: Record<TestStatus, string> = {
  passed: "workspace.pages.artifacts.testPassed",
  failed: "workspace.pages.artifacts.testFailed",
  running: "workspace.pages.artifacts.testRunning"
};

const testClass: Record<TestStatus, string> = {
  passed: "is-test-passed",
  failed: "is-test-failed",
  running: "is-test-running"
};

const approvalLabel: Record<ApprovalStatus, string> = {
  pending: "workspace.pages.artifacts.approvalPending",
  approved: "workspace.pages.artifacts.approvalApproved",
  rejected: "workspace.pages.artifacts.approvalRejected"
};

const approvalClass: Record<ApprovalStatus, string> = {
  pending: "is-approval-pending",
  approved: "is-approval-approved",
  rejected: "is-approval-rejected"
};
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.artifacts.title')"
      :subtitle="t('workspace.pages.artifacts.subtitle')"
    />
    <div class="ws-artifact-list">
      <section
        v-for="artifact in artifacts"
        :key="artifact.id"
        class="ws-card ws-artifact"
        data-test="artifact-item"
      >
        <div class="ws-artifact-head">
          <div class="ws-artifact-id">
            <span class="ws-artifact-name">{{ artifact.name }}</span>
            <span class="ws-artifact-version">{{ artifact.version }}</span>
          </div>
          <div class="ws-artifact-badges">
            <span class="ws-badge" :class="testClass[artifact.testStatus]">
              {{ t(testLabel[artifact.testStatus]) }}
            </span>
            <span class="ws-badge" :class="approvalClass[artifact.approval]">
              {{ t(approvalLabel[artifact.approval]) }}
            </span>
          </div>
        </div>
        <div class="ws-artifact-meta">
          <span>{{ artifact.type }}</span>
          <span>{{ artifact.owner }}</span>
          <span>{{ artifact.updatedAt }}</span>
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

.ws-artifact-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.ws-artifact {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px 16px;
}

.ws-artifact-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.ws-artifact-id {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}

.ws-artifact-name {
  font-weight: 600;
}

.ws-artifact-version {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.ws-artifact-badges {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 紫色克制：测试/审批状态用语义色 */
.ws-badge.is-test-passed,
.ws-badge.is-approval-approved {
  color: var(--ws-color-success-text);
}

.ws-badge.is-test-failed,
.ws-badge.is-approval-rejected {
  color: var(--ws-color-danger-text);
}

.ws-badge.is-test-running,
.ws-badge.is-approval-pending {
  color: var(--ws-color-warning-text);
}

.ws-artifact-meta {
  display: flex;
  gap: 14px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
