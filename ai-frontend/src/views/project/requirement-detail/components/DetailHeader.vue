<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { History, Pencil, Upload } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import type { RequirementEntry } from "../../requirements/requirement-entry";

/**
 * 需求详情页头（S02-UI-02 §2）。
 *
 * 面包屑 Project / Requirements / REQ-001 + 需求 ID 与标题 +
 * 状态/优先级/类型/创建与更新时间元信息行 + 页级操作（Edit/Propose/History）。
 * 不带 title 走 PageFrame 页头——详情页头结构更丰富，由本组件承担
 * （同 CommandCenterHeader 先例）。
 */
interface Props {
  entry: RequirementEntry;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (e: "edit"): void;
  (e: "propose"): void;
  (e: "history"): void;
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

const typeLabel = computed(() =>
  props.entry.type === "functional"
    ? t("pm.requirements.typeFunctional")
    : t("pm.requirements.typeNonFunctional")
);

const priorityLabel = computed(
  () =>
    (
      ({
        high: t("pm.requirements.priorityHigh"),
        medium: t("pm.requirements.priorityMedium"),
        low: t("pm.requirements.priorityLow")
      }) as const
    )[props.entry.priority]
);

/** 模板消费用的解包引用 */
const entry = computed(() => props.entry);
</script>

<template>
  <header class="reqd-header">
    <nav
      class="reqd-header__breadcrumb"
      :aria-label="t('pm.requirements.contextAria')"
    >
      <RouterLink class="reqd-header__crumb" to="/projects">
        {{ t("pm.projects.pageTitle") }}
      </RouterLink>
      <span class="reqd-header__crumb-sep" aria-hidden="true">/</span>
      <RouterLink class="reqd-header__crumb" to="/project/requirements">
        {{ t("pm.requirements.pageTitle") }}
      </RouterLink>
      <span class="reqd-header__crumb-sep" aria-hidden="true">/</span>
      <span
        class="reqd-header__crumb reqd-header__crumb--current"
        aria-current="page"
      >
        {{ entry.code }}
      </span>
    </nav>

    <div class="reqd-header__row">
      <div class="reqd-header__identity">
        <span class="reqd-header__code">{{ entry.code }}</span>
        <h1 class="reqd-header__title">{{ entry.title }}</h1>
      </div>
      <div class="reqd-header__actions">
        <Button variant="secondary" size="sm" @click="emit('edit')">
          <Pencil aria-hidden="true" />
          {{ t("pm.requirements.actionEdit") }}
        </Button>
        <Button variant="primary" size="sm" @click="emit('propose')">
          <Upload aria-hidden="true" />
          {{ t("pm.requirements.actionPropose") }}
        </Button>
        <Button variant="ghost" size="sm" @click="emit('history')">
          <History aria-hidden="true" />
          {{ t("pm.requirements.actionHistory") }}
        </Button>
      </div>
    </div>

    <div class="reqd-header__meta">
      <span class="reqd-header__meta-item">
        <StatusIndicator :status="statusTone" size="sm" />
        <span>{{ statusLabel }}</span>
      </span>
      <Badge variant="neutral" size="sm">{{ typeLabel }}</Badge>
      <Badge
        :variant="entry.priority === 'high' ? 'danger' : 'neutral'"
        size="sm"
      >
        {{ priorityLabel }}
      </Badge>
      <span class="reqd-header__meta-item reqd-header__meta-item--muted">
        {{ t("pm.requirementDetail.overviewCreated") }} {{ entry.created_at }}
      </span>
      <span class="reqd-header__meta-item reqd-header__meta-item--muted">
        {{ t("pm.requirementDetail.overviewUpdated") }} {{ entry.updated_at }}
      </span>
    </div>
  </header>
</template>

<style scoped>
.reqd-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.reqd-header__breadcrumb {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.reqd-header__crumb {
  color: var(--text-secondary);
  text-decoration: none;
}

a.reqd-header__crumb:hover {
  color: var(--accent);
}

a.reqd-header__crumb:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.reqd-header__crumb--current {
  color: var(--text-primary);
  font-weight: var(--weight-medium);
}

.reqd-header__crumb-sep {
  color: var(--text-disabled);
}

.reqd-header__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.reqd-header__identity {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.reqd-header__code {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.reqd-header__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h3);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reqd-header__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}

.reqd-header__meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.reqd-header__meta-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  white-space: nowrap;
}

.reqd-header__meta-item--muted {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
</style>
