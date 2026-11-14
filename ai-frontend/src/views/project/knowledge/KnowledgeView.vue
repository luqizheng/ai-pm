<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronRight } from "lucide-vue-next";
import PageFrame from "@/components/shell/PageFrame.vue";
import {
  confirmKbRevision,
  decidePcProposal,
  getKbDocument,
  getPcObject,
  getPcProposal,
  ingestKbDocument,
  listKbDocuments,
  listPcObjects,
  listPcProposals,
  retrievalQuery,
  type IngestKind,
  type KbDocumentDetail,
  type KbDocumentRow,
  type PcObjectDetail,
  type PcObjectRow,
  type PcProposalDetail,
  type PcProposalMemberView,
  type PcProposalRow,
  type RetrievalQueryResponse
} from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";
import { useRoute } from "vue-router";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import Button from "@/components/ui/button/Button.vue";
import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Input from "@/components/ui/input/Input.vue";
import Select from "@/components/ui/select/Select.vue";
import Tabs from "@/components/ui/tabs/Tabs.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

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

/* ── PC 对象/提案详情（行内展开；按 id 缓存，收起保留缓存）──────────────── */

/** PC 行展开态 + 详情缓存（收起只切展开态；缓存保留——再展不重拉）。 */
const pcOpen = ref<Record<string, boolean>>({});
const pcDetail = ref<Record<string, PcObjectDetail>>({});
const pcDetailLoading = ref<string | null>(null);

/** 提案行展开态 + 详情缓存（同上）。 */
const proposalOpen = ref<Record<string, boolean>>({});
const proposalDetail = ref<Record<string, PcProposalDetail>>({});
const proposalDetailLoading = ref<string | null>(null);

async function openPcDetail(obj: PcObjectRow) {
  if (!canLoad.value || pcDetail.value[obj.object_id]) return;
  pcDetailLoading.value = obj.object_id;
  try {
    pcDetail.value[obj.object_id] = await getPcObject(
      obj.object_id,
      currentProjectId.value as string
    );
  } catch {
    errorText.value = t("s01.knowledge.error.load");
  } finally {
    pcDetailLoading.value = null;
  }
}

/** 行头即开关（与 KB 行一致）：已展开则收起，否则展开并按需拉详情。 */
function togglePcRow(obj: PcObjectRow) {
  pcOpen.value[obj.object_id] = !pcOpen.value[obj.object_id];
  if (pcOpen.value[obj.object_id]) openPcDetail(obj);
}

/** 展开态取最新修订（identity 对象无修订 → null）。 */
function pcRev(obj: PcObjectRow) {
  return pcDetail.value[obj.object_id]?.latest_revision ?? null;
}

async function openProposalDetail(p: PcProposalRow) {
  if (!canLoad.value || proposalDetail.value[p.proposal_id]) return;
  proposalDetailLoading.value = p.proposal_id;
  try {
    proposalDetail.value[p.proposal_id] = await getPcProposal(
      p.proposal_id,
      currentProjectId.value as string
    );
  } catch {
    errorText.value = t("s01.knowledge.error.load");
  } finally {
    proposalDetailLoading.value = null;
  }
}

function toggleProposalRow(p: PcProposalRow) {
  proposalOpen.value[p.proposal_id] = !proposalOpen.value[p.proposal_id];
  if (proposalOpen.value[p.proposal_id]) openProposalDetail(p);
}

/** 成员变更行文本：对象变更带类型/名/修订；关系变更带类型与修订。 */
function memberLabel(m: PcProposalMemberView): string {
  const parts: string[] = [m.change_kind];
  if (m.object_id) {
    parts.push(m.object_type ?? "", m.display_name ?? m.object_key ?? "");
    if (m.object_revision_no != null) parts.push(`v${m.object_revision_no}`);
  } else if (m.relationship_id) {
    parts.push(m.relationship_type ?? "");
    if (m.relationship_revision_no != null)
      parts.push(`v${m.relationship_revision_no}`);
  }
  return parts.filter(Boolean).join(" · ");
}

/** 审批状态文案（pending/approved/rejected；未知原样回显）。 */
function approvalLabel(status: string): string {
  const known = ["pending", "approved", "rejected"];
  return known.includes(status)
    ? t(`s01.knowledge.approvalStatus.${status}`)
    : status;
}

type BadgeTone = "success" | "warning" | "danger" | "neutral";

