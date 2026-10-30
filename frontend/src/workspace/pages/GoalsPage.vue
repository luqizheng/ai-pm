<script setup lang="ts">
import { useI18n } from "vue-i18n";
import PageHeader from "./PageHeader.vue";
import { goals, type MilestoneStatus, type TaskStatus } from "../fixtures";

const { t } = useI18n();

const msLabel: Record<MilestoneStatus, string> = {
  done: "workspace.pages.goals.msDone",
  active: "workspace.pages.goals.msActive",
  pending: "workspace.pages.goals.msPending"
};

const msClass: Record<MilestoneStatus, string> = {
  done: "is-done",
  active: "is-active",
  pending: "is-pending"
};

const taskLabel: Record<TaskStatus, string> = {
  done: "workspace.pages.goals.taskDone",
  doing: "workspace.pages.goals.taskDoing",
  todo: "workspace.pages.goals.taskTodo"
};
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.goals.title')"
      :subtitle="t('workspace.pages.goals.subtitle')"
    />
    <section
      v-for="goal in goals"
      :key="goal.id"
      class="ws-card ws-goal-card"
      data-test="goal-card"
    >
      <div class="ws-goal-head">
        <div>
          <h2 class="ws-goal-name">{{ goal.name }}</h2>
          <p class="ws-goal-desc">{{ goal.description }}</p>
        </div>
        <span class="ws-goal-progress-num">{{ goal.progress }}%</span>
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

      <h3 class="ws-goal-sub">
        {{ t("workspace.pages.goals.milestones") }}
      </h3>
      <ul class="ws-milestones">
        <li
          v-for="ms in goal.milestones"
          :key="ms.id"
          class="ws-milestone"
          data-test="milestone-item"
        >
          <span class="ws-badge" :class="msClass[ms.status]">
            {{ t(msLabel[ms.status]) }}
          </span>
          <span class="ws-milestone-name">{{ ms.name }}</span>
          <span class="ws-milestone-due">{{ ms.due }}</span>
        </li>
      </ul>

      <h3 class="ws-goal-sub">{{ t("workspace.pages.goals.tasks") }}</h3>
      <ul class="ws-tasks">
        <li
          v-for="task in goal.tasks"
          :key="task.id"
          class="ws-task"
          data-test="task-item"
        >
          <span class="ws-badge" :class="`task-${task.status}`">
            {{ t(taskLabel[task.status]) }}
          </span>
          <span class="ws-task-name">{{ task.name }}</span>
          <span class="ws-task-owner">{{ task.owner }}</span>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 960px;
}

.ws-goal-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
}

.ws-goal-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.ws-goal-name {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-goal-desc {
  margin: 4px 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-goal-progress-num {
  font-weight: 700;
  font-variant-numeric: tabular-nums;
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

.ws-goal-sub {
  margin: 8px 0 0;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-milestones,
.ws-tasks {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.ws-milestone,
.ws-task {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: var(--ws-row-height);
  border-bottom: 1px solid var(--ws-border);
}

.ws-milestone:last-child,
.ws-task:last-child {
  border-bottom: none;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

.ws-badge.is-done {
  color: var(--ws-color-success-text);
}

.ws-badge.is-active {
  color: var(--ws-color-info-text);
}

.ws-badge.is-pending,
.ws-badge.task-todo {
  color: var(--ws-text-secondary);
}

.ws-badge.task-done {
  color: var(--ws-color-success-text);
}

.ws-badge.task-doing {
  color: var(--ws-color-info-text);
}

.ws-milestone-name,
.ws-task-name {
  flex: 1;
  min-width: 0;
}

.ws-milestone-due,
.ws-task-owner {
  flex: none;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
