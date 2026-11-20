<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronRight } from "lucide-vue-next";
import PageFrame from "@/components/shell/PageFrame.vue";
import {
  confirmKbRevision,
  getKbDocument,
  getPcObject,
  getProjcObject,
  ingestKbDocument,
  listKbDocuments,
  listPcObjects,
  listProjcObjects,
  retrievalQuery,
  type IngestKind,
  type KbDocumentDetail,
  type KbDocumentRow,
  type ObjectAnalysisSource,
  type PcObjectDetail,
  type PcObjectRow,
  type RetrievalQueryResponse
} from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";
import { useRoute, useRouter } from "vue-router";
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
 * 知识库（KB 文档 + PC 认知对象 + ProjC 对象）——/project/knowledge 落地页。
 *
 * - Phase 2 P2-06：reports 页签退役（Analysis 独立承载报告，/project/analysis）；
 *   页签构成 = kb | pc | projc，默认 kb（裁决 D-1；projc 为闭环补齐加法页签——
 *   正式项目技术认知的查看面，与 PC 面同构只读）。
 * - KB 页签：文档列表（最新修订投影）+ 详情（正文 / 修订历史 / 草稿确认发布）。
 *   确认门在后端（真人 interactive 会话；AI/API Key 403）——按钮仅是入口。
 * - PC / ProjC 页签：认知对象列表（生命周期徽章 + 最新修订摘要 + 来源分析追溯
 *   analysis_sources——版本→提案→分析报告反查）。
 * - 深链：?doc=/?object= 行为不变；?tab= 切页签；?report= 兼容跳转 Analysis
 *   报告详情（§6 定版表）。
 */
const { t } = useI18n();
const shell = useShellStore();
const currentProjectId = computed(() => shell.currentProjectId);

type TabKey = "kb" | "pc" | "projc";
const activeTab = ref<TabKey>("kb");
const loading = ref(false);
const errorText = ref("");

const kbRows = ref<KbDocumentRow[]>([]);
const pcRows = ref<PcObjectRow[]>([]);
const projcRows = ref<PcObjectRow[]>([]);

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
    const [kb, pc, projc] = await Promise.all([
      listKbDocuments(projectId),
      listPcObjects(projectId),
      listProjcObjects(projectId)
    ]);
    kbRows.value = kb.items;
    pcRows.value = pc.items;
    projcRows.value = projc.items;
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

/* ── PC 对象详情（行内展开；按 id 缓存，收起保留缓存）──────────────────── */

/** PC 行展开态 + 详情缓存（收起只切展开态；缓存保留——再展不重拉）。 */
const pcOpen = ref<Record<string, boolean>>({});
const pcDetail = ref<Record<string, PcObjectDetail>>({});
const pcDetailLoading = ref<string | null>(null);

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

/* ── ProjC 对象（与 PC 面同构；第二套独立缓存）────────────────────────── */

const projcOpen = ref<Record<string, boolean>>({});
const projcDetail = ref<Record<string, PcObjectDetail>>({});
const projcDetailLoading = ref<string | null>(null);

async function openProjcDetail(obj: PcObjectRow) {
  if (!canLoad.value || projcDetail.value[obj.object_id]) return;
  projcDetailLoading.value = obj.object_id;
  try {
    projcDetail.value[obj.object_id] = await getProjcObject(
      obj.object_id,
      currentProjectId.value as string
    );
  } catch {
    errorText.value = t("s01.knowledge.error.load");
  } finally {
    projcDetailLoading.value = null;
  }
}

function toggleProjcRow(obj: PcObjectRow) {
  projcOpen.value[obj.object_id] = !projcOpen.value[obj.object_id];
  if (projcOpen.value[obj.object_id]) openProjcDetail(obj);
}

function projcRev(obj: PcObjectRow) {
  return projcDetail.value[obj.object_id]?.latest_revision ?? null;
}

