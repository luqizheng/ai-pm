<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import CommandBar from "@/components/shell/CommandBar.vue";
import ContextPanel from "@/components/shell/ContextPanel.vue";
import SideNav from "@/components/shell/SideNav.vue";
import TopBar from "@/components/shell/TopBar.vue";
import { demoProject, demoProjects } from "@/components/shell/demo";
import ActionRequestResponse from "@/components/ui/ai-interaction/responses/ActionRequestResponse.vue";
import ArtifactResponse from "@/components/ui/ai-interaction/responses/ArtifactResponse.vue";
import DecisionResponse from "@/components/ui/ai-interaction/responses/DecisionResponse.vue";
import ExecutionResultResponse from "@/components/ui/ai-interaction/responses/ExecutionResultResponse.vue";
import QuestionResponse from "@/components/ui/ai-interaction/responses/QuestionResponse.vue";
import RecommendationResponse from "@/components/ui/ai-interaction/responses/RecommendationResponse.vue";
import AiConversationPanel from "@/components/ui/ai-interaction/conversation/AiConversationPanel.vue";
import {
  actionRequestFixture,
  aiIdentity,
  artifactFixture,
  conversationFixture,
  decisionFixture,
  executionResultFixture,
  questionFixture,
  recommendationFixture
} from "@/components/ui/ai-interaction/fixtures";
import type { ConversationMessage } from "@/components/ui/ai-interaction/types";
import { useShellStore } from "@/stores/shell";
import CatalogPage from "./_shared/CatalogPage.vue";
import DesignFrame from "../design-kit/DesignFrame.vue";

/**
 * /design/ai-interaction/conversation —— 会话集成：
 *   - §7 帧：1440×900 画框内真实应用壳（顶栏 / 左导航 / 会话主区 / 右上下文栏 / 指令栏）；
 *   - 原型 A–H：8 个会话内交互，全部使用真实响应卡，控件可点、状态真实变化。
 */
const { t } = useI18n();

/** 画框内的壳依赖项目上下文与指令栏数据，这里按 WorkspaceShell 的做法注入演示数据 */
const shell = useShellStore();
shell.setProjects(demoProjects);
if (!shell.currentProjectId) {
  shell.setCurrentProject(demoProject.id);
}

/** 会话消息：通过 composer 发送时追加为用户消息（演示态，无假回复） */
const messages = ref<ConversationMessage[]>([...conversationFixture]);
let localSequence = 0;

