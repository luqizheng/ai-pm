<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { MessagesSquare } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { useShellStore } from "@/stores/shell";
import CommandBar from "./CommandBar.vue";

/**
 * AI Chat 常驻独立列（AI Collaboration V1.0 / DR-C8）。
 *
 * 结构自上而下：会话头（标题 + 专注模式切换）→ 会话区 → 输入组件。
 *
 * **分阶段说明（DR-C8 渐进迁移第 ① 步）**：本阶段只建立四栏容器与
 * AI Chat 列，输入组件（Composer）由原底部指令栏升级而来、**复用既有
 * 实现**；会话能力整体并入本列是第 ② 步（迁移 `/project/assistant` 的
 * 会话机制，不重复建设会话体系）。在此之前，本列是**常驻的 AI 入口**：
 * 输入区始终在场，完整会话页仍由既有路由承载（顶栏「助手」入口同路由）。
 *
 * 作用域串与右上下文栏共用 `shell.assistantContext`（上下文单一真源）。
 */

const { t } = useI18n();
const router = useRouter();
const shell = useShellStore();

/** 完整会话页路由（与顶栏「助手」入口同路由） */
const ASSISTANT_PATH = "/project/assistant";

/** 当前项目名（切换中显示切换态，不回落旧项目名） */
const scopeLabel = computed(() => {
  const context = shell.assistantContext;
  if (!context.projectId) return t("shell.app.project.switching");
  return context.projectName ?? context.projectId;
});

function openConversation() {
  if (router.currentRoute.value.path !== ASSISTANT_PATH) {
    void router.push(ASSISTANT_PATH);
  }
}
</script>

<template>
  <section class="workspace-chat" :aria-label="t('shell.app.chat.title')">
    <!-- 会话头：标题 + 当前作用域（专注模式开关在顶栏，专注模式下本列收起） -->
    <header class="workspace-chat__header">
      <span class="workspace-chat__title">{{ t("shell.app.chat.title") }}</span>
      <span class="workspace-chat__scope">{{ scopeLabel }}</span>
    </header>

    <!-- 会话区（第 ② 步并入会话能力；当前为空态 + 完整会话入口） -->
    <div class="workspace-chat__body">
      <EmptyState
        :title="t('shell.app.chat.emptyTitle')"
        :description="t('shell.app.chat.emptyHint')"
        :action-label="t('shell.app.chat.openConversation')"
        @action="openConversation"
      />
    </div>

    <!-- 输入组件：原底部指令栏升级为本列的输入区 -->
    <CommandBar class="workspace-commandbar" />

    <!-- 完整会话页入口（会话能力并入本列后移除） -->
    <div class="workspace-chat__footer">
      <Button variant="ghost" size="sm" @click="openConversation">
        <MessagesSquare :size="14" aria-hidden="true" />
        {{ t("shell.app.chat.openConversation") }}
      </Button>
    </div>
  </section>
</template>

<style scoped>
.workspace-chat__header {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-row-height);
  padding: 0 var(--space-4);
  border-bottom: 1px solid var(--ws-border);
}

.workspace-chat__title {
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.workspace-chat__scope {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
}

.workspace-chat__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-4);
}

.workspace-chat__footer {
  flex: none;
  display: flex;
  justify-content: flex-end;
  padding: 0 var(--space-4) var(--space-2);
}
</style>
