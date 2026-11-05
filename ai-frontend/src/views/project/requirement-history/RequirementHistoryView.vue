<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Button from "@/components/ui/button/Button.vue";
import Select from "@/components/ui/select/Select.vue";
import { PageFrame } from "@/components/shell";

import AuditCard from "./components/AuditCard.vue";
import HistoryHeader from "./components/HistoryHeader.vue";
import RevisionDetailCard from "./components/RevisionDetailCard.vue";
import RevisionTimeline from "./components/RevisionTimeline.vue";
import VersionComparison from "./components/VersionComparison.vue";

import {
  currentAuthorFixture,
  requirementsFixture
} from "../requirements/fixtures";
import type { RequirementEntry } from "../requirements/requirement-entry";

/**
 * 修订与历史页（S02-UI-05）。
 *
 * 左列 = 垂直修订时间线（修订号/作者/时间/摘要/来源/关联决策）；
 * 右列 = 选中修订详情 + 审计信息，或（比较模式下）任意两版的并排行内 diff
 * 与元数据差异。Propose New Revision 走显式确认弹窗，仅新增
 * pending_confirmation 修订——本页**不提供**任何直接覆盖/恢复旧版动作（§7）。
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

const orderedRevisions = computed(() =>
  entry.value
    ? [...entry.value.revisions].sort((a, b) => b.version - a.version)
    : []
);

const selectedId = ref(orderedRevisions.value[0]?.id ?? "");
const compareMode = ref(false);
const compareOldId = ref("");
const compareNewId = ref("");
const proposeDialogOpen = ref(false);

watch(requirementId, () => {
  selectedId.value = orderedRevisions.value[0]?.id ?? "";
  compareMode.value = false;
  proposeDialogOpen.value = false;
});

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

function nowLabel(): string {
  return new Date().toISOString().slice(0, 16).replace("T", " ");
}

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

function onProposeConfirm() {
  const item = entry.value;
  if (!item) return;
  const version =
    item.revisions.reduce(
      (max, revision) => Math.max(max, revision.version),
      0
    ) + 1;
  const newId = `${item.id}-rev-${version}`;
  item.revisions.unshift({
    id: newId,
    version,
    state: "pending_confirmation",
    summary: t("pm.requirementHistory.proposeSummary"),
    author: currentAuthorFixture,
    created_at: nowLabel(),
    markdown: item.description,
    origin: "human_edit",
    audit: {
      initiated_by: currentAuthorFixture,
      reason: t("pm.requirementHistory.proposeReason")
    }
  });
  item.updated_at = nowLabel();
  selectedId.value = newId;
  proposeDialogOpen.value = false;
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
