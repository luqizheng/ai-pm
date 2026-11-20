<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { HttpError } from "@digua/http";
import PageFrame from "@/components/shell/PageFrame.vue";
import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import {
  closeAnalysisReport,
  getAnalysisReport,
  synthesizeIdentity,
  type AnalysisReportDetail,
  type CloseReportOutcome,
  type ReportEvidenceRow,
  type ReportProposalRow
} from "@/api/analysis-reports";
import {
  confirmKbRevision,
  decidePcProposal,
  decideProjcProposal
} from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";

/**
 * Analysis 报告详情页（/project/analysis/reports/:reportId，P2-03/P2-04）。
 *
 * 按裁决 D-2 六节呈现（正文与版本以详情 API 单通道回填，每次进入重新拉取——
 * 缓存会陈旧原则）：
 *   1. 基本信息   2. 分析报告正文（当前 KB 修订）   3. PC 提案
 *   3b. ProjC 技术认知提案   4. PC 修订内容   4b. ProjC 对象修订
 *   4c. 分析证据与来源（双域 evidence 投影）
 *   5. D-4 身份综合（裁决 D-2，端点语义不变）
 *   6. 关闭与处置（仅 analyzed 可关；回执含 applied/withdrawn/rejected/锚点
 *      + published_versions 正式对象清单与知识库入口）
 *
 * P3-11 决策接线（本轮闭环补齐）：第 3b 节挂批准/驳回——与 PC 提案同 CAS/
 * 错误处理，走 /v1/pm/projc/proposals/{id}/decide 并行端点；双域提案卡展示
 * AI 分析理由（rationale）与风险/置信/来源元数据；关闭回执列正式发布对象。
 *
 * 审阅交互（P2-04，复用既有端点）：
 *   - KB 确认（第 2 节）：confirmKbRevision——按钮条件 = kb_revision draft
 *     且报告 analyzed；三重门在后端（interactive+真人+成员），前端不重写。
 *   - PC / ProjC 提案批准/驳回（第 3/3b 节）：decidePcProposal /
 *     decideProjcProposal（CAS expected_version_no）——按钮显隐「报告 analyzed
 *     且 status ∈ {submitted,approved,rejected}」；批准=暂记（approvedPending
 *     文案），开放期可互转，closed 冻结不渲染。
 *   - 报告关闭（第 6 节）：closeAnalysisReport——仅 analyzed 可关；409 显式呈现。
 *   - 关闭语义七条 = 本页核心验收（自查表见 Phase 2 验收报告）；单飞守卫
 *     acting/closing 等价迁移；确认范式随迁旧面板单击式，不引入新范式。
 *   - 暂缓语义（不改状态机）：未决提案在关闭时被撤回（逐条审计），关闭后
 *     该仓库可再次分析、未沉淀内容由后续分析重新提案——按现有语义呈现，
 *     不引入新状态。
 *
 * - 无 Findings/Conclusions/Recommendations、无 Report Revision 身份——不虚构
 *   （核验 §3.3/§3.4）；「分析发现」以双域提案 + 修订 + evidence 投影呈现
 *   （分析链产物边界，见 analysis_report.rs 投影注释）。正文权威 = DB KB
 *   修订直读（GAP-5 维持登记），本页只经详情 API 读取，不复制进前端持久
 *   store（● 红线 13）。
 * - 非法/跨项目 Report ID → 显式 404 语义错误态，不静默（GAP-6 随本卡关闭）。
 * - 项目上下文单源 = shell.currentProjectId；项目切换重拉，跨项目 ID 落 404 态。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const currentProjectId = computed(() => shell.currentProjectId);
const reportId = computed(() => String(route.params.reportId ?? ""));

const detail = ref<AnalysisReportDetail | null>(null);
const loading = ref(false);
const loadError = ref("");
/** 404 语义态：报告不存在/跨项目/不可见（后端统一 404 不泄露存在性）。 */
const notFound = ref(false);

const successNotice = ref("");
const actionError = ref("");

/* ── PC 提案过滤：待决（默认）/ 已决 / 全部（旧面板行为等价迁移）────────── */

const proposalFilter = ref<"pending" | "decided" | "all">("pending");

const PROPOSAL_FILTERS = [
  { value: "pending", label: "analysis.filter.pending" },
  { value: "decided", label: "analysis.filter.decided" },
  { value: "all", label: "analysis.filter.all" }
] as const;

const DECIDED_STATUSES = new Set([
  "approved",
  "rejected",
  "failed",
  "applied",
  "withdrawn",
  "expired"
]);

function filteredProposals(proposals: ReportProposalRow[]) {
  if (proposalFilter.value === "all") return proposals;
  if (proposalFilter.value === "decided")
    return proposals.filter(p => DECIDED_STATUSES.has(p.status));
  return proposals.filter(
    p => p.status === "submitted" || p.status === "draft"
  );
}

function proposalStatusVariant(status: string) {
  if (status === "approved" || status === "applied") return "success";
  if (status === "submitted" || status === "draft") return "warning";
  if (status === "rejected" || status === "failed") return "danger";
  return "neutral";
}

