<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";

import AiStatusPill from "@/components/ui/ai-interaction/card/AiStatusPill.vue";
import type { InteractionStatus } from "@/components/ui/ai-interaction/types";
import { DESIGN_PATHS } from "@/router/design";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * /design/ai-interaction —— AI 交互组件库总览：
 * 架构（基座 / 8 类响应 / 控件与会话）+ 11 态生命周期 + 16 帧索引。
 */
const { t } = useI18n();

const statuses: InteractionStatus[] = [
  "created",
  "presented",
  "in-progress",
  "submitted",
  "validating",
  "processing",
  "accepted",
  "rejected",
  "deferred",
  "resolved",
  "failed"
];

interface FrameRow {
  frame: string;
  titleKey: string;
  path: string;
}

const frames: FrameRow[] = [
  {
    frame: "01",
    titleKey: "design.aiInteraction.base.title",
    path: DESIGN_PATHS.aiInteraction.base
  },
  {
    frame: "02",
    titleKey: "design.aiInteraction.informational.title",
    path: DESIGN_PATHS.aiInteraction.informational
  },
  {
    frame: "03-05",
    titleKey: "design.aiInteraction.recommendation.title",
    path: DESIGN_PATHS.aiInteraction.recommendation
  },
  {
    frame: "06-07",
    titleKey: "design.aiInteraction.proposal.title",
    path: DESIGN_PATHS.aiInteraction.proposal
  },
  {
    frame: "08-10",
    titleKey: "design.aiInteraction.question.title",
    path: DESIGN_PATHS.aiInteraction.question
  },
  {
    frame: "11-12",
    titleKey: "design.aiInteraction.decision.title",
    path: DESIGN_PATHS.aiInteraction.decision
  },
  {
    frame: "13",
    titleKey: "design.aiInteraction.artifact.title",
    path: DESIGN_PATHS.aiInteraction.artifact
  },
  {
    frame: "14",
    titleKey: "design.aiInteraction.action.title",
    path: DESIGN_PATHS.aiInteraction.action
  },
  {
    frame: "15-16",
    titleKey: "design.aiInteraction.execution.title",
    path: DESIGN_PATHS.aiInteraction.execution
  },
  {
    frame: "CL",
    titleKey: "design.aiInteraction.controls.title",
    path: DESIGN_PATHS.aiInteraction.controls
  },
  {
    frame: "S7",
    titleKey: "design.aiInteraction.conversation.title",
    path: DESIGN_PATHS.aiInteraction.conversation
  }
];

const architecture = computed(() => [
  {
    title: t("design.aiInteraction.index.archBaseTitle"),
    body: t("design.aiInteraction.index.archBaseText")
  },
  {
    title: t("design.aiInteraction.index.archTypesTitle"),
    body: t("design.aiInteraction.index.archTypesText")
  },
  {
    title: t("design.aiInteraction.index.archControlsTitle"),
    body: t("design.aiInteraction.index.archControlsText")
  }
]);
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.index.title"
    subtitle-key="design.aiInteraction.index.subtitle"
  >
    <section class="ai-overview__section">
      <h2 class="ai-overview__heading">
        {{ t("design.aiInteraction.index.archTitle") }}
      </h2>
      <div class="ai-overview__grid">
        <article
          v-for="item in architecture"
          :key="item.title"
          class="ai-overview__card"
        >
          <h3 class="ai-overview__card-title">{{ item.title }}</h3>
          <p class="ai-overview__card-body">{{ item.body }}</p>
        </article>
      </div>
    </section>

    <section class="ai-overview__section">
      <h2 class="ai-overview__heading">
        {{ t("design.aiInteraction.index.lifecycleTitle") }}
      </h2>
      <p class="ai-overview__note">
        {{ t("ai.card.lifecycle.note") }}
      </p>
      <div class="ai-overview__states">
        <div
          v-for="status in statuses"
          :key="status"
          class="ai-overview__state"
        >
          <AiStatusPill :status="status" />
          <code class="ai-overview__state-code">{{ status }}</code>
        </div>
      </div>
    </section>

    <section class="ai-overview__section">
      <h2 class="ai-overview__heading">
        {{ t("design.aiInteraction.index.framesTitle") }}
      </h2>
      <p class="ai-overview__note">
        {{ t("design.aiInteraction.index.framesHint") }}
      </p>
      <ol class="ai-overview__frames">
        <li
          v-for="row in frames"
          :key="row.frame + row.path"
          class="ai-overview__frame"
        >
          <span class="ai-overview__frame-id">{{ row.frame }}</span>
          <RouterLink class="ai-overview__frame-link" :to="row.path">
            {{ t(row.titleKey).split(" — ")[0] }}
          </RouterLink>
          <code class="ai-overview__frame-path">{{ row.path }}</code>
        </li>
      </ol>
    </section>
  </CatalogPage>
</template>

<style scoped>
.ai-overview__section {
  margin: 0 0 var(--space-8);
}

.ai-overview__heading {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  font-weight: var(--weight-medium);
  color: var(--ws-text-secondary);
  text-transform: uppercase;
  letter-spacing: var(--tracking-caption);
}

.ai-overview__note {
  margin: 0 0 var(--space-3);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  line-height: var(--line-height-micro);
}

.ai-overview__grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: var(--space-3);
}

.ai-overview__card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ai-overview__card-title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.ai-overview__card-body {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}

.ai-overview__states {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2) var(--space-4);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ai-overview__state {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 160px;
}

.ai-overview__state-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-disabled);
}

.ai-overview__frames {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow: hidden;
}

.ai-overview__frame {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: var(--ws-row-height);
  padding: 0 var(--space-4);
  background: var(--ws-surface);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-overview__frame:last-child {
  border-bottom: none;
}

.ai-overview__frame-id {
  width: var(--space-12);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.ai-overview__frame-link {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  text-decoration: none;
}

.ai-overview__frame-link:hover {
  color: var(--ws-accent);
}

.ai-overview__frame-link:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.ai-overview__frame-path {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-disabled);
}
</style>
