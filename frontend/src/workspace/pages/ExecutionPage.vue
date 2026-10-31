<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Flag, CircleCheck } from "lucide-vue-next";
import PageHeader from "./PageHeader.vue";
import { agentRun, type RunEventType } from "../fixtures";

const { t } = useI18n();

const typeLabel: Record<RunEventType, string> = {
  thought: "workspace.pages.execution.thought",
  tool: "workspace.pages.execution.tool",
  state: "workspace.pages.execution.state",
  approval: "workspace.pages.execution.approval"
};

const typeClass: Record<RunEventType, string> = {
  thought: "is-thought",
  tool: "is-tool",
  state: "is-state",
  approval: "is-approval"
};

const selectedId = ref(agentRun.steps[0]?.id ?? "");
const selected = computed(() =>
  agentRun.steps.find(step => step.id === selectedId.value)
);

const stepStatusLabel: Record<string, string> = {
  done: "workspace.pages.execution.stepDone",
  running: "workspace.pages.execution.stepRunning",
  waiting: "workspace.pages.execution.stepWaiting"
};
</script>

<template>
  <div class="ws-page ws-execution">
    <PageHeader
      :title="t('workspace.pages.execution.title')"
      :subtitle="t('workspace.pages.execution.subtitle')"
    />

    <!-- 阶段状态 + 检查点 + 剩余工作（V1.2：不用估算百分比） -->
    <section class="ws-card ws-run-status">
      <div class="ws-run-head">
        <span class="ws-run-title">{{ agentRun.title }}</span>
        <span class="ws-run-agent"
          >{{ agentRun.agent }} · {{ agentRun.goalCode }}</span
        >
      </div>
      <div class="ws-run-phase">
        <Flag :size="14" aria-hidden="true" />
        <span
          >{{ t("workspace.pages.execution.phase") }}：{{
            agentRun.phase
          }}</span
        >
      </div>
      <div class="ws-run-cols">
        <div class="ws-run-col">
          <h3 class="ws-run-col-title">
            <CircleCheck :size="13" aria-hidden="true" />
            {{ t("workspace.pages.execution.checkpoints") }}
          </h3>
          <ul class="ws-run-list">
            <li v-for="(item, i) in agentRun.checkpoints" :key="i">
              {{ item }}
            </li>
          </ul>
        </div>
        <div class="ws-run-col">
          <h3 class="ws-run-col-title is-remaining">
            {{ t("workspace.pages.execution.remaining") }}
          </h3>
          <ul class="ws-run-list">
            <li v-for="(item, i) in agentRun.remaining" :key="i">{{ item }}</li>
          </ul>
        </div>
      </div>
    </section>

    <!-- 中央双栏：左执行步骤 / 右步骤详情 -->
    <div class="ws-exec-body">
      <aside class="ws-card ws-steps" aria-label="execution steps">
        <button
          v-for="step in agentRun.steps"
          :key="step.id"
          type="button"
          class="ws-step ws-focusable"
          :class="{ 'is-selected': selectedId === step.id }"
          data-test="run-step"
          @click="selectedId = step.id"
        >
          <div class="ws-step-head">
            <span class="ws-step-seq">{{ step.seq }}</span>
            <span class="ws-badge" :class="typeClass[step.type]">
              {{ t(typeLabel[step.type]) }}
            </span>
            <span
              v-if="step.checkpoint"
              class="ws-step-checkpoint"
              :title="t('workspace.pages.execution.checkpoints')"
              >◆</span
            >
          </div>
          <div class="ws-step-title">{{ step.title }}</div>
          <div class="ws-step-status">
            {{ t(stepStatusLabel[step.status]) }}
          </div>
        </button>
      </aside>

      <article
        v-if="selected"
        class="ws-card ws-step-detail"
        data-test="step-detail"
      >
        <div class="ws-detail-head">
          <span class="ws-badge" :class="typeClass[selected.type]">
            {{ t(typeLabel[selected.type]) }}
          </span>
          <h2 class="ws-detail-title">{{ selected.title }}</h2>
        </div>
        <p class="ws-detail-text">{{ selected.detail }}</p>

        <div v-if="selected.tool" class="ws-detail-block">
          <h3 class="ws-detail-label">
            {{ t("workspace.pages.execution.toolCall") }}
          </h3>
          <div class="ws-tool" data-test="step-tool">
            <div class="ws-tool-row">
              <span class="ws-tool-key">name</span>
              <span class="ws-tool-val">{{ selected.tool.name }}</span>
              <span class="ws-tool-dur">{{ selected.tool.durationMs }}ms</span>
            </div>
            <div class="ws-tool-row">
              <span class="ws-tool-key">input</span>
              <span class="ws-tool-val">{{ selected.tool.input }}</span>
            </div>
            <div class="ws-tool-row">
              <span class="ws-tool-key">output</span>
              <span class="ws-tool-val">{{ selected.tool.output }}</span>
            </div>
          </div>
        </div>

        <div v-if="selected.logLines?.length" class="ws-detail-block">
          <h3 class="ws-detail-label">
            {{ t("workspace.pages.execution.logs") }}
          </h3>
          <pre class="ws-logs" data-test="step-logs">{{
            selected.logLines.join("\n")
          }}</pre>
        </div>

        <div v-if="selected.codeChange" class="ws-detail-block">
          <h3 class="ws-detail-label">
            {{ t("workspace.pages.execution.codeChange") }}
          </h3>
          <div class="ws-codechange" data-test="step-codechange">
            <span class="ws-codechange-file">{{
              selected.codeChange.file
            }}</span>
            <span class="ws-codechange-nums">
              +{{ selected.codeChange.additions }} / -{{
                selected.codeChange.deletions
              }}
            </span>
          </div>
        </div>
      </article>
    </div>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 1080px;
}

