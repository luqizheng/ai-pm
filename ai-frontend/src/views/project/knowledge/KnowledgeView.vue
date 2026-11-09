<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import PageFrame from "@/components/shell/PageFrame.vue";
import {
  confirmKbRevision,
  getKbDocument,
  listKbDocuments,
  listPcObjects,
  type KbDocumentDetail,
  type KbDocumentRow,
  type PcObjectRow
} from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";

/**
 * 知识库（KB 文档 + PC 认知对象）——/project/knowledge 落地页。
 *
 * - KB 页签：文档列表（最新修订投影）+ 详情（正文 / 修订历史 / 草稿确认发布）。
 *   确认门在后端（真人 interactive 会话；AI/API Key 403）——按钮仅是入口。
 * - PC 页签：认知对象列表（生命周期徽章 + 最新修订摘要）。
 */
const { t } = useI18n();
const shell = useShellStore();
const currentProjectId = computed(() => shell.currentProjectId);

type TabKey = "kb" | "pc";
const activeTab = ref<TabKey>("kb");
const loading = ref(false);
const errorText = ref("");

const kbRows = ref<KbDocumentRow[]>([]);
const pcRows = ref<PcObjectRow[]>([]);

/** 展开的文档详情（按 document_id 缓存，避免来回切换重复拉取）。 */
const detail = ref<KbDocumentDetail | null>(null);
const detailLoading = ref(false);
const confirming = ref(false);
const confirmedNotice = ref("");

const canLoad = computed(() => Boolean(currentProjectId.value));

async function loadLists() {
  if (!canLoad.value) return;
  loading.value = true;
  errorText.value = "";
  try {
    const projectId = currentProjectId.value as string;
    const [kb, pc] = await Promise.all([
      listKbDocuments(projectId),
      listPcObjects(projectId)
    ]);
    kbRows.value = kb.items;
    pcRows.value = pc.items;
  } catch {
    errorText.value = t("s01.knowledge.error.load");
  } finally {
    loading.value = false;
  }
}

async function openDetail(row: KbDocumentRow) {
  if (!canLoad.value) return;
  detailLoading.value = true;
  confirmedNotice.value = "";
  try {
    detail.value = await getKbDocument(
      row.document_id,
      currentProjectId.value as string
    );
  } catch {
    errorText.value = t("s01.knowledge.error.load");
  } finally {
    detailLoading.value = false;
  }
}

async function confirmPublish(row: KbDocumentRow) {
  if (!canLoad.value || confirming.value) return;
  confirming.value = true;
  confirmedNotice.value = "";
  try {
    // 确认对象 = 最新**草稿**修订（不是 revisions[0]——那可能是已发布修订）。
    const draft = detail.value?.revisions.find(r => r.status === "draft");
    if (!draft) return;
    await confirmKbRevision(
      row.document_id,
      draft.revision_id,
      currentProjectId.value as string
    );
    confirmedNotice.value = t("s01.knowledge.notice.confirmed");
    // 确认后刷新列表与详情（状态徽章 / 确认人即时可见）。
    await loadLists();
    if (detail.value) {
      detail.value = await getKbDocument(
        row.document_id,
        currentProjectId.value as string
      );
    }
  } catch {
    confirmedNotice.value = t("s01.knowledge.error.confirm");
  } finally {
    confirming.value = false;
  }
}

function closeDetail() {
  detail.value = null;
  confirmedNotice.value = "";
}

/** 状态徽章语义色（token 语义面；rejected 也可见——生命周期语义保留）。 */
function statusTone(status: string): string {
  if (status === "published") return "tone-ok";
  if (status === "draft") return "tone-warn";
  if (status === "rejected") return "tone-bad";
  return "tone-muted";
}

/** 生命周期徽章（PC）。 */
function lifecycleTone(status: string): string {
  if (status === "active") return "tone-ok";
  if (status === "draft" || status === "identity") return "tone-warn";
  if (status === "deprecated" || status === "superseded") return "tone-muted";
  return "tone-muted";
}

function formatTime(value: string | null): string {
  if (!value) return "—";
  return value.slice(0, 16).replace("T", " ");
}

const latestDraft = computed(() => {
  if (!detail.value) return null;
  return detail.value.revisions.find(r => r.status === "draft") ?? null;
});

onMounted(loadLists);
watch(currentProjectId, () => {
  closeDetail();
  loadLists();
});
</script>

