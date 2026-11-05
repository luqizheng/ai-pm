<script setup lang="ts">
import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Button from "@/components/ui/button/Button.vue";
import { PageFrame } from "@/components/shell";

import AiAnalysisCard from "./components/AiAnalysisCard.vue";
import ChangeSummary from "./components/ChangeSummary.vue";
import DecisionPanel from "./components/DecisionPanel.vue";
import DependencyGraph from "./components/DependencyGraph.vue";
import ImpactHeader from "./components/ImpactHeader.vue";
import ImpactPanel from "./components/ImpactPanel.vue";

import { requirementsFixture } from "../requirements/fixtures";
import { impactAnalysisFixture, impactDecisionOwnerFixture } from "./fixtures";
import type {
  ChangeImpactAnalysis,
  ImpactDecisionKind
} from "./impact-contract";
import type { RequirementEntry } from "../requirements/requirement-entry";

/**
 * 变更影响分析页（S02-UI-07）。
 *
 * 分析对象 = REQ-001 v2（pending_confirmation 拟议修订）。
 * 左列 = 变更摘要 + 依赖图；右列 = 影响面板（A-D）+ AI 分析 + 人类决策。
 * 红线：影响对象全部来自需求域真实关系（不虚构依赖）；AI 产出标注为
 * 分析非事实；接受/拒绝必须经显式确认弹窗，要求更多分析也记录决策。
 */
const route = useRoute();
const router = useRouter();
const { t } = useI18n();

const requirements = ref<RequirementEntry[]>(
  structuredClone(requirementsFixture) as RequirementEntry[]
);
const analysis = ref<ChangeImpactAnalysis>(
  structuredClone(impactAnalysisFixture) as ChangeImpactAnalysis
);

const requirementId = computed(() => String(route.params.requirementId ?? ""));
const matched = computed(
  () => requirementId.value === analysis.value.requirement_id
);

const confirmDialog = ref<null | "accept" | "reject">(null);
const decision = ref<{
  kind: ImpactDecisionKind;
  actor: string;
  acted_at: string;
} | null>(null);

const confirmDialogOpen = computed(() => confirmDialog.value !== null);

const confirmDialogProps = computed(() =>
  confirmDialog.value === "reject"
    ? {
        title: t("pm.requirementImpact.rejectModalTitle"),
        description: t("pm.requirementImpact.rejectModalDesc", {
          code: analysis.value.requirement_code
        }),
        confirmLabel: t("pm.requirementImpact.decisionReject"),
        variant: "danger" as const,
        checkbox: ""
      }
    : {
        title: t("pm.requirementImpact.acceptModalTitle"),
        description: t("pm.requirementImpact.acceptModalDesc", {
          code: analysis.value.requirement_code
        }),
        confirmLabel: t("pm.requirementImpact.decisionAccept"),
        variant: "default" as const,
        checkbox: t("pm.requirementImpact.acceptCheckbox")
      }
);

function nowLabel(): string {
  return new Date().toISOString().slice(0, 16).replace("T", " ");
}

function onAcceptConfirm() {
  const requirement = requirements.value.find(
    item => item.id === analysis.value.requirement_id
  );
  if (requirement) {
    requirement.status = "confirmed";
    for (const revision of requirement.revisions) {
      if (revision.state === "pending_confirmation") {
        revision.state = "approved";
        revision.decision = {
          kind: "confirmed",
          actor: impactDecisionOwnerFixture,
          acted_at: nowLabel()
        };
      } else if (revision.state === "approved") {
        revision.state = "superseded";
      }
    }
    requirement.updated_at = nowLabel();
  }
  decision.value = {
    kind: "accepted",
    actor: impactDecisionOwnerFixture,
    acted_at: nowLabel()
  };
  confirmDialog.value = null;
}

function onRejectConfirm() {
  const requirement = requirements.value.find(
    item => item.id === analysis.value.requirement_id
  );
  if (requirement) {
    requirement.status = "draft";
    for (const revision of requirement.revisions) {
      if (revision.state === "pending_confirmation") {
        revision.state = "rejected";
        revision.decision = {
          kind: "rejected",
          actor: impactDecisionOwnerFixture,
          acted_at: nowLabel()
        };
      }
    }
    requirement.updated_at = nowLabel();
  }
  decision.value = {
    kind: "rejected",
    actor: impactDecisionOwnerFixture,
    acted_at: nowLabel()
  };
  confirmDialog.value = null;
}

function onMoreAnalysis() {
  analysis.value.status = "pending";
  decision.value = {
    kind: "more_analysis",
    actor: impactDecisionOwnerFixture,
    acted_at: nowLabel()
  };
}

function goBackToRequirement() {
  void router.push(`/project/requirements/${analysis.value.requirement_id}`);
}
</script>

<template>
  <PageFrame>
    <EmptyState
      v-if="!matched"
      :title="t('pm.requirementDetail.notFoundTitle')"
      :description="t('pm.requirementDetail.notFoundDesc')"
      :action-label="t('pm.requirementDetail.backToList')"
      @action="router.push('/project/requirements')"
    />

    <template v-else>
      <ImpactHeader :analysis="analysis" />

      <div class="ri-layout">
        <div class="ri-layout__main">
          <ChangeSummary :change="analysis.change" />
          <DependencyGraph :analysis="analysis" />
        </div>
        <div class="ri-layout__side">
          <ImpactPanel :findings="analysis.findings" />
          <AiAnalysisCard :ai="analysis.ai" />
          <DecisionPanel
            :analysis="analysis"
            :decision="decision"
            @accept="confirmDialog = 'accept'"
            @more-analysis="onMoreAnalysis"
            @reject="confirmDialog = 'reject'"
          />
        </div>
      </div>

      <Button
        variant="ghost"
        size="sm"
        class="ri-back"
        @click="goBackToRequirement"
      >
        {{ t("pm.requirementHistory.actionBack") }}
      </Button>

      <ConfirmationDialog
        :open="confirmDialogOpen"
        :title="confirmDialogProps.title"
        :description="confirmDialogProps.description"
        :confirm-label="confirmDialogProps.confirmLabel"
        :cancel-label="t('common.cancel')"
        :variant="confirmDialogProps.variant"
        :checkbox-label="confirmDialogProps.checkbox"
        @update:open="value => !value && (confirmDialog = null)"
        @confirm="
          confirmDialog === 'accept' ? onAcceptConfirm() : onRejectConfirm()
        "
        @cancel="confirmDialog = null"
      />
    </template>
  </PageFrame>
</template>

<style scoped>
.ri-layout {
  display: grid;
  grid-template-columns: minmax(0, 55%) minmax(0, 1fr);
  gap: var(--space-4);
  align-items: start;
  min-width: 0;
}

.ri-layout__main,
.ri-layout__side {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.ri-back {
  align-self: flex-start;
}

@media (max-width: 1100px) {
  .ri-layout {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
