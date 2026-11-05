<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";

import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";

import AcceptanceCard from "./components/AcceptanceCard.vue";
import DetailContextPanel from "./components/DetailContextPanel.vue";
import DetailHeader from "./components/DetailHeader.vue";
import EditSheet from "./components/EditSheet.vue";
import OverviewCard from "./components/OverviewCard.vue";
import SourceSheet from "./components/SourceSheet.vue";
import SpecificationCard from "./components/SpecificationCard.vue";
import TraceabilityCard from "./components/TraceabilityCard.vue";

import {
  currentAuthorFixture,
  requirementsFixture
} from "../requirements/fixtures";
import type {
  RequirementEntry,
  RequirementPriority,
  RequirementType,
  RelatedItem,
  SourceItem
} from "../requirements/requirement-entry";

/**
 * 需求详情与追溯页（S02-UI-02）。
 *
 * 单条需求的检查界面（非任务管理页）：概览 / 规格 / 验收标准 / 追溯链四卡 +
 * 右栏上下文（Sources / Related / Discussions / History）。
 * 路由 = /project/requirements/:requirementId（不占导航，入口来自
 * Requirements Workspace 详情面板「打开详情页」与追溯链关联需求导航）。
 * 数据 = fixtures（mock 先行）；编辑仅生成 draft 修订，提交确认走显式
 * ConfirmationDialog——人类确认红线与 Workspace 一致。
 */
const route = useRoute();
const router = useRouter();
const { t } = useI18n();

const requirements = ref<RequirementEntry[]>(
  structuredClone(requirementsFixture) as RequirementEntry[]
);

const requirementId = computed(() => String(route.params.requirementId ?? ""));

const entry = computed(
  () => requirements.value.find(item => item.id === requirementId.value) ?? null
);

const contextTab = ref("sources");
const sourceSheetOpen = ref(false);
const activeSource = ref<SourceItem | null>(null);
const editSheetOpen = ref(false);
const proposeDialogOpen = ref(false);

watch(requirementId, () => {
  contextTab.value = "sources";
  sourceSheetOpen.value = false;
  activeSource.value = null;
  editSheetOpen.value = false;
  proposeDialogOpen.value = false;
});

function currentRevisionVersion(item: RequirementEntry): number {
  return item.revisions.reduce(
    (max, revision) => Math.max(max, revision.version),
    0
  );
}

function nowLabel(): string {
  return new Date().toISOString().slice(0, 16).replace("T", " ");
}

function onInspect(source: SourceItem) {
  activeSource.value = source;
  sourceSheetOpen.value = true;
}

function onNavigate(item: RelatedItem) {
  if (!item.target_id) return;
  void router.push(`/project/requirements/${item.target_id}`);
}

function onEditSave(payload: {
  title: string;
  description: string;
  type: RequirementType;
  priority: RequirementPriority;
  criteria: string[];
  specification: RequirementEntry["specification"];
}) {
  const item = entry.value;
  if (!item) return;
  item.title = payload.title;
  item.description = payload.description;
  item.type = payload.type;
  item.priority = payload.priority;
  item.acceptance_criteria = payload.criteria.map((statement, index) => ({
    id: `${item.id}-ac-${index + 1}`,
    statement
  }));
  item.specification = payload.specification;
  const version = currentRevisionVersion(item) + 1;
  item.revisions.unshift({
    id: `${item.id}-rev-${version}`,
    version,
    state: "draft",
    summary: t("pm.requirements.editRevisionSummary"),
    author: currentAuthorFixture,
    created_at: nowLabel(),
    markdown: item.description
  });
  item.updated_at = nowLabel();
  editSheetOpen.value = false;
  contextTab.value = "history";
}

function onDialogConfirm() {
  const item = entry.value;
  if (!item) return;
  const version = currentRevisionVersion(item) + 1;
  item.revisions.unshift({
    id: `${item.id}-rev-${version}`,
    version,
    state: "pending_confirmation",
    summary: t("pm.requirements.proposeRevisionSummary"),
    author: currentAuthorFixture,
    created_at: nowLabel(),
    markdown: item.description
  });
  item.status = "pending_confirmation";
  item.updated_at = nowLabel();
  proposeDialogOpen.value = false;
  contextTab.value = "history";
}

function goBackToList() {
  void router.push("/project/requirements");
}
</script>

<template>
  <PageFrame>
    <EmptyState
      v-if="!entry"
      :title="t('pm.requirementDetail.notFoundTitle')"
      :description="t('pm.requirementDetail.notFoundDesc')"
      :action-label="t('pm.requirementDetail.backToList')"
      @action="goBackToList"
    />

    <template v-else>
      <DetailHeader
        :entry="entry"
        @edit="editSheetOpen = true"
        @propose="proposeDialogOpen = true"
        @history="router.push(`/project/requirements/${entry.id}/history`)"
        @impact="router.push(`/project/requirements/${entry.id}/impact`)"
      />

      <div class="reqd-layout">
        <div class="reqd-layout__main">
          <OverviewCard :entry="entry" />
          <SpecificationCard
            :specification="entry.specification"
            :description="entry.description"
          />
          <AcceptanceCard :criteria="entry.acceptance_criteria" />
          <TraceabilityCard
            :entry="entry"
            @inspect="onInspect"
            @navigate="onNavigate"
          />
        </div>
        <DetailContextPanel
          :entry="entry"
          :active-tab="contextTab"
          @update:active-tab="contextTab = $event"
          @inspect="onInspect"
          @navigate="onNavigate"
          @history="router.push(`/project/requirements/${entry.id}/history`)"
        />
      </div>

      <SourceSheet
        v-model:open="sourceSheetOpen"
        :source="activeSource"
        :entry-code="entry.code"
      />
      <EditSheet
        v-model:open="editSheetOpen"
        :entry="entry"
        @save="onEditSave"
      />

      <ConfirmationDialog
        :open="proposeDialogOpen"
        :title="t('pm.requirements.proposeDialogTitle')"
        :description="
          t('pm.requirements.proposeDialogDesc', { code: entry.code })
        "
        :confirm-label="t('common.confirm')"
        :cancel-label="t('common.cancel')"
        @update:open="value => !value && (proposeDialogOpen = false)"
        @confirm="onDialogConfirm"
        @cancel="proposeDialogOpen = false"
      />
    </template>
  </PageFrame>
</template>

<style scoped>
.reqd-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 32%;
  gap: var(--space-4);
  align-items: start;
  min-width: 0;
}

.reqd-layout__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

@media (max-width: 1100px) {
  .reqd-layout {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
