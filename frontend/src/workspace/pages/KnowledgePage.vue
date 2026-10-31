<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { FileText, Folder, Link2 } from "lucide-vue-next";
import PageHeader from "./PageHeader.vue";
import { docTree, knowledgeDocs } from "../fixtures";
import {
  getDocument,
  listDocuments,
  type KnowledgeDocument,
  type KnowledgeDocumentSummary
} from "@/api/documents";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import { useRoute } from "vue-router";

const { t } = useI18n();
/** mock 模式继续用演示数据；真后端模式按项目读取 Git 知识文档。单测环境（MODE=test）一律走 fixtures。 */
const useMock =
  import.meta.env.VITE_USE_MOCK === "1" || import.meta.env.MODE === "test";
const scope = useScopeStore();
const route = useRoute();

/**
 * 项目来源：显式 scope 选择 > 路由里是真实 UUID 时用它；都没有时提示先选项目。
 * workspace 路由参数可能是演示 id（proj-worldmap），不能直接当 project_id。
 */
const projectId = computed<string | undefined>(() => {
  if (scope.projectIdParam) return scope.projectIdParam;
  const param = String(route.params.projectId ?? "");
  if (/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(param)) {
    return param;
  }
  return undefined;
});

/** 真后端模式下用页面内项目选择器（workspace 壳没有全局 ScopeSelect）。 */
const selectedProjectId = computed({
  get: () =>
    scope.currentProjectId === SCOPE_ALL ? "" : scope.currentProjectId,
  set: (value: string) => {
    scope.currentProjectId = value || SCOPE_ALL;
  }
});

interface DisplayDoc {
  id: string;
  title: string;
  version: number;
  updatedAt: string;
  source: string;
  body: string[];
  review: "approved" | "draft";
  approvedFacts: string[];
  aiSummary: string;
  references: { kind: string; title: string; id: string }[];
}

interface TreeNode {
  id: string;
  name: string;
  children?: TreeNode[];
}

const selectedId = ref(useMock ? (knowledgeDocs[0]?.id ?? "") : "");
const summaries = ref<KnowledgeDocumentSummary[]>([]);
const detail = ref<KnowledgeDocument | null>(null);
const failed = ref(false);
const loading = ref(false);

const typeLabels = computed<Record<string, string>>(() => ({
  requirement: t("workspace.pages.knowledge.types.requirement"),
  decision: t("workspace.pages.knowledge.types.decision"),
  architecture: t("workspace.pages.knowledge.types.architecture"),
  knowledge: t("workspace.pages.knowledge.types.knowledge")
}));

function markdownToParagraphs(markdown: string): string[] {
  return markdown
    .split(/\n{2,}/)
    .map(block => block.trim())
    .filter(Boolean);
}

function toDisplay(doc: KnowledgeDocument): DisplayDoc {
  return {
    id: doc.document_id,
    title: doc.doc_key,
    version: doc.revision_no,
    updatedAt: "",
    source: doc.canonical_path,
    body: markdownToParagraphs(doc.markdown),
    review: "approved",
    approvedFacts: [],
    aiSummary: "",
    references: []
  };
}

function summaryToDisplay(doc: KnowledgeDocumentSummary): DisplayDoc {
  return {
    id: doc.id,
    title: doc.doc_key,
    version: doc.revision_no,
    updatedAt: doc.updated_at,
    source: doc.canonical_path,
    body: [],
    review: "approved",
    approvedFacts: [],
    aiSummary: "",
    references: []
  };
}

const mockDocs = knowledgeDocs as unknown as DisplayDoc[];
const docs = computed<DisplayDoc[]>(() =>
  useMock ? mockDocs : summaries.value.map(summaryToDisplay)
);

const selected = computed<DisplayDoc | null>(() => {
  if (useMock) {
    return (knowledgeDocs.find(doc => doc.id === selectedId.value) ??
      null) as unknown as DisplayDoc | null;
  }
  if (detail.value && detail.value.document_id === selectedId.value) {
    return toDisplay(detail.value);
  }
  return docs.value.find(doc => doc.id === selectedId.value) ?? null;
});

const groups = computed<TreeNode[]>(() => {
  if (useMock) return docTree.filter(node => node.children?.length) as TreeNode[];
  const byType = new Map<string, TreeNode[]>();
  for (const doc of summaries.value) {
    const children = byType.get(doc.doc_type) ?? [];
    children.push({ id: doc.id, name: doc.doc_key });
    byType.set(doc.doc_type, children);
  }
  return [...byType.entries()].map(([type, children]) => ({
    id: type,
    name: typeLabels.value[type] ?? type,
    children
  }));
});

const loose = computed<TreeNode[]>(() =>
  useMock ? (docTree.filter(node => !node.children?.length) as TreeNode[]) : []
);

async function loadDetail(id: string) {
  if (useMock || !projectId.value) return;
  try {
    detail.value = await getDocument(id, projectId.value);
  } catch {
    detail.value = null;
    failed.value = true;
  }
}

async function loadList() {
  if (useMock) return;
  failed.value = false;
  detail.value = null;
  if (!projectId.value) {
    summaries.value = [];
    selectedId.value = "";
    return;
  }
  loading.value = true;
  try {
    const result = await listDocuments(projectId.value);
    summaries.value = result.items ?? [];
    selectedId.value = summaries.value[0]?.id ?? "";
    if (selectedId.value) await loadDetail(selectedId.value);
  } catch {
    failed.value = true;
    summaries.value = [];
    selectedId.value = "";
  } finally {
    loading.value = false;
  }
}

function onPick(node: TreeNode) {
  selectedId.value = node.id;
  if (!useMock) void loadDetail(node.id);
}

