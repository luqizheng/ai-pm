<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import PageFrame from "@/components/shell/PageFrame.vue";
import {
  confirmKbRevision,
  decidePcProposal,
  getKbDocument,
  ingestKbDocument,
  listKbDocuments,
  listPcObjects,
  listPcProposals,
  retrievalQuery,
  type IngestKind,
  type KbDocumentDetail,
  type KbDocumentRow,
  type PcObjectRow,
  type PcProposalRow,
  type RetrievalQueryResponse
} from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";
import { useRoute } from "vue-router";
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

type TabKey = "kb" | "pc" | "proposals";
const activeTab = ref<TabKey>("kb");
const loading = ref(false);
const errorText = ref("");

const kbRows = ref<KbDocumentRow[]>([]);
const pcRows = ref<PcObjectRow[]>([]);
const proposalRows = ref<PcProposalRow[]>([]);

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
    const [kb, pc, proposals] = await Promise.all([
      listKbDocuments(projectId),
      listPcObjects(projectId),
      listPcProposals(projectId)
    ]);
    kbRows.value = kb.items;
    pcRows.value = pc.items;
    proposalRows.value = proposals.items;
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

/* ── PC 提案审批（P5；CAS + 治理授权门在后端）────────────────────────── */

const decidingId = ref<string | null>(null);
const decisionNotice = ref("");

async function decide(row: PcProposalRow, action: "approve" | "reject") {
  if (!canLoad.value || decidingId.value) return;
  decidingId.value = row.proposal_id;
  decisionNotice.value = "";
  try {
    await decidePcProposal(row.proposal_id, currentProjectId.value as string, {
      action,
      expected_version_no: row.expected_version_no
    });
    decisionNotice.value = t(
      `s01.knowledge.notice.${action === "approve" ? "approved" : "rejected"}`
    );
    await loadLists();
  } catch {
    decisionNotice.value = t("s01.knowledge.error.decide");
  } finally {
    decidingId.value = null;
  }
}

function proposalTone(status: string): string {
  if (status === "approved" || status === "applied") return "tone-ok";
  if (status === "submitted") return "tone-warn";
  if (status === "rejected" || status === "failed") return "tone-bad";
  return "tone-muted";
}

/* ── 深链（搜索结果 → 自动展开 / 切页签）────────────────────────────── */

const route = useRoute();

async function applyDeepLink() {
  const docId = route.query.doc;
  const objectId = route.query.object;
  if (typeof docId === "string" && docId) {
    activeTab.value = "kb";
    if (!detail.value || detail.value.document.document_id !== docId) {
      const row = kbRows.value.find(r => r.document_id === docId);
      if (row) await openDetail(row);
    }
    return;
  }
  if (typeof objectId === "string" && objectId) {
    activeTab.value = "pc";
  }
}

/* ── 影响分析（遍历可视化；检索响应自带 traversal）───────────────────── */

const impactQuery = ref("");
const impactRunning = ref(false);
const impactResult = ref<RetrievalQueryResponse | null>(null);
const impactError = ref("");

/** pc:{uuid} 节点映射为展示名（来自已加载的 PC 对象列表）；其余截短。 */
function nodeLabel(node: string): string {
  if (node.startsWith("pc:")) {
    const id = node.slice(3);
    const match = pcRows.value.find(o => o.object_id === id);
    if (match) return match.display_name;
  }
  return node.length > 18 ? `${node.slice(0, 15)}…` : node;
}

