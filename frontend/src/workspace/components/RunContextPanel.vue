<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { formatDate } from "@/api/format";
import type { RunBaseline } from "../fixtures";

/**
 * Run 执行上下文面板（T-070 §6.4）：租约 / 执行器 / 阶段 / 各仓基线。
 * AI 执行中心的 mock 分支与真后端分支共用；**只做内容定位**——
 * `executor_session` + `workspace_path` 即「日志定位」文本，不读日志内容
 * （设计稿 §二非目标：日志读取随 M8 / T-081 跨机暴露一并设计）。
 */
defineProps<{
  leaseExpiresAt?: string | null;
  heartbeatAt?: string | null;
  executorKind?: string | null;
  executorSession?: string | null;
  stage?: string | null;
  baselines?: RunBaseline[];
}>();

const { t } = useI18n();
</script>

<template>
  <div class="ws-ctx" data-test="run-context">
    <div class="ws-ctx-row">
      <span class="ws-ctx-key">{{ t("workspace.pages.execution.lease") }}</span>
      <span v-if="leaseExpiresAt" class="ws-ctx-val" data-test="run-lease">
        {{
          t("workspace.pages.execution.leaseUntil", {
            time: formatDate(leaseExpiresAt)
          })
        }}
        <span v-if="heartbeatAt" class="ws-ctx-sub">
          {{
            t("workspace.pages.execution.heartbeat", {
              time: formatDate(heartbeatAt)
            })
          }}
        </span>
      </span>
      <span v-else class="ws-ctx-val is-muted" data-test="run-lease">{{
        t("workspace.pages.execution.noLease")
      }}</span>
    </div>
    <div class="ws-ctx-row">
      <span class="ws-ctx-key">{{
        t("workspace.pages.execution.executor")
      }}</span>
      <span
        v-if="executorKind || executorSession"
        class="ws-ctx-val"
        data-test="run-executor"
      >
        <span>{{ executorKind ?? "—" }}</span>
        <span v-if="executorSession" class="ws-ctx-sub ws-ctx-mono">
          {{
            t("workspace.pages.execution.executorSession", {
              session: executorSession
            })
          }}
        </span>
      </span>
      <span v-else class="ws-ctx-val is-muted" data-test="run-executor">—</span>
    </div>
    <div v-if="stage" class="ws-ctx-row">
      <span class="ws-ctx-key">{{ t("workspace.pages.execution.stage") }}</span>
      <span class="ws-ctx-val">{{ stage }}</span>
    </div>
    <div v-if="baselines?.length" class="ws-ctx-row">
      <span class="ws-ctx-key">{{
        t("workspace.pages.execution.baselines")
      }}</span>
      <span class="ws-ctx-val ws-ctx-stack">
        <span
          v-for="baseline in baselines"
          :key="baseline.repositoryName"
          class="ws-ctx-baseline"
          data-test="run-baseline"
        >
          <span class="ws-ctx-repo">{{ baseline.repositoryName }}</span>
          <span class="ws-ctx-sub ws-ctx-mono"
            >{{ baseline.baseRef }}@{{ baseline.baseCommit }}</span
          >
          <span class="ws-ctx-sub ws-ctx-mono"
            >{{ t("workspace.pages.execution.logLocator") }}:
            {{ baseline.workspacePath }}</span
          >
        </span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.ws-ctx {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-ctx-row {
  display: flex;
  gap: 10px;
  padding: 8px 10px;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}

.ws-ctx-row + .ws-ctx-row {
  border-top: 1px solid var(--ws-border);
}

.ws-ctx-key {
  flex: none;
  width: 76px;
  color: var(--ws-text-secondary);
}

.ws-ctx-val {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  overflow-wrap: anywhere;
}

.ws-ctx-val.is-muted {
  color: var(--ws-text-secondary);
}

.ws-ctx-stack {
  flex-direction: column;
  gap: 6px;
}

.ws-ctx-sub {
  color: var(--ws-text-secondary);
}

.ws-ctx-mono {
  font-family: var(--font-mono, monospace);
}

.ws-ctx-baseline {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
}

.ws-ctx-repo {
  font-weight: 600;
}
</style>
