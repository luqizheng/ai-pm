<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { GitCommitHorizontal, RefreshCw } from "lucide-vue-next";

import { Button } from "@/components/ui/button";
import { Badge, type BadgeVariant } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  analyzeExternalCommit,
  answerClarification,
  closeAdoption,
  decideAdoption,
  discoverExternalCommits,
  getCommitDiff,
  getExternalCommit,
  listExternalCommits,
  type Clarification,
  type ExternalCommitDetail,
  type ExternalCommitListItem
} from "@/api/externalCommits";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 I — External commit adoption（P0 采纳闭环，迁移 0054）。
 *
 * 「发现 ≠ 采纳」：状态机 discovered → analyzed → awaiting_adoption →
 * adopted → closed（failed 可重试）逐态呈现，徽标分列，不合并口径。
 * 动作链：扫描发现（任意人）→ 分析（人类触发；失败重试）→ 澄清答复
 * （人类面，AI 主体后端 403）→ 采纳决定（approved/no_update/rejected）
 * → 关闭（知识提案全部终态才放行）。
 *
 * 知识更新建议携带拟稿全文（proposed_markdown）——人批准前可见实际内容，
 * 批准后走既有提案管线（质量门 + 红线人类确认），本页不直接写权威正文。
 */
const { t } = useI18n();
const toast = useToast();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);
const scanning = ref(false);
const acting = ref(false);
const items = ref<ExternalCommitListItem[]>([]);
const statusFilter = ref<string>("");
const selectedId = ref<string>("");

/** 详情（选中行）：行数据 + 澄清列表 */
const detail = ref<ExternalCommitDetail | null>(null);
const clarifications = ref<Clarification[]>([]);
const detailLoading = ref(false);
const answerDrafts = ref<Record<string, string>>({});
const decisionNote = ref("");
const diffText = ref<string | null>(null);
const diffTruncated = ref(false);

const FILTERS = ["", "discovered", "analyzed", "awaiting_adoption", "adopted", "closed", "failed"];

const selectedRow = computed(
  () => items.value.find(row => row.id === selectedId.value) ?? null
);

/** 状态徽标语义（发现≠采纳：discovered 与 adopted 徽标语义分离） */
function statusVariant(status: string): BadgeVariant {
  switch (status) {
    case "discovered":
      return "neutral";
    case "analyzed":
      return "accent";
    case "awaiting_adoption":
      return "accent";
    case "adopted":
      return "accent";
    case "closed":
      return "neutral";
    case "failed":
      return "danger";
    default:
      return "neutral";
  }
}

