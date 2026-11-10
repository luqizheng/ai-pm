<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { ClipboardCheck, Plus, RefreshCw, ShieldCheck } from "lucide-vue-next";
import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import {
  confirmRevision,
  createRequirement,
  getRequirement,
  listProjectRequirements,
  listRequirementCriteria,
  listRequirementSources,
  rejectRevision,
  toCriteria,
  toEntryFromListItem,
  toRequirementEntry,
  toSourceItems,
  type RequirementCoreDetail
} from "@/api/requirements";
import {
  draftAndSubmit,
  latestRevision,
  latestDraftRevision,
  submitDraft,
  upsertDraft
} from "./requirement-commands";
import type {
  RequirementCreatePayload,
  RequirementEntry
} from "./requirement-entry";

import ContextSidePanel from "./components/ContextSidePanel.vue";
import RequirementCreateDialog from "./components/RequirementCreateDialog.vue";
import RequirementDetail from "./components/RequirementDetail.vue";
import RequirementsList from "./components/RequirementsList.vue";
import RevisionSheet from "./components/RevisionSheet.vue";
import SummaryStrip from "./components/SummaryStrip.vue";

/**
 * Requirements Workspace 页根（S02-UI-01，2026-11-08 接真）。
 *
 * 数据 = pm-api REST（S02-I1/I3 契约层 `@/api/requirements`）：
 *   - 列表/详情/来源为真实读模型；状态迁移全部走后端命令，成功后回刷；
 *   - 确认带 `expected_content_hash`（审批人核准哈希）+ `expected_row_revision`
 *     CAS 令牌（I1 §7），漂移由后端 409 拒绝；
 *   - 人类确认/拒绝是显式操作（ConfirmationDialog），AI 无任何状态入口；
 *   - 编辑 = 起草/更新一份正文修订（title/type/priority 创建后不可改）；
 *   - 验收标准（S02-I5，2026-11-08 二批接线）只读呈现 + 确认漂移提示；
 *     治理动作（增删改/裁决）归详情页验收卡；
 *   - 诚实缺位：discussions / analysis / ai_suggestions（AI 编排切片）、
 *     规格面（S02-I5 specification）、导入（未接线）不展示伪功能。
 */
const router = useRouter();
const route = useRoute();
const { t } = useI18n();
const scope = useScopeStore();

const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  if (scope.currentProjectId && scope.currentProjectId !== SCOPE_ALL) {
    return scope.currentProjectId;
  }
  return "";
});

const requirements = ref<RequirementEntry[]>([]);
const detailRaw = ref<RequirementCoreDetail | null>(null);
const selectedId = ref("");
const statusFilter = ref("all");
const editing = ref(false);
const contextTab = ref("sources");
const revisionOpen = ref(false);
const createOpen = ref(false);

const loading = ref(true);
const loadError = ref("");
const detailLoading = ref(false);
const acting = ref(false);
const actionError = ref("");
const driftHint = ref(false);

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

/** 下一需求编码：按现有 REQ-NNN 序列递推（项目内唯一由后端 UNIQUE 兜底）。 */
const nextCode = computed(() => {
  let max = 0;
  for (const entry of requirements.value) {
    const matched = /^REQ-(\d+)$/.exec(entry.code);
    if (matched) max = Math.max(max, Number(matched[1]));
  }
  return `REQ-${String(max + 1).padStart(3, "0")}`;
});

const dialog = ref<"confirm" | "reject" | null>(null);

