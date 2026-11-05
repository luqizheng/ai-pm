<script setup lang="ts">
import type { Component } from "vue";
import {
  CopyCheck,
  FileCheck2,
  GitMerge,
  GitPullRequest,
  OctagonAlert
} from "lucide-vue-next";
import Card from "@/components/ui/card/Card.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import SectionHead from "./SectionHead.vue";
import type { ActivityItem, ActivityLinkIcon } from "../activity";

/**
 * 最近项目动态卡（Pixso「Project Command Center」3:7722）。
 *
 * 结构：SectionHead（标题 + 「仅高价值事件」chip + 「全部动态」入口）→ 时间线
 * （轨道点 + 连线 ｜ 首行=时间+类别+描述，次行=关联对象链接）。
 * 轨道点为装饰性分类标记（aria-hidden，语义色 token 着色）；类别文本的
 * 可读语义由 StatusIndicator chip 承担。文案由调用方翻译后传入。
 */
const LINK_ICONS: Record<ActivityLinkIcon, Component> = {
  "git-pull-request": GitPullRequest,
  "file-check": FileCheck2,
  "copy-check": CopyCheck,
  "octagon-alert": OctagonAlert,
  "git-merge": GitMerge
};

interface Props {
  title: string;
  meta?: string;
  actionLabel?: string;
  items: ActivityItem[];
}

withDefaults(defineProps<Props>(), { meta: "", actionLabel: "" });

const emit = defineEmits<{
  (e: "action"): void;
  (e: "open-link", item: ActivityItem): void;
}>();
</script>

<template>
  <Card class="pm-act">
    <template #header>
      <SectionHead
        :title="title"
        :meta="meta"
        :action-label="actionLabel"
        @action="emit('action')"
      />
    </template>

    <ul class="pm-act__timeline">
      <li
        v-for="(item, index) in items"
        :key="item.id"
        class="pm-act__entry"
      >
        <span class="pm-act__rail" aria-hidden="true">
          <span class="pm-act__dot" :class="`pm-act__dot--${item.tone}`" />
          <span
            v-if="index < items.length - 1"
            class="pm-act__line"
          />
        </span>
        <div class="pm-act__body">
          <div class="pm-act__main">
            <span class="pm-act__time">{{ item.time }}</span>
            <StatusIndicator
              class="pm-act__category"
              :status="item.tone"
              size="sm"
              :label="item.categoryLabel"
            />
            <span class="pm-act__text" :title="item.text">{{ item.text }}</span>
          </div>
          <button
            v-if="item.linkLabel"
            type="button"
            class="pm-act__link"
            @click="emit('open-link', item)"
          >
            <component
              :is="LINK_ICONS[item.linkIcon ?? 'git-pull-request']"
              class="pm-act__link-icon"
              aria-hidden="true"
            />
            <span>{{ item.linkLabel }}</span>
          </button>
        </div>
      </li>
    </ul>
  </Card>
</template>

<style scoped>
.pm-act :deep(.ui-card__header) {
  padding: var(--space-3) var(--space-3) 0;
  border-bottom: 0;
}

.pm-act :deep(.ui-card__body) {
  padding: 0 var(--space-3) var(--space-3);
}

.pm-act__timeline {
  margin: 0;
  padding: 0;
  list-style: none;
}

.pm-act__entry {
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr);
  gap: var(--space-2);
}

/* 轨道：分类色点 + 到下一行的连线（末行不画线） */
.pm-act__rail {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-1);
  padding-top: var(--space-1);
}

.pm-act__dot {
  flex: none;
  width: 7px;
  height: 7px;
  border-radius: var(--radius-full);
}

.pm-act__dot--success {
  background-color: var(--ws-color-success);
}

.pm-act__dot--info {
  background-color: var(--ws-color-info);
}

.pm-act__dot--warning {
  background-color: var(--ws-color-warning);
}

.pm-act__dot--danger {
  background-color: var(--ws-color-danger);
}

.pm-act__dot--accent {
  background-color: var(--accent);
}

.pm-act__dot--neutral {
  background-color: var(--text-secondary);
}

.pm-act__line {
  flex: 1;
  width: var(--border-1);
  background: var(--border-subtle);
}

.pm-act__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  padding-block: var(--space-1) var(--space-3);
}

.pm-act__main {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.pm-act__time {
  flex: none;
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
}

.pm-act__category {
  flex: none;
}

.pm-act__text {
  overflow: hidden;
  min-width: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-act__link {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  align-self: flex-start;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--accent);
  cursor: pointer;
  transition: color var(--duration-fast) var(--ease-standard);
}

.pm-act__link:hover {
  color: var(--accent-hover);
}

.pm-act__link:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.pm-act__link-icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  flex: none;
}
</style>