async function load(): Promise<void> {
  if (!hasProject.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const rows = await listExternalCommits(
      projectId.value,
      statusFilter.value || undefined
    );
    items.value = rows;
    if (!selectedId.value || !rows.some(row => row.id === selectedId.value)) {
      selectedId.value = rows[0]?.id ?? "";
    }
    if (selectedId.value) {
      await loadDetail(selectedId.value);
    } else {
      detail.value = null;
      clarifications.value = [];
    }
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

async function loadDetail(commitId: string): Promise<void> {
  detailLoading.value = true;
  diffText.value = null;
  try {
    const res = await getExternalCommit(commitId);
    detail.value = res.commit;
    clarifications.value = res.clarifications;
    answerDrafts.value = {};
    decisionNote.value = res.commit.adoption_note ?? "";
    const diff = await getCommitDiff(commitId).catch(() => null);
    if (diff) {
      diffText.value = diff.diff;
      diffTruncated.value = diff.truncated;
    }
  } catch (err) {
    detail.value = null;
    clarifications.value = [];
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    detailLoading.value = false;
  }
}

function select(row: ExternalCommitListItem): void {
  selectedId.value = row.id;
  void loadDetail(row.id);
}

/** 扫描外部提交（幂等：锚点 + UNIQUE 去重；首扫只建锚） */
async function scan(): Promise<void> {
  if (!hasProject.value || scanning.value) return;
  scanning.value = true;
  try {
    const outcome = await discoverExternalCommits(projectId.value);
    const failed = outcome.repositories.filter(repo => repo.error);
    toast.toast({
      title: t("pa06.commits.scanDone", {
        count: outcome.discovered_commits,
        repos: outcome.scanned_repositories
      }),
      variant: failed.length > 0 ? "danger" : "info"
    });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    scanning.value = false;
  }
}

/** AI 分析（人类触发；失败行落 failed 可重试，不吞错） */
async function analyze(): Promise<void> {
  const row = selectedRow.value;
  if (!row || acting.value) return;
  acting.value = true;
  try {
    const outcome = await analyzeExternalCommit(row.id);
    if (outcome.status === "failed") {
      toast.toast({
        title: t("pa06.commits.analyzeFailed", { error: outcome.error ?? "" }),
        variant: "danger"
      });
    } else {
      toast.toast({ title: t("pa06.commits.analyzedToast") });
    }
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

/** 澄清答复（人类面通道；全答复 → awaiting_adoption） */
async function answer(clarification: Clarification): Promise<void> {
  const row = selectedRow.value;
  const draft = (answerDrafts.value[clarification.id] ?? "").trim();
  if (!row || !draft || acting.value) return;
  acting.value = true;
  try {
    await answerClarification(row.id, clarification.id, draft);
    toast.toast({ title: t("pa06.commits.answeredToast") });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

/** 采纳决定（approved/no_update/rejected；AI 主体后端 403） */
async function decide(
  decision: "approved" | "no_update" | "rejected"
): Promise<void> {
  const row = selectedRow.value;
  if (!row || acting.value) return;
  acting.value = true;
  try {
    const outcome = await decideAdoption(
      row.id,
      decision,
      decisionNote.value.trim() || undefined
    );
    toast.toast({
      title: t(
        decision === "approved"
          ? "pa06.commits.approvedToast"
          : decision === "no_update"
            ? "pa06.commits.noUpdateToast"
            : "pa06.commits.rejectedToast"
      )
    });
    if (outcome.status === "adopted" && outcome.knowledge_proposals.length > 0) {
      toast.toast({ title: t("pa06.commits.proposalsNote") });
    }
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

/** 关闭（adopted → closed；知识提案未终态 → 后端 409 列出 pending） */
async function close(): Promise<void> {
  const row = selectedRow.value;
  if (!row || acting.value) return;
  acting.value = true;
  try {
    await closeAdoption(row.id);
    toast.toast({ title: t("pa06.commits.closedToast") });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

onMounted(load);
watch(projectId, () => {
  selectedId.value = "";
  void load();
});
watch(statusFilter, () => {
  selectedId.value = "";
  void load();
});
</script>

<template>
  <CollaborationFrame>
    <ScreenFeedback
      v-if="loading"
      state="loading"
      :title="$t('pa06.commits.loadingEyebrow')"
    />
    <ScreenFeedback
      v-else-if="error"
      state="error"
      :description="error"
      @retry="load"
    />
    <template v-else>
      <template v-if="!hasProject">
        <ScreenFeedback state="empty" :title="$t('pa06.frame.noProject')" />
      </template>
      <CollabLayout v-else>
        <div class="ec-head">
          <div class="ec-head__row">
            <GitCommitHorizontal :size="16" aria-hidden="true" />
            <h2 class="ec-head__title">
              {{ $t("pa06.commits.headline") }}
            </h2>
            <Badge variant="neutral">{{
              $t("pa06.commits.discoverBadge")
            }}</Badge>
            <div class="ec-head__spacer" />
            <select
              v-model="statusFilter"
              class="ec-filter"
              :aria-label="$t('pa06.commits.filterLabel')"
              data-testid="ec-filter"
            >
              <option
                v-for="value in FILTERS"
                :key="value"
                :value="value"
              >
                {{
                  value === ""
                    ? $t("pa06.commits.filterAll")
                    : $t(`pa06.commits.status.${value}`)
                }}
              </option>
            </select>
            <Button
              variant="primary"
              size="sm"
              :disabled="scanning"
              data-testid="ec-scan"
              @click="scan"
            >
              <RefreshCw :size="13" aria-hidden="true" />
              {{ $t("pa06.commits.scanAction") }}
            </Button>
          </div>
          <p class="ec-head__meta">{{ $t("pa06.commits.headlineMeta") }}</p>
        </div>

        <!-- 空态：不抹历史（空=当前过滤口径下无记录） -->
        <ScreenFeedback
          v-if="items.length === 0"
          state="empty"
          :title="$t('pa06.commits.emptyTitle')"
          :description="$t('pa06.commits.emptyHint')"
        />
        <div v-else class="ec-body">
          <!-- 队列 -->
          <ul class="ec-list" data-testid="ec-list">
            <li v-for="row in items" :key="row.id">
              <button
                type="button"
                class="ec-row"
                :class="{ 'is-selected': row.id === selectedId }"
                :data-testid="`ec-row-${row.commit_sha.slice(0, 8)}`"
                @click="select(row)"
              >
                <span class="ec-row__sha">{{ row.commit_sha.slice(0, 8) }}</span>
                <span class="ec-row__subject">{{ row.subject }}</span>
                <span class="ec-row__meta">
                  {{ row.repository_name ?? row.repository_id.slice(0, 8) }} ·
                  {{ row.author_name }} ·
                  {{ new Date(row.committed_at).toLocaleString() }}
                </span>
                <span class="ec-row__badges">
                  <Badge :variant="statusVariant(row.status)">
                    {{ $t(`pa06.commits.status.${row.status}`) }}
                  </Badge>
                  <Badge
                    v-if="row.status === 'analyzed' && row.open_questions > 0"
                    variant="danger"
                  >
                    {{ $t("pa06.commits.openQuestions", { count: row.open_questions }) }}
                  </Badge>
                </span>
              </button>
            </li>
          </ul>

          <!-- 详情 -->
          <div class="ec-detail" data-testid="ec-detail">
            <ScreenFeedback
              v-if="detailLoading && !detail"
              state="loading"
              :title="$t('pa06.commits.loadingEyebrow')"
            />
            <template v-else-if="detail">
              <div class="ec-detail__block">
                <p class="ec-eyebrow">{{ $t("pa06.commits.changeSection") }}</p>
                <p class="ec-detail__subject">{{ detail.subject }}</p>
                <p class="ec-detail__meta">
                  {{ detail.commit_sha }} · {{ detail.branch }} ·
                  {{ detail.author_name }} ·
                  {{ new Date(detail.committed_at).toLocaleString() }}
                </p>
                <ul class="ec-files">
                  <li
                    v-for="file in detail.changed_files.slice(0, 12)"
                    :key="file.path"
                    class="ec-files__item"
                  >
                    <Badge variant="neutral">{{ file.change }}</Badge>
                    <span class="ec-files__path">{{ file.path }}</span>
                  </li>
                </ul>
                <pre
                  v-if="diffText"
                  class="ec-diff"
                  data-testid="ec-diff"
                ><code>{{ diffText }}</code></pre>
                <p v-if="diffTruncated" class="ec-note">
                  {{ $t("pa06.commits.diffTruncated") }}
                </p>
              </div>

              <!-- 分析（AI 产物恒 draft/unverified） -->
              <div
                v-if="detail.analysis"
                class="ec-detail__block"
                data-testid="ec-analysis"
              >
                <p class="ec-eyebrow">
                  {{ $t("pa06.commits.analysisSection") }}
                  <Badge variant="accent">{{
                    $t(
                      detail.analysis.analysis_mode === "llm"
                        ? "pa06.commits.modeLlm"
                        : "pa06.commits.modeDerived"
                    )
                  }}</Badge>
                </p>
                <p class="ec-detail__text">{{ detail.analysis.summary }}</p>
                <div
                  v-if="detail.analysis.deviation"
                  class="ec-deviation"
                  :class="{ 'is-detected': detail.analysis.deviation.detected }"
                >
                  {{ $t("pa06.commits.deviationLabel") }}:
                  {{ detail.analysis.deviation.note }}
                </div>
                <ul v-if="detail.analysis.related_objects.length" class="ec-related">
                  <li v-for="rel in detail.analysis.related_objects" :key="rel.id">
                    <Badge variant="neutral">{{ rel.domain }}</Badge>
                    {{ rel.title }} — {{ rel.relation }}
                  </li>
                </ul>

                <!-- 知识更新建议（拟稿全文人批准前可见） -->
                <div
                  v-if="detail.analysis.knowledge_updates.length"
                  class="ec-updates"
                >
                  <p class="ec-eyebrow">{{ $t("pa06.commits.updatesSection") }}</p>
                  <div
                    v-for="update in detail.analysis.knowledge_updates"
                    :key="update.doc_key"
                    class="ec-update"
                  >
                    <p class="ec-update__title">
                      {{ update.doc_key }} · {{ update.doc_type }}
                    </p>
                    <p class="ec-update__summary">{{ update.summary }}</p>
                    <pre class="ec-update__body"><code>{{ update.proposed_markdown }}</code></pre>
                  </div>
                </div>
              </div>
              <div
                v-else-if="detail.status === 'discovered' || detail.status === 'failed'"
                class="ec-detail__block"
              >
                <p v-if="detail.last_error" class="ec-error" data-testid="ec-error">
                  {{ detail.last_error }}
                </p>
                <p class="ec-note">{{ $t("pa06.commits.notAnalyzed") }}</p>
              </div>

              <!-- 澄清（人类面；not_needed=显式记录非跳过） -->
              <div
                v-if="detail.clarification_state !== 'not_needed' || clarifications.length"
                class="ec-detail__block"
                data-testid="ec-clarifications"
              >
                <p class="ec-eyebrow">
                  {{ $t("pa06.commits.clarifySection") }}
                  <Badge
                    :variant="detail.clarification_state === 'pending' ? 'danger' : 'neutral'"
                  >
                    {{ $t(`pa06.commits.clarifyState.${detail.clarification_state}`) }}
                  </Badge>
                </p>
                <div v-for="row in clarifications" :key="row.id" class="ec-question">
                  <p class="ec-question__text">{{ row.question }}</p>
                  <p v-if="row.status === 'answered'" class="ec-question__answer">
                    {{ row.answer }}
                  </p>
                  <div v-else class="ec-question__form">
                    <input
                      v-model="answerDrafts[row.id]"
                      type="text"
                      class="ec-input"
                      :placeholder="$t('pa06.commits.answerPlaceholder')"
                      :aria-label="$t('pa06.commits.answerPlaceholder')"
                      :data-testid="`ec-answer-${row.id.slice(0, 8)}`"
                    />
                    <Button
                      variant="primary"
                      size="sm"
                      :disabled="acting || !(answerDrafts[row.id] ?? '').trim()"
                      @click="answer(row)"
                    >
                      {{ $t("pa06.commits.answerAction") }}
                    </Button>
                  </div>
                </div>
                <p v-if="!clarifications.length" class="ec-note">
                  {{ $t("pa06.commits.notNeededNote") }}
                </p>
              </div>

              <!-- 决定与关闭 -->
              <div class="ec-detail__block" data-testid="ec-actions">
                <p class="ec-eyebrow">{{ $t("pa06.commits.decisionSection") }}</p>
                <p
                  v-if="detail.status === 'closed'"
                  class="ec-note"
                >
                  {{
                    $t("pa06.commits.closedSummary", {
                      decision: $t(
                        `pa06.commits.decision.${detail.adoption_decision ?? "no_update"}`
                      )
                    })
                  }}
                </p>
                <template v-else-if="detail.status === 'awaiting_adoption'">
                  <input
                    v-model="decisionNote"
                    type="text"
                    class="ec-input"
                    :placeholder="$t('pa06.commits.notePlaceholder')"
                    :aria-label="$t('pa06.commits.notePlaceholder')"
                    data-testid="ec-note"
                  />
                  <div class="ec-actions">
                    <Button
                      variant="primary"
                      size="sm"
                      :disabled="acting"
                      data-testid="ec-approve"
                      @click="decide('approved')"
                    >
                      {{ $t("pa06.commits.approveAction") }}
                    </Button>
                    <Button
                      variant="secondary"
                      size="sm"
                      :disabled="acting"
                      data-testid="ec-no-update"
                      @click="decide('no_update')"
                    >
                      {{ $t("pa06.commits.noUpdateAction") }}
                    </Button>
                    <Button
                      variant="secondary"
                      size="sm"
                      :disabled="acting"
                      data-testid="ec-reject"
                      @click="decide('rejected')"
                    >
                      {{ $t("pa06.commits.rejectAction") }}
                    </Button>
                  </div>
                  <p class="ec-note">{{ $t("pa06.commits.decisionNote") }}</p>
                </template>
                <template v-else-if="detail.status === 'adopted'">
                  <ul class="ec-proposals">
                    <li
                      v-for="(proposal, index) in detail.knowledge_proposals"
                      :key="index"
                    >
                      {{ proposal.doc_key }} ·
                      {{ $t(`pa06.commits.proposalOutcome.${proposal.outcome}`) }}
                    </li>
                  </ul>
                  <Button
                    variant="primary"
                    size="sm"
                    :disabled="acting"
                    data-testid="ec-close"
                    @click="close"
                  >
                    {{ $t("pa06.commits.closeAction") }}
                  </Button>
                  <p class="ec-note">{{ $t("pa06.commits.closeNote") }}</p>
                </template>
                <Button
                  v-else-if="detail.status === 'discovered' || detail.status === 'failed'"
                  variant="primary"
                  size="sm"
                  :disabled="acting"
                  data-testid="ec-analyze"
                  @click="analyze"
                >
                  {{
                    detail.status === "failed"
                      ? $t("pa06.commits.retryAction")
                      : $t("pa06.commits.analyzeAction")
                  }}
                </Button>
              </div>
            </template>
          </div>
        </div>

        <template #context>
          <CollabContextPanel :title="$t('pa06.commits.ctxTitle')">
            <p class="ec-ctx-text">{{ $t("pa06.commits.ctxIntro") }}</p>
            <CollabContextSection :label="$t('pa06.commits.ctxChain')">
              {{ $t("pa06.commits.ctxChainText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.commits.ctxKnowledge')">
              {{ $t("pa06.commits.ctxKnowledgeText") }}
            </CollabContextSection>
            <p class="ec-ctx-note">{{ $t("pa06.commits.ctxNote") }}</p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.ec-head {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ec-head__row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.ec-head__spacer {
  flex: 1;
}

.ec-head__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h3);
}

.ec-head__meta {
  margin: 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.ec-filter {
  height: var(--size-control-sm);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  color: var(--text-primary);
  font-family: var(--font-ui);
}

.ec-body {
  display: grid;
  grid-template-columns: minmax(280px, 5fr) minmax(0, 7fr);
  gap: var(--space-4);
  align-items: start;
}

.ec-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ec-row {
  width: 100%;
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-areas:
    "sha subject"
    "meta badges";
  gap: 2px var(--space-2);
  text-align: left;
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  padding: var(--space-2);
  cursor: pointer;
  font-family: var(--font-ui);
}

.ec-row:hover {
  background: var(--surface-sunken);
}

.ec-row.is-selected {
  border-color: var(--ws-accent);
  background: var(--surface-sunken);
}

.ec-row__sha {
  grid-area: sha;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-accent);
}

.ec-row__subject {
  grid-area: subject;
  color: var(--text-primary);
  font-size: var(--font-size-body);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ec-row__meta {
  grid-area: meta;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ec-row__badges {
  grid-area: badges;
  display: inline-flex;
  gap: var(--space-1);
}

.ec-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.ec-detail__block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
}

.ec-eyebrow {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--text-secondary);
}

.ec-detail__subject {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
}

.ec-detail__meta,
.ec-detail__text {
  margin: 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  word-break: break-word;
}

.ec-files {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.ec-files__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.ec-diff,
.ec-update__body {
  margin: 0;
  max-height: 16rem;
  overflow: auto;
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-2);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  white-space: pre-wrap;
  word-break: break-word;
}

.ec-deviation {
  border-radius: var(--radius-sm);
  border: var(--border-1) solid var(--border-strong);
  padding: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.ec-deviation.is-detected {
  border-color: var(--status-danger-border, var(--border-strong));
}

.ec-related {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.ec-updates {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ec-update {
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  padding: var(--space-2);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ec-update__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.ec-update__summary {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.ec-question {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ec-question__text {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
}

.ec-question__answer {
  margin: 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.ec-question__form {
  display: flex;
  gap: var(--space-2);
}

.ec-input {
  flex: 1;
  min-width: 0;
  height: var(--size-control-sm);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  padding: 0 var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.ec-actions {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.ec-proposals {
  list-style: none;
  margin: 0;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.ec-note {
  margin: 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.ec-error {
  margin: 0;
  color: var(--status-danger-text, var(--text-primary));
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  word-break: break-word;
}

.ec-ctx-text,
.ec-ctx-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}
</style>