function proposalVariant(status: string): BadgeTone {
  if (status === "approved" || status === "applied") return "success";
  if (status === "submitted") return "warning";
  if (status === "rejected" || status === "failed") return "danger";
  return "neutral";
}

/** 页签项（Tabs 组件契约；计数随列表刷新）。 */
const tabItems = computed(() => [
  {
    value: "kb",
    label: `${t("s01.knowledge.tab.kb")} (${kbRows.value.length})`
  },
  {
    value: "pc",
    label: `${t("s01.knowledge.tab.pc")} (${pcRows.value.length})`
  },
  {
    value: "proposals",
    label: `${t("s01.knowledge.tab.proposals")} (${proposalRows.value.length})`
  }
]);

/** 摄入文档类型（Select 组件契约）。 */
const docTypeOptions = computed(() => [
  { value: "knowledge", label: t("s01.knowledge.ingest.typeKnowledge") },
  { value: "architecture", label: t("s01.knowledge.ingest.typeArchitecture") },
  { value: "decision", label: t("s01.knowledge.ingest.typeDecision") },
  { value: "requirement", label: t("s01.knowledge.ingest.typeRequirement") }
]);

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
    const row = pcRows.value.find(o => o.object_id === objectId);
    if (row) {
      pcOpen.value[objectId] = true;
      await openPcDetail(row);
    }
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

/** 行头即开关：已展开则收起，否则展开（箭头随态旋转）。 */
function toggleRow(row: KbDocumentRow) {
  if (detail.value?.document.document_id === row.document_id) {
    closeDetail();
  } else {
    openDetail(row);
  }
}

/** 状态徽章语义（rejected 也可见——生命周期语义保留）。 */
function statusVariant(status: string): BadgeTone {
  if (status === "published") return "success";
  if (status === "draft") return "warning";
  if (status === "rejected") return "danger";
  return "neutral";
}

