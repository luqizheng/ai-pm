<script setup lang="ts">
import { useI18n } from "vue-i18n";

import AiResponseCard from "@/components/ui/ai-interaction/card/AiResponseCard.vue";
import AiStatusPill from "@/components/ui/ai-interaction/card/AiStatusPill.vue";
import {
  aiIdentity,
  conversationFixture,
  informationalFixture,
  projectEvidence,
  recommendationFixture
} from "@/components/ui/ai-interaction/fixtures";
import type {
  AiEvidence,
  FooterAction,
  InteractionStatus
} from "@/components/ui/ai-interaction/types";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 帧 01 — 卡片基座。
 *
 * 用 AiResponseCard 直接组装一张「全段位」卡（六段位一次看全），
 * 再逐帧证明段位是**按需出现**的：缺省段位不渲染、密度只改尺寸、
 * 长正文可折叠、状态徽标只表达交互进度。
 */
const { t } = useI18n();

/** 结论句取自会话样例封套（页面内不硬编码中文文案） */
const baseConclusion =
  conversationFixture.find(message => message.interaction)?.interaction
    ?.conclusion ?? "";

/** 全段位卡：证据区默认展开，否则证据段看不到内容 */
const evidence: AiEvidence = { ...projectEvidence, defaultOpen: true };

const fullCardActions: FooterAction[] = [
  {
    key: "accept",
    label: t("ai.recommendation.actions.accept"),
    variant: "primary"
  },
  {
    key: "reject",
    label: t("ai.recommendation.actions.reject"),
    variant: "danger"
  },
  {
    key: "discuss",
    label: t("ai.recommendation.actions.discuss"),
    variant: "ghost"
  }
];

/** 密度档位（只影响宽度与留白，段位集合不变） */
const densities = ["default", "conversation", "compact"] as const;

/** 每档对应的宽度 token（设计稿口径，便于对照 tokens.css） */
const densityToken: Record<(typeof densities)[number], string> = {
  default: "--size-ai-card-max-width",
  conversation: "--size-ai-card-conversation-width",
  compact: "--size-ai-card-compact-width"
};

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

