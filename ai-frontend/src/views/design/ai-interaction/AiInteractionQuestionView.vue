<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";

import { questionFixture } from "@/components/ui/ai-interaction/fixtures";
import QuestionResponse from "@/components/ui/ai-interaction/responses/QuestionResponse.vue";
import { createMockSubmit } from "@/components/ui/ai-interaction/submitAdapter";
import type { QuestionPayload } from "@/components/ui/ai-interaction/types";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 交付帧 08/09/10 —— Question 响应卡。
 *
 * 08 Pending：五种题型同卡，真实可填（单选 / 多选 / 数字 / 日期 / 多行）；
 * 09 Validation error：空必填时点提交 → 卡片内校验错误，不发起提交；
 * 10 Submitted：预填必填答案后真实提交一次，卡片转为只读答案汇总。
 */
const submit = createMockSubmit({ latencyMs: 300 });
const replaySubmit = createMockSubmit({ latencyMs: 0 });

/** 帧 10 的样例答案：仅必填题预填，其余保留「未填写」展示 */
const submittedPayload: QuestionPayload = {
  ...questionFixture,
  questions: questionFixture.questions.map(question => {
    if (!question.required) return question;
    if (question.kind === "multi") {
      return { ...question, defaultValue: ["forest"] };
    }
    if (question.kind === "single") {
      return { ...question, defaultValue: "basic" };
    }
    return { ...question, defaultValue: "2026-11-11" };
  })
};

const submittedHost = ref<HTMLElement | null>(null);

/**
 * 复现「已提交」帧：挂载后对预填卡片触发一次真实提交
 * （等价于用户点击 Submit all answers），只读汇总由卡片自身渲染。
 */
onMounted(async () => {
  await nextTick();
  const buttons =
    submittedHost.value?.querySelectorAll<HTMLButtonElement>(
      ".ai-card__footer button"
    ) ?? [];
  buttons[buttons.length - 1]?.click();
});
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.question.title"
    subtitle-key="design.aiInteraction.question.subtitle"
  >
    <section class="question-page__legend">
      <p class="question-page__legend-item">
        <span class="question-page__legend-label">08 — Pending</span>
        <span class="question-page__legend-text">
          Five question kinds share one card; answers stay local until Submit
          all answers, and the progress bar counts answered questions.
        </span>
      </p>
      <p class="question-page__legend-item">
        <span class="question-page__legend-label">09 — Validation error</span>
        <span class="question-page__legend-text">
          Empty required fields block the submit: missing questions are
          highlighted, the first invalid field is focused, and no request is
          sent.
        </span>
      </p>
      <p class="question-page__legend-item">
        <span class="question-page__legend-label">10 — Submitted</span>
        <span class="question-page__legend-text">
          A successful submit turns the card read-only: recorded answers plus
          submit time and record reference; the form is gone.
        </span>
      </p>
    </section>

    <DesignFrame
      label="08 — Question / Pending (multi-question)"
      width="default"
      note="One card, five live inputs. Fill anything and the answered counter / progress bar update immediately; Save draft keeps the answers locally without submitting."
    >
      <QuestionResponse
        :payload="questionFixture"
        :submit="submit"
        timestamp="10:03"
      />
    </DesignFrame>

    <DesignFrame
      label="09 — Question / Validation error"
      width="default"
      note="Reproduce: leave the required fields empty and click Submit all answers on this card. The card marks the missing questions and does not call the submit adapter."
    >
      <QuestionResponse
        :payload="questionFixture"
        :submit="submit"
        timestamp="10:04"
      />
    </DesignFrame>

    <DesignFrame
      label="10 — Question / Submitted (read-only summary)"
      width="default"
      note="This frame replays one real submit on mount with prefilled required answers; the read-only summary, submit time and record reference are rendered by the card itself."
    >
      <div ref="submittedHost" class="question-page__host">
        <QuestionResponse
          :payload="submittedPayload"
          initial-status="submitted"
          :submit="replaySubmit"
          timestamp="10:06"
        />
      </div>
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.question-page__legend {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.question-page__legend-item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
}

.question-page__legend-label {
  flex: none;
  min-width: var(--space-12);
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.question-page__legend-text {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.question-page__host {
  width: 100%;
}
</style>