/** approved 在 close 前语义 =「已批准·待发布」（暂记，非正式发布），其余走词典。 */
function proposalStatusLabel(status: string): string {
  if (status === "approved")
    return t("analysis.proposalStatus.approvedPending");
  return t(`analysis.proposalStatus.${status}`);
}

/** 提案来源词典（AI 推荐 vs 人工/其他通道的显式区分）。 */
function originLabel(origin: string): string {
  return t(`analysis.proposalOrigin.${origin}`, origin);
}

/** 双域证据合并视图（带域标记；pc 在前）。 */
type DomainEvidence = ReportEvidenceRow & { domain: "pc" | "projc" };
const allEvidence = computed<DomainEvidence[]>(() => {
  if (!detail.value) return [];
  return [
    ...detail.value.pc_evidence.map(e => ({ ...e, domain: "pc" as const })),
    ...detail.value.projc_evidence.map(e => ({ ...e, domain: "projc" as const }))
  ];
});

/** 证据 description JSON 防御性解析：快照通道存
 * {head_commit_sha, llm_evidence_refs[], commit_count…}；解析失败原样回显。 */
interface ParsedEvidence {
  headCommitSha: string | null;
  refs: string[];
  commitCount: number | null;
  fileCount: number | null;
}
function parseEvidenceDescription(
  row: DomainEvidence
): ParsedEvidence | null {
  if (!row.description) return null;
  try {
    const raw = JSON.parse(row.description) as Record<string, unknown>;
    if (typeof raw !== "object" || raw === null) return null;
    const refs = Array.isArray(raw.llm_evidence_refs)
      ? (raw.llm_evidence_refs.filter(v => typeof v === "string") as string[])
      : [];
    return {
      headCommitSha:
        typeof raw.head_commit_sha === "string" ? raw.head_commit_sha : null,
      refs,
      commitCount: typeof raw.commit_count === "number" ? raw.commit_count : null,
      fileCount: typeof raw.file_count === "number" ? raw.file_count : null
    };
  } catch {
    return null;
  }
}

/** 证据类型词典（未知值原样回退）。 */
function evidenceTypeLabel(type: string): string {
  return t(`analysis.evidenceType.${type}`, type);
}

/* ── 审阅交互（P2-04：KB 确认 / PC 提案决定；三重门与 CAS 均在后端）──────── */

/** 单飞守卫（KB 确认与 PC 决策共用；等价迁移旧面板 acting）。 */
const acting = ref(false);

/** KB 确认按钮条件（原样保留）：KB 修订 draft 且报告 analyzed。 */
const canConfirmKb = computed(
  () =>
    detail.value?.status === "analyzed" &&
    detail.value?.kb_revision?.status === "draft"
);

/** PC 决策按钮显隐（原样保留）：报告 analyzed 且决定未冻结可互转。 */
function canDecide(proposal: ReportProposalRow): boolean {
  return (
    detail.value?.status === "analyzed" &&
    (proposal.status === "submitted" ||
      proposal.status === "approved" ||
      proposal.status === "rejected")
  );
}

function actionErrorText(error: unknown, fallbackKey: string): string {
  // 权限负向：无权限/非真人 403 显式呈现，不重试不吞错
  if (error instanceof HttpError && error.status === 403) {
    return t("analysis.error.forbidden");
  }
  return t(fallbackKey);
}

async function confirmKb(): Promise<void> {
  if (!currentProjectId.value || acting.value) return;
  const revision = detail.value?.kb_revision;
  if (!revision || revision.status !== "draft") return;
  acting.value = true;
  successNotice.value = "";
  actionError.value = "";
  try {
    await confirmKbRevision(
      revision.document_id,
      revision.revision_id,
      currentProjectId.value
    );
    successNotice.value = t("analysis.notice.kbConfirmed");
    // 动作后刷新（refreshAfterAction 等价）：draft → published 即时可见
    await loadDetail();
  } catch (error) {
    actionError.value = actionErrorText(error, "analysis.error.kbConfirm");
  } finally {
    acting.value = false;
  }
}

/** 提案决策（PC 与 ProjC 双域同构；CAS expected_version_no，门在后端）。 */
async function decideProposal(
  proposal: ReportProposalRow,
  action: "approve" | "reject",
  domain: "pc" | "projc" = "pc"
): Promise<void> {
  if (!currentProjectId.value || acting.value) return;
  acting.value = true;
  successNotice.value = "";
  actionError.value = "";
  try {
    const payload = {
      action,
      expected_version_no: proposal.expected_version_no
    };
    if (domain === "projc") {
      await decideProjcProposal(
        proposal.proposal_id,
        currentProjectId.value,
        payload
      );
    } else {
      await decidePcProposal(
        proposal.proposal_id,
        currentProjectId.value,
        payload
      );
    }
    successNotice.value = t(
      action === "approve"
        ? "analysis.notice.approved"
        : "analysis.notice.rejected"
    );
    await loadDetail();
  } catch (error) {
    actionError.value = actionErrorText(error, "analysis.error.decide");
  } finally {
    acting.value = false;
  }
}

/* ── 详情加载（每次进入重拉；动作后刷新同通道）────────────────────────── */

