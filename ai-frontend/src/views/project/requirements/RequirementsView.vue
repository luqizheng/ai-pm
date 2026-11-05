<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Ellipsis, FolderInput, Plus, ShieldCheck } from "lucide-vue-next";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { PageFrame } from "@/components/shell";

import AiAssistBar from "./components/AiAssistBar.vue";
import BaselineSheet from "./components/BaselineSheet.vue";
import ConflictSheet from "./components/ConflictSheet.vue";
import ContextSidePanel from "./components/ContextSidePanel.vue";
import RequirementDetail from "./components/RequirementDetail.vue";
import RequirementsList from "./components/RequirementsList.vue";
import RevisionSheet from "./components/RevisionSheet.vue";
import SummaryStrip from "./components/SummaryStrip.vue";

import {
  aiProposalPool,
  baselineFixture,
  currentAuthorFixture,
  requirementsFixture
} from "./fixtures";
import type {
  AiActionKind,
  AiSuggestion,
  BaselineSnapshot,
  RequirementEntry,
  RequirementPriority,
  RequirementType
} from "./requirement-entry";
import type { ConflictResolutionOption } from "./fixtures";

/**
 * Requirements Workspace 页根（S02-UI-01）。
 *
 * 结构 = 页头（Import/Add/Baseline/More）→ 概览摘要条 → 两栏主区
 * （列表 + 详情 ｜ 上下文面板）→ AI 协作栏 → 修订/冲突/基线抽屉 + 显式确认弹窗。
 * 数据 = fixtures（mock 先行）；状态迁移在页内本地完成，用于演示交互闭环：
 *   - Confirm / Reject / Propose Revision / 建立基线必须经 ConfirmationDialog 显式确认；
 *   - AI 动作只向「AI Assistance」追加提案，绝不直接改需求状态；
 *   - Baseline 只接受 confirmed 需求（红线由 BaselineSheet 承载）。
 */
const { t } = useI18n();

const requirements = ref<RequirementEntry[]>(
  structuredClone(requirementsFixture) as RequirementEntry[]
);
const baseline = ref<BaselineSnapshot>({ ...baselineFixture });

const selectedId = ref(requirements.value[0]?.id ?? "");
const statusFilter = ref("all");
const editing = ref(false);
const contextTab = ref("sources");

const revisionOpen = ref(false);
const conflictOpen = ref(false);
const baselineOpen = ref(false);

type DialogKind = "confirm" | "reject" | "propose" | "baseline";
const dialog = ref<{ kind: DialogKind; codes: string[] } | null>(null);

const selectedEntry = computed(
  () => requirements.value.find(entry => entry.id === selectedId.value) ?? null
);

const counts = computed(() => {
  const result = {
    total: requirements.value.length,
    draft: 0,
    pending_confirmation: 0,
    confirmed: 0,
    conflict: 0
  };
  for (const entry of requirements.value) {
    result[entry.status] += 1;
  }
  return result;
});

const dialogProps = computed(() => {
  const current = selectedEntry.value;
  switch (dialog.value?.kind) {
    case "confirm":
      return {
        title: t("pm.requirements.confirmDialogTitle"),
        description: current
          ? t("pm.requirements.confirmDialogDesc", {
              code: current.code,
              version: currentRevisionVersion(current)
            })
          : "",
        variant: "default" as const,
        checkbox: t("pm.requirements.confirmCheckbox")
      };
    case "reject":
      return {
        title: t("pm.requirements.rejectDialogTitle"),
        description: current
          ? t("pm.requirements.rejectDialogDesc", { code: current.code })
          : "",
        variant: "danger" as const,
        checkbox: ""
      };
    case "propose":
      return {
        title: t("pm.requirements.proposeDialogTitle"),
        description: current
          ? t("pm.requirements.proposeDialogDesc", { code: current.code })
          : "",
        variant: "default" as const,
        checkbox: ""
      };
    case "baseline":
      return {
        title: t("pm.requirements.baselineConfirmTitle"),
        description: t("pm.requirements.baselineConfirmDesc", {
          version: baseline.value.version + 1,
          count: dialog.value?.codes.length ?? 0
        }),
        variant: "default" as const,
        checkbox: t("pm.requirements.baselineConfirmCheckbox")
      };
    default:
      return {
        title: "",
        description: "",
        variant: "default" as const,
        checkbox: ""
      };
  }
});

