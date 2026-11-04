<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import { EntryButton } from "@/components/ui/entry-button";
import { CommandInput } from "@/components/ui/command-input";
import { Suggestion } from "@/components/ui/suggestion";
import { AiStatusIndicator } from "@/components/ui/ai-status-indicator";
import { HumanApprovalIndicator } from "@/components/ui/human-approval-indicator";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/**
 * /design/components/ai —— DS/AI 族样板。
 * 视觉契约（AGENTS.md）：AI 建议 = accent 色；人工确认提示 = 次文字色。
 */
const { t } = useI18n();

const aiInput = ref("");
const aiStatuses = ["idle", "thinking", "waiting", "done", "failed"] as const;
const approvalStates = [
  "pending",
  "approved",
  "rejected",
  "not-required"
] as const;
const suggestionState = ref<"default" | "accepted" | "rejected">("default");
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">{{ t("design.components.ai.title") }}</h1>
      <p class="design-page__subtitle">
        {{ t("design.components.ai.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>EntryButton (13:34559)</h2>
      <div class="design-page__row">
        <Specimen label="Variant=default"
          ><EntryButton label="Ask AI"
        /></Specimen>
        <Specimen label="Variant=accent"
          ><EntryButton label="Suggested by AI" variant="accent"
        /></Specimen>
        <Specimen label="Variant=ghost"
          ><EntryButton label="AI insights" variant="ghost"
        /></Specimen>
        <Specimen label="State=Loading"
          ><EntryButton label="Working…" loading
        /></Specimen>
        <Specimen label="State=Disabled"
          ><EntryButton label="Ask AI" disabled
        /></Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>CommandInput (13:34680)</h2>
      <div class="design-page__row">
        <Specimen label="State=Default + context" width="360px">
          <CommandInput v-model="aiInput" placeholder="Ask about this project…">
            <template #context>
              <HumanApprovalIndicator
                state="not-required"
                label="Context: Atlas · Development"
              />
            </template>
          </CommandInput>
        </Specimen>
        <Specimen label="State=Loading" width="360px">
          <CommandInput model-value="" loading />
        </Specimen>
        <Specimen label="State=Disabled" width="360px">
          <CommandInput disabled />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>Suggestion (13:34757) — {{ suggestionState }}</h2>
      <div class="design-page__row">
        <Specimen label="State=default" width="360px">
          <Suggestion
            title="Split this requirement"
            body="Two acceptance criteria are mixed; consider splitting into two items."
          >
            <template #actions>
              <button
                type="button"
                class="design-page__mini"
                @click="suggestionState = 'accepted'"
              >
                Accept
              </button>
              <button
                type="button"
                class="design-page__mini"
                @click="suggestionState = 'rejected'"
              >
                Dismiss
              </button>
            </template>
          </Suggestion>
        </Specimen>
        <Specimen label="State=accepted" width="360px">
          <Suggestion
            title="Apply patch set A"
            state="accepted"
            body="Approved changes applied."
          />
        </Specimen>
        <Specimen label="State=rejected" width="360px">
          <Suggestion
            title="Auto-rename modules"
            state="rejected"
            body="Rejected by reviewer."
          />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>AI StatusIndicator (13:34798)</h2>
      <div class="design-page__row">
        <Specimen v-for="s in aiStatuses" :key="s" :label="`Status=${s}`">
          <AiStatusIndicator :status="s" :label="s" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>
        HumanApprovalIndicator (13:34834) — secondary text color, not accent
      </h2>
      <div class="design-page__row">
        <Specimen v-for="s in approvalStates" :key="s" :label="`State=${s}`">
          <HumanApprovalIndicator :state="s" :label="s" actor="Lead: Alex" />
        </Specimen>
      </div>
    </section>
  </DesignLayout>
</template>

<style scoped>
.design-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-page__group {
  margin: 0 0 var(--space-8);
}

.design-page__group h2 {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--space-4);
}

.design-page__mini {
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-primary);
  cursor: pointer;
}

.design-page__mini:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}
</style>
