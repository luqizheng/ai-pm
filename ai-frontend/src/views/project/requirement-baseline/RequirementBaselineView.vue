<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { PageFrame } from "@/components/shell";

import BaselineComparison from "./components/BaselineComparison.vue";
import BaselineDetail from "./components/BaselineDetail.vue";
import BaselineHeader from "./components/BaselineHeader.vue";
import BaselineList from "./components/BaselineList.vue";
import BaselineOverview from "./components/BaselineOverview.vue";
import PrepareWizard from "./components/PrepareWizard.vue";

import { requirementsFixture } from "../requirements/fixtures";
import {
  baselineOwnerFixture,
  baselinesFixture,
  currentBaseline
} from "./fixtures";
import type { BaselineEntryItem, BaselineRecord } from "./baseline-contract";

/**
 * 需求基线管理页（S02-UI-06）。
 *
 * 三种模式：browse（概览 + 历史 + 详情）/ prepare（五步准备向导）/
 * compare（两基线比较）。红线：基线只含 confirmed 需求的正式修订
 * （问题需求四类锁定明示，不静默含入）；建立基线必须经显式确认弹窗，
 * 绝不自动建立；历史基线保留原引用（版本不可覆盖）。
 * 数据 = fixtures（mock 先行）；契约见 baseline-contract.ts（R3：基线
 * 由各正式 Requirement Revision 的集合表达，无独立实体表）。
 */
const { t } = useI18n();

const baselines = ref<BaselineRecord[]>(
  structuredClone(baselinesFixture) as BaselineRecord[]
);
const requirements = ref(
  structuredClone(requirementsFixture) as typeof requirementsFixture
);

const mode = ref<"browse" | "prepare" | "compare">("browse");
const selectedId = ref(currentBaseline(baselinesFixture)?.id ?? "");

const current = computed(() => currentBaseline(baselines.value));
const orderedBaselines = computed(() =>
  [...baselines.value].sort((a, b) => b.version - a.version)
);
const selectedBaseline = computed(
  () => baselines.value.find(item => item.id === selectedId.value) ?? null
);

const confirmDialogOpen = ref(false);
const pendingCodes = ref<string[]>([]);

function onViewCurrent() {
  if (current.value) selectedId.value = current.value.id;
  mode.value = "browse";
}

function onWizardConfirm(codes: string[]) {
  pendingCodes.value = codes;
  confirmDialogOpen.value = true;
}

function onDialogConfirm() {
  const version = (current.value?.version ?? 0) + 1;
  const entries: BaselineEntryItem[] = pendingCodes.value.map(code => {
    const requirement = requirements.value.find(item => item.code === code);
    const approved = [...(requirement?.revisions ?? [])]
      .filter(revision => revision.state === "approved")
      .sort((a, b) => b.version - a.version)[0];
    return {
      requirement_id: requirement?.id ?? "",
      requirement_code: code,
      title: requirement?.title ?? "",
      type: requirement?.type ?? "functional",
      priority: requirement?.priority ?? "medium",
      revision_version: approved?.version ?? 1
    };
  });

  for (const baseline of baselines.value) {
    if (baseline.status === "active") baseline.status = "superseded";
  }
  const newId = `BL-${String(baselines.value.length + 1).padStart(3, "0")}`;
  baselines.value.unshift({
    id: newId,
    version,
    description: t("pm.requirementBaseline.newBaselineDescription", {
      count: entries.length
    }),
    established_at: new Date().toISOString().slice(0, 16).replace("T", " "),
    established_by: baselineOwnerFixture,
    status: "active",
    entries
  });
  selectedId.value = newId;
  confirmDialogOpen.value = false;
  mode.value = "browse";
}
</script>

<template>
  <PageFrame>
    <BaselineHeader
      :compare-mode="mode === 'compare'"
      @view-current="onViewCurrent"
      @prepare="mode = 'prepare'"
      @compare="mode = 'compare'"
    />

    <BaselineOverview :baseline="current" />

    <template v-if="mode === 'prepare'">
      <PrepareWizard
        :requirements="requirements"
        :current-baseline="current"
        @cancel="mode = 'browse'"
        @confirm="onWizardConfirm"
      />
    </template>

    <template v-else-if="mode === 'compare'">
      <BaselineComparison :baselines="orderedBaselines" />
    </template>

    <div v-else class="rb-layout">
      <BaselineList
        :baselines="orderedBaselines"
        :selected-id="selectedId"
        @select="selectedId = $event"
        @compare="mode = 'compare'"
      />
      <BaselineDetail :baseline="selectedBaseline" />
    </div>

    <ConfirmationDialog
      :open="confirmDialogOpen"
      :title="t('pm.requirementBaseline.confirmModalTitle')"
      :description="
        t('pm.requirementBaseline.confirmModalDesc', {
          version: (current?.version ?? 0) + 1,
          count: pendingCodes.length
        })
      "
      :confirm-label="t('pm.requirementBaseline.wizardEstablish')"
      :cancel-label="t('common.cancel')"
      :checkbox-label="t('pm.requirementBaseline.confirmCheckbox')"
      @update:open="value => !value && (confirmDialogOpen = false)"
      @confirm="onDialogConfirm"
      @cancel="confirmDialogOpen = false"
    />
  </PageFrame>
</template>

<style scoped>
.rb-layout {
  display: grid;
  grid-template-columns: 35% minmax(0, 1fr);
  gap: var(--space-4);
  align-items: start;
  min-width: 0;
}

@media (max-width: 1100px) {
  .rb-layout {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
