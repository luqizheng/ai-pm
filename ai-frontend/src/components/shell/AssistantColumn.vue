<script setup lang="ts">
import { computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Plus } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Select from "@/components/ui/select/Select.vue";
import { AiConversationPanel } from "@/components/ui/ai-interaction/conversation";
import { useAssistantConversation } from "@/composables/useAssistantConversation";
import { useShellStore } from "@/stores/shell";
import CommandBar from "./CommandBar.vue";

/**
 * AI Chat 常驻独立列（AI Collaboration V1.0 / DR-C8 第 ② 步）。
 *
 * **会话能力与 `/project/assistant` 会话页共用同一实现**
 * （`@/composables/useAssistantConversation`）——不重复建设会话体系：
 * 列表 / 首屏 / 增量轮询 / 发送 / 响应卡写回 / 上下文锚点均来自该 composable。
 *
 * 与会话页的差异只有两处：
 *   1. 界面是**窄列**形态（会话切换器 + 时间线 + 底部 Composer）；
 *   2. 项目落点取 **shell 当前项目**（`assistantContext.projectId`）——
 *      会话页取路由/scope 的项目，列取壳内活态项目；两者同为「当前项目」。
 *
 * 指令栏（Composer）以 `embedded` 模式工作：**就地发送**，不再入队并跳转
 * 会话页（入队交接仍是会话页与顶栏「助手」入口的通路）。
 */

const { t } = useI18n();
const shell = useShellStore();

/** 生效项目：壳内活态项目（切换中为 null——不暴露旧项目数据）。 */
const projectId = computed(() => shell.assistantContext.projectId ?? "");

const {
  conversations,
  selectedId,
  timeline,
  loading,
  loadError,
  messagesLoading,
  sendError,
  pendingText,
  drafting,
  panelTitle,
  conversationOptions,
  load,
  reload,
  onSelect,
  handleSend,
  resend,
  handleCardSubmit,
  startConversation
} = useAssistantConversation({
  projectId,
  fallbackTitle: t("assistant.untitled")
});

/** 项目切换 → 重拉会话列表（切换期间项目为 null → 清空）。 */
watch(projectId, next => {
  if (!next) return;
  void load();
});

// 首屏即拉（列随壳挂载，不必等用户交互）。
void load();

/** 底部 Composer 就地发送（不再入队 + 跳转）。 */
function onSubmit(text: string) {
  void handleSend(text);
}
</script>

<template>
  <section class="workspace-chat" :aria-label="t('shell.app.chat.title')">
    <!-- 会话头：当前项目 + 会话切换 -->
    <header class="workspace-chat__header">
      <span class="workspace-chat__title">{{ t("shell.app.chat.title") }}</span>
      <Select
        v-if="conversations.length > 0 || drafting"
        class="workspace-chat__switcher"
        :model-value="selectedId"
        :options="conversationOptions"
        :placeholder="t('assistant.newPlaceholder')"
        :aria-label="t('assistant.switcherLabel')"
        @update:model-value="onSelect"
      />
      <Button
        v-if="conversations.length > 0 && !drafting"
        variant="secondary"
        size="sm"
        class="workspace-chat__new"
        :aria-label="t('assistant.newConversation')"
        @click="startConversation"
      >
        <Plus aria-hidden="true" />
      </Button>
      <span v-if="messagesLoading" class="workspace-chat__status" role="status">
        {{ t("assistant.messagesLoading") }}
      </span>
    </header>

    <!-- 会话区 -->
    <div class="workspace-chat__body">
      <EmptyState
        v-if="loading && conversations.length === 0"
        :title="t('assistant.loadingTitle')"
        :description="t('assistant.loadingDesc')"
      />
      <EmptyState
        v-else-if="conversations.length === 0 && !drafting"
        :title="t('shell.app.chat.emptyTitle')"
        :description="t('shell.app.chat.emptyHint')"
      />
      <EmptyState
        v-else-if="!projectId"
        :title="t('assistant.noProjectTitle')"
        :description="t('assistant.noProjectDesc')"
      />
      <template v-else>
        <p v-if="loadError" class="workspace-chat__error">{{ loadError }}</p>
        <AiConversationPanel
          :messages="timeline"
          :title="panelTitle"
          :context-label="shell.assistantContext.projectName ?? ''"
          :submit="handleCardSubmit"
        />
        <p v-if="sendError" class="workspace-chat__error">
          {{ sendError }}
          <Button
            v-if="pendingText"
            variant="secondary"
            size="sm"
            @click="resend"
          >
            {{ t("assistant.resend") }}
          </Button>
        </p>
      </template>
    </div>

    <!-- 输入组件：原底部指令栏升级为本列的输入区（就地发送） -->
    <CommandBar embedded class="workspace-commandbar" @submit="onSubmit" />
  </section>
</template>

<style scoped>
.workspace-chat__header {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--ws-row-height);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--ws-border);
}

.workspace-chat__title {
  flex: none;
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.workspace-chat__switcher {
  flex: 1;
  min-width: 0;
}

.workspace-chat__new {
  flex: none;
}

.workspace-chat__status {
  flex: none;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
}

.workspace-chat__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-3);
}

.workspace-chat__error {
  color: var(--ws-danger, #f07e88);
  font-size: var(--ws-font-size-caption);
}
</style>