async function runImpact() {
  if (!canLoad.value || impactRunning.value || !impactQuery.value.trim())
    return;
  impactRunning.value = true;
  impactError.value = "";
  impactResult.value = null;
  try {
    impactResult.value = await retrievalQuery(
      impactQuery.value.trim(),
      currentProjectId.value as string
    );
  } catch {
    impactError.value = t("s01.knowledge.error.load");
  } finally {
    impactRunning.value = false;
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

/* ── 摄入（上传/粘贴 → 解析 → draft；确认走既有详情按钮）────────────── */

const ingestOpen = ref(false);
const ingestDocKey = ref("");
const ingestDocType = ref("knowledge");
const ingestKind = ref<IngestKind>("markdown");
const ingestText = ref("");
const ingestFileName = ref("");
const ingestBase64 = ref("");
const ingestRunning = ref(false);
const ingestNotice = ref("");
const ingestError = ref("");

function pickFile(event: Event) {
  const inputEl = event.target as HTMLInputElement;
  const file = inputEl.files?.[0];
  if (!file) return;
  ingestFileName.value = file.name;
  // doc_key 自动建议：文件名主干（小写、空白/下划线→-）。
  const stem = file.name.replace(/\.[^.]+$/, "");
  ingestDocKey.value =
    ingestDocKey.value || stem.toLowerCase().replace(/[\s_]+/g, "-");
  const ext = (file.name.split(".").pop() ?? "").toLowerCase();
  if (ext === "docx") {
    ingestKind.value = "docx";
  } else if (ext === "txt") {
    ingestKind.value = "text";
  } else {
    ingestKind.value = "markdown";
  }
  const reader = new FileReader();
  reader.onload = () => {
    // dataURL 去前缀即 base64（对 docx 二进制同样安全）。
    const result = String(reader.result ?? "");
    ingestBase64.value = result.slice(result.indexOf(",") + 1);
    ingestText.value = "";
  };
  reader.readAsDataURL(file);
}

function switchToPaste() {
  // 粘贴模式：清文件态，kind 固定 markdown。
  ingestFileName.value = "";
  ingestBase64.value = "";
  ingestKind.value = "markdown";
}

async function runIngest() {
  if (!canLoad.value || ingestRunning.value) return;
  const docKey = ingestDocKey.value.trim();
  if (!docKey) {
    ingestError.value = t("s01.knowledge.error.ingestKey");
    return;
  }
  // 内容来源二选一：文件（base64）或粘贴文本。
  let contentB64 = ingestBase64.value;
  let kind = ingestKind.value;
  if (!contentB64) {
    if (!ingestText.value.trim()) {
      ingestError.value = t("s01.knowledge.error.ingestContent");
      return;
    }
    kind = "markdown";
    contentB64 = btoa(
      String.fromCharCode(...new TextEncoder().encode(ingestText.value))
    );
  }
  ingestRunning.value = true;
  ingestError.value = "";
  ingestNotice.value = "";
  try {
    const result = await ingestKbDocument(currentProjectId.value as string, {
      doc_key: docKey,
      doc_type: ingestDocType.value,
      kind,
      content_b64: contentB64
    });
    ingestNotice.value = t("s01.knowledge.notice.ingested", {
      no: result.revision_no
    });
    // 刷新列表并打开新 draft 详情（确认按钮就在详情里）。
    await loadLists();
    const row = kbRows.value.find(r => r.document_id === result.document_id);
    if (row) await openDetail(row);
  } catch {
    ingestError.value = t("s01.knowledge.error.ingest");
  } finally {
    ingestRunning.value = false;
  }
}

onMounted(async () => {
  await loadLists();
  await applyDeepLink();
});
watch(currentProjectId, async () => {
  closeDetail();
  impactResult.value = null;
  await loadLists();
  // 项目就绪后再放深链（搜索点击先于项目注入到达时，此处兜底）。
  await applyDeepLink();
});
// 同页深链（搜索结果二次点击）也响应。
watch(
  () => route.query,
  () => {
    applyDeepLink();
  }
);
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
        <button
          role="tab"
          :aria-selected="activeTab === 'proposals'"
          class="knowledge-tab"
          :class="{ 'is-active': activeTab === 'proposals' }"
          @click="activeTab = 'proposals'"
        >
          {{ t("s01.knowledge.tab.proposals") }} ({{ proposalRows.length }})
        </button>
      </div>

      <!-- 影响分析（遍历可视化；正式关系多跳） -->
      <details class="impact-panel">
        <summary>{{ t("s01.knowledge.impact.title") }}</summary>
        <div class="impact-body">
          <form class="impact-form" @submit.prevent="runImpact">
            <input
              v-model="impactQuery"
              class="impact-input"
              :placeholder="t('s01.knowledge.impact.placeholder')"
            />
            <button class="kb-confirm" :disabled="impactRunning" type="submit">
              {{
                impactRunning
                  ? t("s01.knowledge.impact.running")
                  : t("s01.knowledge.impact.run")
              }}
            </button>
          </form>
          <p v-if="impactError" class="knowledge-error" role="alert">
            {{ impactError }}
          </p>
          <template v-if="impactResult">
            <p v-if="impactResult.degraded" class="knowledge-hint">
              {{
                t("s01.knowledge.impact.degraded", {
                  reason: impactResult.degradation_reason ?? ""
                })
              }}
            </p>
            <ol
              v-if="impactResult.evidence.traversal.edges.length"
              class="impact-path"
            >
              <li
                v-for="edge in impactResult.evidence.traversal.edges"
                :key="edge.relationship_id + edge.traversal_direction"
              >
                <span class="impact-node">{{ nodeLabel(edge.source_id) }}</span>
                <span class="impact-edge">
                  {{ edge.traversal_direction === "reverse" ? "←" : "→" }}
                  {{ edge.relationship_type }}
                  {{ edge.traversal_direction === "reverse" ? "←" : "→" }}
                </span>
                <span class="impact-node">{{ nodeLabel(edge.target_id) }}</span>
              </li>
            </ol>
            <p v-else class="knowledge-hint">
              {{ t("s01.knowledge.impact.noEdges") }}
            </p>
            <ul
              v-if="
                impactResult.evidence.gaps.length ||
                impactResult.evidence.traversal.gaps.length
              "
              class="impact-gaps"
            >
              <li
                v-for="(gap, gi) in [
                  ...impactResult.evidence.gaps,
                  ...impactResult.evidence.traversal.gaps
                ]"
                :key="gi"
              >
                {{ t("s01.knowledge.impact.gap") }}: {{ gap.kind }} —
                {{ gap.detail }}
              </li>
            </ul>
          </template>
        </div>
      </details>

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

      <!-- 摄入面板（KB 页签）：上传/粘贴 → 解析 → draft（确认在详情里） -->
      <details
        v-if="activeTab === 'kb'"
        class="impact-panel"
        :open="ingestOpen"
      >
        <summary @click.prevent="ingestOpen = !ingestOpen">
          {{ t("s01.knowledge.ingest.title") }}
        </summary>
        <div class="impact-body">
          <div class="ingest-row">
            <label class="ingest-file">
              <input
                type="file"
                accept=".md,.markdown,.txt,.docx"
                @change="pickFile"
              />
              <span>{{
                ingestFileName || t("s01.knowledge.ingest.pickFile")
              }}</span>
            </label>
            <button
              v-if="ingestFileName"
              class="kb-close"
              @click="switchToPaste"
            >
              {{ t("s01.knowledge.ingest.switchPaste") }}
            </button>
          </div>
          <div class="ingest-row">
            <input
              v-model="ingestDocKey"
              class="impact-input"
              :placeholder="t('s01.knowledge.ingest.docKey')"
            />
            <select v-model="ingestDocType" class="impact-input ingest-select">
              <option value="knowledge">
                {{ t("s01.knowledge.ingest.typeKnowledge") }}
              </option>
              <option value="architecture">
                {{ t("s01.knowledge.ingest.typeArchitecture") }}
              </option>
              <option value="decision">
                {{ t("s01.knowledge.ingest.typeDecision") }}
              </option>
              <option value="requirement">
                {{ t("s01.knowledge.ingest.typeRequirement") }}
              </option>
            </select>
          </div>
          <textarea
            v-if="!ingestFileName"
            v-model="ingestText"
            class="impact-input ingest-textarea"
            :placeholder="t('s01.knowledge.ingest.pastePlaceholder')"
            rows="6"
          />
          <div class="ingest-actions">
            <button
              class="kb-confirm ingest-run"
              :disabled="ingestRunning"
              @click="runIngest"
            >
              {{
                ingestRunning
                  ? t("s01.knowledge.ingest.running")
                  : t("s01.knowledge.ingest.run")
              }}
            </button>
          </div>
          <p v-if="ingestNotice" class="kb-notice" role="status">
            {{ ingestNotice }}
          </p>
          <p v-if="ingestError" class="knowledge-error" role="alert">
            {{ ingestError }}
          </p>
          <p class="knowledge-hint">{{ t("s01.knowledge.ingest.hint") }}</p>
        </div>
      </details>

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

      <!-- PC 提案审批 -->
      <ul
        v-if="activeTab === 'proposals' && proposalRows.length"
        class="pc-list"
      >
        <li v-for="p in proposalRows" :key="p.proposal_id" class="pc-row">
          <div class="pc-head">
            <span class="pc-name">{{ p.title }}</span>
            <span class="kb-badge" :class="proposalTone(p.status)">{{
              t(`s01.knowledge.proposalStatus.${p.status}`)
            }}</span>
            <span class="pc-type"
              >v{{ p.expected_version_no }} · {{ p.member_count }}
              {{ t("s01.knowledge.proposal.members") }}</span
            >
          </div>
          <p v-if="decisionNotice" class="kb-notice" role="status">
            {{ decisionNotice }}
          </p>
          <div v-if="p.status === 'submitted'" class="kb-detail-actions">
            <button
              class="kb-confirm"
              :disabled="decidingId === p.proposal_id"
              @click="decide(p, 'approve')"
            >
              {{ t("s01.knowledge.proposal.approve") }}
            </button>
            <button
              class="kb-close"
              :disabled="decidingId === p.proposal_id"
              @click="decide(p, 'reject')"
            >
              {{ t("s01.knowledge.proposal.reject") }}
            </button>
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

.impact-panel {
  border: 1px solid var(--border-subtle, var(--ws-border, #d0d0d0));
  border-radius: 10px;
  padding: 10px 14px;
}

.impact-panel summary {
  cursor: pointer;
  font-size: 13px;
  color: var(--text-secondary, var(--ws-text, #555));
}

.impact-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3, 12px);
  padding-top: var(--space-3, 12px);
}

.impact-form {
  display: flex;
  gap: var(--space-2, 8px);
}

.impact-input {
  flex: 1;
  border: 1px solid var(--border-subtle, var(--ws-border, #d0d0d0));
  border-radius: 8px;
  padding: 6px 10px;
  font-size: 13px;
  background: var(--surface-card, var(--ws-surface, #fff));
  color: var(--text-primary, var(--ws-text, #111));
}

.impact-path {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12px;
}

.impact-node {
  font-weight: 600;
  color: var(--text-primary, var(--ws-text, #111));
}

.impact-edge {
  color: var(--accent-subtle, var(--ws-accent, #2563eb));
  margin: 0 6px;
}

.impact-gaps {
  list-style: none;
  margin: 0;
  padding: 0;
  font-size: 11px;
  color: var(--text-warning, #8a5a00);
}

.ingest-row {
  display: flex;
  gap: var(--space-2, 8px);
  align-items: center;
}

.ingest-file {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  border: 1px dashed var(--border-subtle, var(--ws-border, #d0d0d0));
  border-radius: 8px;
  padding: 6px 12px;
  font-size: 13px;
  cursor: pointer;
  color: var(--text-secondary, var(--ws-text, #555));
}

.ingest-file input[type="file"] {
  display: none;
}

.ingest-select {
  max-width: 160px;
}

.ingest-textarea {
  resize: vertical;
  font-family: inherit;
  min-height: 120px;
}
</style>