/** 生命周期徽章（PC）。 */
function lifecycleVariant(status: string): BadgeTone {
  if (status === "active") return "success";
  if (status === "draft" || status === "identity") return "warning";
  return "neutral";
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
  pcOpen.value = {};
  pcDetail.value = {};
  proposalOpen.value = {};
  proposalDetail.value = {};
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
      <!-- Tabs 仅作页签触发行；内容块跨页签共享（影响分析等），按 activeTab 条件渲染。 -->
      <Tabs v-model="activeTab" class="knowledge-tabs" :tabs="tabItems" />

      <!-- 影响分析（遍历可视化；正式关系多跳） -->
      <details class="impact-panel">
        <summary>{{ t("s01.knowledge.impact.title") }}</summary>
        <div class="impact-body">
          <form class="impact-form" @submit.prevent="runImpact">
            <Input
              v-model="impactQuery"
              class="impact-input"
              :placeholder="t('s01.knowledge.impact.placeholder')"
            />
            <Button type="submit" :loading="impactRunning">
              {{
                impactRunning
                  ? t("s01.knowledge.impact.running")
                  : t("s01.knowledge.impact.run")
              }}
            </Button>
          </form>
          <Alert v-if="impactError" variant="danger" size="sm" role="alert">
            {{ impactError }}
          </Alert>
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

      <Alert v-if="errorText" variant="danger" size="sm" role="alert">
        {{ errorText }}
      </Alert>
      <p v-if="loading" class="knowledge-hint">
        {{ t("s01.knowledge.hint.loading") }}
      </p>
      <EmptyState
        v-else-if="!loading && !kbRows.length && activeTab === 'kb'"
        size="sm"
        :title="t('s01.knowledge.hint.emptyKb')"
      />
      <EmptyState
        v-else-if="!loading && !pcRows.length && activeTab === 'pc'"
        size="sm"
        :title="t('s01.knowledge.hint.emptyPc')"
      />

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
            <Button
              v-if="ingestFileName"
              variant="secondary"
              size="sm"
              @click="switchToPaste"
            >
              {{ t("s01.knowledge.ingest.switchPaste") }}
            </Button>
          </div>
          <div class="ingest-row">
            <Input
              v-model="ingestDocKey"
              class="impact-input"
              :placeholder="t('s01.knowledge.ingest.docKey')"
            />
            <!-- SelectRoot 无 DOM：max-width 落在包裹层。 -->
            <div class="ingest-select">
              <Select v-model="ingestDocType" :options="docTypeOptions" />
            </div>
          </div>
          <Textarea
            v-if="!ingestFileName"
            v-model="ingestText"
            :placeholder="t('s01.knowledge.ingest.pastePlaceholder')"
            :rows="6"
          />
          <div class="ingest-actions">
            <Button
              class="ingest-run"
              :loading="ingestRunning"
              @click="runIngest"
            >
              {{
                ingestRunning
                  ? t("s01.knowledge.ingest.running")
                  : t("s01.knowledge.ingest.run")
              }}
            </Button>
          </div>
          <Alert v-if="ingestNotice" variant="success" size="sm" role="status">
            {{ ingestNotice }}
          </Alert>
          <Alert v-if="ingestError" variant="danger" size="sm" role="alert">
            {{ ingestError }}
          </Alert>
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
          <button
            class="kb-row-head"
            :aria-expanded="detail?.document.document_id === row.document_id"
            @click="toggleRow(row)"
          >
            <span class="kb-doc-key">{{ row.doc_key }}</span>
            <Badge :variant="statusVariant(row.status)">{{
              t(`s01.knowledge.status.${row.status}`)
            }}</Badge>
            <span class="kb-meta">
              {{ t("s01.knowledge.field.revision") }} {{ row.revision_no }} ·
              {{ formatTime(row.published_at ?? row.updated_at) }}
            </span>
            <ChevronRight
              class="kb-row-chevron"
              :class="{
                'is-open': detail?.document.document_id === row.document_id
              }"
              aria-hidden="true"
            />
          </button>

          <div
            v-if="detail?.document.document_id === row.document_id"
            class="kb-detail"
          >
            <p v-if="detailLoading" class="knowledge-hint">
              {{ t("s01.knowledge.hint.loading") }}
            </p>
            <template v-else>
              <Alert
                v-if="confirmedNotice"
                variant="success"
                size="sm"
                role="status"
              >
                {{ confirmedNotice }}
              </Alert>
              <div class="kb-detail-actions">
                <Button
                  v-if="latestDraft"
                  class="kb-confirm"
                  :loading="confirming"
                  @click="confirmPublish(row)"
                >
                  {{
                    confirming
                      ? t("s01.knowledge.action.confirming")
                      : t("s01.knowledge.action.confirm")
                  }}
                </Button>
              </div>

              <details class="kb-revisions" open>
                <summary>
                  {{ t("s01.knowledge.field.revisions") }} ({{
                    detail.revisions.length
                  }})
                </summary>
                <ul>
                  <li v-for="rev in detail.revisions" :key="rev.revision_id">
                    <Badge :variant="statusVariant(rev.status)">{{
                      t(`s01.knowledge.status.${rev.status}`)
                    }}</Badge>
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

      <!-- PC 提案审批（行内展开详情：理由/成员变更/审批历史） -->
      <ul
        v-if="activeTab === 'proposals' && proposalRows.length"
        class="pc-list"
      >
        <li
          v-for="p in proposalRows"
          :key="p.proposal_id"
          class="pc-row"
          :class="{ 'is-open': Boolean(proposalOpen[p.proposal_id]) }"
        >
          <button
            class="pc-row-head"
            :aria-expanded="Boolean(proposalOpen[p.proposal_id])"
            @click="toggleProposalRow(p)"
          >
            <span class="pc-name">{{ p.title }}</span>
            <Badge :variant="proposalVariant(p.status)">{{
              t(`s01.knowledge.proposalStatus.${p.status}`)
            }}</Badge>
            <span class="pc-type"
              >v{{ p.expected_version_no }} · {{ p.member_count }}
              {{ t("s01.knowledge.proposal.members") }}</span
            >
            <ChevronRight
              class="kb-row-chevron"
              :class="{ 'is-open': Boolean(proposalOpen[p.proposal_id]) }"
              aria-hidden="true"
            />
          </button>
          <Alert
            v-if="decisionNotice"
            variant="success"
            size="sm"
            role="status"
          >
            {{ decisionNotice }}
          </Alert>
          <div v-if="proposalOpen[p.proposal_id]" class="kb-detail">
            <p
              v-if="
                proposalDetailLoading === p.proposal_id &&
                !proposalDetail[p.proposal_id]
              "
              class="knowledge-hint"
            >
              {{ t("s01.knowledge.hint.loading") }}
            </p>
            <template v-else-if="proposalDetail[p.proposal_id]">
              <p
                v-if="proposalDetail[p.proposal_id]?.rationale"
                class="pc-summary"
              >
                {{ proposalDetail[p.proposal_id]?.rationale }}
              </p>
              <div class="pc-detail-meta">
                <span class="pc-type">
                  {{ t("s01.knowledge.proposal.governance") }}:
                  {{ proposalDetail[p.proposal_id]?.governance_level }}
                </span>
                <span class="pc-type">
                  {{ t("s01.knowledge.proposal.risk") }}:
                  {{ proposalDetail[p.proposal_id]?.risk_level }}
                </span>
                <span class="pc-type">
                  {{ t("s01.knowledge.proposal.confidence") }}:
                  {{ proposalDetail[p.proposal_id]?.confidence }}
                </span>
                <span class="pc-type">
                  {{ t("s01.knowledge.proposal.origin") }}:
                  {{ proposalDetail[p.proposal_id]?.origin_kind }}
                </span>
              </div>
              <details class="kb-revisions" open>
                <summary>
                  {{ t("s01.knowledge.proposal.membersList") }} ({{
                    proposalDetail[p.proposal_id]?.members.length ?? 0
                  }})
                </summary>
                <ul>
                  <li
                    v-for="m in proposalDetail[p.proposal_id]?.members ?? []"
                    :key="m.object_id ?? m.relationship_id ?? m.change_kind"
                  >
                    {{ memberLabel(m) }}
                  </li>
                </ul>
              </details>
              <details class="kb-revisions" open>
                <summary>
                  {{ t("s01.knowledge.proposal.approvals") }} ({{
                    proposalDetail[p.proposal_id]?.approvals.length ?? 0
                  }})
                </summary>
                <p
                  v-if="
                    !(proposalDetail[p.proposal_id]?.approvals ?? []).length
                  "
                  class="knowledge-hint"
                >
                  {{ t("s01.knowledge.proposal.noApprovals") }}
                </p>
                <ul v-else>
                  <li
                    v-for="a in proposalDetail[p.proposal_id]?.approvals ?? []"
                    :key="a.approval_id"
                  >
                    <Badge :variant="proposalVariant(a.status)">{{
                      approvalLabel(a.status)
                    }}</Badge>
                    <span v-if="a.decided_by_name">
                      · {{ a.decided_by_name }}</span
                    >
                    <span v-if="a.decision_reason">
                      · {{ a.decision_reason }}</span
                    >
                    <span v-if="a.decided_at">
                      · {{ formatTime(a.decided_at) }}</span
                    >
                  </li>
                </ul>
              </details>
            </template>
          </div>
          <div v-if="p.status === 'submitted'" class="kb-detail-actions">
            <Button
              class="kb-confirm"
              :loading="decidingId === p.proposal_id"
              @click="decide(p, 'approve')"
            >
              {{ t("s01.knowledge.proposal.approve") }}
            </Button>
            <Button
              variant="secondary"
              :disabled="decidingId === p.proposal_id"
              @click="decide(p, 'reject')"
            >
              {{ t("s01.knowledge.proposal.reject") }}
            </Button>
          </div>
        </li>
      </ul>

      <!-- PC 对象列表（行内展开详情：最新修订全文） -->
      <ul v-if="activeTab === 'pc' && pcRows.length" class="pc-list">
        <li
          v-for="obj in pcRows"
          :key="obj.object_id"
          class="pc-row"
          :class="{ 'is-open': Boolean(pcOpen[obj.object_id]) }"
        >
          <button
            class="pc-row-head"
            :aria-expanded="Boolean(pcOpen[obj.object_id])"
            @click="togglePcRow(obj)"
          >
            <span class="pc-name">{{ obj.display_name }}</span>
            <span class="pc-type">{{ obj.object_type }}</span>
            <Badge :variant="lifecycleVariant(obj.lifecycle_status)">{{
              t(`s01.knowledge.lifecycle.${obj.lifecycle_status}`)
            }}</Badge>
            <ChevronRight
              class="kb-row-chevron"
              :class="{ 'is-open': Boolean(pcOpen[obj.object_id]) }"
              aria-hidden="true"
            />
          </button>
          <div v-if="!pcOpen[obj.object_id]" class="pc-row-body">
            <p v-if="obj.change_summary" class="pc-summary">
              {{ obj.change_summary }}
            </p>
            <p v-if="obj.excerpt" class="pc-excerpt">{{ obj.excerpt }}</p>
          </div>
          <div v-else class="kb-detail">
            <p
              v-if="
                pcDetailLoading === obj.object_id && !pcDetail[obj.object_id]
              "
              class="knowledge-hint"
            >
              {{ t("s01.knowledge.hint.loading") }}
            </p>
            <template v-else-if="pcDetail[obj.object_id]">
              <p v-if="!pcRev(obj)" class="knowledge-hint">
                {{ t("s01.knowledge.detail.noRevision") }}
              </p>
              <template v-else>
                <div class="pc-detail-meta">
                  <span class="pc-type">
                    {{ t("s01.knowledge.field.revision") }}
                    v{{ pcRev(obj)?.revision_no }}
                  </span>
                  <Badge
                    :variant="pcRev(obj)?.versioned ? 'success' : 'neutral'"
                  >
                    {{
                      pcRev(obj)?.versioned
                        ? t("s01.knowledge.detail.versioned")
                        : t("s01.knowledge.detail.unversioned")
                    }}
                  </Badge>
                  <span v-if="pcRev(obj)?.analysis_mode" class="pc-type">
                    {{ t("s01.knowledge.detail.analysisMode") }}:
                    {{ pcRev(obj)?.analysis_mode }}
                  </span>
                </div>
                <p v-if="pcRev(obj)?.summary" class="pc-summary">
                  {{ pcRev(obj)?.summary }}
                </p>
                <MarkdownViewer :model-value="pcRev(obj)?.content ?? ''" />
              </template>
            </template>
          </div>
        </li>
      </ul>
    </div>
  </PageFrame>