let initialised = false;

onMounted(async () => {
  if (useMock) {
    selectedId.value = knowledgeDocs[0]?.id ?? "";
    return;
  }
  try {
    await scope.ensureLoaded();
  } catch {
    failed.value = true;
    return;
  }
  initialised = true;
  await loadList();
});

watch(projectId, () => {
  // 首轮由 onMounted 统一加载；这里只处理用户切换项目。
  if (!useMock && initialised) void loadList();
});

const refLabel: Record<string, string> = {
  decision: "workspace.pages.decisions.title",
  requirement: "workspace.pages.requirements.title",
  artifact: "workspace.pages.artifacts.title"
};
</script>

<template>
  <div class="ws-page ws-knowledge">
    <PageHeader
      :title="t('workspace.pages.knowledge.title')"
      :subtitle="t('workspace.pages.knowledge.subtitle')"
    />
    <div v-if="!useMock" class="ws-knowledge-toolbar">
      <label class="ws-knowledge-label" for="knowledge-project">
        {{ t("workspace.pages.knowledge.project") }}
      </label>
      <select
        id="knowledge-project"
        v-model="selectedProjectId"
        class="ws-knowledge-select"
      >
        <option value="">
          {{ t("workspace.pages.knowledge.pickProjectShort") }}
        </option>
        <option v-for="p in scope.projects" :key="p.id" :value="p.id">
          {{ p.name }}
        </option>
      </select>
    </div>
    <!-- V1.2 三层：知识树 | Markdown 阅读区 | 证据与引用关系 -->
    <div class="ws-knowledge-body">
      <aside class="ws-card ws-tree" aria-label="doc tree">
        <p v-if="!useMock && !projectId" class="ws-tree-hint">
          {{ t("workspace.pages.knowledge.pickProject") }}
        </p>
        <p v-else-if="failed" class="ws-tree-hint">
          {{ t("workspace.pages.knowledge.loadFailed") }}
        </p>
        <p v-else-if="!useMock && !loading && !docs.length" class="ws-tree-hint">
          {{ t("workspace.pages.knowledge.empty") }}
        </p>
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
        <div class="ws-reader-head">
          <h2 class="ws-reader-title">{{ selected.title }}</h2>
          <span
            class="ws-badge"
            :class="
              selected.review === 'approved'
                ? 'is-review-approved'
                : 'is-review-draft'
            "
            data-test="doc-review"
          >
            {{
              t(
                selected.review === "approved"
                  ? "workspace.pages.knowledgeExtra.reviewApproved"
                  : "workspace.pages.knowledgeExtra.reviewAiDraft"
              )
            }}
          </span>
        </div>
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
        <!-- V1.2：人工批准的正式事实（AI 摘要不得覆盖） -->
        <div
          v-if="selected.approvedFacts.length"
          class="ws-facts"
          data-test="doc-approved-facts"
        >
          <h3 class="ws-reader-label">
            {{ t("workspace.pages.knowledgeExtra.approvedFacts") }}
          </h3>
          <ul class="ws-facts-list">
            <li v-for="(fact, i) in selected.approvedFacts" :key="i">
              {{ fact }}
            </li>
          </ul>
        </div>
        <div v-if="selected.aiSummary" class="ws-ai-summary" data-test="doc-ai-summary">
          <h3 class="ws-reader-label">
            {{ t("workspace.pages.knowledgeExtra.aiSummary") }}
          </h3>
          <p>{{ selected.aiSummary }}</p>
        </div>
      </article>

      <aside
        v-if="selected"
        class="ws-card ws-evidence"
        aria-label="evidence"
        data-test="doc-evidence"
      >
        <h3 class="ws-reader-label">
          <Link2 :size="13" aria-hidden="true" />
          {{ t("workspace.pages.knowledgeExtra.references") }}
        </h3>
        <p v-if="!selected.references.length" class="ws-tree-hint">
          {{ t("workspace.pages.knowledge.referencesPending") }}
        </p>
        <div
          v-for="(ref, i) in selected.references"
          :key="i"
          class="ws-ref"
          data-test="doc-reference"
        >
          <span class="ws-ref-kind">{{ t(refLabel[ref.kind]) }}</span>
          <span class="ws-ref-title">{{ ref.title }}</span>
          <span class="ws-ref-id">{{ ref.id }}</span>
        </div>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 1200px;
}

.ws-knowledge-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-knowledge-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-knowledge-select {
  min-height: 32px;
  padding: 0 8px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
}

.ws-knowledge-body {
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr) 240px;
  gap: 12px;
  align-items: start;
}

.ws-tree,
.ws-evidence {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px;
}

.ws-tree-hint {
  margin: 0;
  padding: 6px 8px;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
  color: var(--ws-text-secondary);
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

.ws-reader-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.ws-reader-title {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 紫色克制：审核状态用语义色 */
.ws-badge.is-review-approved {
  color: var(--ws-color-success-text);
}

.ws-badge.is-review-draft {
  color: var(--ws-color-warning-text);
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

.ws-reader-label {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 4px;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

/* 人工批准的正式事实：与 AI 摘要视觉分离 */
.ws-facts {
  padding: 10px 12px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-facts-list {
  margin: 0;
  padding-left: 16px;
  font-size: var(--ws-font-size-caption);
  line-height: 1.7;
}

.ws-ai-summary {
  padding: 10px 12px;
  border: 1px dashed var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ws-ai-summary p {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  line-height: 1.7;
  color: var(--ws-text-secondary);
}

/* 证据与引用关系（右层） */
.ws-evidence {
  gap: 8px;
}

.ws-ref {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-ref-kind {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-ref-title {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
}

.ws-ref-id {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}
</style>
