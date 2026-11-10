<script setup lang="ts">
import { computed } from "vue";

import Badge from "@/components/ui/badge/Badge.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type { ProjectGoalView } from "@/api/goal";

/**
 * 项目目标卡（S01 · I4）。
 *
 * 数据 = `getProjectGoal()`（元数据 + 知识文档正文两次调用）。
 *
 * 三种状态必须可辨（这是本卡的全部意义）：
 * - `missing`：项目没有正式 Goal——后端显式信号，**不得臆测目标内容**；
 * - `ready`：元数据 + 正文都在；
 * - `body_unavailable`：元数据在但正文读不出来（知识仓未配置 / 文件缺失 /
 *   hash 不符）——**这不是「没有目标」**，是有目标但此刻读不到。
 *
 * 只读：修改目标必须走知识文档提案 + 人类确认（Agent 只能提案），本卡不
 * 提供编辑入口。
 */
const props = defineProps<{
  /** 卡标题 */
  title: string;
  /** 元数据行文案 */
  labels: {
    revision: string;
    publishedAt: string;
    commit: string;
  };
  /** 三态文案 */
  states: {
    missingTitle: string;
    missingHint: string;
    bodyUnavailableTitle: string;
    bodyUnavailableHint: string;
  };
  /** 状态徽标文案 */
  badges: {
    ready: string;
    missing: string;
    bodyUnavailable: string;
  };
  /** 数据（null = 加载中/未加载，整卡不渲染正文区） */
  data: ProjectGoalView | null;
}>();

const state = computed(() => props.data?.state ?? null);
const metadata = computed(() => props.data?.metadata ?? null);
const markdown = computed(() => props.data?.markdown ?? "");

const badgeVariant = computed<"success" | "warning" | "neutral">(() => {
  switch (state.value) {
    case "ready":
      return "success";
    case "body_unavailable":
      return "warning";
    default:
      return "neutral";
  }
});

const badgeLabel = computed(() => {
  switch (state.value) {
    case "ready":
      return props.badges.ready;
    case "body_unavailable":
      return props.badges.bodyUnavailable;
    default:
      return props.badges.missing;
  }
});
</script>

<template>
  <Card class="goal-card" data-testid="cc-goal">
    <template #header>
      <div class="goal-card__head">
        <span>{{ title }}</span>
        <Badge
          v-if="state"
          :variant="badgeVariant"
          size="sm"
          data-testid="cc-goal-badge"
        >
          {{ badgeLabel }}
        </Badge>
      </div>
    </template>

    <!-- 未建立：显式缺失信号 -->
    <EmptyState
      v-if="state === 'missing'"
      :title="states.missingTitle"
      :description="states.missingHint"
      size="sm"
      data-testid="cc-goal-missing"
    />

    <template v-else>
      <!-- 元数据照常显示：只要文档存在就展示（body_unavailable 也属于
           「文档存在」，与 missing 严格区分） -->
      <dl v-if="metadata" class="goal-meta">
        <div class="goal-meta__row">
          <dt>{{ labels.revision }}</dt>
          <dd>{{ metadata.revision_no }}</dd>
        </div>
        <div class="goal-meta__row">
          <dt>{{ labels.publishedAt }}</dt>
          <dd>{{ metadata.published_at }}</dd>
        </div>
        <div class="goal-meta__row">
          <dt>{{ labels.commit }}</dt>
          <dd class="goal-meta__mono">
            {{ metadata.git_commit_sha.slice(0, 10) }}
          </dd>
        </div>
      </dl>

      <!-- 正文读不出来：这不是「没有目标」 -->
      <EmptyState
        v-if="state === 'body_unavailable'"
        :title="states.bodyUnavailableTitle"
        :description="states.bodyUnavailableHint"
        size="sm"
        data-testid="cc-goal-body-unavailable"
      />

      <pre
        v-else-if="markdown"
        class="goal-markdown"
        data-testid="cc-goal-markdown"
        >{{ markdown }}</pre
      >
    </template>
  </Card>
</template>

<style scoped>
.goal-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.goal-card__head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.goal-meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-5);
  margin: 0;
}

.goal-meta__row {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.goal-meta__row dt {
  color: var(--ws-text-secondary);
}

.goal-meta__row dd {
  margin: 0;
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-primary);
}

.goal-meta__mono {
  font-family: var(--font-mono);
}

.goal-markdown {
  margin: 0;
  max-height: var(--size-ai-evidence-max-height);
  overflow: auto;
  padding: var(--space-4);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-body);
  color: var(--ws-text-primary);
  white-space: pre-wrap;
  background: var(--surface-base);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}
</style>