</template>

<style scoped>
.knowledge-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-width: 860px;
}

/* Tabs 仅作触发行；面板 slot 留空（内容跨页签共享），去掉面板内边距。 */
.knowledge-tabs :deep(.ui-tabs__panel) {
  padding-top: 0;
}

.knowledge-hint {
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.kb-list,
.pc-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.kb-row,
.pc-row {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-card);
}

.kb-row.is-open {
  border-color: var(--accent);
}

.kb-row-head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  background: transparent;
  border: 0;
  cursor: pointer;
  text-align: left;
}

.kb-row-head:hover {
  background: var(--ws-hover-veil);
}

.kb-row-chevron {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-tertiary);
  transition: transform var(--duration-fast) var(--ease-decelerate);
}

.kb-row-head:hover .kb-row-chevron {
  color: var(--text-secondary);
}

.kb-row-chevron.is-open {
  transform: rotate(90deg);
}

.kb-doc-key {
  font-weight: 600;
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.kb-meta {
  margin-left: auto;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.kb-detail {
  border-top: 1px solid var(--border-subtle);
  padding: var(--space-3) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.kb-detail-actions {
  display: flex;
  gap: var(--space-2);
}

.kb-revisions ul {
  list-style: none;
  margin: var(--space-2) 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.kb-revisions summary,
.kb-content summary {
  cursor: pointer;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.pc-row {
  display: flex;
  flex-direction: column;
}

.pc-row-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  background: transparent;
  border: 0;
  cursor: pointer;
  text-align: left;
}

.pc-row-head:hover {
  background: var(--ws-hover-veil);
}

.pc-row-head .pc-type {
  margin-left: auto;
}

.pc-row-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: 0 var(--space-4) var(--space-3);
}

.pc-detail-meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1) var(--space-3);
}

.pc-name {
  font-weight: 600;
  font-size: var(--font-size-body);
}

.pc-type {
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.pc-summary,
.pc-excerpt {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.pc-excerpt {
  color: var(--text-tertiary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.impact-panel {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: var(--space-3) var(--space-4);
}

.impact-panel summary {
  cursor: pointer;
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.impact-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding-top: var(--space-3);
}

.impact-form {
  display: flex;
  gap: var(--space-2);
}

.impact-input {
  flex: 1;
}

.impact-path {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.impact-node {
  font-weight: 600;
  color: var(--text-primary);
}

.impact-edge {
  color: var(--accent);
  margin: 0 var(--space-2);
}

.impact-gaps {
  list-style: none;
  margin: 0;
  padding: 0;
  font-size: var(--font-size-micro);
  color: var(--text-warning);
}

.ingest-row {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}

.ingest-file {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-body);
  cursor: pointer;
  color: var(--text-secondary);
}

.ingest-file input[type="file"] {
  display: none;
}

.ingest-select {
  max-width: 160px;
}
</style>
