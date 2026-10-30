<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { FileText, Folder } from "lucide-vue-next";
import PageHeader from "./PageHeader.vue";
import { docTree, knowledgeDocs, type DocNode } from "../fixtures";

const { t } = useI18n();

const selectedId = ref(knowledgeDocs[0]?.id ?? "");
const selected = computed(() =>
  knowledgeDocs.find(doc => doc.id === selectedId.value)
);

const groups = docTree.filter(node => node.children?.length);
const loose = docTree.filter(node => !node.children?.length);

function onPick(node: DocNode) {
  if (knowledgeDocs.some(doc => doc.id === node.id)) {
    selectedId.value = node.id;
  }
}
</script>

<template>
  <div class="ws-page ws-knowledge">
    <PageHeader
      :title="t('workspace.pages.knowledge.title')"
      :subtitle="t('workspace.pages.knowledge.subtitle')"
    />
    <div class="ws-knowledge-body">
      <aside class="ws-card ws-tree" aria-label="doc tree">
        <template v-for="group in groups" :key="group.id">
          <div class="ws-tree-group">
            <Folder :size="14" aria-hidden="true" />
            <span>{{ group.name }}</span>
          </div>
          <button
            v-for="child in group.children"
            :key="child.id"
            type="button"
            class="ws-tree-node ws-focusable"
            :class="{ 'is-selected': selectedId === child.id }"
            data-test="doc-node"
            @click="onPick(child)"
          >
            <FileText :size="14" aria-hidden="true" />
            <span class="ws-tree-name">{{ child.name }}</span>
          </button>
        </template>
        <button
          v-for="node in loose"
          :key="node.id"
          type="button"
          class="ws-tree-node ws-focusable"
          :class="{ 'is-selected': selectedId === node.id }"
          data-test="doc-node"
          @click="onPick(node)"
        >
          <FileText :size="14" aria-hidden="true" />
          <span class="ws-tree-name">{{ node.name }}</span>
        </button>
      </aside>

      <article v-if="selected" class="ws-card ws-reader" data-test="doc-reader">
        <h2 class="ws-reader-title">{{ selected.title }}</h2>
        <div class="ws-reader-meta">
          <span
            >{{ t("workspace.pages.knowledge.version") }}
            {{ selected.version }}</span
          >
          <span
            >{{ t("workspace.pages.knowledge.updated") }}
            {{ selected.updatedAt }}</span
          >
        </div>
        <div class="ws-reader-source">{{ selected.source }}</div>
        <div class="ws-reader-body">
          <p v-for="(para, i) in selected.body" :key="i">{{ para }}</p>
        </div>
      </article>
    </div>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 1080px;
}

.ws-knowledge-body {
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  gap: 12px;
  align-items: start;
}

.ws-tree {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px;
}

.ws-tree-group {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 8px 4px;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-tree-node {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 32px;
  padding: 0 8px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
  cursor: pointer;
  text-align: left;
}

.ws-tree-node:hover {
  background: var(--ws-elevated);
}

/* 紫色克制：选中态用 AI 紫（选中语义） */
.ws-tree-node.is-selected {
  background: var(--ws-elevated);
  color: var(--ws-accent);
  font-weight: 600;
}

.ws-tree-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ws-reader {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
}

.ws-reader-title {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-reader-meta {
  display: flex;
  gap: 12px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-reader-source {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-reader-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ws-reader-body p {
  margin: 0;
  font-size: var(--ws-font-size-body);
  line-height: 1.8;
}
</style>
