<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type { BaselineRecord } from "../baseline-contract";

/**
 * 基线详情（S02-UI-06 §5）。
 *
 * A. 基线元数据（标识/版本/描述/确立时间/确认人）
 * B. 含入需求（ID / 标题 / 类型 / 优先级 / 含入修订——基线引用正式修订）
 * C. 基线摘要（总数 / 功能 / 非功能 + 类型分布条）
 */
interface Props {
  baseline: BaselineRecord | null;
}

const props = defineProps<Props>();
const { t } = useI18n();

const functionalCount = computed(
  () =>
    props.baseline?.entries.filter(entry => entry.type === "functional")
      .length ?? 0
);
const nonFunctionalCount = computed(
  () =>
    props.baseline?.entries.filter(entry => entry.type === "non_functional")
      .length ?? 0
);
const total = computed(() => props.baseline?.entries.length ?? 0);

const functionalPercent = computed(() =>
  total.value === 0
    ? 0
    : Math.round((functionalCount.value / total.value) * 100)
);

const typeLabel = (type: string) =>
  type === "functional"
    ? t("pm.requirements.typeFunctional")
    : t("pm.requirements.typeNonFunctional");

const priorityLabel = (priority: string) =>
  ({
    high: t("pm.requirements.priorityHigh"),
    medium: t("pm.requirements.priorityMedium"),
    low: t("pm.requirements.priorityLow")
  })[priority] ?? priority;
</script>

<template>
  <section class="rbd" :aria-label="t('pm.requirementBaseline.detailTitle')">
    <EmptyState
      v-if="!props.baseline"
      size="sm"
      :title="t('pm.requirementBaseline.detailEmptyTitle')"
      :description="t('pm.requirementBaseline.detailEmptyDesc')"
    />

    <template v-else>
      <article
        class="rbd__card"
        :aria-label="t('pm.requirementBaseline.detailMetaTitle')"
      >
        <h4 class="rbd__card-title">
          {{ t("pm.requirementBaseline.detailMetaTitle") }}
        </h4>
        <dl class="rbd__meta">
          <div>
            <dt>{{ t("pm.requirementBaseline.overviewId") }}</dt>
            <dd class="rbd__mono">{{ props.baseline.id }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirementBaseline.overviewVersion") }}</dt>
            <dd class="rbd__mono">
              {{
                t("pm.requirementBaseline.baselineVersion", {
                  version: props.baseline.version
                })
              }}
            </dd>
          </div>
          <div>
            <dt>{{ t("pm.requirementBaseline.overviewDate") }}</dt>
            <dd>{{ props.baseline.established_at }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirementBaseline.overviewBy") }}</dt>
            <dd>{{ props.baseline.established_by }}</dd>
          </div>
        </dl>
        <p class="rbd__desc">{{ props.baseline.description }}</p>
      </article>

      <article
        class="rbd__card"
        :aria-label="t('pm.requirementBaseline.detailIncludedTitle')"
      >
        <h4 class="rbd__card-title">
          {{ t("pm.requirementBaseline.detailIncludedTitle") }}
        </h4>
        <ul v-if="props.baseline.entries.length > 0" class="rbd__entries">
          <li
            v-for="entry in props.baseline.entries"
            :key="entry.requirement_code"
            class="rbd__entry"
          >
            <span class="rbd__entry-code">{{ entry.requirement_code }}</span>
            <span class="rbd__entry-title">{{ entry.title }}</span>
            <Badge variant="neutral" size="sm">{{
              typeLabel(entry.type)
            }}</Badge>
            <Badge
              :variant="entry.priority === 'high' ? 'danger' : 'neutral'"
              size="sm"
            >
              {{ priorityLabel(entry.priority) }}
            </Badge>
            <span class="rbd__entry-rev">
              {{
                t("pm.requirements.revisionVersion", {
                  version: entry.revision_version
                })
              }}
            </span>
          </li>
        </ul>
        <p v-else class="rbd__empty">
          {{ t("pm.requirementBaseline.detailIncludedEmpty") }}
        </p>
      </article>

      <article
        class="rbd__card"
        :aria-label="t('pm.requirementBaseline.detailSummaryTitle')"
      >
        <h4 class="rbd__card-title">
          {{ t("pm.requirementBaseline.detailSummaryTitle") }}
        </h4>
        <dl class="rbd__summary">
          <div>
            <dt>{{ t("pm.requirementBaseline.summaryTotal") }}</dt>
            <dd>{{ total }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirements.typeFunctional") }}</dt>
            <dd>{{ functionalCount }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirements.typeNonFunctional") }}</dt>
            <dd>{{ nonFunctionalCount }}</dd>
          </div>
        </dl>
        <div
          v-if="total > 0"
          class="rbd__distribution"
          role="img"
          :aria-label="
            t('pm.requirementBaseline.summaryDistribution', {
              functional: functionalCount,
              nonFunctional: nonFunctionalCount
            })
          "
        >
          <span
            class="rbd__distribution-seg rbd__distribution-seg--functional"
            :style="{ width: `${functionalPercent}%` }"
          />
          <span
            class="rbd__distribution-seg rbd__distribution-seg--non-functional"
            :style="{ width: `${100 - functionalPercent}%` }"
          />
        </div>
        <p v-if="total > 0" class="rbd__distribution-legend">
          {{
            t("pm.requirementBaseline.summaryDistribution", {
              functional: functionalCount,
              nonFunctional: nonFunctionalCount
            })
          }}
        </p>
      </article>
    </template>
  </section>
</template>

<style scoped>
.rbd {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.rbd__card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rbd__card-title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rbd__meta {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.rbd__meta dt {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbd__meta dd {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.rbd__mono {
  font-family: var(--font-mono);
}

.rbd__desc {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.rbd__entries {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.rbd__entry {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--size-nav-item);
  padding: 0 var(--space-3);
  border-bottom: var(--hairline) solid var(--border-default);
}

.rbd__entries li:last-child .rbd__entry {
  border-bottom: 0;
}

.rbd__entry-code {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rbd__entry-title {
  flex: 1 1 0;
  min-width: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rbd__entry-rev {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
}

.rbd__empty {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}

.rbd__summary {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.rbd__summary dt {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbd__summary dd {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.rbd__distribution {
  display: flex;
  height: var(--size-progress-track);
  border-radius: var(--radius-full);
  overflow: hidden;
  background: var(--surface-raised);
}

.rbd__distribution-seg--functional {
  background: var(--accent);
}

.rbd__distribution-seg--non-functional {
  background: var(--ws-color-info);
}

.rbd__distribution-legend {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

@media (max-width: 1100px) {
  .rbd__meta {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