/** 来源分析追溯（展开详情的 analysis_sources；未发布 → 空数组）。 */
function objSources(obj: PcObjectRow): ObjectAnalysisSource[] {
  return (
    pcDetail.value[obj.object_id]?.analysis_sources ??
    projcDetail.value[obj.object_id]?.analysis_sources ??
    []
  );
}

function openSourceReport(reportId: string): void {
  void router.push(`/project/analysis/reports/${reportId}`);
}

function shortSha(sha: string): string {
  return sha.slice(0, 12);
}

type BadgeTone = "success" | "warning" | "danger" | "neutral";

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
    value: "projc",
    label: `${t("s01.knowledge.tab.projc")} (${projcRows.value.length})`
  }
]);

/** 摄入文档类型（Select 组件契约）。 */
const docTypeOptions = computed(() => [
  { value: "knowledge", label: t("s01.knowledge.ingest.typeKnowledge") },
  { value: "architecture", label: t("s01.knowledge.ingest.typeArchitecture") },
  { value: "decision", label: t("s01.knowledge.ingest.typeDecision") },
  { value: "requirement", label: t("s01.knowledge.ingest.typeRequirement") }
]);

/* ── 深链（搜索结果 → 自动展开 / 切页签；?report= 兼容跳转）────────────── */

const route = useRoute();
const router = useRouter();

/** 兼容跳转幂等守卫：同一 report 在本组件生命周期内只跳一次
 * （挂载与「项目就绪重放」各调一次 applyDeepLink，重复 replace 无意义）。 */
let jumpedReport: string | null = null;