async function loadDetail() {
  if (!currentProjectId.value || !reportId.value) return;
  loading.value = true;
  loadError.value = "";
  notFound.value = false;
  try {
    detail.value = await getAnalysisReport(
      reportId.value,
      currentProjectId.value
    );
  } catch (error) {
    if (error instanceof HttpError && error.status === 404) {
      // 不存在/跨项目/跨租户统一 404：显式错误态（GAP-6 关闭）
      notFound.value = true;
    } else {
      loadError.value = t("analysis.error.load");
    }
  } finally {
    loading.value = false;
  }
}

/* ── D-4 身份综合（裁决 D-2：端点/权限/审批语义不变）────────────────────── */

const synthesizing = ref(false);
const synthesisOutcome = ref<{
  profile_proposal_id: string | null;
  vision_proposal_id: string | null;
  goal_proposal_id: string | null;
  goal_proposal_status: string | null;
  uncertainty_notes: string[];
} | null>(null);

/** 综合门：当前报告 analyzed（综合提案挂当前 open 报告；后端 409 兜底）。 */
const hasAnalyzedReport = computed(() => detail.value?.status === "analyzed");

async function runSynthesis(): Promise<void> {
  if (!currentProjectId.value || synthesizing.value) return;
  synthesizing.value = true;
  successNotice.value = "";
  actionError.value = "";
  try {
    const outcome = await synthesizeIdentity(currentProjectId.value);
    synthesisOutcome.value = outcome;
    successNotice.value = t("analysis.detail.synthesisDone", {
      goal: outcome.goal_proposal_status ?? ""
    });
    // 动作后刷新（refreshAfterAction 等价）：新提案经同一详情通道进入第 3 节
    await loadDetail();
  } catch {
    actionError.value = t("analysis.detail.synthesisFailed");
  } finally {
    synthesizing.value = false;
  }
}

/* ── 报告关闭（仅 analyzed；确认预览 + 回执）──────────────────────────── */

const closeConfirmOpen = ref(false);
const closing = ref(false);

/** 关闭确认框预览：将发布的已批准提案数（暂记→close 统一发布）。 */
const approvedCount = computed(
  () => detail.value?.proposals.filter(p => p.status === "approved").length ?? 0
);
/** 关闭确认框预览：ProjC 已批准提案数（双域分流同语义）。 */
const approvedProjcCount = computed(
  () =>
    detail.value?.projc_proposals.filter(p => p.status === "approved").length ??
    0
);

/** 关闭回执：本次发布的正式对象清单（映射提案标题 + 领域徽标 + 版本号）。 */
const publishedEntries = ref<
  { domain: "pc" | "projc"; proposal_id: string; version_no: number; title: string }[]
>([]);

async function doClose(): Promise<void> {
  if (!currentProjectId.value || !detail.value || closing.value) return;
  closing.value = true;
  actionError.value = "";
  try {
    const outcome: CloseReportOutcome = await closeAnalysisReport(
      detail.value.id,
      currentProjectId.value
    );
    successNotice.value = t("analysis.detail.closeReceipt", {
      applied: outcome.applied_proposals,
      withdrawn: outcome.withdrawn_proposals,
      rejected: outcome.rejected_kb ? 1 : 0,
      commit: outcome.last_analyzed_commit.slice(0, 12)
    });
    // P3-11：ProjC 双域计数追加（任一非零才追加句）
    if (
      outcome.projc_applied > 0 ||
      outcome.projc_withdrawn > 0 ||
      outcome.projc_publish_failed_proposals > 0
    ) {
      successNotice.value += " " + t("analysis.detail.closeReceiptProjc", {
        projcApplied: outcome.projc_applied,
        projcWithdrawn: outcome.projc_withdrawn,
        projcFailed: outcome.projc_publish_failed_proposals
      });
    }
    // 正式对象清单（确认后反馈：领域 + 版本号 + 提案标题映射）。
    const titleOf = (id: string): string =>
      [...(detail.value?.proposals ?? []), ...(detail.value?.projc_proposals ?? [])].find(
        p => p.proposal_id === id
      )?.title ?? id;
    publishedEntries.value = outcome.published_versions.map(v => ({
      ...v,
      title: titleOf(v.proposal_id)
    }));
    closeConfirmOpen.value = false;
    // 关闭后回填 closed 态（决策按钮消失等语义，P2-04 测试化）
    await loadDetail();
  } catch (error) {
    // 后端 409（非 analyzed 等）与 403（无权限/非真人）显式呈现，不静默不重试
    actionError.value = actionErrorText(error, "analysis.detail.closeError");
  } finally {
    closing.value = false;
  }
}

/* ── 数据重置与加载编排（声明在后，供上方动作复用）────────────────────── */

// 上下文/路由任一变化即整体重置并重拉——每次进入详情重新拉取（缓存陈旧原则）。
watch(
  [reportId, currentProjectId],
  () => {
    detail.value = null;
    successNotice.value = "";
    actionError.value = "";
    synthesisOutcome.value = null;
    proposalFilter.value = "pending";
    closeConfirmOpen.value = false;
    publishedEntries.value = [];
    void loadDetail();
  },
  { immediate: true }
);