const dialogProps = computed(() => {
  const current = selectedEntry.value;
  switch (dialog.value) {
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

// ───────────────────────────────────────────────────────── 数据加载

async function loadDetail(id: string) {
  detailLoading.value = true;
  try {
    const [detail, sources, criteria] = await Promise.all([
      getRequirement(id),
      listRequirementSources(id),
      listRequirementCriteria(id)
    ]);
    const entry = toRequirementEntry(detail);
    entry.sources = toSourceItems(sources.items);
    entry.acceptance_criteria = toCriteria(criteria.items);
    detailRaw.value = detail;
    const index = requirements.value.findIndex(item => item.id === id);
    if (index >= 0) requirements.value.splice(index, 1, entry);
    else requirements.value.unshift(entry);
  } finally {
    detailLoading.value = false;
  }
}

async function load() {
  if (!projectId.value) {
    requirements.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = "";
  try {
    const { items } = await listProjectRequirements(projectId.value);
    requirements.value = items.map(toEntryFromListItem);
    if (
      !selectedId.value ||
      !items.some(item => item.id === selectedId.value)
    ) {
      selectedId.value = items[0]?.id ?? "";
    }
    if (selectedId.value) await loadDetail(selectedId.value);
  } catch (error) {
    loadError.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

function reload() {
  void load();
}

// ───────────────────────────────────────────────────────── 交互

function onSelect(id: string) {
  selectedId.value = id;
  editing.value = false;
  contextTab.value = "sources";
  driftHint.value = false;
  void loadDetail(id);
}

function onStatusFilter(value: string) {
  statusFilter.value = value === "total" ? "all" : value;
}

function onDetailAction(
  action:
    | "edit"
    | "propose"
    | "confirm"
    | "reject"
    | "resolve"
    | "history"
    | "open"
) {
  if (action === "edit") {
    editing.value = true;
    return;
  }
  if (action === "open") {
    void router.push(`/project/requirements/${selectedId.value}`);
    return;
  }
  if (action === "history") {
    revisionOpen.value = true;
    return;
  }
  if (action === "propose") {
    void runCommand(runPropose);
    return;
  }
  dialog.value = action === "confirm" ? "confirm" : "reject";
}

/** 命令统一编排：在途闸 + 错误呈现 + 成功回刷列表与详情。 */
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

/** 编辑保存：最新修订为 draft 则更新之，否则起草新修订（S02-I1 语义）。 */
function saveEdit(payload: { markdown: string; summary: string }) {
  return runCommand(async () => {
    const detail = detailRaw.value;
    const entry = selectedEntry.value;
    if (!detail || !entry) return;
    await upsertDraft(entry.id, detail, {
      markdown: payload.markdown,
      summary: payload.summary || undefined
    });
    editing.value = false;
  });
}

/** 提交确认：有 draft 修订直接 submit；否则以当前正文起草并 submit。 */
async function runPropose() {
  const detail = detailRaw.value;
  const entry = selectedEntry.value;
  if (!detail || !entry) return;
  if (latestDraftRevision(detail)) {
    await submitDraft(entry.id, detail);
    return;
  }
  const markdown = entry.body_markdown ?? entry.description;
  if (!markdown.trim()) {
    actionError.value = t("pm.requirements.proposeNoBody");
    return;
  }
  await draftAndSubmit(entry.id, {
    markdown,
    summary: t("pm.requirements.proposeRevisionSummary")
  });
}

/** 正式确认：携带审批人核准哈希 + 行级 CAS 令牌（I1 §7，漂移 409）。 */
async function runConfirm() {
  const detail = detailRaw.value;
  const entry = selectedEntry.value;
  if (!detail || !entry || dialog.value !== "confirm") return;
  const pending = latestRevision(detail);
  if (!pending || pending.state !== "pending_confirmation") return;
  const result = await confirmRevision(entry.id, pending.id, {
    expected_content_hash: pending.content_hash,
    expected_row_revision: detail.requirement.row_revision
  });
  // Q-12 一致性提示：验收标准在该修订提交后又被修改 = 漂移
  driftHint.value = result.criteria_drift === true;
  dialog.value = null;
}

async function runReject() {
  const detail = detailRaw.value;
  const entry = selectedEntry.value;
  if (!detail || !entry || dialog.value !== "reject") return;
  const pending = latestRevision(detail);
  if (!pending || pending.state !== "pending_confirmation") return;
  await rejectRevision(entry.id, pending.id);
  dialog.value = null;
}

function onDialogConfirm() {
  if (dialog.value === "confirm") void runCommand(runConfirm);
  else if (dialog.value === "reject") void runCommand(runReject);
}

function onDialogCancel() {
  dialog.value = null;
}

function onDetailSave(payload: { markdown: string; summary: string }) {
  void saveEdit(payload);
}

function onCreate(payload: RequirementCreatePayload) {
  void runCommand(async () => {
    const { requirement } = await createRequirement(projectId.value, payload);
    createOpen.value = false;
    await load();
    selectedId.value = requirement.id;
    await loadDetail(requirement.id);
  });
}

function openCreateDialog() {
  createOpen.value = true;
}

onMounted(() => {
  void load();
});

// 项目上下文变化（壳切项目 / 路由带入 project_id）必须重载——
// 需求列表是项目级读模型，不重载会停留在上一个项目的数据。
watch(projectId, () => {
  void load();
});
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('pm.projects.pageTitle'), to: '/projects' }]"
    :title="t('pm.requirements.pageTitle')"
    :description="t('pm.requirements.subtitle')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        @click="router.push('/project/requirements/review')"
      >
        <ClipboardCheck aria-hidden="true" />
        {{ t("pm.requirementReview.entryLabel") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        @click="router.push('/project/requirements/baselines')"
      >
        <ShieldCheck aria-hidden="true" />
        {{ t("pm.requirements.baselineEntry") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        :disabled="!projectId"
        @click="openCreateDialog"
      >
        <Plus aria-hidden="true" />
        {{ t("pm.requirements.actionAdd") }}
      </Button>
    </template>

    <Alert
      v-if="loadError"
      variant="danger"
      :title="t('pm.requirements.loadError')"
    >
      <span>{{ loadError }}</span>
      <Button variant="secondary" size="sm" @click="reload">
        <RefreshCw aria-hidden="true" />
        {{ t("pm.requirements.loadRetry") }}
      </Button>
    </Alert>

    <Alert
      v-if="actionError"
      variant="danger"
      :title="t('pm.requirements.actionError')"
      dismissible
      @close="actionError = ''"
    >
      <span>{{ actionError }}</span>
    </Alert>

    <Alert
      v-if="driftHint"
      variant="warning"
      :title="t('pm.requirements.driftHintTitle')"
      dismissible
      @close="driftHint = false"
    >
      <span>{{ t("pm.requirements.driftHintBody") }}</span>
    </Alert>

    <EmptyState
      v-if="!projectId"
      :title="t('pm.requirements.noProjectTitle')"
      :description="t('pm.requirements.noProjectDesc')"
    />

    <template v-else>
      <EmptyState
        v-if="loading && requirements.length === 0"
        :title="t('pm.requirements.loadingTitle')"
        :description="t('pm.requirements.loadingDesc')"
      />

      <template v-else>
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

        <RevisionSheet v-model:open="revisionOpen" :entry="selectedEntry" />

        <RequirementCreateDialog
          v-model:open="createOpen"
          :next-code="nextCode"
          @create="onCreate"
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
      </template>
    </template>
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
