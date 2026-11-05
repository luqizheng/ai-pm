<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import type { ChangeImpactAnalysis } from "../impact-contract";

/**
 * 变更影响分析页头（S02-UI-07 §2）。
 *
 * 标题 Change Impact Analysis + 副标题 + 五要素元信息行：
 * 需求 ID / 标题 / 当前基线版本（未含入时明示）/ 拟议修订 / 分析状态。
 */
interface Props {
  analysis: ChangeImpactAnalysis;
}

const props = defineProps<Props>();
const { t } = useI18n();

const statusMeta = computed(() =>
  props.analysis.status === "analyzed"
    ? { tone: "success" as StatusIndicatorTone, key: "statusAnalyzed" }
    : { tone: "warning" as StatusIndicatorTone, key: "statusPending" }
);

const baselineLabel = computed(() =>
  props.analysis.baseline_version !== null
    ? t("pm.requirements.revisionVersion", {
        version: props.analysis.baseline_version
      })
    : t("pm.requirementImpact.baselineNotIncluded")
);
</script>

<template>
  <header class="rih">
    <nav
      class="rih__breadcrumb"
      :aria-label="t('pm.requirementImpact.breadcrumbAria')"
    >
      <RouterLink class="rih__crumb" to="/projects">
        {{ t("pm.projects.pageTitle") }}
      </RouterLink>
      <span class="rih__crumb-sep" aria-hidden="true">/</span>
      <RouterLink class="rih__crumb" to="/project/requirements">
        {{ t("pm.requirements.pageTitle") }}
      </RouterLink>
      <span class="rih__crumb-sep" aria-hidden="true">/</span>
      <RouterLink
        class="rih__crumb"
        :to="`/project/requirements/${props.analysis.requirement_id}`"
      >
        {{ props.analysis.requirement_code }}
      </RouterLink>
      <span class="rih__crumb-sep" aria-hidden="true">/</span>
      <span class="rih__crumb rih__crumb--current" aria-current="page">
        {{ t("pm.requirementImpact.pageTitle") }}
      </span>
    </nav>

    <div class="rih__row">
      <h1 class="rih__title">{{ t("pm.requirementImpact.pageTitle") }}</h1>
    </div>
    <p class="rih__subtitle">{{ t("pm.requirementImpact.pageSubtitle") }}</p>

    <div class="rih__meta">
      <span class="rih__meta-item">
        <span class="rih__meta-label">{{
          t("pm.requirementDetail.overviewId")
        }}</span>
        <span class="rih__mono">{{ props.analysis.requirement_code }}</span>
      </span>
      <span class="rih__meta-item rih__meta-item--title">{{
        props.analysis.title
      }}</span>
      <span class="rih__meta-item">
        <span class="rih__meta-label">{{
          t("pm.requirementImpact.currentBaseline")
        }}</span>
        <span class="rih__mono">{{ baselineLabel }}</span>
      </span>
      <span class="rih__meta-item">
        <span class="rih__meta-label">{{
          t("pm.requirementImpact.proposedRevision")
        }}</span>
        <span class="rih__mono">
          {{
            t("pm.requirements.revisionVersion", {
              version: props.analysis.proposed_revision
            })
          }}
        </span>
      </span>
      <span class="rih__meta-item">
        <StatusIndicator :status="statusMeta.tone" size="sm" />
        <span>{{ t(`pm.requirementImpact.${statusMeta.key}`) }}</span>
      </span>
    </div>
  </header>
</template>

<style scoped>
.rih {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.rih__breadcrumb {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.rih__crumb {
  color: var(--text-secondary);
  text-decoration: none;
}

a.rih__crumb:hover {
  color: var(--accent);
}

a.rih__crumb:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.rih__crumb--current {
  color: var(--text-primary);
  font-weight: var(--weight-medium);
}

.rih__crumb-sep {
  color: var(--text-disabled);
}

.rih__row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.rih__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h3);
  color: var(--text-primary);
}

.rih__subtitle {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rih__meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
  padding-top: var(--space-1);
}

.rih__meta-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  white-space: nowrap;
}

.rih__meta-item--title {
  font-weight: var(--weight-medium);
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 280px;
}

.rih__meta-label {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.rih__mono {
  font-family: var(--font-mono);
}
</style>