function onSend(text: string): void {
  localSequence += 1;
  messages.value = [
    ...messages.value,
    {
      id: `conversation-local-${localSequence}`,
      role: "user",
      createdAt: new Date().toISOString(),
      text
    }
  ];
}
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.conversation.title"
    subtitle-key="design.aiInteraction.conversation.subtitle"
  >
    <p class="conversation-view__intro">{{ t("ai.conversation.intro") }}</p>

    <DesignFrame
      label="§7 — Conversation integration (1440 × 900)"
      width="full"
    >
      <div class="conversation-frame">
        <div class="conversation-frame__grid">
          <TopBar class="workspace-topbar" />
          <SideNav class="workspace-leftnav" />
          <main class="workspace-main">
            <div class="workspace-main-scroll conversation-frame__scroll">
              <AiConversationPanel
                class="conversation-frame__panel"
                :messages="messages"
                :title="t('ai.conversation.title')"
                :context-label="t('ai.conversation.contextLabel')"
                @send="onSend"
              />
            </div>
            <CommandBar class="workspace-commandbar" />
          </main>
          <ContextPanel class="workspace-context" />
        </div>
      </div>
    </DesignFrame>

    <section class="conversation-prototypes">
      <h2 class="conversation-prototypes__heading">
        {{ t("ai.conversation.prototype.heading") }}
      </h2>
      <p class="conversation-prototypes__hint">
        {{ t("ai.conversation.prototype.hint") }}
      </p>

      <div class="conversation-prototypes__grid">
        <DesignFrame
          data-prototype="A"
          label="A — Accept recommendation"
          :note="t('ai.conversation.prototype.a')"
          width="full"
        >
          <RecommendationResponse
            :payload="recommendationFixture"
            :identity="aiIdentity"
            timestamp="10:01"
          />
        </DesignFrame>

        <DesignFrame
          data-prototype="B"
          label="B — Reject with reason"
          :note="t('ai.conversation.prototype.b')"
          width="full"
        >
          <RecommendationResponse
            :payload="recommendationFixture"
            :identity="aiIdentity"
            timestamp="10:02"
          />
        </DesignFrame>

        <DesignFrame
          data-prototype="C"
          label="C — Multi-select answers"
          :note="t('ai.conversation.prototype.c')"
          width="full"
        >
          <QuestionResponse
            :payload="questionFixture"
            :identity="aiIdentity"
            timestamp="10:03"
          />
        </DesignFrame>

        <DesignFrame
          data-prototype="D"
          label="D — Edit AI-generated requirement"
          :note="t('ai.conversation.prototype.d')"
          width="full"
        >
          <ArtifactResponse
            :payload="artifactFixture"
            :identity="aiIdentity"
            timestamp="10:04"
          />
        </DesignFrame>

        <DesignFrame
          data-prototype="E"
          label="E — Submit multiple answers"
          :note="t('ai.conversation.prototype.e')"
          width="full"
        >
          <QuestionResponse
            :payload="questionFixture"
            :identity="aiIdentity"
            timestamp="10:05"
          />
        </DesignFrame>

        <DesignFrame
          data-prototype="F"
          label="F — Submit formal decision"
          :note="t('ai.conversation.prototype.f')"
          width="full"
        >
          <DecisionResponse
            :payload="decisionFixture"
            :identity="aiIdentity"
            timestamp="10:06"
          />
        </DesignFrame>

        <DesignFrame
          data-prototype="G"
          label="G — Confirm high-consequence action"
          :note="t('ai.conversation.prototype.g')"
          width="full"
        >
          <ActionRequestResponse
            :payload="actionRequestFixture"
            :identity="aiIdentity"
            timestamp="10:07"
          />
        </DesignFrame>

        <DesignFrame
          data-prototype="H"
          label="H — Review agent result"
          :note="t('ai.conversation.prototype.h')"
          width="full"
        >
          <ExecutionResultResponse
            :payload="executionResultFixture"
            :identity="aiIdentity"
            timestamp="10:08"
          />
        </DesignFrame>
      </div>
    </section>
  </CatalogPage>
</template>

<style scoped>
.conversation-view__intro {
  margin: 0 0 var(--space-6);
  max-width: var(--size-ai-conversation-max-width);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.conversation-frame {
  width: 1440px;
  max-width: 100%;
}

.conversation-frame__grid {
  display: grid;
  grid-template-columns:
    var(--ws-sidebar-width)
    minmax(0, 1fr)
    var(--ws-context-width);
  grid-template-rows: var(--ws-topbar-height) minmax(0, 1fr);
  height: 900px;
  overflow: hidden;
  background: var(--ws-canvas);
  color: var(--ws-text-primary);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  font-size: var(--ws-font-size-body);
}

.conversation-frame__scroll {
  display: flex;
  flex-direction: column;
}

.conversation-frame__panel {
  flex: 1;
  min-height: 0;
}

.conversation-prototypes {
  margin: 0;
}

.conversation-prototypes__heading {
  margin: 0 0 var(--space-2);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  font-weight: var(--weight-medium);
  color: var(--ws-text-secondary);
  letter-spacing: var(--tracking-caption);
  text-transform: uppercase;
}

.conversation-prototypes__hint {
  margin: 0 0 var(--space-5);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.conversation-prototypes__grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
  gap: var(--space-4) var(--space-6);
  align-items: start;
}
</style>