<template>
  <PageFrame
    :title="t('s01.knowledge.page.title')"
    :description="t('s01.knowledge.page.description')"
  >
    <div class="knowledge-page">
      <div class="knowledge-tabs" role="tablist">
        <button
          role="tab"
          :aria-selected="activeTab === 'kb'"
          class="knowledge-tab"
          :class="{ 'is-active': activeTab === 'kb' }"
          @click="activeTab = 'kb'"
        >
          {{ t("s01.knowledge.tab.kb") }} ({{ kbRows.length }})
        </button>
        <button
          role="tab"
          :aria-selected="activeTab === 'pc'"
          class="knowledge-tab"
          :class="{ 'is-active': activeTab === 'pc' }"
          @click="activeTab = 'pc'"
        >
          {{ t("s01.knowledge.tab.pc") }} ({{ pcRows.length }})
        </button>
      </div>

      <p v-if="errorText" class="knowledge-error" role="alert">
        {{ errorText }}
      </p>
      <p v-if="loading" class="knowledge-hint">
        {{ t("s01.knowledge.hint.loading") }}
      </p>
      <p
        v-else-if="!loading && !kbRows.length && activeTab === 'kb'"
        class="knowledge-hint"
      >
        {{ t("s01.knowledge.hint.emptyKb") }}
      </p>
      <p
        v-else-if="!loading && !pcRows.length && activeTab === 'pc'"
        class="knowledge-hint"
      >
        {{ t("s01.knowledge.hint.emptyPc") }}
      </p>

      <!-- KB 文档列表 -->
      <ul v-if="activeTab === 'kb' && kbRows.length" class="kb-list">
        <li
          v-for="row in kbRows"
          :key="row.document_id"
          class="kb-row"
          :class="{
            'is-open': detail?.document.document_id === row.document_id
          }"
        >
          <button class="kb-row-head" @click="openDetail(row)">
            <span class="kb-doc-key">{{ row.doc_key }}</span>
            <span class="kb-badge" :class="statusTone(row.status)">{{
              t(`s01.knowledge.status.${row.status}`)
            }}</span>
            <span class="kb-meta">
              {{ t("s01.knowledge.field.revision") }} {{ row.revision_no }} ·
              {{ formatTime(row.published_at ?? row.updated_at) }}
            </span>
          </button>

          <div
            v-if="detail?.document.document_id === row.document_id"
            class="kb-detail"
          >
            <p v-if="detailLoading" class="knowledge-hint">
              {{ t("s01.knowledge.hint.loading") }}
            </p>
            <template v-else>
              <p v-if="confirmedNotice" class="kb-notice" role="status">
                {{ confirmedNotice }}
              </p>
              <div class="kb-detail-actions">
                <button
                  v-if="latestDraft"
                  class="kb-confirm"
                  :disabled="confirming"
                  @click="confirmPublish(row)"
                >
                  {{
                    confirming
                      ? t("s01.knowledge.action.confirming")
                      : t("s01.knowledge.action.confirm")
                  }}
                </button>
                <button class="kb-close" @click="closeDetail">
                  {{ t("s01.knowledge.action.close") }}
                </button>
              </div>

              <details class="kb-revisions" open>
                <summary>
                  {{ t("s01.knowledge.field.revisions") }} ({{
                    detail.revisions.length
                  }})
                </summary>
                <ul>
                  <li v-for="rev in detail.revisions" :key="rev.revision_id">
                    <span class="kb-badge" :class="statusTone(rev.status)">{{
                      t(`s01.knowledge.status.${rev.status}`)
                    }}</span>
                    {{ t("s01.knowledge.field.revision") }}
                    {{ rev.revision_no }} ·
                    {{ formatTime(rev.published_at ?? rev.created_at) }}
                  </li>
                </ul>
              </details>

              <details class="kb-content" open>
                <summary>{{ t("s01.knowledge.field.content") }}</summary>
                <MarkdownViewer :model-value="detail.content ?? ''" />
              </details>
            </template>
          </div>
        </li>
      </ul>

      <!-- PC 对象列表 -->
      <ul v-if="activeTab === 'pc' && pcRows.length" class="pc-list">
        <li v-for="obj in pcRows" :key="obj.object_id" class="pc-row">
          <div class="pc-head">
            <span class="pc-name">{{ obj.display_name }}</span>
            <span class="pc-type">{{ obj.object_type }}</span>
            <span
              class="kb-badge"
              :class="lifecycleTone(obj.lifecycle_status)"
              >{{ t(`s01.knowledge.lifecycle.${obj.lifecycle_status}`) }}</span
            >
          </div>
          <p v-if="obj.change_summary" class="pc-summary">
            {{ obj.change_summary }}
          </p>
          <p v-if="obj.excerpt" class="pc-excerpt">{{ obj.excerpt }}</p>
        </li>
      </ul>
    </div>
  </PageFrame>
