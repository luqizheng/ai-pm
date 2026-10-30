<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { goals, type Goal } from "../fixtures";

const { t } = useI18n();

/** 当前推进中的里程碑；无则取首个 */
function activeMilestone(goal: Goal): string {
  return (
    goal.milestones.find(m => m.status === "active")?.name ??
    goal.milestones[0]?.name ??
    ""
  );
}
</script>

<template>
  <section class="ws-goals" aria-label="goals">
    <h2 class="ws-section-title">{{ t("workspace.console.goals") }}</h2>
    <div class="ws-goal-list">
      <div
        v-for="goal in goals"
        :key="goal.id"
        class="ws-card ws-goal"
        data-test="goal-item"
      >
        <div class="ws-goal-head">
          <span class="ws-goal-name">{{ goal.name }}</span>
          <span class="ws-goal-milestone">{{ activeMilestone(goal) }}</span>
        </div>
        <div
          class="ws-goal-progress"
          role="progressbar"
          :aria-valuenow="goal.progress"
          aria-valuemin="0"
          aria-valuemax="100"
        >
          <div class="ws-goal-bar" :style="{ width: `${goal.progress}%` }" />
        </div>
        <div class="ws-goal-progress-text">{{ goal.progress }}%</div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ws-section-title {
  margin: 0 0 8px;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-goal-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ws-goal {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 12px 16px;
}

.ws-goal-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.ws-goal-name {
  font-weight: 600;
}

.ws-goal-milestone {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-goal-progress {
  height: 4px;
  border-radius: 999px;
  background: var(--ws-elevated);
  overflow: hidden;
}

/* 进度是执行状态 → 语义蓝，不用 AI 紫（紫色克制） */
.ws-goal-bar {
  height: 100%;
  border-radius: 999px;
  background: var(--ws-color-info);
}

.ws-goal-progress-text {
  align-self: flex-end;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}
</style>
