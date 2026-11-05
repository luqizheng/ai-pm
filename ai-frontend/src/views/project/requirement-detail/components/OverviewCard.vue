<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import type { RequirementEntry } from "../../requirements/requirement-entry";

/**
 * 需求概览卡（S02-UI-02 §3A）。
 *
 * Requirement ID / 标题 / 描述 / 类型 / 优先级 / 生命周期状态 /
 * 创建日期 / 最后修改日期。
 */
interface Props {
  entry: RequirementEntry;
}

const props = defineProps<Props>();
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
</script>

<template>
  <section
    class="reqd-card"
    :aria-label="t('pm.requirementDetail.overviewTitle')"
  >
    <h3 class="reqd-card__title">
      {{ t("pm.requirementDetail.overviewTitle") }}
    </h3>
    <dl class="reqd-overview__facts">
      <div>
        <dt>{{ t("pm.requirementDetail.overviewId") }}</dt>
        <dd class="reqd-overview__mono">{{ entry.code }}</dd>
      </div>
      <div>
        <dt>{{ t("pm.requirements.detailType") }}</dt>
        <dd>{{ typeLabel }}</dd>
      </div>
      <div>
        <dt>{{ t("pm.requirements.detailPriority") }}</dt>
        <dd>{{ priorityLabel }}</dd>
      </div>
      <div>
        <dt>{{ t("pm.requirements.detailStatus") }}</dt>
        <dd class="reqd-overview__status">
          <StatusIndicator :status="statusTone" size="sm" />
          <span>{{ statusLabel }}</span>
        </dd>
      </div>
      <div>
        <dt>{{ t("pm.requirementDetail.overviewCreated") }}</dt>
        <dd>{{ entry.created_at }}</dd>
      </div>
      <div>
        <dt>{{ t("pm.requirementDetail.overviewUpdated") }}</dt>
        <dd>{{ entry.updated_at }}</dd>
      </div>
    </dl>
    <p class="reqd-overview__desc">{{ entry.description }}</p>
  </section>
</template>

<style scoped>
.reqd-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.reqd-card__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.reqd-overview__facts {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.reqd-overview__facts dt {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.reqd-overview__facts dd {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.reqd-overview__mono {
  font-family: var(--font-mono);
}

.reqd-overview__status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.reqd-overview__desc {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

@media (max-width: 1100px) {
  .reqd-overview__facts {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
