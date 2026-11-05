<script setup lang="ts">
import {
  executionFailureFixture,
  executionResultFixture
} from "@/components/ui/ai-interaction/fixtures";
import ExecutionResultResponse from "@/components/ui/ai-interaction/responses/ExecutionResultResponse.vue";
import { createMockSubmit } from "@/components/ui/ai-interaction/submitAdapter";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 交付帧 15/16 —— Execution Result 响应卡。
 *
 * 15 Partial：部分成功（变更文件 + 警告 + 失败信息 + 日志 / diff 详情）；
 * 16 Failed & Retry：失败可重试，Retry 真实提交重试请求。
 *
 * 架构边界：Executor Runtime / Task 管理与会话组件分离 —— 本卡只呈现
 * 运行时返回的结果，不拥有任务调度、重试编排与日志管道。
 */
const submit = createMockSubmit({ latencyMs: 300 });
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.execution.title"
    subtitle-key="design.aiInteraction.execution.subtitle"
  >
    <section class="execution-page__legend">
      <p class="execution-page__legend-item">
        <span class="execution-page__legend-label">Runtime boundary</span>
        <span class="execution-page__legend-text">
          Executor Runtime and Task management are separate from conversation
          components: this card renders the result the runtime returns and never
          owns task scheduling, retry orchestration or the log pipeline.
        </span>
      </p>
      <p class="execution-page__legend-item">
        <span class="execution-page__legend-label">State channels</span>
        <span class="execution-page__legend-text">
          Execution state is carried by colour + icon + text together; logs,
          warnings and the diff stay collapsed until the execution details
          action is expanded.
        </span>
      </p>
      <p class="execution-page__legend-item">
        <span class="execution-page__legend-label">Actions</span>
        <span class="execution-page__legend-text">
          Accept / Request changes / Retry only record the user's action; they
          never rewrite the execution result. Retry appears only when
          failure.retryable is true.
        </span>
      </p>
    </section>

    <DesignFrame
      label="15 — Execution / Partial result"
      width="default"
      note="Partial: 12/14 specs pass and two edge cases fail because a terrain snapshot is missing. Changed files, warnings and failure details are shown; the execution details action expands the unified diff and the execution log."
    >
      <ExecutionResultResponse
        :payload="executionResultFixture"
        :submit="submit"
        timestamp="10:04"
      />
    </DesignFrame>

    <DesignFrame
      label="16 — Execution / Failed &amp; retry"
      width="default"
      note="Failed: the runtime timed out before the task started and no file changed. Retry is available (failure.retryable = true) — click Retry to submit a real retry request; the card then states that the execution record is authoritative."
    >
      <ExecutionResultResponse
        :payload="executionFailureFixture"
        :submit="submit"
        timestamp="10:02"
      />
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.execution-page__legend {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.execution-page__legend-item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
}

.execution-page__legend-label {
  flex: none;
  min-width: var(--space-12);
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.execution-page__legend-text {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}
</style>
