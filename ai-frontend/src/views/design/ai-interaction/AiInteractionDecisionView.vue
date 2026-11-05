<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";

import { decisionFixture } from "@/components/ui/ai-interaction/fixtures";
import DecisionResponse from "@/components/ui/ai-interaction/responses/DecisionResponse.vue";
import { createMockSubmit } from "@/components/ui/ai-interaction/submitAdapter";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 交付帧 11/12 —— Decision 响应卡。
 *
 * 11 Pending：选选项 + 补充意见 + 以本人身份确认 + 二次确认弹层 + 正式提交；
 * 12 Recorded：只读决策记录（决策人 = 人类 / 时间 / 记录号）。
 */
const submit = createMockSubmit({ latencyMs: 300 });
const replaySubmit = createMockSubmit({ latencyMs: 0 });

const recordedHost = ref<HTMLElement | null>(null);

async function nextBeat(): Promise<void> {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 0));
}

/** 弹层由 Teleport 挂到 body：等它出现后再点确认，避免时序抖动 */
async function confirmDialogIfPresent(): Promise<void> {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    const button = document.querySelector<HTMLElement>(
      ".ui-confirm__btn--primary"
    );
    if (button) {
      button.click();
      return;
    }
    await nextBeat();
  }
}

/**
 * 重放一次真实决策：选择 AI 推荐项 → 勾选「以本人身份提交」
 * → 点正式提交 → 弹层二次确认，结果由卡片自身渲染为只读记录。
 */
async function replayDecision(): Promise<void> {
  const host = recordedHost.value;
  if (!host) return;
  host.querySelector<HTMLElement>(".ai-decision__option")?.click();
  await nextBeat();
  host
    .querySelector<HTMLElement>(".ai-decision__confirm .ui-checkbox__root")
    ?.click();
  await nextBeat();
  const buttons = host.querySelectorAll<HTMLButtonElement>(
    ".ai-card__footer button"
  );
  buttons[buttons.length - 1]?.click();
  await nextBeat();
  await confirmDialogIfPresent();
}

onMounted(async () => {
  await nextTick();
  void replayDecision();
});
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.decision.title"
    subtitle-key="design.aiInteraction.decision.subtitle"
  >
    <section class="decision-page__legend">
      <p class="decision-page__legend-item">
        <span class="decision-page__swatch is-ai" aria-hidden="true" />
        <span class="decision-page__legend-label">AI recommendation</span>
        <span class="decision-page__legend-text">
          Accent outline + sparkle badge. A suggestion only — it pre-selects
          nothing and never counts as a decision.
        </span>
      </p>
      <p class="decision-page__legend-item">
        <span class="decision-page__swatch is-user" aria-hidden="true" />
        <span class="decision-page__legend-label">Your choice</span>
        <span class="decision-page__legend-text">
          Success badge on the option you actually pick. The choice is captured
          locally before any submission happens.
        </span>
      </p>
      <p class="decision-page__legend-item">
        <span class="decision-page__swatch is-record" aria-hidden="true" />
        <span class="decision-page__legend-label">Formal submission</span>
        <span class="decision-page__legend-text">
          Self-confirmation checkbox + confirmation dialog. Only this step
          records a decision, with decision maker = human, time and record
          number.
        </span>
      </p>
    </section>

    <DesignFrame
      label="11 — Decision / Pending (recommendation vs. choice vs. submit)"
      width="default"
      note="Pick an option, add an optional comment, tick the self-confirmation checkbox and press Submit. A confirmation dialog guards the formal submit; without the checkbox the card refuses to open it."
    >
      <DecisionResponse
        :payload="decisionFixture"
        :submit="submit"
        timestamp="10:10"
      />
    </DesignFrame>

    <DesignFrame
      label="12 — Decision / Recorded (read-only)"
      width="default"
      note="Recorded: read-only summary with decision maker = human, decided-at time and decision record number. This frame replays one real submit on mount (option → self-confirm → dialog → record)."
    >
      <div ref="recordedHost" class="decision-page__host">
        <DecisionResponse
          :payload="decisionFixture"
          :submit="replaySubmit"
          timestamp="10:12"
        />
      </div>
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.decision-page__legend {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.decision-page__legend-item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
}

.decision-page__swatch {
  flex: none;
  width: var(--space-2);
  height: var(--space-2);
  border-radius: var(--radius-full);
}

.decision-page__swatch.is-ai {
  background: var(--ws-accent);
}

.decision-page__swatch.is-user {
  background: var(--ws-color-success);
}

.decision-page__swatch.is-record {
  background: var(--ws-text-secondary);
}

.decision-page__legend-label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
}

.decision-page__legend-text {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.decision-page__host {
  width: 100%;
}
</style>
