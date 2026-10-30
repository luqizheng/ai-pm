<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { attentionItems } from "../fixtures";
import { useWorkspaceStore } from "../store";

const { t } = useI18n();
const store = useWorkspaceStore();
</script>

<template>
  <section class="ws-attention" aria-label="attention">
    <header class="ws-section-header">
      <h2 class="ws-section-title">
        {{ t("workspace.console.needAttention") }}
      </h2>
      <span class="ws-section-count">
        {{
          t("workspace.console.needAttentionCount", {
            count: attentionItems.length
          })
        }}
      </span>
    </header>
    <div class="ws-attention-list">
      <button
        v-for="item in attentionItems"
        :key="item.id"
        type="button"
        class="ws-card ws-attention-item ws-focusable"
        :class="{ 'ws-selected-ring': store.selectedAttentionId === item.id }"
        data-test="attention-item"
        @click="store.selectAttention(item.id)"
      >
        <span class="ws-attention-kind" :class="`is-${item.kind}`">
          {{ t(`workspace.console.kind.${item.kind}`) }}
        </span>
        <span class="ws-attention-title">{{ item.title }}</span>
        <span class="ws-attention-desc">{{ item.description }}</span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.ws-section-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
}

.ws-section-title {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-section-count {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-attention-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ws-attention-item {
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-areas:
    "kind title"
    "kind desc";
  align-items: center;
  column-gap: 12px;
  padding: 12px 16px;
  cursor: pointer;
  text-align: left;
  color: inherit;
  font: inherit;
}

.ws-attention-item:hover {
  background: var(--ws-elevated);
}

/* 紫色克制：类型徽标用语义色文字变体；选中态描边用 --ws-focus-ring（AI 紫） */
.ws-attention-kind {
  grid-area: kind;
  align-self: center;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

.ws-attention-kind.is-decision {
  color: var(--ws-color-warning-text);
}

.ws-attention-kind.is-acceptance {
  color: var(--ws-color-info-text);
}

.ws-attention-kind.is-blocker {
  color: var(--ws-color-danger-text);
}

.ws-attention-title {
  grid-area: title;
  font-weight: 600;
}

.ws-attention-desc {
  grid-area: desc;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