</template>

<style scoped>
.knowledge-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-4, 16px);
  max-width: 860px;
}

.knowledge-tabs {
  display: flex;
  gap: var(--space-2, 8px);
}

.knowledge-tab {
  border: 1px solid var(--border-subtle, var(--ws-border, #d0d0d0));
  background: transparent;
  color: var(--text-secondary, var(--ws-text, #333));
  border-radius: 999px;
  padding: 6px 14px;
  cursor: pointer;
  font-size: 13px;
}

.knowledge-tab.is-active {
  background: var(--accent-subtle, var(--ws-accent, #2563eb));
  border-color: transparent;
  color: var(--text-on-accent, #fff);
}

.knowledge-hint,
.knowledge-error {
  font-size: 13px;
  color: var(--text-secondary, var(--ws-text, #333));
}

.knowledge-error {
  color: var(--text-danger, #b42318);
}

.kb-list,
.pc-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2, 8px);
}

.kb-row,
.pc-row {
  border: 1px solid var(--border-subtle, var(--ws-border, #d0d0d0));
  border-radius: 10px;
  background: var(--surface-card, var(--ws-surface, #fff));
}

.kb-row.is-open {
  border-color: var(--accent-subtle, var(--ws-accent, #2563eb));
}

.kb-row-head {
  display: flex;
  align-items: center;
  gap: var(--space-3, 12px);
  width: 100%;
  padding: 10px 14px;
  background: transparent;
  border: 0;
  cursor: pointer;
  text-align: left;
}

.kb-doc-key {
  font-weight: 600;
  font-size: 13px;
  color: var(--text-primary, var(--ws-text, #111));
}

.kb-meta {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-tertiary, var(--ws-text, #777));
}

.kb-badge {
  font-size: 11px;
  border-radius: 999px;
  padding: 2px 8px;
  border: 1px solid transparent;
}

.tone-ok {
  background: var(--surface-success-subtle, #e7f5ec);
  color: var(--text-success, #1b7a43);
}

.tone-warn {
  background: var(--surface-warning-subtle, #fdf3e0);
  color: var(--text-warning, #8a5a00);
}

.tone-bad {
  background: var(--surface-danger-subtle, #fdeceb);
  color: var(--text-danger, #b42318);
}

.tone-muted {
  background: var(--surface-muted, #f2f2f2);
  color: var(--text-secondary, #555);
}

.kb-detail {
  border-top: 1px solid var(--border-subtle, var(--ws-border, #e5e5e5));
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: var(--space-3, 12px);
}

.kb-detail-actions {
  display: flex;
  gap: var(--space-2, 8px);
}

.kb-confirm {
  background: var(--accent-subtle, var(--ws-accent, #2563eb));
  color: var(--text-on-accent, #fff);
  border: 0;
  border-radius: 8px;
  padding: 6px 14px;
  font-size: 13px;
  cursor: pointer;
}

.kb-confirm:disabled {
  opacity: 0.6;
  cursor: default;
}

.kb-close {
  background: transparent;
  border: 1px solid var(--border-subtle, var(--ws-border, #d0d0d0));
  border-radius: 8px;
  padding: 6px 14px;
  font-size: 13px;
  cursor: pointer;
  color: var(--text-secondary, var(--ws-text, #333));
}

.kb-notice {
  font-size: 12px;
  color: var(--text-success, #1b7a43);
}

.kb-revisions ul {
  list-style: none;
  margin: 6px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--text-secondary, var(--ws-text, #555));
}

.kb-revisions summary,
.kb-content summary {
  cursor: pointer;
  font-size: 12px;
  color: var(--text-secondary, var(--ws-text, #555));
}

.pc-row {
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.pc-head {
  display: flex;
  align-items: center;
  gap: var(--space-2, 8px);
}

.pc-name {
  font-weight: 600;
  font-size: 13px;
}

.pc-type {
  font-size: 11px;
  color: var(--text-tertiary, #777);
}

.pc-summary,
.pc-excerpt {
  margin: 0;
  font-size: 12px;
  color: var(--text-secondary, var(--ws-text, #555));
}

.pc-excerpt {
  color: var(--text-tertiary, #777);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