function currentRevisionVersion(entry: RequirementEntry): number {
  return entry.revisions.reduce(
    (max, revision) => Math.max(max, revision.version),
    0
  );
}

function nowLabel(): string {
  return new Date().toISOString().slice(0, 16).replace("T", " ");
}

function onSelect(id: string) {
  selectedId.value = id;
  editing.value = false;
  contextTab.value = "sources";
}

function onStatusFilter(value: string) {
  statusFilter.value = value === "total" ? "all" : value;
}

function onDetailAction(
  action: "edit" | "propose" | "confirm" | "reject" | "resolve" | "history"
) {
  if (action === "edit") {
    editing.value = true;
    return;
  }
  if (action === "resolve") {
    conflictOpen.value = true;
    return;
  }
  if (action === "history") {
    revisionOpen.value = true;
    return;
  }
  dialog.value = { kind: action, codes: [] };
}

function onDetailSave(payload: {
  title: string;
  description: string;
  type: RequirementType;
  priority: RequirementPriority;
  criteria: string[];
}) {
  const entry = selectedEntry.value;
  if (!entry) return;
  entry.title = payload.title;
  entry.description = payload.description;
  entry.type = payload.type;
  entry.priority = payload.priority;
  entry.acceptance_criteria = payload.criteria.map((statement, index) => ({
    id: `${entry.id}-ac-${index + 1}`,
    statement
  }));
  const version = currentRevisionVersion(entry) + 1;
  entry.revisions.unshift({
    id: `${entry.id}-rev-${version}`,
    version,
    state: "draft",
    summary: t("pm.requirements.editRevisionSummary"),
    author: currentAuthorFixture,
    created_at: nowLabel(),
    markdown: entry.description
  });
  entry.updated_at = nowLabel();
  editing.value = false;
}

function onDialogConfirm() {
  const kind = dialog.value?.kind;
  const entry = selectedEntry.value;
  if (kind === "confirm" && entry) {
    entry.status = "confirmed";
    for (const revision of entry.revisions) {
      if (revision.state === "pending_confirmation")
        revision.state = "approved";
      else if (revision.state === "approved") revision.state = "superseded";
    }
    entry.updated_at = nowLabel();
  } else if (kind === "reject" && entry) {
    entry.status = "draft";
    for (const revision of entry.revisions) {
      if (revision.state === "pending_confirmation")
        revision.state = "rejected";
    }
    entry.updated_at = nowLabel();
  } else if (kind === "propose" && entry) {
    const version = currentRevisionVersion(entry) + 1;
    entry.revisions.unshift({
      id: `${entry.id}-rev-${version}`,
      version,
      state: "pending_confirmation",
      summary: t("pm.requirements.proposeRevisionSummary"),
      author: currentAuthorFixture,
      created_at: nowLabel(),
      markdown: entry.description
    });
    entry.status = "pending_confirmation";
    entry.updated_at = nowLabel();
  } else if (kind === "baseline") {
    baseline.value = {
      version: baseline.value.version + 1,
      established_at: nowLabel(),
      established_by: currentAuthorFixture,
      requirement_codes: [...(dialog.value?.codes ?? [])]
    };
    baselineOpen.value = false;
  }
  dialog.value = null;
}

function onDialogCancel() {
  dialog.value = null;
}

const conflictOptionKeys: Record<ConflictResolutionOption, string> = {
  scope_limit: "conflictOptionScope",
  revise_draft: "conflictOptionRevise",
  defer: "conflictOptionDefer"
};

function onConflictApply(option: ConflictResolutionOption) {
  const entry = selectedEntry.value;
  if (!entry) return;
  entry.status = "draft";
  entry.discussions.unshift({
    id: `${entry.id}-dsc-${entry.discussions.length + 1}`,
    author: currentAuthorFixture,
    body: t("pm.requirements.conflictResolvedNote", {
      option: t(`pm.requirements.${conflictOptionKeys[option]}`)
    }),
    created_at: nowLabel()
  });
  entry.updated_at = nowLabel();
  conflictOpen.value = false;
  contextTab.value = "discussions";
}

function onBaselineRequestEstablish(codes: string[]) {
  dialog.value = { kind: "baseline", codes };
}

function onBaselineHistory() {
  baselineOpen.value = false;
  if (selectedEntry.value) revisionOpen.value = true;
}

