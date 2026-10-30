<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { PanelRightClose, Sparkles } from "lucide-vue-next";
import { useWorkspaceStore } from "../store";
import { attentionItems, contextAdvice, contextPlaceholders } from "../fixtures";

const { t } = useI18n();
const store = useWorkspaceStore();

/** 未选中时默认展示首条待办（与 V1.1 原型一致：地形渲染架构评审即选中态） */
const activeItem = computed(
  () => store.selectedAttention ?? attentionItems[0]
);

const advice = computed(() => store.selectedAttention?.advice ?? contextAdvice);
</script>

<template>
  <div class="ws-context-panel">
    <div class="ws-context-header">
      <span class="ws-context-title">{{ t("workspace.context.title") }}</span>
      <button
        type="button"
        class="ws-icon-btn ws-focusable"
        data-test="context-collapse"
        :aria-label="t('workspace.context.collapse')"
        @click="store.toggleContext()"
      >
        <PanelRightClose :size="16" aria-hidden="true" />
      </button>
    </div>

    <section class="ws-context-section">
      <h3 class="ws-context-heading">
        {{ t("workspace.context.currentTodo") }}
      </h3>
      <div class="ws-card ws-context-todo">
        <span class="ws-context-todo-kind" :class="`is-${activeItem.kind}`">
          {{ t(`workspace.context.kind.${activeItem.kind}`) }}
        </span>
        <div class="ws-context-todo-title">{{ activeItem.title }}</div>
        <div class="ws-context-todo-desc">{{ activeItem.description }}</div>
      </div>
    </section>

    <section class="ws-context-section">
      <h3 class="ws-context-heading">
        <Sparkles :size="14" aria-hidden="true" />
        {{ t("workspace.context.aiAdvice") }}
      </h3>
      <p class="ws-context-advice">{{ advice }}</p>
    </section>

    <section
      v-for="placeholder in contextPlaceholders"
      :key="placeholder"
      class="ws-context-section"
    >
      <div class="ws-context-placeholder">
        {{ placeholder }}
      </div>
    </section>
  </div>
</template>

<style scoped>
.ws-context-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px 12px;
  min-height: 100%;
}

.ws-context-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.ws-context-title {
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-context-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ws-context-heading {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-context-todo {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.ws-context-todo-kind {
  align-self: flex-start;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 紫色克制：类型徽标用语义色文字变体，不用 AI 紫 */
.ws-context-todo-kind.is-decision {
  color: var(--ws-color-warning-text);
}

.ws-context-todo-kind.is-acceptance {
  color: var(--ws-color-info-text);
}

.ws-context-todo-kind.is-blocker {
  color: var(--ws-color-danger-text);
}

.ws-context-todo-title {
  font-weight: 600;
}

.ws-context-todo-desc {
  color: var(--ws-text-secondary);
}

.ws-context-advice {
  margin: 0;
  font-size: var(--ws-font-size-body);
  line-height: 1.6;
}

.ws-context-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 120px;
  border: 1px dashed var(--ws-border);
  border-radius: var(--ws-radius-card);
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-body);
}
</style>