/** 六段位标注（设计稿口径英文；源码不得硬编码中文文案） */
const segments = [
  {
    name: "Header",
    detail: "AI identity · type badge · status · timestamp · more"
  },
  {
    name: "Summary",
    detail: "title · description · key conclusion (accent block)"
  },
  {
    name: "Content",
    detail: "markdown body · tables · code blocks"
  },
  {
    name: "Evidence",
    detail: "analysis · sources · related objects"
  },
  {
    name: "Interaction",
    detail: "type-specific controls (slot)"
  },
  {
    name: "Footer",
    detail: "accept / reject / defer / details"
  }
];
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.base.title"
    subtitle-key="design.aiInteraction.base.subtitle"
  >
    <DesignFrame
      label="01 — Interactive Response Card / Base"
      width="wide"
      note="All six segments in a single card. Each segment renders only when its data or slot is present — an absent segment leaves no gap behind."
    >
      <AiResponseCard
        type="recommendation"
        :identity="aiIdentity"
        status="in-progress"
        :title="t('ai.recommendation.title')"
        :description="recommendationFixture.summary"
        :conclusion="baseConclusion"
        timestamp="10:01"
        datetime="2026-11-05T10:01:00+08:00"
        :evidence="evidence"
        :actions="fullCardActions"
        footer-wrap
      >
        <MarkdownViewer
          :model-value="recommendationFixture.explanationMarkdown"
        />

        <template #interaction>
          <p class="base-page__slot-hint">
            Interaction slot — rendered only when the response type supplies its
            own controls.
          </p>
        </template>
      </AiResponseCard>

      <ul class="base-page__segments">
        <li
          v-for="segment in segments"
          :key="segment.name"
          class="base-page__segment"
        >
          <code class="base-page__segment-name">{{ segment.name }}</code>
          <span class="base-page__segment-detail">{{ segment.detail }}</span>
        </li>
      </ul>
    </DesignFrame>

    <DesignFrame
      label="01a — Optional segments"
      width="narrow"
      note="Header + Summary only: without content, evidence, interaction or footer data the card stays a short message instead of a block."
    >
      <AiResponseCard
        type="informational"
        :identity="aiIdentity"
        :title="t('ai.informational.title')"
        timestamp="10:04"
      />
    </DesignFrame>

    <DesignFrame
      label="01b — Density (default / conversation / compact)"
      width="full"
      note="Density only changes width and spacing. The segment set and the information hierarchy stay identical across the three cards."
    >
      <div class="base-page__densities">
        <div
          v-for="density in densities"
          :key="density"
          class="base-page__density"
          :class="`is-${density}`"
        >
          <p class="base-page__density-label">
            <code class="base-page__density-name">{{ density }}</code>
            <span class="base-page__density-token">
              var({{ densityToken[density] }})
            </span>
          </p>
          <AiResponseCard
            type="recommendation"
            :identity="aiIdentity"
            :density="density"
            :title="recommendationFixture.recommendedAction"
            :description="recommendationFixture.summary"
            timestamp="10:01"
          />
        </div>
      </div>
    </DesignFrame>

    <DesignFrame
      label="01c — Long content collapse"
      width="wide"
      note="Long bodies are clipped to the collapsed height; the toggle is the only extra control and the footer stays outside the clipped area."
    >
      <AiResponseCard
        type="informational"
        :identity="aiIdentity"
        density="conversation"
        :title="t('ai.informational.title')"
        timestamp="10:08"
        :collapsible="true"
        :default-collapsed="true"
      >
        <MarkdownViewer :model-value="informationalFixture.contentMarkdown" />
      </AiResponseCard>
    </DesignFrame>

    <DesignFrame
      label="01d — Status badges (11 states)"
      width="full"
      note="Status pills describe the interaction lifecycle only; they are not business entity states and never imply approval."
    >
      <div class="base-page__statuses">
        <div v-for="status in statuses" :key="status" class="base-page__status">
          <AiStatusPill :status="status" />
          <code class="base-page__status-code">{{ status }}</code>
        </div>
      </div>
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.base-page__slot-hint {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  color: var(--ws-text-secondary);
  background: var(--ws-elevated);
  border-left: var(--size-indicator) solid var(--ws-accent);
  border-radius: var(--radius-xs);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.base-page__segments {
  display: flex;
  flex-direction: column;
  margin: var(--space-3) 0 0;
  padding: 0;
  list-style: none;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow: hidden;
}

.base-page__segment {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  min-height: var(--size-control-sm);
  padding: var(--space-1) var(--space-3);
  background: var(--ws-surface);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.base-page__segment:last-child {
  border-bottom: none;
}

.base-page__segment-name {
  flex: none;
  width: 96px;
  color: var(--ws-accent);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.base-page__segment-detail {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.base-page__densities {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.base-page__density {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

/* 每档卡片按对应宽度 token 收敛，宽度差异即密度差异 */
.base-page__density.is-default {
  max-width: var(--size-ai-card-max-width);
}

.base-page__density.is-conversation {
  max-width: var(--size-ai-card-conversation-width);
}

.base-page__density.is-compact {
  max-width: var(--size-ai-card-compact-width);
}

.base-page__density-label {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-nano);
  letter-spacing: var(--tracking-caption);
  text-transform: uppercase;
}

.base-page__density-name {
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.base-page__density-token {
  color: var(--ws-text-disabled);
  font-family: var(--font-mono);
}

.base-page__statuses {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2) var(--space-4);
}

.base-page__status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 160px;
}

.base-page__status-code {
  color: var(--ws-text-disabled);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
}

@media (prefers-reduced-motion: reduce) {
  .base-page__segment {
    transition: none;
  }
}
</style>
