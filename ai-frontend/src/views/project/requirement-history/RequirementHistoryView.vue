<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import Select from "@/components/ui/select/Select.vue";
import { PageFrame } from "@/components/shell";

import AuditCard from "./components/AuditCard.vue";
import HistoryHeader from "./components/HistoryHeader.vue";
import RevisionDetailCard from "./components/RevisionDetailCard.vue";
import RevisionTimeline from "./components/RevisionTimeline.vue";
import VersionComparison from "./components/VersionComparison.vue";

import {
  getRequirement,
  toRequirementEntry,
  type RequirementCoreDetail
} from "@/api/requirements";
import { draftAndSubmit } from "../requirements/requirement-commands";
import type { RequirementEntry } from "../requirements/requirement-entry";

/**
 * 修订与历史页（S02-UI-05，2026-11-08 接真）。
 *
 * 左列 = 垂直修订时间线（修订号/作者/时间/摘要/来源/关联决策）；
 * 右列 = 选中修订详情 + 审计信息，或（比较模式下）任意两版的并排行内 diff
 * 与元数据差异。数据 = GET /v1/pm/requirements/{id}（修订链 + 确认记录
 * 映射为关联决策）；Propose New Revision 走显式确认弹窗 = 以当前正文起草
 * 新修订并 submit——本页**不提供**任何直接覆盖/恢复旧版动作（§7）。
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

const selectedId = ref("");
const compareMode = ref(false);
const compareOldId = ref("");
const compareNewId = ref("");
const proposeDialogOpen = ref(false);

const orderedRevisions = computed(() =>
  entry.value
    ? [...entry.value.revisions].sort((a, b) => b.version - a.version)
    : []
);

const selectedRevision = computed(
  () =>
    orderedRevisions.value.find(item => item.id === selectedId.value) ?? null
);

const previousRevision = computed(
  () =>
    orderedRevisions.value.find(item => item.id === compareOldId.value) ?? null
);
const nextRevision = computed(
  () =>
    orderedRevisions.value.find(item => item.id === compareNewId.value) ?? null
);

const compareOptions = computed(() =>
  orderedRevisions.value.map(revision => ({
    value: revision.id,
    label: t("pm.requirements.revisionVersion", { version: revision.version })
  }))
);

async function load() {
  if (!requirementId.value) return;
  loading.value = true;
  loadError.value = "";
  notFound.value = false;
  try {
    const detail = await getRequirement(requirementId.value);
    entry.value = toRequirementEntry(detail);
    detailRaw.value = detail;
    if (
      !selectedId.value ||
      !detail.revisions.some(revision => revision.id === selectedId.value)
    ) {
      selectedId.value = orderedRevisions.value[0]?.id ?? "";
    }
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
    selectedId.value = "";
    compareMode.value = false;
    proposeDialogOpen.value = false;
    void load();
  },
  // immediate 必须开：真实应用直达 URL 时参数已是现值，无变更不触发；
  // 单测里 mount 后才 push 路由（undefined→值）会掩盖这一点。
  { immediate: true }
);

function onSelect(id: string) {
  selectedId.value = id;
}

function onCompare() {
  if (!compareMode.value) {
    compareOldId.value = orderedRevisions.value[1]?.id ?? "";
    compareNewId.value = orderedRevisions.value[0]?.id ?? "";
  }
  compareMode.value = !compareMode.value;
}

function onBack() {
  void router.push(`/project/requirements/${requirementId.value}`);
}

/** 提交确认：显式弹窗 → 以当前正文起草新修订并 submit（draft 不覆盖旧版）。 */
function onProposeConfirm() {
  const detail = detailRaw.value;
  const current = entry.value;
  if (!detail || !current) return;
  if (acting.value) return;
  acting.value = true;
  actionError.value = "";
  void (async () => {
    try {
      const markdown = current.body_markdown ?? current.description;
      if (!markdown.trim()) {
        actionError.value = t("pm.requirements.proposeNoBody");
        proposeDialogOpen.value = false;
        return;
      }
      const { revision } = await draftAndSubmit(requirementId.value, {
        markdown,
        summary: t("pm.requirementHistory.proposeSummary")
      });
      selectedId.value = revision.id;
      proposeDialogOpen.value = false;
      await load();
    } catch (error) {
      actionError.value =
        error instanceof Error ? error.message : String(error ?? "");
    } finally {
      acting.value = false;
    }
  })();
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
      <HistoryHeader
        :entry="entry"
        :compare-mode="compareMode"
        @back="onBack"
        @compare="onCompare"
        @propose="proposeDialogOpen = true"
      />

      <div class="rh-layout">
        <RevisionTimeline
          :revisions="orderedRevisions"
          :selected-id="selectedId"
          @select="onSelect"
        />

        <div class="rh-layout__detail">
          <template v-if="compareMode">
            <div class="rh-layout__picks">
              <label class="rh-layout__pick">
                <span>{{ t("pm.requirementHistory.compareOld") }}</span>
                <Select
                  :model-value="compareOldId"
                  :options="compareOptions"
                  size="sm"
                  :aria-label="t('pm.requirementHistory.compareOldAria')"
                  @update:model-value="compareOldId = $event"
                />
              </label>
              <label class="rh-layout__pick">
                <span>{{ t("pm.requirementHistory.compareNew") }}</span>
                <Select
                  :model-value="compareNewId"
                  :options="compareOptions"
                  size="sm"
                  :aria-label="t('pm.requirementHistory.compareNewAria')"
                  @update:model-value="compareNewId = $event"
                />
              </label>
            </div>
            <VersionComparison
              :previous="previousRevision"
              :next="nextRevision"
            />
          </template>

          <template v-else>
            <RevisionDetailCard :revision="selectedRevision" :entry="entry" />
            <AuditCard :revision="selectedRevision" :entry="entry" />
          </template>
        </div>
      </div>

      <ConfirmationDialog
        :open="proposeDialogOpen"
        :title="t('pm.requirementHistory.proposeModalTitle')"
        :description="t('pm.requirementHistory.proposeModalDesc')"
        :confirm-label="t('common.confirm')"
        :cancel-label="t('common.cancel')"
        @update:open="value => !value && (proposeDialogOpen = false)"
        @confirm="onProposeConfirm"
        @cancel="proposeDialogOpen = false"
      />

      <p class="rh-note">{{ t("pm.requirementHistory.noOverwriteNote") }}</p>

      <Button
        variant="ghost"
        size="sm"
        class="rh-back-list"
        @click="goBackToList"
      >
        {{ t("pm.requirementDetail.backToList") }}
      </Button>
    </template>
  </PageFrame>
</template>

<style scoped>
.rh-layout {
  display: grid;
  grid-template-columns: 40% minmax(0, 1fr);
  gap: var(--space-4);
  align-items: start;
  min-width: 0;
}

.rh-layout__detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.rh-layout__picks {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.rh-layout__pick {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rh-note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rh-back-list {
  align-self: flex-start;
}

@media (max-width: 1100px) {
  .rh-layout {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
