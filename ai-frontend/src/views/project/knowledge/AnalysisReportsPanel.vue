<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { ChevronRight } from "lucide-vue-next";
import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import {
  closeAnalysisReport,
  getAnalysisReport,
  listAnalysisReports,
  type AnalysisReportDetail,
  type AnalysisReportRow,
  type AnalysisReportStatus,
  type ReportProposalRow
} from "@/api/analysis-reports";
import { confirmKbRevision, decidePcProposal } from "@/api/knowledge";

/**
 * D-5 · 「分析报告」页签面板：先报告、后报告内容——按状态分区（分析中 /
 * 待审理 / 已关闭 / 失败），行展开逐条审阅（KB 确认发布 / PC 提案批准驳回，
 * 复用既有端点），全部处置后「关闭报告」释放再分析门。
 */

const { t } = useI18n();
const props = defineProps<{ projectId: string | null }>();

const rows = ref<AnalysisReportRow[]>([]);
const loading = ref(false);
const errorText = ref("");
const notice = ref("");

const detailCache = ref<Record<string, AnalysisReportDetail>>({});
const openMap = ref<Record<string, boolean>>({});
const detailLoading = ref<string | null>(null);

const closeConfirm = ref<AnalysisReportRow | null>(null);
const closing = ref(false);
const acting = ref(false);

/** analyzing 行存在时 3s 轮询（分析中 → 待审理自动浮现）。 */
let pollTimer: ReturnType<typeof setInterval> | null = null;

const GROUPS: AnalysisReportStatus[] = [
  "analyzing",
  "analyzed",
  "closed",
  "failed"
];

function statusVariant(
  status: string
): "success" | "warning" | "danger" | "neutral" {
  if (status === "analyzed") return "warning";
  if (status === "closed") return "success";
  if (status === "failed") return "danger";
  return "neutral";
}

function shortSha(sha: string): string {
  return sha.slice(0, 12);
}

function formatTime(value: string | null): string {
  if (!value) return "—";
  return value.slice(0, 16).replace("T", " ");
}

const grouped = computed(() =>
  GROUPS.map(status => ({
    status,
    items: rows.value.filter(row => row.status === status)
  })).filter(group => group.items.length > 0)
);

const hasAnalyzing = computed(() =>
  rows.value.some(row => row.status === "analyzing")
);

function pendingTotal(row: AnalysisReportRow): number {
  return row.pending_proposals + (row.has_pending_kb ? 1 : 0);
}

async function loadLists(silent = false) {
  if (!props.projectId) return;
  if (!silent) {
    loading.value = true;
    errorText.value = "";
  }
  try {
    const result = await listAnalysisReports(props.projectId);
    rows.value = result.items;
  } catch {
    if (!silent) errorText.value = t("s01.knowledge.error.load");
  } finally {
    if (!silent) loading.value = false;
  }
}

