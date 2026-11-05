<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import type { RevisionRecord } from "../../requirements/requirement-entry";
import {
  collapseContext,
  diffCriteria,
  diffLines
} from "../revision-diff";

/**
 * 版本比较（S02-UI-05 §5 + Deliverable 3）。
 *
 * 左 = 上一修订，右 = 新修订；正文用行内 diff（行级 LCS）：
 * 新增行 success 底、删除行 danger 底删除线、远离差异的上下文行折叠；
 * 元数据差异单独呈现：变更的验收标准（按陈述匹配）/ 变更的优先级或分类。
 * 只用两类语义色，避免过多颜色（§5）。
 */
interface Props {
  previous: RevisionRecord | null;
  next: RevisionRecord | null;
}

const props = defineProps<Props>();
const { t } = useI18n();

const rows = computed(() => {
  if (!props.previous || !props.next) return [];
  return collapseContext(diffLines(props.previous.markdown, props.next.markdown));
});

const addedCount = computed(
  () => rows.value.filter(row => row.kind === "added").length
);
const removedCount = computed(
  () => rows.value.filter(row => row.kind === "removed").length
);

const criteriaDiff = computed(() => {
  if (!props.previous?.criteria || !props.next?.criteria) return [];
  return diffCriteria(props.previous.criteria, props.next.criteria);
});

const hasCriteriaDiff = computed(() =>
  criteriaDiff.value.some(item => item.kind !== "unchanged")
);

const priorityChanged = computed(
  () =>
    props.previous?.priority !== undefined &&
    props.next?.priority !== undefined &&
    props.previous.priority !== props.next.priority
);

const typeChanged = computed(
  () =>
    props.previous?.type !== undefined &&
    props.next?.type !== undefined &&
    props.previous.type !== props.next.type
);

const priorityLabel = (priority: "high" | "medium" | "low") =>
  ({
    high: t("pm.requirements.priorityHigh"),
    medium: t("pm.requirements.priorityMedium"),
    low: t("pm.requirements.priorityLow")
  })[priority];

const typeLabel = (type: "functional" | "non_functional") =>
  type === "functional"
    ? t("pm.requirements.typeFunctional")
    : t("pm.requirements.typeNonFunctional");
</script>

<template>
  <section class="rhc" :aria-label="t('pm.requirementHistory.compareTitle')">
    <div class="rhc__head">
      <h3 class="rhc__title">{{ t("pm.requirementHistory.compareTitle") }}</h3>
      <span
        v-if="props.previous && props.next"
        class="rhc__versions"
      >
        <Badge variant="neutral" size="sm">
          {{ t("pm.requirementHistory.compareOld", { version: props.previous.version }) }}
        </Badge>
        <Badge variant="accent" size="sm">
          {{ t("pm.requirementHistory.compareNew", { version: props.next.version }) }}
        </Badge>
        <span v-if="addedCount + removedCount > 0" class="rhc__stat">
          {{ t("pm.requirementHistory.compareStat", { added: addedCount, removed: removedCount }) }}
        </span>
      </span>
    </div>

    <EmptyState
      v-if="!props.previous || !props.next"
      size="sm"
      :title="t('pm.requirementHistory.compareEmptyTitle')"
      :description="t('pm.requirementHistory.compareEmptyDesc')"
    />

    <template v-else>
      <div class="rhc__diff">
        <template v-for="(row, index) in rows" :key="index">
          <p v-if="!row.collapsed" class="rhc__row" :class="`rhc__row--${row.kind}`">
            <span class="rhc__marker" aria-hidden="true">
              {{ row.kind === "added" ? "+" : row.kind === "removed" ? "−" : "" }}
            </span>
            <span v-if="row.text" class="rhc__row-text">{{ row.text }}</span>
            <span v-else class="rhc__row-text rhc__row-text--blank">&nbsp;</span>
          </p>
          <p v-else class="rhc__row rhc__row--collapsed" aria-hidden="true">⋯</p>
        </template>
      </div>

      <div v-if="hasCriteriaDiff" class="rhc__section">
        <span class="rhc__section-label">
          {{ t("pm.requirementHistory.compareCriteriaLabel") }}
        </span>
        <ul class="rhc__criteria">
          <li
            v-for="(item, index) in criteriaDiff"
            :key="index"
            :class="`rhc__criterion--${item.kind}`"
          >
            <span class="rhc__criterion-kind">
              {{ item.kind === "added" ? "+" : item.kind === "removed" ? "−" : "" }}
            </span>
            <span>{{ item.statement }}</span>
          </li>
        </ul>
      </div>

      <div v-if="priorityChanged || typeChanged" class="rhc__section">
        <span class="rhc__section-label">
          {{ t("pm.requirementHistory.compareMetaLabel") }}
        </span>
        <p v-if="priorityChanged" class="rhc__meta-change">
          {{ t("pm.requirementHistory.comparePriorityChange", {
            from: priorityLabel(props.previous!.priority!),
            to: priorityLabel(props.next!.priority!)
          }) }}
        </p>
        <p v-if="typeChanged" class="rhc__meta-change">
          {{ t("pm.requirementHistory.compareTypeChange", {
            from: typeLabel(props.previous!.type!),
            to: typeLabel(props.next!.type!)
          }) }}
        </p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.rhc {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rhc__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rhc__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rhc__versions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.rhc__stat {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.rhc__diff {
  display: flex;
  flex-direction: column;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  overflow: hidden;
  font-family: var(--font-mono);
}

.rhc__row {
  display: flex;
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  min-height: var(--line-height-micro);
  padding: 0 var(--space-2);
}

.rhc__marker {
  flex: none;
  width: var(--space-4);
  color: var(--text-disabled);
  user-select: none;
}

.rhc__row-text {
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text-primary);
}

.rhc__row-text--blank {
  min-height: var(--line-height-micro);
}

.rhc__row--added {
  background: color-mix(in srgb, var(--ws-color-success) 14%, transparent);
}

.rhc__row--added .rhc__marker {
  color: var(--success-strong);
}

.rhc__row--removed {
  background: color-mix(in srgb, var(--ws-color-danger) 12%, transparent);
}

.rhc__row--removed .rhc__row-text {
  text-decoration: line-through;
  color: var(--text-secondary);
}

.rhc__row--removed .rhc__marker {
  color: var(--danger-strong);
}

.rhc__row--collapsed {
  justify-content: center;
  color: var(--text-disabled);
  background: var(--surface-raised);
}

.rhc__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rhc__section-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.rhc__criteria {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rhc__criterion--added {
  display: flex;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--success-strong);
}

.rhc__criterion--removed {
  display: flex;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--danger-strong);
  text-decoration: line-through;
}

.rhc__criterion--unchanged {
  display: none;
}

.rhc__criterion-kind {
  font-family: var(--font-mono);
  flex: none;
}

.rhc__meta-change {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}
</style>