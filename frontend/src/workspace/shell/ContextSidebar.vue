<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { FileText, PanelRightClose, Sparkles } from "lucide-vue-next";
import { useWorkspaceStore } from "../store";
import {
  attentionItems,
  contextAdvice,
  contextPlaceholders,
  knowledgeDocs
} from "../fixtures";

const { t } = useI18n();
const store = useWorkspaceStore();
const route = useRoute();

/** V1.2：右栏随中央页面变化，并区分「上下文 / 证据」两个页签。 */
const activeTab = ref<"context" | "evidence">("context");
const pageName = computed(() => String(route.name ?? ""));

/** 未选中时默认展示首条待办（与 V1.1 原型一致：地形渲染架构评审即选中态） */
const activeItem = computed(() => store.selectedAttention ?? attentionItems[0]);

const advice = computed(() => store.selectedAttention?.advice ?? contextAdvice);

const isKnowledgePage = computed(() => pageName.value.includes("knowledge"));

const fields = computed(() => [
  {
    key: t("workspace.context.fields.workspace"),
    value: String(route.meta?.title ?? t("workspace.context.title"))
  },
  {
    key: t("workspace.context.fields.task"),
    value: activeItem.value.title
  },
  {
    key: t("workspace.context.fields.owner"),
    value: isKnowledgePage.value
      ? t("workspace.context.owner.aiPm")
      : t("workspace.context.owner.devAgent")
  },
  {
    key: t("workspace.context.fields.permission"),
    value: isKnowledgePage.value
      ? t("workspace.context.permission.readOnly")
      : t("workspace.context.permission.highRisk")
  }
]);

const evidence = computed(() =>
  knowledgeDocs.flatMap(doc => doc.references).slice(0, 5)
);
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

    <div class="ws-context-tabs" role="tablist">
      <button
        type="button"
        role="tab"
        class="ws-context-tab ws-focusable"
        :class="{ 'is-active': activeTab === 'context' }"
        :aria-selected="activeTab === 'context'"
        data-test="context-tab-context"
        @click="activeTab = 'context'"
      >
        {{ t("workspace.context.tabs.context") }}
      </button>
      <button
        type="button"
        role="tab"
        class="ws-context-tab ws-focusable"
        :class="{ 'is-active': activeTab === 'evidence' }"
        :aria-selected="activeTab === 'evidence'"
        data-test="context-tab-evidence"
        @click="activeTab = 'evidence'"
      >
        {{ t("workspace.context.tabs.evidence") }}
      </button>
    </div>

    <template v-if="activeTab === 'context'">
      <section class="ws-context-section" data-test="context-fields">
        <div
          v-for="field in fields"
          :key="field.key"
          class="ws-context-field"
        >
          <span class="ws-context-field-key">{{ field.key }}</span>
          <span class="ws-context-field-value">{{ field.value }}</span>
        </div>
      </section>

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
    </template>

    <section v-else class="ws-context-section" data-test="context-evidence">
      <h3 class="ws-context-heading">
        <FileText :size="14" aria-hidden="true" />
        {{ t("workspace.context.tabs.evidence") }}
      </h3>
      <ul v-if="evidence.length" class="ws-context-evidence">
        <li
          v-for="(ref, index) in evidence"
          :key="`${ref.id}-${index}`"
          class="ws-context-evidence-item"
        >
          <span class="ws-context-evidence-title">{{ ref.title }}</span>
          <span class="ws-context-evidence-id">{{ ref.id }}</span>
        </li>
      </ul>
      <p v-else class="ws-context-advice">
        {{ t("workspace.context.evidenceEmpty") }}
      </p>
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

.ws-context-tabs {
  display: flex;
  gap: 4px;
  padding: 2px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ws-context-tab {
  flex: 1;
  min-height: 28px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.ws-context-tab.is-active {
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
  font-weight: 600;
}

.ws-context-field {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.ws-context-field-key {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-context-field-value {
  font-size: var(--ws-font-size-caption);
  text-align: right;
}

.ws-context-evidence {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.ws-context-evidence-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-context-evidence-title {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
}

.ws-context-evidence-id {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
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
