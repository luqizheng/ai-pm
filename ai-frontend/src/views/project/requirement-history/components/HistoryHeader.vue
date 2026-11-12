<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeft, ArrowLeftRight, Upload } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import type { RequirementEntry } from "../../requirements/requirement-entry";
import { currentRevision } from "../../requirements/requirement-entry";

/**
 * 修订历史页头（S02-UI-05 §2）。
 *
 * 标题 Requirement History + 需求 ID / 标题 / 当前版本 / 当前状态 /
 * 最后修改时间 + 三动作（返回需求 / 比较版本 / 提议新修订）。
 * 不带 title 走 PageFrame——页头结构更丰富，由本组件承担。
 */
interface Props {
  entry: RequirementEntry;
  compareMode: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "back"): void;
  (e: "compare"): void;
  (e: "propose"): void;
}>();

const { t } = useI18n();

const statusLabel = computed(
  () =>
    (
      ({
        draft: t("pm.requirements.statusDraft"),
        pending_confirmation: t("pm.requirements.statusPending"),
        confirmed: t("pm.requirements.statusConfirmed"),
        conflict: t("pm.requirements.statusConflict")
      }) as const
    )[props.entry.status]
);

const statusTone = computed<StatusIndicatorTone>(
  () =>
    (
      ({
        draft: "neutral",
        pending_confirmation: "warning",
        confirmed: "success",
        conflict: "danger"
      }) as const
    )[props.entry.status]
);

const currentVersion = computed(
  () => currentRevision(props.entry)?.version ?? 1
);
</script>

<template>
  <header class="rh-header">
    <nav
      class="rh-header__breadcrumb"
      :aria-label="t('pm.requirementHistory.breadcrumbAria')"
    >
      <RouterLink class="rh-header__crumb" to="/projects">
        {{ t("pm.projects.pageTitle") }}
      </RouterLink>
      <span class="rh-header__crumb-sep" aria-hidden="true">/</span>
      <RouterLink class="rh-header__crumb" to="/project/requirements">
        {{ t("pm.requirements.pageTitle") }}
      </RouterLink>
      <span class="rh-header__crumb-sep" aria-hidden="true">/</span>
      <RouterLink
        class="rh-header__crumb"
        :to="`/project/requirements/${entry.id}`"
      >
        {{ entry.code }}
      </RouterLink>
      <span class="rh-header__crumb-sep" aria-hidden="true">/</span>
      <span
        class="rh-header__crumb rh-header__crumb--current"
        aria-current="page"
      >
        {{ t("pm.requirementHistory.pageTitle") }}
      </span>
    </nav>

    <div class="rh-header__row">
      <h1 class="rh-header__title">
        {{ t("pm.requirementHistory.pageTitle") }}
      </h1>
      <div class="rh-header__actions">
        <Button variant="secondary" size="sm" @click="emit('back')">
          <ArrowLeft aria-hidden="true" />
          {{ t("pm.requirementHistory.actionBack") }}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          :aria-pressed="props.compareMode"
          @click="emit('compare')"
        >
          <ArrowLeftRight aria-hidden="true" />
          {{ t("pm.requirementHistory.actionCompare") }}
        </Button>
        <Button variant="primary" size="sm" @click="emit('propose')">
          <Upload aria-hidden="true" />
          {{ t("pm.requirementHistory.actionPropose") }}
        </Button>
      </div>
    </div>

    <div class="rh-header__meta">
      <span class="rh-header__meta-item">
        <span class="rh-header__meta-label">{{
          t("pm.requirementDetail.overviewId")
        }}</span>
        <span class="rh-header__mono">{{ entry.code }}</span>
      </span>
      <span class="rh-header__meta-item rh-header__meta-item--title">{{
        entry.title
      }}</span>
      <span class="rh-header__meta-item">
        <span class="rh-header__meta-label">{{
          t("pm.requirementHistory.currentVersion")
        }}</span>
        <span class="rh-header__mono">
          {{
            t("pm.requirementDetail.currentVersion", {
              version: currentVersion
            })
          }}
        </span>
      </span>
      <span class="rh-header__meta-item">
        <StatusIndicator :status="statusTone" size="sm" />
        <span>{{ statusLabel }}</span>
      </span>
      <span class="rh-header__meta-item">
        <span class="rh-header__meta-label">{{
          t("pm.requirements.detailUpdated")
        }}</span>
        <span>{{ entry.updated_at }}</span>
      </span>
    </div>
  </header>
</template>

<style scoped>
.rh-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.rh-header__breadcrumb {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.rh-header__crumb {
  color: var(--text-secondary);
  text-decoration: none;
}

a.rh-header__crumb:hover {
  color: var(--accent);
}

a.rh-header__crumb:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.rh-header__crumb--current {
  color: var(--text-primary);
  font-weight: var(--weight-medium);
}

.rh-header__crumb-sep {
  color: var(--text-disabled);
}

.rh-header__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.rh-header__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h3);
  color: var(--text-primary);
}

.rh-header__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}

.rh-header__meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.rh-header__meta-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.rh-header__meta-item--title {
  font-weight: var(--weight-medium);
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 320px;
}

.rh-header__meta-label {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.rh-header__mono {
  font-family: var(--font-mono);
}
</style>
