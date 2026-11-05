<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type { BaselineRecord } from "../baseline-contract";

/**
 * 基线概览卡（S02-UI-06 §3）。
 *
 * 当前生效基线的六要素：标识符 / 版本 / 确立日期 / 确立人 /
 * 含入需求数 / 基线状态。
 */
interface Props {
  baseline: BaselineRecord | null;
}

const props = defineProps<Props>();
const { t } = useI18n();

const statusLabel = computed(() =>
  props.baseline?.status === "active"
    ? t("pm.requirementBaseline.statusActive")
    : t("pm.requirementBaseline.statusSuperseded")
);
</script>

<template>
  <section class="rbo" :aria-label="t('pm.requirementBaseline.overviewTitle')">
    <div class="rbo__head">
      <h3 class="rbo__title">
        {{ t("pm.requirementBaseline.overviewTitle") }}
      </h3>
      <Badge
        v-if="props.baseline"
        :variant="props.baseline.status === 'active' ? 'success' : 'neutral'"
        size="sm"
      >
        {{ statusLabel }}
      </Badge>
    </div>

    <EmptyState
      v-if="!props.baseline"
      size="sm"
      :title="t('pm.requirementBaseline.overviewEmptyTitle')"
      :description="t('pm.requirementBaseline.overviewEmptyDesc')"
    />

    <dl v-else class="rbo__facts">
      <div>
        <dt>{{ t("pm.requirementBaseline.overviewId") }}</dt>
        <dd class="rbo__mono">{{ props.baseline.id }}</dd>
      </div>
      <div>
        <dt>{{ t("pm.requirementBaseline.overviewVersion") }}</dt>
        <dd class="rbo__mono">
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
      <div>
        <dt>{{ t("pm.requirementBaseline.overviewCount") }}</dt>
        <dd class="rbo__count">{{ props.baseline.entries.length }}</dd>
      </div>
    </dl>
  </section>
</template>

<style scoped>
.rbo {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rbo__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.rbo__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rbo__facts {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.rbo__facts dt {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbo__facts dd {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.rbo__mono {
  font-family: var(--font-mono);
}

.rbo__count {
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-variant-numeric: tabular-nums;
}

@media (max-width: 1100px) {
  .rbo__facts {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