function syncPolling(): void {
  if (hasAnalyzing.value && pollTimer === null) {
    pollTimer = setInterval(() => {
      void loadLists(true);
    }, 3000);
  } else if (!hasAnalyzing.value && pollTimer !== null) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

watch(hasAnalyzing, syncPolling, { immediate: true });

watch(
  () => props.projectId,
  pid => {
    detailCache.value = {};
    openMap.value = {};
    notice.value = "";
    void loadLists();
    void pid;
  },
  { immediate: true }
);

// 深链 ?report={id}：打开对应报告（列表就绪后按行匹配展开）。
const route = useRoute();
watch(
  () => route.query.report,
  async reportId => {
    if (typeof reportId !== "string" || !reportId || !props.projectId) return;
    if (!rows.value.length) await loadLists();
    const row = rows.value.find(item => item.id === reportId);
    if (!row) return;
    openMap.value[reportId] = true;
    await openDetail(row);
  },
  { immediate: true }
);

async function toggleRow(row: AnalysisReportRow): Promise<void> {
  if (row.status === "analyzing") return;
  const wasOpen = Boolean(openMap.value[row.id]);
  openMap.value[row.id] = !wasOpen;
  if (!wasOpen) await openDetail(row);
}

async function openDetail(row: AnalysisReportRow): Promise<void> {
  if (!props.projectId || detailCache.value[row.id]) return;
  detailLoading.value = row.id;
  try {
    detailCache.value[row.id] = await getAnalysisReport(
      row.id,
      props.projectId
    );
  } catch {
    errorText.value = t("s01.knowledge.error.load");
  } finally {
    detailLoading.value = null;
  }
}

async function refreshAfterAction(rowId: string): Promise<void> {
  delete detailCache.value[rowId];
  await loadLists(true);
  const row = rows.value.find(item => item.id === rowId);
  if (row && openMap.value[rowId]) await openDetail(row);
}

async function confirmKb(row: AnalysisReportRow): Promise<void> {
  if (!props.projectId || acting.value) return;
  const detail = detailCache.value[row.id];
  const revision = detail?.kb_revision;
  if (!revision || revision.status !== "draft") return;
  acting.value = true;
  try {
    await confirmKbRevision(
      revision.document_id,
      revision.revision_id,
      props.projectId
    );
    notice.value = t("s01.knowledge.notice.confirmed");
    await refreshAfterAction(row.id);
  } catch {
    notice.value = t("s01.knowledge.error.confirm");
  } finally {
    acting.value = false;
  }
}

async function decideProposal(
  row: AnalysisReportRow,
  proposal: ReportProposalRow,
  action: "approve" | "reject"
): Promise<void> {
  if (!props.projectId || acting.value) return;
  acting.value = true;
  try {
    await decidePcProposal(proposal.proposal_id, props.projectId, {
      action,
      expected_version_no: proposal.expected_version_no
    });
    notice.value = t(
      `s01.knowledge.notice.${action === "approve" ? "approved" : "rejected"}`
    );
    await refreshAfterAction(row.id);
  } catch {
    notice.value = t("s01.knowledge.error.decide");
  } finally {
    acting.value = false;
  }
}

async function doClose(): Promise<void> {
  const row = closeConfirm.value;
  if (!row || !props.projectId || closing.value) return;
  closing.value = true;
  try {
    const outcome = await closeAnalysisReport(row.id, props.projectId);
    notice.value = t("s01.knowledge.report.closeNotice", {
      commit: outcome.last_analyzed_commit.slice(0, 12)
    });
    closeConfirm.value = null;
    delete detailCache.value[row.id];
    await loadLists(true);
  } catch {
    notice.value = t("s01.knowledge.report.closeError");
  } finally {
    closing.value = false;
  }
}

function proposalStatusVariant(status: string) {
  if (status === "approved" || status === "applied") return "success";
  if (status === "submitted" || status === "draft") return "warning";
  if (status === "rejected" || status === "failed") return "danger";
  return "neutral";
}

onBeforeUnmount(() => {
  if (pollTimer !== null) clearInterval(pollTimer);
});
</script>

<template>
  <div class="report-panel">
    <Alert v-if="errorText" variant="danger" size="sm" role="alert">
      {{ errorText }}
    </Alert>
    <Alert v-if="notice" variant="success" size="sm" role="status">
      {{ notice }}
    </Alert>
    <p v-if="loading" class="rp-hint">
      {{ t("s01.knowledge.hint.loading") }}
    </p>
    <EmptyState
      v-else-if="!rows.length"
      size="sm"
      :title="t('s01.knowledge.report.empty')"
    />
    <template v-else>
      <section v-for="group in grouped" :key="group.status" class="rp-group">
        <h4 class="rp-group-title">
          {{ t(`s01.knowledge.report.status.${group.status}`) }}
          <span class="rp-group-count">{{ group.items.length }}</span>
        </h4>
        <ul class="rp-list">
          <li
            v-for="row in group.items"
            :key="row.id"
            class="rp-row"
            :class="{ 'is-open': Boolean(openMap[row.id]) }"
          >
            <button
              class="rp-head"
              :aria-expanded="Boolean(openMap[row.id])"
              @click="toggleRow(row)"
            >
              <span class="rp-title">
                {{ row.repository_name }} @ {{ shortSha(row.commit_sha) }}
              </span>
              <Badge :variant="statusVariant(row.status)">
                {{ t(`s01.knowledge.report.status.${row.status}`) }}
              </Badge>
              <Badge v-if="row.run_mode === 'incremental'" variant="neutral">
                {{ t("s01.knowledge.report.incrementalBadge") }}
              </Badge>
              <span class="rp-meta">
                <template v-if="row.status === 'analyzing'">
                  {{
                    row.stage_detail || t("s01.knowledge.report.analyzingHint")
                  }}
                </template>
                <template v-else-if="row.status === 'analyzed'">
                  {{
                    pendingTotal(row) > 0
                      ? t("s01.knowledge.report.pendingItems", {
                          n: pendingTotal(row)
                        })
                      : t("s01.knowledge.report.allDisposed")
                  }}
                </template>
                <template v-else-if="row.status === 'closed'">
                  {{
                    t("s01.knowledge.report.closedAt", {
                      time: formatTime(row.closed_at)
                    })
                  }}
                </template>
                <template v-else-if="row.status === 'failed'">
                  {{ row.error_code || t("s01.knowledge.report.failedReason") }}
                </template>
                <template v-else>{{ formatTime(row.updated_at) }}</template>
              </span>
              <ChevronRight
                v-if="row.status !== 'analyzing'"
                class="rp-chevron"
                :class="{ 'is-open': Boolean(openMap[row.id]) }"
                aria-hidden="true"
              />
            </button>

            <p
              v-if="row.status === 'failed' && row.error_detail"
              class="rp-error"
              role="alert"
            >
              {{ row.error_detail }}
            </p>

            <div v-if="openMap[row.id]" class="rp-detail">
              <p v-if="detailLoading === row.id" class="rp-hint">
                {{ t("s01.knowledge.report.loading") }}
              </p>
              <template v-else-if="detailCache[row.id]">
                <details class="rp-sec" open>
                  <summary>
                    {{
                      detailCache[row.id].kb_revision
                        ? t("s01.knowledge.report.kbSec", {
                            no: detailCache[row.id].kb_revision?.revision_no,
                            status: detailCache[row.id].kb_revision?.status
                          })
                        : t("s01.knowledge.report.kbMissing")
                    }}
                  </summary>
                  <template v-if="detailCache[row.id].kb_revision">
                    <MarkdownViewer
                      :model-value="
                        detailCache[row.id].kb_revision?.content ?? ''
                      "
                    />
                    <div
                      v-if="
                        detailCache[row.id].kb_revision?.status === 'draft' &&
                        row.status === 'analyzed'
                      "
                      class="rp-actions"
                    >
                      <Button
                        size="sm"
                        :loading="acting"
                        @click="confirmKb(row)"
                      >
                        {{ t("s01.knowledge.action.confirm") }}
                      </Button>
                    </div>
                  </template>
                </details>

                <details class="rp-sec" open>
                  <summary>
                    {{
                      t("s01.knowledge.report.proposalSec", {
                        n: detailCache[row.id].proposals.length
                      })
                    }}
                  </summary>
                  <p
                    v-if="!detailCache[row.id].proposals.length"
                    class="rp-hint"
                  >
                    {{ t("s01.knowledge.report.noProposals") }}
                  </p>
                  <ul v-else class="rp-proposals">
                    <li
                      v-for="proposal in detailCache[row.id].proposals"
                      :key="proposal.proposal_id"
                      class="rp-proposal"
                    >
                      <div class="rp-proposal-head">
                        <span class="rp-proposal-title">{{
                          proposal.title
                        }}</span>
                        <Badge
                          :variant="proposalStatusVariant(proposal.status)"
                        >
                          {{ proposal.status }}
                        </Badge>
                        <span class="rp-meta">
                          v{{ proposal.expected_version_no }} ·
                          {{ proposal.member_count }}
                          {{ t("s01.knowledge.proposal.members") }}
                        </span>
                      </div>
                      <div
                        v-if="
                          (proposal.status === 'submitted' ||
                            proposal.status === 'draft') &&
                          row.status === 'analyzed'
                        "
                        class="rp-actions"
                      >
                        <Button
                          size="sm"
                          :loading="acting"
                          @click="decideProposal(row, proposal, 'approve')"
                        >
                          {{ t("s01.knowledge.proposal.approve") }}
                        </Button>
                        <Button
                          size="sm"
                          variant="secondary"
                          :disabled="acting"
                          @click="decideProposal(row, proposal, 'reject')"
                        >
                          {{ t("s01.knowledge.proposal.reject") }}
                        </Button>
                      </div>
                    </li>
                  </ul>
                </details>

                <details class="rp-sec">
                  <summary>
                    {{
                      t("s01.knowledge.report.pcSec", {
                        n: detailCache[row.id].pc_revisions.length
                      })
                    }}
                  </summary>
                  <p
                    v-if="!detailCache[row.id].pc_revisions.length"
                    class="rp-hint"
                  >
                    {{ t("s01.knowledge.report.noPcRevisions") }}
                  </p>
                  <details
                    v-for="revision in detailCache[row.id].pc_revisions"
                    :key="revision.object_id + ':' + revision.revision_no"
                    class="rp-pc-item"
                  >
                    <summary>
                      {{ revision.display_name }} · {{ revision.object_type }} ·
                      v{{ revision.revision_no }}
                    </summary>
                    <p v-if="revision.change_summary" class="rp-hint">
                      {{ revision.change_summary }}
                    </p>
                    <MarkdownViewer :model-value="revision.content ?? ''" />
                  </details>
                </details>

                <div
                  v-if="row.status === 'analyzed'"
                  class="rp-actions rp-close-row"
                >
                  <Button
                    size="sm"
                    variant="danger"
                    @click="closeConfirm = row"
                  >
                    {{ t("s01.knowledge.report.close") }}
                  </Button>
                  <span class="rp-hint">
                    {{ t("s01.knowledge.report.closeHint") }}
                  </span>
                </div>
              </template>
            </div>
          </li>
        </ul>
      </section>
    </template>

    <ConfirmationDialog
      :open="Boolean(closeConfirm)"
      :title="t('s01.knowledge.report.closeConfirmTitle')"
      :description="
        closeConfirm
          ? t('s01.knowledge.report.closeConfirmDescription', {
              repo: closeConfirm.repository_name,
              proposals: closeConfirm.pending_proposals,
              kb: closeConfirm.has_pending_kb ? 1 : 0
            })
          : ''
      "
      :confirm-label="t('s01.knowledge.report.close')"
      :cancel-label="t('s01.knowledge.action.cancel')"
      :disabled="closing"
      variant="danger"
      @update:open="value => (!value ? (closeConfirm = null) : null)"
      @confirm="doClose"
      @cancel="closeConfirm = null"
    />
  </div>
</template>

<style scoped>
.report-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-width: 860px;
}