/* ── 展示辅助 ──────────────────────────────────────────────────────────── */

function statusVariant(
  status: string
): "success" | "warning" | "danger" | "neutral" {
  if (status === "analyzed") return "warning";
  if (status === "closed") return "success";
  if (status === "failed") return "danger";
  return "neutral";
}

function kbStatusVariant(
  status: string
): "success" | "warning" | "danger" | "neutral" {
  if (status === "published") return "success";
  if (status === "draft") return "warning";
  if (status === "rejected") return "danger";
  return "neutral";
}

function shortSha(sha: string): string {
  return sha.slice(0, 12);
}

function formatTime(value: string | null): string {
  if (!value) return "—";
  return value.slice(0, 16).replace("T", " ");
}

const pageTitle = computed(() =>
  detail.value
    ? `${detail.value.repository_name} @ ${shortSha(detail.value.commit_sha)}`
    : t("analysis.detail.title")
);

/** 正式知识查看入口：知识库页按本次发布域定位页签（KB 文档 | PC 对象 | ProjC 对象）。 */
function navigateToKnowledge(): void {
  const domain = publishedEntries.value[0]?.domain ?? "pc";
  void router.push(`/project/knowledge?tab=${domain}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('analysis.page.title'), to: '/project/analysis' }
    ]"
    :title="pageTitle"
  >
    <div class="an-detail">
      <EmptyState
        v-if="!currentProjectId"
        size="sm"
        :title="t('analysis.state.noProject')"
      />
      <template v-else>
        <p v-if="loading" class="an-hint">
          {{ t("analysis.detail.loading") }}
        </p>
        <EmptyState
          v-else-if="notFound"
          size="sm"
          :title="t('analysis.error.notFound')"
        />
        <template v-else>
          <Alert v-if="loadError" variant="danger" size="sm" role="alert">
            {{ loadError }}
          </Alert>
          <Alert v-if="successNotice" variant="success" size="sm" role="status">
            {{ successNotice }}
          </Alert>
          <Alert v-if="actionError" variant="danger" size="sm" role="alert">
            {{ actionError }}
          </Alert>

          <template v-if="detail">
            <!-- 第 1 节 · 基本信息 -->
            <section class="an-sec" aria-labelledby="an-sec-basic">
              <h3 id="an-sec-basic" class="an-sec-title">
                {{ t("analysis.detail.secBasic") }}
              </h3>
              <dl class="an-facts">
                <div class="an-fact">
                  <dt>{{ t("analysis.detail.fieldRepository") }}</dt>
                  <dd>
                    {{ detail.repository_name }} @
                    {{ shortSha(detail.commit_sha) }}
                  </dd>
                </div>
                <div class="an-fact">
                  <dt>{{ t("analysis.detail.fieldStatus") }}</dt>
                  <dd>
                    <Badge :variant="statusVariant(detail.status)">
                      {{ t(`analysis.status.${detail.status}`) }}
                    </Badge>
                  </dd>
                </div>
                <div class="an-fact">
                  <dt>{{ t("analysis.detail.fieldRunMode") }}</dt>
                  <dd>
                    {{
                      detail.run_mode === "incremental"
                        ? t("analysis.detail.runModeIncremental")
                        : t("analysis.detail.runModeFull")
                    }}
                    <template v-if="detail.since_commit">
                      · {{ t("analysis.detail.fieldSinceCommit") }}
                      {{ shortSha(detail.since_commit) }}
                    </template>
                  </dd>
                </div>
                <div class="an-fact">
                  <dt>{{ t("analysis.detail.fieldCreatedAt") }}</dt>
                  <dd>{{ formatTime(detail.created_at) }}</dd>
                </div>
                <div class="an-fact">
                  <dt>{{ t("analysis.detail.fieldUpdatedAt") }}</dt>
                  <dd>{{ formatTime(detail.updated_at) }}</dd>
                </div>
                <div v-if="detail.closed_at" class="an-fact">
                  <dt>{{ t("analysis.detail.fieldClosedAt") }}</dt>
                  <dd>{{ formatTime(detail.closed_at) }}</dd>
                </div>
                <div
                  v-if="
                    detail.status === 'failed' &&
                    (detail.error_code || detail.error_detail)
                  "
                  class="an-fact"
                >
                  <dt>{{ t("analysis.detail.fieldError") }}</dt>
                  <dd class="an-error-text">
                    {{ detail.error_code || "" }}
                    <template v-if="detail.error_detail">
                      {{ detail.error_detail }}
                    </template>
                  </dd>
                </div>
              </dl>
            </section>

            <!-- 第 2 节 · 分析报告正文（当前 KB 修订；P2-04 挂 KB 确认） -->
            <section class="an-sec" aria-labelledby="an-sec-kb">
              <h3 id="an-sec-kb" class="an-sec-title">
                {{ t("analysis.detail.secKb") }}
              </h3>
              <p v-if="!detail.kb_revision" class="an-hint">
                {{ t("analysis.detail.kbMissing") }}
              </p>
              <template v-else>
                <div class="an-kb-meta">
                  <Badge :variant="kbStatusVariant(detail.kb_revision.status)">
                    {{ t(`analysis.kbStatus.${detail.kb_revision.status}`) }}
                  </Badge>
                  <span class="an-hint">
                    {{
                      t("analysis.detail.kbSecMeta", {
                        no: detail.kb_revision.revision_no
                      })
                    }}
                    <template v-if="detail.kb_revision.source_commit_sha">
                      ·
                      {{
                        t("analysis.detail.kbCommitAnchor", {
                          commit: shortSha(detail.kb_revision.source_commit_sha)
                        })
                      }}
                    </template>
                  </span>
                </div>
                <MarkdownViewer
                  :model-value="detail.kb_revision.content ?? ''"
                />
                <!-- KB 确认（P2-04）：draft + analyzed 才渲染；三重门在后端 -->
                <div v-if="canConfirmKb" class="an-actions">
                  <Button size="sm" :loading="acting" @click="confirmKb">
                    {{ t("analysis.action.confirm") }}
                  </Button>
                </div>
              </template>
            </section>

            <!-- 第 3 节 · PC 提案（P2-04 挂批准/驳回） -->
            <section class="an-sec" aria-labelledby="an-sec-proposals">
              <h3 id="an-sec-proposals" class="an-sec-title">
                {{
                  t("analysis.detail.secProposals", {
                    n: detail.proposals.length
                  })
                }}
              </h3>
              <div
                v-if="detail.proposals.length"
                class="an-filter"
                role="group"
              >
                <button
                  v-for="f in PROPOSAL_FILTERS"
                  :key="f.value"
                  class="an-filter-chip"
                  :class="{ 'is-active': proposalFilter === f.value }"
                  type="button"
                  @click="proposalFilter = f.value"
                >
                  {{ t(f.label) }}
                </button>
              </div>
              <p v-if="!detail.proposals.length" class="an-hint">
                {{ t("analysis.detail.noProposals") }}
              </p>
              <p
                v-else-if="!filteredProposals(detail.proposals).length"
                class="an-hint"
              >
                {{ t("analysis.detail.noMatchingProposals") }}
              </p>
              <ul v-else class="an-proposals">
                <li
                  v-for="proposal in filteredProposals(detail.proposals)"
                  :key="proposal.proposal_id"
                  class="an-proposal"
                >
                  <div class="an-proposal-head">
                    <Badge variant="neutral">
                      {{ originLabel(proposal.origin_kind) }}
                    </Badge>
                    <span class="an-proposal-title">{{ proposal.title }}</span>
                    <Badge :variant="proposalStatusVariant(proposal.status)">
                      {{ proposalStatusLabel(proposal.status) }}
                    </Badge>
                    <span class="an-meta">
                      v{{ proposal.expected_version_no }} ·
                      {{ proposal.member_count }}
                      {{ t("analysis.proposal.members") }}
                    </span>
                  </div>
                  <!-- AI 分析理由 + 风险/置信（审阅判断依据；加法投影） -->
                  <div
                    v-if="proposal.rationale || proposal.risk_level || proposal.confidence"
                    class="an-proposal-basis"
                  >
                    <p v-if="proposal.rationale" class="an-proposal-rationale">
                      <span class="an-basis-label">{{
                        t("analysis.proposal.rationaleLabel")
                      }}</span>
                      {{ proposal.rationale }}
                    </p>
                    <p class="an-basis-meta">
                      <template v-if="proposal.risk_level">
                        {{ t("analysis.proposal.risk") }}
                        {{ t(`analysis.proposal.riskLevel.${proposal.risk_level}`, proposal.risk_level) }}
                      </template>
                      <template v-if="proposal.confidence">
                        · {{ t("analysis.proposal.confidence") }}
                        {{ t(`analysis.proposal.confidenceLevel.${proposal.confidence}`, proposal.confidence) }}
                      </template>
                    </p>
                  </div>
                  <!-- 批准/驳回（P2-04）：显隐与后端门一致——报告 analyzed 且
                       决定未冻结（submitted/approved/rejected 可互转，原样保留）；
                       approved=暂记（非正式发布），closed 冻结后不渲染 -->
                  <div v-if="canDecide(proposal)" class="an-actions">
                    <Button
                      v-if="proposal.status !== 'approved'"
                      size="sm"
                      :loading="acting"
                      @click="decideProposal(proposal, 'approve', 'pc')"
                    >
                      {{ t("analysis.proposal.approve") }}
                    </Button>
                    <Button
                      v-if="proposal.status !== 'rejected'"
                      size="sm"
                      variant="secondary"
                      :disabled="acting"
                      @click="decideProposal(proposal, 'reject', 'pc')"
                    >
                      {{ t("analysis.proposal.reject") }}
                    </Button>
                  </div>
                </li>
              </ul>
            </section>

            <!-- 第 3b 节 · ProjC 技术认知提案（P3-11 域；决策已接线：
                 /v1/pm/projc/proposals/{id}/decide 与 PC P5 同构同门） -->
            <section class="an-sec" aria-labelledby="an-sec-projc">
              <h3 id="an-sec-projc" class="an-sec-title">
                {{
                  t("analysis.detail.sectProjcProposals", {
                    n: detail.projc_proposals.length
                  })
                }}
              </h3>
              <p v-if="!detail.projc_proposals.length" class="an-hint">
                {{ t("analysis.detail.noProjcProposals") }}
              </p>
              <ul v-else class="an-proposals">
                <li
                  v-for="proposal in detail.projc_proposals"
                  :key="proposal.proposal_id"
                  class="an-proposal"
                >
                  <div class="an-proposal-head">
                    <Badge variant="neutral">
                      {{ t("analysis.detail.techBadge") }}
                    </Badge>
                    <Badge variant="neutral">
                      {{ originLabel(proposal.origin_kind) }}
                    </Badge>
                    <span class="an-proposal-title">{{ proposal.title }}</span>
                    <Badge :variant="proposalStatusVariant(proposal.status)">
                      {{ proposalStatusLabel(proposal.status) }}
                    </Badge>
                    <span class="an-meta">
                      v{{ proposal.expected_version_no }} ·
                      {{ proposal.member_count }}
                      {{ t("analysis.proposal.members") }}
                    </span>
                  </div>
                  <div
                    v-if="proposal.rationale || proposal.risk_level || proposal.confidence"
                    class="an-proposal-basis"
                  >
                    <p v-if="proposal.rationale" class="an-proposal-rationale">
                      <span class="an-basis-label">{{
                        t("analysis.proposal.rationaleLabel")
                      }}</span>
                      {{ proposal.rationale }}
                    </p>
                    <p class="an-basis-meta">
                      <template v-if="proposal.risk_level">
                        {{ t("analysis.proposal.risk") }}
                        {{ t(`analysis.proposal.riskLevel.${proposal.risk_level}`, proposal.risk_level) }}
                      </template>
                      <template v-if="proposal.confidence">
                        · {{ t("analysis.proposal.confidence") }}
                        {{ t(`analysis.proposal.confidenceLevel.${proposal.confidence}`, proposal.confidence) }}
                      </template>
                    </p>
                  </div>
                  <!-- 决策动作与 PC 提案同显隐规则：报告 analyzed 且未冻结 -->
                  <div v-if="canDecide(proposal)" class="an-actions">
                    <Button
                      v-if="proposal.status !== 'approved'"
                      size="sm"
                      :loading="acting"
                      @click="decideProposal(proposal, 'approve', 'projc')"
                    >
                      {{ t("analysis.proposal.approve") }}
                    </Button>
                    <Button
                      v-if="proposal.status !== 'rejected'"
                      size="sm"
                      variant="secondary"
                      :disabled="acting"
                      @click="decideProposal(proposal, 'reject', 'projc')"
                    >
                      {{ t("analysis.proposal.reject") }}
                    </Button>
                  </div>
                </li>
              </ul>
            </section>

            <!-- 第 4 节 · PC 修订内容 -->
            <section class="an-sec" aria-labelledby="an-sec-pc">
              <h3 id="an-sec-pc" class="an-sec-title">
                {{
                  t("analysis.detail.secPcRevisions", {
                    n: detail.pc_revisions.length
                  })
                }}
              </h3>
              <p v-if="!detail.pc_revisions.length" class="an-hint">
                {{ t("analysis.detail.noPcRevisions") }}
              </p>
              <details
                v-for="revision in detail.pc_revisions"
                :key="revision.object_id + ':' + revision.revision_no"
                class="an-pc-item"
              >
                <summary>
                  {{ revision.display_name }} · {{ revision.object_type }} · v{{
                    revision.revision_no
                  }}
                </summary>
                <p v-if="revision.change_summary" class="an-hint">
                  {{ revision.change_summary }}
                </p>
                <MarkdownViewer :model-value="revision.content ?? ''" />
              </details>
            </section>

            <!-- 第 4b 节 · ProjC 对象修订（P3-11 最小渲染：只读列表） -->
            <section class="an-sec" aria-labelledby="an-sec-projc-rev">
              <h3 id="an-sec-projc-rev" class="an-sec-title">
                {{
                  t("analysis.detail.sectProjcRevisions", {
                    n: detail.projc_revisions.length
                  })
                }}
              </h3>
              <p v-if="!detail.projc_revisions.length" class="an-hint">
                {{ t("analysis.detail.noProjcRevisions") }}
              </p>
              <details
                v-for="revision in detail.projc_revisions"
                :key="revision.object_id + ':' + revision.revision_no"
                class="an-pc-item"
              >
                <summary>
                  <Badge variant="neutral" class="an-projc-badge">
                    {{ t("analysis.detail.techBadge") }}
                  </Badge>
                  {{ revision.display_name }} · {{ revision.object_type }} · v{{
                    revision.revision_no
                  }}
                </summary>
                <p v-if="revision.change_summary" class="an-hint">
                  {{ revision.change_summary }}
                </p>
                <MarkdownViewer :model-value="revision.content ?? ''" />
              </details>
            </section>

            <!-- 第 4c 节 · 分析证据与来源（双域 evidence 投影；审阅判断依据） -->
            <section class="an-sec" aria-labelledby="an-sec-evidence">
              <h3 id="an-sec-evidence" class="an-sec-title">
                {{ t("analysis.detail.secEvidence", { n: allEvidence.length }) }}
              </h3>
              <p v-if="!allEvidence.length" class="an-hint">
                {{ t("analysis.detail.noEvidence") }}
              </p>
              <ul v-else class="an-evidence">
                <li
                  v-for="(evidence, index) in allEvidence"
                  :key="evidence.domain + ':' + index"
                  class="an-evidence-item"
                >
                  <div class="an-proposal-head">
                    <Badge :variant="evidence.domain === 'projc' ? 'neutral' : 'info'">
                      {{
                        evidence.domain === "projc"
                          ? t("analysis.detail.techBadge")
                          : t("analysis.detail.pcBadge")
                      }}
                    </Badge>
                    <span class="an-evidence-object">{{ evidence.object_name }}</span>
                    <Badge variant="neutral">
                      {{ evidenceTypeLabel(evidence.evidence_type) }}
                    </Badge>
                    <span class="an-meta">
                      <template v-if="evidence.confidence">
                        {{ t("analysis.proposal.confidence") }}
                        {{ t(`analysis.proposal.confidenceLevel.${evidence.confidence}`, evidence.confidence) }}
                      </template>
                    </span>
                  </div>
                  <p class="an-evidence-locator">
                    <template v-if="evidence.file_path">
                      {{ evidence.file_path }}<template v-if="evidence.start_line">:{{ evidence.start_line }}<template v-if="evidence.end_line">-{{ evidence.end_line }}</template></template>
                    </template>
                    <template v-if="evidence.commit_sha">
                      · {{ t("analysis.evidence.commit") }} {{ shortSha(evidence.commit_sha) }}
                    </template>
                    <template v-if="evidence.section_anchor">
                      · {{ t("analysis.evidence.section") }} {{ evidence.section_anchor }}
                    </template>
                  </p>
                  <template
                    v-if="parseEvidenceDescription(evidence)"
                  >
                    <p
                      v-if="parseEvidenceDescription(evidence)!.refs.length"
                      class="an-evidence-refs"
                    >
                      <span class="an-basis-label">{{
                        t("analysis.evidence.citedRefs")
                      }}</span>
                      <code
                        v-for="ref in parseEvidenceDescription(evidence)!.refs"
                        :key="ref"
                        class="an-evidence-ref"
                      >{{ ref }}</code>
                    </p>
                    <p
                      v-if="parseEvidenceDescription(evidence)!.headCommitSha"
                      class="an-hint"
                    >
                      {{
                        t("analysis.evidence.headCommit", {
                          commit: parseEvidenceDescription(evidence)!.headCommitSha!.slice(0, 12),
                          commits: parseEvidenceDescription(evidence)!.commitCount ?? "—",
                          files: parseEvidenceDescription(evidence)!.fileCount ?? "—"
                        })
                      }}
                    </p>
                  </template>
                  <p v-if="evidence.excerpt" class="an-evidence-excerpt">
                    {{ evidence.excerpt }}
                  </p>
                </li>
              </ul>
            </section>

            <!-- 第 5 节 · D-4 身份综合（裁决 D-2；端点语义不变） -->
            <section class="an-sec" aria-labelledby="an-sec-synthesis">
              <h3 id="an-sec-synthesis" class="an-sec-title">
                {{ t("analysis.detail.secSynthesis") }}
              </h3>
              <div class="an-synthesis-row">
                <Button
                  size="sm"
                  :loading="synthesizing"
                  :disabled="!hasAnalyzedReport"
                  @click="runSynthesis"
                >
                  {{ t("analysis.detail.synthesisRun") }}
                </Button>
                <span class="an-hint">
                  {{ t("analysis.detail.synthesisHint") }}
                </span>
              </div>
              <dl v-if="synthesisOutcome" class="an-facts">
                <div class="an-fact">
                  <dt>{{ t("analysis.detail.synthesisProfile") }}</dt>
                  <dd>
                    {{
                      synthesisOutcome.profile_proposal_id
                        ? t("analysis.detail.synthesisProduced")
                        : t("analysis.detail.synthesisMissing")
                    }}
                  </dd>
                </div>
                <div class="an-fact">
                  <dt>{{ t("analysis.detail.synthesisVision") }}</dt>
                  <dd>
                    {{
                      synthesisOutcome.vision_proposal_id
                        ? t("analysis.detail.synthesisProduced")
                        : t("analysis.detail.synthesisMissing")
                    }}
                  </dd>
                </div>
                <div class="an-fact">
                  <dt>{{ t("analysis.detail.synthesisGoal") }}</dt>
                  <dd>
                    <template v-if="synthesisOutcome.goal_proposal_id">
                      {{ t("analysis.detail.synthesisProduced") }}
                      <template v-if="synthesisOutcome.goal_proposal_status">
                        · {{ synthesisOutcome.goal_proposal_status }}
                      </template>
                    </template>
                    <template v-else>
                      {{ t("analysis.detail.synthesisMissing") }}
                    </template>
                  </dd>
                </div>
              </dl>
            </section>

            <!-- 第 6 节 · 关闭与处置（仅 analyzed 可关；七条语义验收在 P2-04） -->
            <section class="an-sec" aria-labelledby="an-sec-close">
              <h3 id="an-sec-close" class="an-sec-title">
                {{ t("analysis.detail.secClose") }}
              </h3>
              <div v-if="detail.status === 'analyzed'" class="an-close-row">
                <Button
                  size="sm"
                  variant="danger"
                  @click="closeConfirmOpen = true"
                >
                  {{ t("analysis.detail.close") }}
                </Button>
                <span class="an-hint">
                  {{ t("analysis.detail.closeHint") }}
                </span>
              </div>
              <p v-else class="an-hint">
                {{ t("analysis.detail.closeNotAvailable") }}
              </p>
              <!-- 确认后反馈：本次发布的正式知识对象（领域 + 版本）+ 查看入口 -->
              <template v-if="detail.status === 'closed' && publishedEntries.length">
                <div class="an-published">
                  <p class="an-basis-label">
                    {{ t("analysis.detail.publishedTitle") }}
                  </p>
                  <ul class="an-published-list">
                    <li
                      v-for="entry in publishedEntries"
                      :key="entry.domain + entry.proposal_id"
                      class="an-published-item"
                    >
                      <Badge :variant="entry.domain === 'projc' ? 'neutral' : 'info'">
                        {{
                          entry.domain === "projc"
                            ? t("analysis.detail.techBadge")
                            : t("analysis.detail.pcBadge")
                        }}
                      </Badge>
                      <span>{{ entry.title }}</span>
                      <span class="an-meta">
                        v{{ entry.version_no }}
                      </span>
                    </li>
                  </ul>
                  <p class="an-hint">{{ t("analysis.detail.publishedHint") }}</p>
                  <Button
                    size="sm"
                    variant="secondary"
                    @click="navigateToKnowledge"
                  >
                    {{ t("analysis.detail.openKnowledge") }}
                  </Button>
                </div>
              </template>
            </section>
          </template>
        </template>
      </template>

      <ConfirmationDialog
        :open="closeConfirmOpen"
        :title="t('analysis.detail.closeConfirmTitle')"
        :description="
          detail
            ? t('analysis.detail.closeConfirmDescription', {
                repo: detail.repository_name,
                publish: approvedCount,
                projcPublish: approvedProjcCount,
                proposals: detail.pending_proposals,
                kb: detail.has_pending_kb ? 1 : 0
              })
            : ''
        "
        :confirm-label="t('analysis.detail.close')"
        :cancel-label="t('analysis.action.cancel')"
        :disabled="closing"
        variant="danger"
        @update:open="value => (!value ? (closeConfirmOpen = false) : null)"
        @confirm="doClose"
        @cancel="closeConfirmOpen = false"
      />
    </div>
  </PageFrame>
</template>

<style scoped>
.an-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-width: 860px;
}

.an-sec {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-card);
  padding: var(--space-3) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.an-sec-title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--text-primary);
}

.an-facts {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.an-fact {
  display: flex;
  gap: var(--space-3);
  font-size: var(--font-size-caption);
}

.an-fact dt {
  flex: none;
  width: 88px;
  color: var(--text-tertiary);
}

.an-fact dd {
  margin: 0;
  color: var(--text-secondary);
}

.an-error-text {
  color: var(--text-warning);
  white-space: pre-wrap;
}

.an-kb-meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.an-filter {
  display: flex;
  gap: var(--space-2);
}

.an-filter-chip {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full, 999px);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  padding: 2px var(--space-3);
  cursor: pointer;
}

.an-filter-chip.is-active {
  border-color: var(--accent);
  color: var(--accent);
  font-weight: 600;
}

.an-proposals {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.an-proposal {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.an-proposal-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.an-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.an-proposal-title {
  font-weight: 600;
  font-size: var(--font-size-body);
}

.an-meta {
  margin-left: auto;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.an-projc-badge {
  margin-right: var(--space-1);
}

.an-proposal-basis {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.an-proposal-rationale {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
  line-height: 1.5;
}

.an-basis-label {
  display: inline-block;
  margin-right: var(--space-2);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
  font-weight: 600;
}

.an-basis-meta {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.an-evidence {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.an-evidence-item {
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.an-evidence-object {
  font-weight: 600;
  font-size: var(--font-size-caption);
}

.an-evidence-locator {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  font-family: var(--font-family-mono, monospace);
}

.an-evidence-refs {
  margin: 0;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1);
}

.an-evidence-ref {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 0 var(--space-1);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.an-evidence-excerpt {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
  white-space: pre-wrap;
}

.an-published {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border-top: 1px dashed var(--border-subtle);
  padding-top: var(--space-2);
}

.an-published-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.an-published-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.an-pc-item {
  border-top: 1px dashed var(--border-subtle);
  padding-top: var(--space-2);
}

.an-pc-item summary {
  cursor: pointer;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.an-synthesis-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.an-close-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding-top: var(--space-2);
  border-top: 1px dashed var(--border-subtle);
}

.an-hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}
</style>