.ws-run-status {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px;
}

.ws-run-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.ws-run-title {
  font-weight: 600;
  font-size: var(--ws-font-size-section);
}

.ws-run-agent {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-run-phase {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--ws-font-size-body);
  color: var(--ws-color-info-text);
}

.ws-run-cols {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.ws-run-col-title {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 4px;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-color-success-text);
}

.ws-run-col-title.is-remaining {
  color: var(--ws-color-warning-text);
}

.ws-run-list {
  margin: 0;
  padding-left: 16px;
  font-size: var(--ws-font-size-caption);
  line-height: 1.7;
}

/* 双栏：左步骤右详情 */
.ws-exec-body {
  display: grid;
  grid-template-columns: 340px minmax(0, 1fr);
  gap: 12px;
  align-items: start;
}

.ws-steps {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px;
}

.ws-step {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-primary);
  cursor: pointer;
  text-align: left;
  font: inherit;
}

.ws-step:hover {
  background: var(--ws-elevated);
}

/* 紫色克制：选中态用 AI 紫 */
.ws-step.is-selected {
  background: var(--ws-elevated);
  box-shadow: var(--ws-focus-ring);
}

.ws-step-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-step-seq {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 1px solid var(--ws-border);
  font-size: var(--ws-font-size-caption);
  font-variant-numeric: tabular-nums;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 四类事件四色（语义色系，紫只给 AI 思考） */
.ws-badge.is-thought {
  color: var(--ws-accent);
}

.ws-badge.is-tool {
  color: var(--ws-color-info-text);
}

.ws-badge.is-state {
  color: var(--ws-color-success-text);
}

.ws-badge.is-approval {
  color: var(--ws-color-warning-text);
}

.ws-step-checkpoint {
  margin-left: auto;
  color: var(--ws-color-success-text);
  font-size: 10px;
}

.ws-step-title {
  font-weight: 600;
  font-size: var(--ws-font-size-body);
}

.ws-step-status {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-step-detail {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
}

.ws-detail-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-detail-title {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-detail-text {
  margin: 0;
  font-size: var(--ws-font-size-body);
  line-height: 1.7;
}

.ws-detail-label {
  margin: 0 0 4px;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-detail-block {
  display: flex;
  flex-direction: column;
}

.ws-tool {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-tool-row {
  display: flex;
  gap: 8px;
  font-size: var(--ws-font-size-caption);
}

.ws-tool-key {
  flex: none;
  width: 52px;
  color: var(--ws-text-secondary);
}

.ws-tool-val {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}

.ws-tool-dur {
  flex: none;
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.ws-logs {
  margin: 0;
  padding: 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
  font-family: var(--font-mono, monospace);
  font-size: var(--ws-font-size-caption);
  line-height: 1.7;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.ws-codechange {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
  font-size: var(--ws-font-size-caption);
}

.ws-codechange-file {
  font-family: var(--font-mono, monospace);
}

.ws-codechange-nums {
  color: var(--ws-color-info-text);
  font-variant-numeric: tabular-nums;
}
</style>