.rp-group-title {
  margin: 0 0 var(--space-2);
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--text-secondary);
}

.rp-group-count {
  margin-left: var(--space-2);
  color: var(--text-tertiary);
}

.rp-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rp-row {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-card);
  display: flex;
  flex-direction: column;
}

.rp-row.is-open {
  border-color: var(--accent);
}

.rp-head {
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

.rp-head:hover {
  background: var(--ws-hover-veil);
}

.rp-title {
  font-weight: 600;
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.rp-meta {
  margin-left: auto;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
  text-align: right;
}

.rp-chevron {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-tertiary);
  transition: transform var(--duration-fast) var(--ease-decelerate);
}

.rp-chevron.is-open {
  transform: rotate(90deg);
}

.rp-error {
  margin: 0;
  padding: 0 var(--space-4) var(--space-3);
  font-size: var(--font-size-caption);
  color: var(--text-warning);
  white-space: pre-wrap;
}

.rp-detail {
  border-top: 1px solid var(--border-subtle);
  padding: var(--space-3) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rp-sec summary {
  cursor: pointer;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.rp-pc-item {
  margin-top: var(--space-2);
}

.rp-pc-item summary {
  cursor: pointer;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.rp-hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.rp-actions {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}

.rp-close-row {
  padding-top: var(--space-2);
  border-top: 1px dashed var(--border-subtle);
}

.rp-proposals {
  list-style: none;
  margin: var(--space-2) 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rp-proposal {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rp-proposal-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.rp-proposal-title {
  font-weight: 600;
  font-size: var(--font-size-body);
}
</style>
