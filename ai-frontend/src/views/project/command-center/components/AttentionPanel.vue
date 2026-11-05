<script setup lang="ts">
import { ChevronRight, TriangleAlert } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import Card from "@/components/ui/card/Card.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type {
  AttentionColumnLabels,
  AttentionCount,
  AttentionItem
} from "../attention";

/**
 * 需关注事项面板（Pixso「Project Command Center」3:7341 attention-panel）。
 *
 * 结构：头（警示图标 + 标题 + 待处理 chip ｜ 紧急/告知 计数 + Inbox 入口）
 * → 列头带（事项/类别/业务对象/状态/责任 + 直达）→ 事项行。
 * 设计稿四个状态都只有面板壳、未给行样例 —— 行按列头规格落地，items 为空时
 * 面板即设计稿的默认态（壳 + 列头）。文案由调用方翻译后传入。
 */
interface Props {
  /** 面板标题（如「需关注事项」） */
  title: string;
  /** 待处理 chip 文案（如「3 项待处理 · 1 项紧急」） */
  pendingLabel?: string;
  /** 右侧计数对（紧急/告知） */
  counts?: AttentionCount[];
  /** 「前往 Inbox 处理全部」入口文案；不传不渲染 */
  actionLabel?: string;
  /** 列头文案 */
  columnLabels: AttentionColumnLabels;
  /** 事项行（默认空） */
  items?: AttentionItem[];
}

withDefaults(defineProps<Props>(), {
  pendingLabel: "",
  counts: () => [],
  actionLabel: "",
  items: () => []
});

const emit = defineEmits<{
  (e: "inbox"): void;
  (e: "open", item: AttentionItem): void;
}>();

const countColor = (tone: "danger" | "warning") =>
  tone === "danger" ? "var(--danger-strong)" : "var(--warning-strong)";
</script>

<template>
  <Card class="pm-attention">
    <template #header>
      <div class="pm-attention__head">
        <div class="pm-attention__title-group">
          <TriangleAlert class="pm-attention__icon" aria-hidden="true" />
          <h3 class="pm-attention__title">{{ title }}</h3>
          <Badge v-if="pendingLabel" variant="warning" size="sm">
            {{ pendingLabel }}
          </Badge>
        </div>
        <div class="pm-attention__aside">
          <span
            v-for="count in counts"
            :key="count.label"
            class="pm-attention__count"
          >
            <span class="pm-attention__count-label">{{ count.label }}</span>
            <span
              class="pm-attention__count-num"
              :style="{ color: countColor(count.tone) }"
            >
              {{ count.count }}
            </span>
          </span>
          <button
            v-if="actionLabel"
            type="button"
            class="pm-attention__inbox"
            @click="emit('inbox')"
          >
            {{ actionLabel }}
          </button>
        </div>
      </div>
    </template>

    <div class="pm-attention__row pm-attention__row--head" aria-hidden="true">
      <span class="pm-attention__col-head">{{ columnLabels.item }}</span>
      <span class="pm-attention__col-head">{{ columnLabels.category }}</span>
      <span class="pm-attention__col-head">{{ columnLabels.object }}</span>
      <span class="pm-attention__col-head">{{ columnLabels.status }}</span>
      <span class="pm-attention__col-head">{{ columnLabels.owner }}</span>
      <span class="pm-attention__col-head pm-attention__col-head--end">
        {{ columnLabels.goto }}
      </span>
    </div>

    <ul class="pm-attention__items">
      <li v-for="item in items" :key="item.id" class="pm-attention__row">
        <span class="pm-attention__item" :title="item.title">{{
          item.title
        }}</span>
        <span class="pm-attention__category">{{ item.category }}</span>
        <span class="pm-attention__object">{{ item.object }}</span>
        <StatusIndicator
          class="pm-attention__status"
          :status="item.statusTone"
          size="sm"
          :label="item.statusLabel"
        />
        <span class="pm-attention__owner">{{ item.owner }}</span>
        <button
          type="button"
          class="pm-attention__open"
          :aria-label="`${columnLabels.goto}：${item.title}`"
          @click="emit('open', item)"
        >
          <ChevronRight aria-hidden="true" />
        </button>
      </li>
    </ul>
  </Card>
</template>

<style scoped>
.pm-attention :deep(.ui-card__header) {
  padding: var(--space-3) var(--space-3);
  border-bottom: 0;
}

.pm-attention :deep(.ui-card__body) {
  display: flex;
  flex-direction: column;
  padding: 0 var(--space-3) var(--space-2);
}

.pm-attention__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  min-width: 0;
}

.pm-attention__title-group {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.pm-attention__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--warning-strong);
}

.pm-attention__title {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
  white-space: nowrap;
}

.pm-attention__aside {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex: none;
}

.pm-attention__count {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-1);
}

.pm-attention__count-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.pm-attention__count-num {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  font-variant-numeric: tabular-nums;
}

.pm-attention__inbox {
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--accent);
  cursor: pointer;
  transition: color var(--duration-fast) var(--ease-standard);
}

.pm-attention__inbox:hover {
  color: var(--accent-hover);
}

.pm-attention__inbox:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 列头带与行共用六列网格：事项 ｜ 类别 ｜ 业务对象 ｜ 状态 ｜ 责任 ｜ 直达 */
.pm-attention__row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto auto auto 32px;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) 0;
}

.pm-attention__row--head {
  border-top: var(--border-1) solid var(--border-subtle);
  border-bottom: var(--border-1) solid var(--border-subtle);
  padding-block: var(--space-1);
}

.pm-attention__col-head {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-disabled);
  white-space: nowrap;
}

.pm-attention__col-head--end {
  text-align: right;
}

.pm-attention__items {
  margin: 0;
  padding: 0;
  list-style: none;
}

.pm-attention__items .pm-attention__row + .pm-attention__row {
  border-top: var(--border-1) solid var(--border-subtle);
}

.pm-attention__item {
  overflow: hidden;
  min-width: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-attention__category,
.pm-attention__object,
.pm-attention__owner {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  white-space: nowrap;
}

.pm-attention__status {
  flex: none;
}

.pm-attention__open {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  width: 100%;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--accent);
  cursor: pointer;
}

.pm-attention__open svg {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
}

.pm-attention__open:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}
</style>
