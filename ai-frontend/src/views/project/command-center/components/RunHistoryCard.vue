<script setup lang="ts">
import Card from "@/components/ui/card/Card.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import SectionHead from "./SectionHead.vue";
import type { RunColumnLabels, RunEntry } from "../run-history";

/**
 * 最近运行记录卡（Pixso「Project Command Center」3:7581）。
 *
 * 结构：SectionHead（标题 + 时间窗 chip + 「运行历史」入口）→ 列头带
 * （运行/Agent/触发/结果/耗时）→ 运行行。Agent 名走 accent（AI 主体可辨，
 * AGENTS.md：AI 参与必须可辨）。文案由调用方翻译后传入。
 */
interface Props {
  title: string;
  meta?: string;
  actionLabel?: string;
  columnLabels: RunColumnLabels;
  entries: RunEntry[];
}

withDefaults(defineProps<Props>(), { meta: "", actionLabel: "" });

const emit = defineEmits<{ (e: "action"): void }>();
</script>

<template>
  <Card class="pm-runs">
    <template #header>
      <SectionHead
        :title="title"
        :meta="meta"
        :action-label="actionLabel"
        @action="emit('action')"
      />
    </template>

    <div class="pm-runs__row pm-runs__row--head" aria-hidden="true">
      <span class="pm-runs__col-head">{{ columnLabels.run }}</span>
      <span class="pm-runs__col-head">{{ columnLabels.agent }}</span>
      <span class="pm-runs__col-head">{{ columnLabels.trigger }}</span>
      <span class="pm-runs__col-head">{{ columnLabels.result }}</span>
      <span class="pm-runs__col-head pm-runs__col-head--end">
        {{ columnLabels.duration }}
      </span>
    </div>

    <ul class="pm-runs__rows">
      <li v-for="entry in entries" :key="entry.id" class="pm-runs__row">
        <span class="pm-runs__run-id">{{ entry.runId }}</span>
        <span class="pm-runs__agent">{{ entry.agent }}</span>
        <span class="pm-runs__trigger" :title="entry.trigger">{{ entry.trigger }}</span>
        <StatusIndicator
          class="pm-runs__status"
          :status="entry.statusTone"
          size="sm"
          :label="entry.statusLabel"
        />
        <span class="pm-runs__duration">{{ entry.duration }}</span>
      </li>
    </ul>
  </Card>
</template>

<style scoped>
.pm-runs :deep(.ui-card__header) {
  padding: var(--space-3) var(--space-3) 0;
  border-bottom: 0;
}

.pm-runs :deep(.ui-card__body) {
  display: flex;
  flex-direction: column;
  padding: 0 var(--space-3) var(--space-2);
}

/* 列头带与行共用五列网格：运行 ｜ Agent ｜ 触发 ｜ 结果 ｜ 耗时 */
.pm-runs__row {
  display: grid;
  grid-template-columns: 76px 84px minmax(0, 1fr) auto 48px;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) 0;
}

.pm-runs__row--head {
  border-top: var(--border-1) solid var(--border-subtle);
  border-bottom: var(--border-1) solid var(--border-subtle);
  padding-block: var(--space-1);
}

.pm-runs__col-head {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-disabled);
  white-space: nowrap;
}

.pm-runs__col-head--end {
  text-align: right;
}

.pm-runs__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}

.pm-runs__rows .pm-runs__row + .pm-runs__row {
  border-top: var(--border-1) solid var(--border-subtle);
}

.pm-runs__run-id,
.pm-runs__duration {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.pm-runs__duration {
  text-align: right;
}

.pm-runs__agent {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--accent);
  white-space: nowrap;
}

.pm-runs__trigger {
  overflow: hidden;
  min-width: 0;
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-runs__status {
  flex: none;
}
</style>
