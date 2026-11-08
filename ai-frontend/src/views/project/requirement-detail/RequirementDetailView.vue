<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";

import Alert from "@/components/ui/alert/Alert.vue";
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
  addRequirementCriterion,
  getRequirement,
  listRequirementCriteria,
  listRequirementSources,
  recordCriterionVerdict,
  removeRequirementCriterion,
  toCriteria,
  toRequirementEntry,
  toSourceItems,
  type CriterionVerdictInput,
  type RequirementCoreDetail
} from "@/api/requirements";
import {
  draftAndSubmit,
  latestDraftRevision,
  submitDraft,
  upsertDraft
} from "../requirements/requirement-commands";
import type {
  CriterionVerdict,
  RelatedItem,
  RequirementEntry,
  SourceItem,
  VerificationKind
} from "../requirements/requirement-entry";

/**
 * 需求详情与追溯页（S02-UI-02，2026-11-08 接真）。
 *
 * 单条需求的检查界面（非任务管理页）：概览 / 规格 / 验收标准 / 追溯链四卡 +
 * 右栏上下文（Sources / Related / Discussions / History）。
 * 路由 = /project/requirements/:requirementId（不占导航，入口来自
 * Requirements Workspace 详情面板「打开详情页」与追溯链关联需求导航）。
 * 数据 = pm-api REST（S02-I1 详情 + I3 来源 + I5 验收标准）；编辑/提交走
 * 共享命令助手（requirement-commands），确认红线与 Workspace 一致；验收卡
 * 治理动作（增/删/裁决）直接落 REST（后端闸 = Lead OR 显式 Approver，
 * 非授权 403 走错误态）；规格/关联（S02-I5 规格面/I4/AI 编排）诚实空缺。
 */
const route = useRoute();
const router = useRouter();
const { t } = useI18n();

const requirementId = computed(() => String(route.params.requirementId ?? ""));

const entry = ref<RequirementEntry | null>(null);
const detailRaw = ref<RequirementCoreDetail | null>(null);
const loading = ref(true);
const loadError = ref("");
const notFound = ref(false);
const acting = ref(false);
const actionError = ref("");

const contextTab = ref("sources");
const sourceSheetOpen = ref(false);
const activeSource = ref<SourceItem | null>(null);
const editSheetOpen = ref(false);
const proposeDialogOpen = ref(false);

async function load() {
  if (!requirementId.value) return;
  loading.value = true;
  loadError.value = "";
  notFound.value = false;
  try {
    const [detail, sources, criteria] = await Promise.all([
      getRequirement(requirementId.value),
      listRequirementSources(requirementId.value),
      listRequirementCriteria(requirementId.value)
    ]);
    const mapped = toRequirementEntry(detail);
    mapped.sources = toSourceItems(sources.items);
    mapped.acceptance_criteria = toCriteria(criteria.items);
    entry.value = mapped;
    detailRaw.value = detail;
  } catch (error) {
    entry.value = null;
    detailRaw.value = null;
    const message =
      error instanceof Error ? error.message : String(error ?? "");
    if (/404|not found/i.test(message)) notFound.value = true;
    else loadError.value = message;
  } finally {
    loading.value = false;
  }
}

watch(
  requirementId,
  () => {
    contextTab.value = "sources";
    sourceSheetOpen.value = false;
    activeSource.value = null;
    editSheetOpen.value = false;
    proposeDialogOpen.value = false;
    void load();
  },
  { immediate: true }
);

/** 命令统一编排：在途闸 + 错误呈现 + 成功回刷。 */
async function runCommand(command: () => Promise<void>) {
  if (acting.value) return;
  acting.value = true;
  actionError.value = "";
  try {
    await command();
    await load();
  } catch (error) {
    actionError.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    acting.value = false;
  }
}

function onInspect(source: SourceItem) {
  activeSource.value = source;
  sourceSheetOpen.value = true;
}

function onNavigate(item: RelatedItem) {
  if (!item.target_id) return;
  void router.push(`/project/requirements/${item.target_id}`);
}

// ── 验收标准治理命令（S02-I5：后端闸 = 真人 + Lead OR 显式 Approver）

function onCriterionAdd(payload: {
  statement: string;
  verification_kind: VerificationKind;
}) {
  return runCommand(async () => {
    await addRequirementCriterion(requirementId.value, payload);
  });
}

function onCriterionRemove(criterionId: string) {
  return runCommand(async () => {
    await removeRequirementCriterion(requirementId.value, criterionId);
  });
}

function onCriterionVerdict(payload: {
  criterionId: string;
  verdict: Extract<CriterionVerdict, "passed" | "failed">;
}) {
  const body: CriterionVerdictInput = { verdict: payload.verdict };
  return runCommand(async () => {
    await recordCriterionVerdict(
      requirementId.value,
      payload.criterionId,
      body
    );
  });
}

function onEditSave(payload: { markdown: string; summary: string }) {
  const detail = detailRaw.value;
  const current = entry.value;
  if (!detail || !current) return;
  return runCommand(async () => {
    await upsertDraft(current.id, detail, {
      markdown: payload.markdown,
      summary: payload.summary || undefined
    });
    editSheetOpen.value = false;
  });
}

function onProposeConfirm() {
  const detail = detailRaw.value;
  const current = entry.value;
  if (!detail || !current) return;
  return runCommand(async () => {
    if (latestDraftRevision(detail)) {
      await submitDraft(current.id, detail);
      return;
    }
    const markdown = current.body_markdown ?? current.description;
    if (!markdown.trim()) {
      actionError.value = t("pm.requirements.proposeNoBody");
      return;
    }
    await draftAndSubmit(current.id, {
      markdown,
      summary: t("pm.requirements.proposeRevisionSummary")
    });
    proposeDialogOpen.value = false;
  });
}

function goBackToList() {
  void router.push("/project/requirements");
}
</script>

<template>
  <PageFrame>
    <Alert
      v-if="actionError"
      variant="danger"
      :title="t('pm.requirements.actionError')"
      dismissible
      @close="actionError = ''"
    >
      <span>{{ actionError }}</span>
    </Alert>

    <EmptyState
      v-if="loading && !entry"
      :title="t('pm.requirements.loadingTitle')"
      :description="t('pm.requirements.loadingDesc')"
    />

    <EmptyState
      v-else-if="loadError"
      :title="t('pm.requirements.loadError')"
      :description="loadError"
      :action-label="t('pm.requirements.loadRetry')"
      @action="load()"
    />

    <EmptyState
      v-else-if="notFound || !entry"
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
            :body-markdown="entry.body_markdown"
          />
          <AcceptanceCard
            :criteria="entry.acceptance_criteria"
            :acting="acting"
            @add="onCriterionAdd"
            @remove="onCriterionRemove"
            @verdict="onCriterionVerdict"
          />
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
        @confirm="onProposeConfirm"
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