function appendAiSuggestion(suggestion: AiSuggestion) {
  const entry = selectedEntry.value ?? requirements.value[0];
  if (!entry) return;
  if (entry.ai_suggestions.some(item => item.id === suggestion.id)) return;
  entry.ai_suggestions.push({ ...suggestion });
  if (selectedId.value !== entry.id) selectedId.value = entry.id;
  contextTab.value = "ai";
}

function onAiAction(kind: AiActionKind) {
  const proposal = aiProposalPool[kind]?.[0];
  if (proposal) appendAiSuggestion(proposal);
}

function onAiAsk(text: string) {
  appendAiSuggestion({
    id: `ais-ask-${Date.now()}`,
    body: text,
    created_at: nowLabel()
  });
}

function onAddRequirement() {
  const seq = requirements.value.length + 1;
  const code = `REQ-${String(seq).padStart(3, "0")}`;
  const id = `req-${seq}`;
  requirements.value.unshift({
    id,
    code,
    title: t("pm.requirements.newRequirementTitle"),
    description: "",
    type: "functional",
    priority: "medium",
    status: "draft",
    source_label: t("pm.requirements.newRequirementSource"),
    updated_at: nowLabel(),
    acceptance_criteria: [],
    revisions: [
      {
        id: `${id}-rev-1`,
        version: 1,
        state: "draft",
        summary: t("pm.requirements.newRevisionSummary"),
        author: currentAuthorFixture,
        created_at: nowLabel(),
        markdown: ""
      }
    ],
    sources: [],
    discussions: [],
    analysis: [],
    ai_suggestions: [],
    related: []
  });
  selectedId.value = id;
  editing.value = true;
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('pm.projects.pageTitle'), to: '/projects' }]"
    :title="t('pm.requirements.pageTitle')"
    :description="t('pm.requirements.subtitle')"
  >
    <template #actions>
      <Button variant="secondary" size="sm" @click="baselineOpen = true">
        <ShieldCheck aria-hidden="true" />
        {{ t("pm.requirements.baselineEntry") }}
      </Button>
      <Button variant="secondary" size="sm">
        <FolderInput aria-hidden="true" />
        {{ t("pm.requirements.actionImport") }}
      </Button>
      <Button variant="primary" size="sm" @click="onAddRequirement">
        <Plus aria-hidden="true" />
        {{ t("pm.requirements.actionAdd") }}
      </Button>
      <Button variant="ghost" size="sm">
        <Ellipsis aria-hidden="true" />
        {{ t("pm.requirements.actionMore") }}
      </Button>
    </template>

    <SummaryStrip
      :counts="counts"
      :active="statusFilter"
      @select="onStatusFilter"
    />

    <div class="req-workspace">
      <div class="req-workspace__main">
        <RequirementsList
          :entries="requirements"
          :selected-id="selectedId"
          :status-filter="statusFilter"
          @select="onSelect"
          @update:status-filter="onStatusFilter"
        />
        <RequirementDetail
          v-if="selectedEntry"
          :entry="selectedEntry"
          :editing="editing"
          @action="onDetailAction"
          @save="onDetailSave"
          @cancel="editing = false"
        />
      </div>
      <ContextSidePanel
        :entry="selectedEntry"
        :active-tab="contextTab"
        @update:active-tab="contextTab = $event"
      />
    </div>

    <AiAssistBar @ask="onAiAsk" @action="onAiAction" />

    <RevisionSheet v-model:open="revisionOpen" :entry="selectedEntry" />
    <ConflictSheet
      v-model:open="conflictOpen"
      :entry="selectedEntry"
      @apply="onConflictApply"
    />
    <BaselineSheet
      v-model:open="baselineOpen"
      :requirements="requirements"
      :baseline="baseline"
      @request-establish="onBaselineRequestEstablish"
      @open-history="onBaselineHistory"
    />

    <ConfirmationDialog
      :open="dialog !== null"
      :title="dialogProps.title"
      :description="dialogProps.description"
      :confirm-label="t('common.confirm')"
      :cancel-label="t('common.cancel')"
      :variant="dialogProps.variant"
      :checkbox-label="dialogProps.checkbox"
      @update:open="value => !value && onDialogCancel()"
      @confirm="onDialogConfirm"
      @cancel="onDialogCancel"
    />
  </PageFrame>
</template>

<style scoped>
.req-workspace {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 32%;
  gap: var(--space-4);
  align-items: start;
  min-width: 0;
}

.req-workspace__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

@media (max-width: 1100px) {
  .req-workspace {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