async function applyDeepLink() {
  const reportId = route.query.report;
  if (typeof reportId === "string" && reportId) {
    if (reportId === jumpedReport) return;
    // Phase 2 P2-06 兼容跳转（方案 §6 定版表）：报告已迁 Analysis 详情页。
    // 保留 project_id 等其余 query，report 自身升级为路径参数——不退化为丢参
    // redirect。跳转失败（非法/跨项目 ID）由详情页 GAP-6 显式错误态呈现。
    // report 与 doc/object 同现时本分支先行跳离（组合自然消解，测试固化）。
    jumpedReport = reportId;
    const { report: _report, ...rest } = route.query;
    await router.replace({
      path: `/project/analysis/reports/${reportId}`,
      query: rest
    });
    return;
  }
  const docId = route.query.doc;
  const objectId = route.query.object;
  const tabQuery = route.query.tab;
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
    return;
  }
  // 报告关闭回执跳转（?tab=pc|projc）：定位到对应正式知识页签。
  if (tabQuery === "pc" || tabQuery === "projc") {
    activeTab.value = tabQuery;
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
  projcOpen.value = {};
  projcDetail.value = {};
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
      <EmptyState
        v-else-if="!loading && !projcRows.length && activeTab === 'projc'"
        size="sm"
        :title="t('s01.knowledge.hint.emptyProjc')"
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
                <!-- 来源分析追溯（版本→提案→分析报告反查；闭环溯源） -->
                <div v-if="objSources(obj).length" class="pc-sources">
                  <span class="pc-type">{{
                    t("s01.knowledge.detail.sourceAnalysis")
                  }}</span>
                  <ul class="pc-source-list">
                    <li
                      v-for="source in objSources(obj)"
                      :key="source.version_no + ':' + (source.report_id ?? '')"
                      class="pc-source-item"
                    >
                      <span>v{{ source.version_no }}</span>
                      <template v-if="source.report_id">
                        <button
                          class="pc-source-link"
                          type="button"
                          @click="openSourceReport(source.report_id)"
                        >
                          {{ source.repository_name }}
                          @{{ shortSha(source.commit_sha ?? "") }}
                        </button>
                      </template>
                      <span v-else class="pc-type">{{
                        t("s01.knowledge.detail.sourceAnalysisNone")
                      }}</span>
                    </li>
                  </ul>
                </div>
              </template>
            </template>
          </div>
        </li>
      </ul>

      <!-- ProjC 对象列表（项目技术认知；与 PC 面同构只读 + 来源追溯） -->
      <ul v-if="activeTab === 'projc' && projcRows.length" class="pc-list">
        <li
          v-for="obj in projcRows"
          :key="obj.object_id"
          class="pc-row"
          :class="{ 'is-open': Boolean(projcOpen[obj.object_id]) }"
        >
          <button
            class="pc-row-head"
            :aria-expanded="Boolean(projcOpen[obj.object_id])"
            @click="toggleProjcRow(obj)"
          >
            <span class="pc-name">{{ obj.display_name }}</span>
            <span class="pc-type">{{ obj.object_type }}</span>
            <Badge :variant="lifecycleVariant(obj.lifecycle_status)">{{
              t(`s01.knowledge.lifecycle.${obj.lifecycle_status}`)
            }}</Badge>
            <ChevronRight
              class="kb-row-chevron"
              :class="{ 'is-open': Boolean(projcOpen[obj.object_id]) }"
              aria-hidden="true"
            />
          </button>
          <div v-if="!projcOpen[obj.object_id]" class="pc-row-body">
            <p v-if="obj.change_summary" class="pc-summary">
              {{ obj.change_summary }}
            </p>
            <p v-if="obj.excerpt" class="pc-excerpt">{{ obj.excerpt }}</p>
          </div>
          <div v-else class="kb-detail">
            <p
              v-if="
                projcDetailLoading === obj.object_id &&
                !projcDetail[obj.object_id]
              "
              class="knowledge-hint"
            >
              {{ t("s01.knowledge.hint.loading") }}
            </p>
            <template v-else-if="projcDetail[obj.object_id]">
              <p v-if="!projcRev(obj)" class="knowledge-hint">
                {{ t("s01.knowledge.detail.noRevision") }}
              </p>
              <template v-else>
                <div class="pc-detail-meta">
                  <span class="pc-type">
                    {{ t("s01.knowledge.field.revision") }}
                    v{{ projcRev(obj)?.revision_no }}
                  </span>
                  <Badge
                    :variant="projcRev(obj)?.versioned ? 'success' : 'neutral'"
                  >
                    {{
                      projcRev(obj)?.versioned
                        ? t("s01.knowledge.detail.versioned")
                        : t("s01.knowledge.detail.unversioned")
                    }}
                  </Badge>
                  <span v-if="projcRev(obj)?.analysis_mode" class="pc-type">
                    {{ t("s01.knowledge.detail.analysisMode") }}:
                    {{ projcRev(obj)?.analysis_mode }}
                  </span>
                </div>
                <p v-if="projcRev(obj)?.summary" class="pc-summary">
                  {{ projcRev(obj)?.summary }}
                </p>
                <MarkdownViewer :model-value="projcRev(obj)?.content ?? ''" />
                <div v-if="objSources(obj).length" class="pc-sources">
                  <span class="pc-type">{{
                    t("s01.knowledge.detail.sourceAnalysis")
                  }}</span>
                  <ul class="pc-source-list">
                    <li
                      v-for="source in objSources(obj)"
                      :key="source.version_no + ':' + (source.report_id ?? '')"
                      class="pc-source-item"
                    >
                      <span>v{{ source.version_no }}</span>
                      <template v-if="source.report_id">
                        <button
                          class="pc-source-link"
                          type="button"
                          @click="openSourceReport(source.report_id)"
                        >
                          {{ source.repository_name }}
                          @{{ shortSha(source.commit_sha ?? "") }}
                        </button>
                      </template>
                      <span v-else class="pc-type">{{
                        t("s01.knowledge.detail.sourceAnalysisNone")
                      }}</span>
                    </li>
                  </ul>
                </div>
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

.pc-sources {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  border-top: 1px dashed var(--border-subtle);
  padding-top: var(--space-2);
}

.pc-source-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.pc-source-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pc-source-link {
  background: transparent;
  border: 0;
  padding: 0;
  cursor: pointer;
  font-size: var(--font-size-caption);
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 2px;
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
