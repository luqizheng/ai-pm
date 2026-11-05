<script setup lang="ts">
import Card from "@/components/ui/card/Card.vue";
import ProgressIndicator from "@/components/ui/progress-indicator/ProgressIndicator.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import SectionHead from "./SectionHead.vue";
import type {
  DeliverableColumnLabels,
  DeliverableRow
} from "../deliverables";

/**
 * 需求与交付物卡（Pixso「Project Command Center」3:7631）。
 *
 * 结构：SectionHead（标题 + 分列口径 chip + 「查看全部」入口）→ 列头带
 * （需求/验收/AI/交付物）→ 需求行（编号+标题 ｜ 人工验收 ｜ AI 执行 ｜ 覆盖率）
 * → 独立性脚注。人工验收与 AI 执行两列状态独立记录、互不推导（设计稿口径）。
 * 文案由调用方翻译后传入。
 */
interface Props {
  title: string;
  meta?: string;
  actionLabel?: string;
  columnLabels: DeliverableColumnLabels;
  rows: DeliverableRow[];
  /** 底部独立性脚注；不传不渲染 */
  footnote?: string;
}

withDefaults(defineProps<Props>(), { meta: "", actionLabel: "", footnote: "" });

const emit = defineEmits<{ (e: "action"): void }>();
</script>

<template>
  <Card class="pm-deliv">
    <template #header>
      <SectionHead
        :title="title"
        :meta="meta"
        :action-label="actionLabel"
        @action="emit('action')"
      />
    </template>

    <div class="pm-deliv__row pm-deliv__row--head" aria-hidden="true">
      <span class="pm-deliv__col-head">{{ columnLabels.requirement }}</span>
      <span class="pm-deliv__col-head">{{ columnLabels.acceptance }}</span>
      <span class="pm-deliv__col-head">{{ columnLabels.ai }}</span>
      <span class="pm-deliv__col-head">{{ columnLabels.deliverable }}</span>
    </div>

    <ul class="pm-deliv__rows">
      <li v-for="row in rows" :key="row.id" class="pm-deliv__row">
        <span class="pm-deliv__req">
          <span class="pm-deliv__code">{{ row.code }}</span>
          <span class="pm-deliv__title" :title="row.title">{{ row.title }}</span>
        </span>
        <span class="pm-deliv__state">
          <template v-if="row.acceptance">
            <StatusIndicator
              :status="row.acceptance.tone"
              size="sm"
              :label="row.acceptance.label"
            />
          </template>
          <span v-else class="pm-deliv__state-none">—</span>
        </span>
        <span class="pm-deliv__state">
          <template v-if="row.ai">
            <StatusIndicator
              :status="row.ai.tone"
              size="sm"
              :label="row.ai.label"
            />
          </template>
          <span v-else class="pm-deliv__state-none">—</span>
        </span>
        <span class="pm-deliv__coverage">
          <span v-if="row.coverageLabel" class="pm-deliv__coverage-label">
            {{ row.coverageLabel }}
          </span>
          <ProgressIndicator
            v-if="row.coverage !== null && row.coverage !== undefined"
            class="pm-deliv__coverage-bar"
            :value="row.coverage"
            :variant="row.coverage >= 100 ? 'success' : 'accent'"
            size="sm"
          />
        </span>
      </li>
    </ul>

    <p v-if="footnote" class="pm-deliv__footnote">{{ footnote }}</p>
  </Card>
</template>

<style scoped>
.pm-deliv :deep(.ui-card__header) {
  padding: var(--space-3) var(--space-3) 0;
  border-bottom: 0;
}

.pm-deliv :deep(.ui-card__body) {
  display: flex;
  flex-direction: column;
  padding: 0 var(--space-3) var(--space-2);
}

/* 列头带与行共用四列网格：需求 ｜ 人工验收 ｜ AI 执行 ｜ 交付物覆盖率 */
.pm-deliv__row {
  display: grid;
  grid-template-columns: minmax(0, 1.4fr) auto auto 72px;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) 0;
}

.pm-deliv__row--head {
  border-top: var(--border-1) solid var(--border-subtle);
  border-bottom: var(--border-1) solid var(--border-subtle);
  padding-block: var(--space-1);
}

.pm-deliv__col-head {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-disabled);
  white-space: nowrap;
}

.pm-deliv__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}

.pm-deliv__rows .pm-deliv__row + .pm-deliv__row {
  border-top: var(--border-1) solid var(--border-subtle);
}

.pm-deliv__req {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.pm-deliv__code {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
}

.pm-deliv__title {
  overflow: hidden;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-deliv__state {
  display: flex;
  min-width: 0;
}

.pm-deliv__state-none {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.pm-deliv__coverage {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.pm-deliv__coverage-label {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.pm-deliv__coverage-bar {
  display: block;
}

/* 独立性脚注（设计稿底部 #FAFAFB 带的 token 化等价：上描边 + 弱化文字） */
.pm-deliv__footnote {
  margin: 0;
  padding-top: var(--space-2);
  border-top: var(--border-1) solid var(--border-subtle);
  font-size: var(--font-size-nano);
  line-height: 1.4;
  color: var(--text-disabled);
}
</style>
