<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ArrowLeftRight } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type { BaselineRecord } from "../baseline-contract";

/**
 * 基线历史列表（S02-UI-06 §4）。
 *
 * 每条记录：基线 ID / 版本 / 描述 / 含入需求数 / 确立日期 / 负责人 / 状态；
 * 卡头带「比较基线」入口（§7，比较编排由页根承担）。
 */
interface Props {
  baselines: BaselineRecord[];
  selectedId: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "select", id: string): void;
  (e: "compare"): void;
}>();

const { t } = useI18n();

const statusLabel = (status: string) =>
  status === "active"
    ? t("pm.requirementBaseline.statusActive")
    : t("pm.requirementBaseline.statusSuperseded");
</script>

<template>
  <section class="rbl" :aria-label="t('pm.requirementBaseline.listAria')">
    <div class="rbl__head">
      <h3 class="rbl__title">{{ t("pm.requirementBaseline.listTitle") }}</h3>
      <button
        type="button"
        class="rbl__compare-link"
        :disabled="props.baselines.length < 2"
        @click="emit('compare')"
      >
        <ArrowLeftRight aria-hidden="true" />
        <span>{{ t("pm.requirementBaseline.actionCompare") }}</span>
      </button>
    </div>

    <EmptyState
      v-if="props.baselines.length === 0"
      size="sm"
      :title="t('pm.requirementBaseline.listEmpty')"
    />

    <ul v-else class="rbl__rows">
      <li v-for="baseline in props.baselines" :key="baseline.id">
        <button
          type="button"
          class="rbl__row"
          :class="{ 'rbl__row--selected': baseline.id === props.selectedId }"
          :aria-current="baseline.id === props.selectedId ? 'true' : undefined"
          @click="emit('select', baseline.id)"
        >
          <span class="rbl__row-top">
            <span class="rbl__id">{{ baseline.id }}</span>
            <span class="rbl__version">
              {{
                t("pm.requirementBaseline.baselineVersion", {
                  version: baseline.version
                })
              }}
            </span>
            <Badge
              :variant="baseline.status === 'active' ? 'success' : 'neutral'"
              size="sm"
            >
              {{ statusLabel(baseline.status) }}
            </Badge>
          </span>
          <span class="rbl__desc">{{ baseline.description }}</span>
          <span class="rbl__meta">
            <span>{{
              t("pm.requirementBaseline.listCount", {
                count: baseline.entries.length
              })
            }}</span>
            <span class="rbl__meta-owner">{{ baseline.established_by }}</span>
            <span class="rbl__meta-time">{{ baseline.established_at }}</span>
          </span>
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.rbl {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rbl__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.rbl__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rbl__compare-link {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
  cursor: pointer;
}

.rbl__compare-link:hover:not(:disabled) {
  color: var(--accent-hover);
}

.rbl__compare-link:disabled {
  color: var(--text-disabled);
  cursor: not-allowed;
}

.rbl__compare-link:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.rbl__compare-link svg {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
}

.rbl__rows {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
  overflow-y: auto;
}

.rbl__row {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: transparent;
  padding: var(--space-3);
  font-family: var(--font-ui);
  text-align: left;
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

.rbl__row:hover {
  background: var(--ws-hover-veil);
}

.rbl__row--selected {
  background: var(--ws-color-accent-surface);
  border-color: var(--accent);
}

.rbl__row:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.rbl__row-top {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rbl__id {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rbl__version {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbl__desc {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rbl__meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  flex-wrap: wrap;
}

.rbl__meta-owner {
  color: var(--text-primary);
}

.rbl__meta-time {
  margin-left: auto;
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
}
</style>
