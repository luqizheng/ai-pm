<script setup lang="ts">
/**
 * S01-01 · 项目基线确认。
 *
 * UX 依据：`docs/ui/s01/s01-01-project-baseline-confirmation-ux-v1.md`
 *
 * 四个连续但可回看的用户任务：
 *   ① 理解提案 ② 审查变化 ③ 处理风险 ④ 正式确认
 *
 * 硬语义：
 *   - Proposal ≠ 正式 Baseline；当前生效基线始终可见且唯一。
 *   - 代码事实与 AI 推断必须在视觉与文案上可区分。
 *   - `ai_status`（AI 分析状态）≠ `human_decision`（人类裁决）。
 *   - 存在未裁决 P0 时，确认入口禁用并说明原因。
 *   - 正式确认走二次确认弹窗，弹窗复述关键事实。
 *   - 冲突 / 过期 / 权限不足不得出现「已生效」反馈。
 */
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/shell/components";
import { useConfirm } from "@/shell/composables/useConfirm";
import { formatDateTime } from "@/api/format";
import {
  confirmBaseline,
  decideBaselineRisk,
  getBaselineOverview,
  getBaselineProposal,
  listBaselineProposals,
  rejectBaselineProposal,
  reviseBaselineProposal
} from "@/api/initialization";
import type {
  BaselineDiffEntry,
  BaselineOverview,
  BaselineProposal,
  BaselineRisk
} from "@/api/types/initialization";

const { t } = useI18n();
const route = useRoute();
const toast = useToast();
const confirm = useConfirm();

const projectId = computed(() => String(route.params.projectId ?? ""));

const overview = ref<BaselineOverview | null>(null);
const proposals = ref<BaselineProposal[]>([]);
const proposal = ref<BaselineProposal | null>(null);
const loading = ref(false);
const busy = ref(false);
const confirming = ref(false);

/** 差异视图默认优先呈现变化内容，「未变化」可单独筛出。 */
const diffFilter = ref<"changed" | "all" | BaselineDiffEntry["diff_type"]>(
  "changed"
);

const diffOptions = computed(() => [
  { value: "changed", label: t("s01.initialization.baseline.diffViewDefault") },
  { value: "all", label: t("s01.initialization.baseline.diffViewFull") },
  { value: "added", label: t("s01.initialization.baseline.diffAdded") },
  { value: "modified", label: t("s01.initialization.baseline.diffModified") },
  { value: "removed", label: t("s01.initialization.baseline.diffRemoved") },
  { value: "unchanged", label: t("s01.initialization.baseline.diffUnchanged") }
]);

const visibleDiff = computed(() => {
  const entries = proposal.value?.diff ?? [];
  if (diffFilter.value === "all") return entries;
  if (diffFilter.value === "changed") {
    return entries.filter(entry => entry.diff_type !== "unchanged");
  }
  return entries.filter(entry => entry.diff_type === diffFilter.value);
});

/** §4.3：P0 必须由人类裁决；未裁决则阻断普通确认。 */
const unresolvedP0 = computed(() => {
  const risks = proposal.value?.risks ?? [];
  return risks.filter(risk => risk.risk_level === "P0" && !risk.human_decision);
});

const canConfirm = computed(
  () =>
    proposal.value?.status === "pending_review" &&
    unresolvedP0.value.length === 0
);

/** 生成中 / 失败 / 过期 一律不展示可用的确认按钮。 */
const confirmDisabledReason = computed(() => {
  const status = proposal.value?.status;
  if (!proposal.value) return null;
  if (status === "generating") {
    return t("s01.initialization.baseline.proposalStatus.generating");
  }
  if (status === "failed") {
    return t("s01.initialization.baseline.proposalStatus.failed");
  }
  if (status === "expired") {
    return t("s01.initialization.baseline.expiredHint");
  }
  if (unresolvedP0.value.length > 0) {
    return t("s01.initialization.baseline.unresolvedP0");
  }
  return null;
});

const diffStats = computed(() => {
  const entries = proposal.value?.diff ?? [];
  return {
    added: entries.filter(e => e.diff_type === "added").length,
    modified: entries.filter(e => e.diff_type === "modified").length,
    removed: entries.filter(e => e.diff_type === "removed").length
  };
});

function riskVariant(level: string): "danger" | "warning" | "neutral" {
  if (level === "P0") return "danger";
  if (level === "P1") return "warning";
  return "neutral";
}

/** 代码事实与 AI 推断必须视觉可区分，不能混排成同一可信度。 */
function sourceVariant(
  type: string
): "success" | "info" | "warning" | "neutral" {
  if (type === "code_fact" || type === "git_commit") return "success";
  if (type === "doc_fact") return "info";
  return "warning";
}

function failure(error: unknown) {
  toast.toast({
    variant: "danger",
    title: t("s01.initialization.common.operationFailed"),
    description: String(error)
  });
}

async function load() {
  loading.value = true;
  try {
    const [overviewResult, proposalResult] = await Promise.all([
      getBaselineOverview(projectId.value),
      listBaselineProposals(projectId.value, { size: 50 })
    ]);
    overview.value = overviewResult;
    proposals.value = proposalResult.items ?? [];
    const pending =
      proposals.value.find(item => item.status === "pending_review") ??
      proposals.value[0] ??
      null;
    if (pending) {
      proposal.value = await getBaselineProposal(projectId.value, pending.id);
    }
  } catch (error) {
    failure(error);
  } finally {
    loading.value = false;
  }
}

async function selectProposal(id: string) {
  try {
    proposal.value = await getBaselineProposal(projectId.value, id);
  } catch (error) {
    failure(error);
  }
}

async function revise() {
  if (!proposal.value) return;
  const instruction = window.prompt(
    t("s01.initialization.baseline.reviseInstruction")
  );
  if (!instruction) return;
  busy.value = true;
  try {
    proposal.value = await reviseBaselineProposal(
      projectId.value,
      proposal.value.id,
      { instruction }
    );
    toast.toast({
      variant: "success",
      title: t("s01.initialization.baseline.reviseSubmitted")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busy.value = false;
  }
}

async function reject() {
  if (!proposal.value) return;
  const reason = window.prompt(t("s01.initialization.baseline.rejectReason"));
  if (!reason) return;
  busy.value = true;
  try {
    await rejectBaselineProposal(projectId.value, proposal.value.id, {
      reason
    });
    toast.toast({
      variant: "success",
      title: t("s01.initialization.baseline.rejected")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busy.value = false;
  }
}

/** §4.3：每条风险独立裁决，保留操作人、时间与意见。 */
async function decideRisk(risk: BaselineRisk) {
  const note = window.prompt(
    t("s01.initialization.baseline.riskDecisionNote"),
    risk.human_decision ?? ""
  );
  if (note === null) return;
  if (risk.risk_level === "P0" && note.trim() === "") {
    toast.toast({
      variant: "danger",
      title: t("s01.initialization.baseline.riskDecisionNoteRequired")
    });
    return;
  }
  busy.value = true;
  try {
    await decideBaselineRisk(
      projectId.value,
      proposal.value!.id,
      risk.risk_id,
      {
        human_decision: "accepted",
        note
      }
    );
    toast.toast({
      variant: "success",
      title: t("s01.initialization.baseline.riskDecided")
    });
    proposal.value = await getBaselineProposal(
      projectId.value,
      proposal.value!.id
    );
  } catch (error) {
    failure(error);
  } finally {
    busy.value = false;
  }
}

/**
 * §4.4：二次确认弹窗复述关键事实，用户主动确认后才提交。
 * 冲突 / 过期 / 权限不足不得出现「已生效」反馈。
 */
async function confirmBaselineVersion() {
  if (!proposal.value || !canConfirm.value) return;

  const summary = [
    t("s01.initialization.baseline.confirmProposalVersion") +
      ": " +
      String(proposal.value.proposal_version),
    t("s01.initialization.baseline.confirmCommit") +
      ": " +
      (proposal.value.repositories.map(r => r.commit_sha).join(", ") || "—"),
    t("s01.initialization.baseline.confirmDiffStat") +
      ": +" +
      String(diffStats.value.added) +
      " / ~" +
      String(diffStats.value.modified) +
      " / -" +
      String(diffStats.value.removed),
    t("s01.initialization.baseline.confirmRisks") +
      ": " +
      String(unresolvedP0.value.length),
    t("s01.initialization.baseline.confirmResultingVersion") +
      ": " +
      String((overview.value?.current?.version ?? 0) + 1)
  ].join("\n");

  const ok = await confirm.confirm({
    title: t("s01.initialization.baseline.confirmTitle"),
    description: `${t("s01.initialization.baseline.confirmHint")}\n\n${summary}`,
    confirmLabel: t("s01.initialization.baseline.confirmSubmit")
  });
  if (!ok) return;

  confirming.value = true;
  try {
    await confirmBaseline(projectId.value, proposal.value.id, {
      confirmation_note: "",
      expected_current_baseline_id: overview.value?.current?.id ?? null,
      expected_proposal_version: proposal.value.proposal_version,
      idempotency_key: crypto.randomUUID()
    });
    toast.toast({
      variant: "success",
      title: t("s01.initialization.baseline.confirmed")
    });
    await load();
  } catch (error) {
    const text = String(error);
    const conflicted =
      text.includes("baseline_conflict") ||
      text.includes("proposal_version_conflict") ||
      text.includes("baseline_expired") ||
      text.includes("409");
    toast.toast({
      variant: "danger",
      title: conflicted
        ? t("s01.initialization.baseline.confirmFailed")
        : t("s01.initialization.baseline.confirmFailed"),
      description: text
    });
    await load();
  } finally {
    confirming.value = false;
  }
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('s01.initialization.baseline.pageTitle')"
      :subtitle="t('s01.initialization.baseline.subtitle')"
    />

    <!-- 当前生效基线：始终可见、唯一；审核新提案时也不隐藏 -->
    <section class="ui-card ui-card--current">
      <header class="ui-card__head">
        <div class="ui-card__text">
          <h2 class="ui-card__title">
            {{ t("s01.initialization.baseline.currentVersion") }}
          </h2>
          <p class="ui-card__hint">
            {{ t("s01.initialization.baseline.overview") }}
          </p>
        </div>
        <Badge v-if="overview?.current" variant="success">
          {{ t("s01.initialization.baseline.current") }}
        </Badge>
      </header>

      <p v-if="!overview?.current" class="ui-card__empty">
        {{ t("s01.initialization.baseline.noCurrentBaseline") }}
      </p>
      <dl v-else class="facts">
        <div class="facts__row">
          <dt>{{ t("s01.initialization.baseline.historyVersion") }}</dt>
          <dd>V{{ overview.current.version }}</dd>
        </div>
        <div class="facts__row">
          <dt>{{ t("s01.initialization.baseline.historyConfirmedBy") }}</dt>
          <dd>{{ overview.current.confirmed_by_name ?? "—" }}</dd>
        </div>
        <div class="facts__row">
          <dt>{{ t("s01.initialization.baseline.historyConfirmedAt") }}</dt>
          <dd>{{ formatDateTime(overview.current.confirmed_at) }}</dd>
        </div>
        <div class="facts__row">
          <dt>{{ t("s01.initialization.baseline.repositories") }}</dt>
          <dd>
            <span
              v-for="repo in overview.current.repositories"
              :key="repo.repository_id"
              class="repo-ref"
            >
              {{ repo.repository_name }}
              <code>{{ repo.commit_sha ?? "—" }}</code>
            </span>
          </dd>
        </div>
      </dl>
    </section>

    <!-- 提案列表 -->
    <section class="ui-card">
      <header class="ui-card__head">
        <div class="ui-card__text">
          <h2 class="ui-card__title">
            {{ t("s01.initialization.baseline.proposalReview") }}
          </h2>
          <p v-if="proposals.length > 0" class="ui-card__hint">
            {{
              t("s01.initialization.baseline.pendingProposalsCount", {
                count: proposals.length
              })
            }}
          </p>
        </div>
      </header>

      <p v-if="proposals.length === 0" class="ui-card__empty">
        {{ t("s01.initialization.common.empty") }}
      </p>
      <Select
        v-else
        :model-value="proposal?.id ?? ''"
        :options="
          proposals.map(item => ({
            value: item.id,
            label:
              t(`s01.initialization.baseline.proposalStatus.${item.status}`) +
              ' · v' +
              String(item.proposal_version)
          }))
        "
        :aria-label="t('s01.initialization.baseline.proposalReview')"
        @update:model-value="selectProposal(String($event ?? ''))"
      />
    </section>

    <template v-if="proposal">
      <!-- 任务一：理解提案 -->
      <section class="ui-card">
        <h2 class="ui-card__title">
          {{ t("s01.initialization.baseline.understandProposal") }}
        </h2>

        <div class="block">
          <p class="block__label">
            {{ t("s01.initialization.baseline.generationReason") }}
          </p>
          <p class="block__body">{{ proposal.generation_reason }}</p>
        </div>

        <div class="block">
          <p class="block__label">
            {{ t("s01.initialization.baseline.sources") }}
          </p>
          <ul class="source-list">
            <li
              v-for="(source, index) in proposal.sources"
              :key="`${source.label}-${index}`"
              class="source"
            >
              <Badge :variant="sourceVariant(source.source_type)">
                {{
                  t(
                    `s01.initialization.cognition.evidenceType.${source.source_type}`
                  )
                }}
              </Badge>
              <span class="source__label">{{ source.label }}</span>
              <span v-if="source.locator" class="source__locator">
                <code>{{ source.locator }}</code>
              </span>
            </li>
          </ul>
        </div>

        <div class="block">
          <p class="block__label">
            {{ t("s01.initialization.baseline.repositories") }}
          </p>
          <ul class="source-list">
            <li
              v-for="repo in proposal.repositories"
              :key="repo.repository_id"
              class="source"
            >
              <span class="source__label">{{ repo.repository_name }}</span>
              <span class="source__locator"
                ><code>{{ repo.repository_url }}</code></span
              >
              <span class="source__locator">
                {{ repo.branch_name ?? "—" }} @ {{ repo.commit_sha ?? "—" }}
              </span>
            </li>
          </ul>
        </div>

        <div class="block">
          <p class="block__label">
            {{ t("s01.initialization.baseline.contentFull") }}
          </p>
          <div class="sections">
            <article
              v-for="section in proposal.content"
              :key="section.key"
              class="section"
            >
              <h3 class="section__title">{{ section.title }}</h3>
              <p class="section__body">{{ section.body }}</p>
            </article>
          </div>
        </div>
      </section>

      <!-- 任务二：审查变化 -->
      <section class="ui-card">
        <header class="ui-card__head">
          <div class="ui-card__text">
            <h2 class="ui-card__title">
              {{ t("s01.initialization.baseline.reviewDiff") }}
            </h2>
            <p v-if="!overview?.current" class="ui-card__hint">
              {{ t("s01.initialization.baseline.firstTimeNotice") }}
            </p>
          </div>
          <Select
            v-model="diffFilter"
            :options="diffOptions"
            :aria-label="t('s01.initialization.baseline.diffFilter')"
          />
        </header>

        <p v-if="visibleDiff.length === 0" class="ui-card__empty">
          {{ t("s01.initialization.common.empty") }}
        </p>
        <ul v-else class="diff-list">
          <li
            v-for="(entry, index) in visibleDiff"
            :key="`${entry.section}-${index}`"
            class="diff"
            :class="`diff--${entry.diff_type}`"
          >
            <div class="diff__head">
              <Badge
                :variant="
                  entry.diff_type === 'added'
                    ? 'success'
                    : entry.diff_type === 'removed'
                      ? 'danger'
                      : entry.diff_type === 'modified'
                        ? 'warning'
                        : 'neutral'
                "
              >
                {{
                  t(
                    `s01.initialization.baseline.diff${entry.diff_type[0].toUpperCase()}${entry.diff_type.slice(1)}`
                  )
                }}
              </Badge>
              <span class="diff__section">{{ entry.section }}</span>
              <span v-if="entry.field" class="diff__field">{{
                entry.field
              }}</span>
            </div>
            <div v-if="entry.diff_type === 'modified'" class="diff__pair">
              <div class="diff__side">
                <p class="diff__sideLabel">
                  {{ t("s01.initialization.baseline.diffBefore") }}
                </p>
                <p class="diff__value">{{ entry.before ?? "—" }}</p>
              </div>
              <div class="diff__side">
                <p class="diff__sideLabel">
                  {{ t("s01.initialization.baseline.diffAfter") }}
                </p>
                <p class="diff__value">{{ entry.after ?? "—" }}</p>
              </div>
            </div>
            <div
              v-else-if="entry.diff_type !== 'unchanged'"
              class="diff__single"
            >
              <p class="diff__value">
                {{ entry.after ?? entry.before ?? "—" }}
              </p>
            </div>
            <p v-if="entry.reason" class="diff__reason">
              {{ t("s01.initialization.baseline.diffReason") }}:
              {{ entry.reason }}
            </p>
          </li>
        </ul>
      </section>

      <!-- 任务三：处理风险 -->
      <section class="ui-card">
        <header class="ui-card__head">
          <div class="ui-card__text">
            <h2 class="ui-card__title">
              {{ t("s01.initialization.baseline.handleRisk") }}
            </h2>
            <p
              v-if="unresolvedP0.length > 0"
              class="ui-card__hint ui-card__hint--danger"
            >
              {{ t("s01.initialization.baseline.p0BlockedHint") }}
            </p>
          </div>
        </header>

        <p v-if="(proposal.risks ?? []).length === 0" class="ui-card__empty">
          {{ t("s01.initialization.common.empty") }}
        </p>
        <ul v-else class="risk-list">
          <li v-for="risk in proposal.risks" :key="risk.risk_id" class="risk">
            <div class="risk__head">
              <Badge :variant="riskVariant(risk.risk_level)">
                {{ risk.risk_level }}
              </Badge>
              <span class="risk__title">{{ risk.risk_title }}</span>
              <Badge v-if="risk.human_decision" variant="success">
                {{ t("s01.initialization.baseline.humanDecided") }}
              </Badge>
              <Badge v-else-if="risk.ai_status" variant="warning">
                {{ t("s01.initialization.baseline.aiMarkedResolved") }}
              </Badge>
              <Badge v-else variant="neutral">
                {{ t("s01.initialization.baseline.undecided") }}
              </Badge>
            </div>
            <p class="risk__description">{{ risk.risk_description }}</p>
            <dl class="facts">
              <div class="facts__row">
                <dt>{{ t("s01.initialization.baseline.aiStatus") }}</dt>
                <dd>{{ risk.ai_status ?? "—" }}</dd>
              </div>
              <div class="facts__row">
                <dt>{{ t("s01.initialization.baseline.humanDecision") }}</dt>
                <dd>{{ risk.human_decision ?? "—" }}</dd>
              </div>
              <div v-if="risk.evidence_ref" class="facts__row">
                <dt>{{ t("s01.initialization.baseline.evidence") }}</dt>
                <dd>
                  <code>{{ risk.evidence_ref }}</code>
                </dd>
              </div>
            </dl>
            <Button
              v-if="!risk.human_decision"
              size="sm"
              variant="secondary"
              :disabled="busy"
              @click="decideRisk(risk)"
            >
              {{ t("s01.initialization.baseline.decideRisk") }}
            </Button>
          </li>
        </ul>
      </section>

      <!-- 任务四：正式确认 -->
      <section class="ui-card ui-card--confirm">
        <h2 class="ui-card__title">
          {{ t("s01.initialization.baseline.confirmBaseline") }}
        </h2>

        <p
          v-if="confirmDisabledReason"
          class="ui-card__hint ui-card__hint--danger"
        >
          {{ confirmDisabledReason }}
        </p>

        <div class="actions">
          <Button
            size="sm"
            variant="secondary"
            :disabled="busy"
            @click="revise"
          >
            {{ t("s01.initialization.baseline.revise") }}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            :disabled="busy"
            @click="reject"
          >
            {{ t("s01.initialization.baseline.reject") }}
          </Button>
          <Button
            :loading="confirming"
            :disabled="!canConfirm || confirming"
            @click="confirmBaselineVersion"
          >
            {{ t("s01.initialization.baseline.confirmSubmit") }}
          </Button>
        </div>
      </section>
    </template>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.ui-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.ui-card--current {
  border-color: var(--accent);
}

.ui-card--confirm {
  border-color: var(--warning);
}

.ui-card__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.ui-card__title {
  margin: 0;
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ui-card__hint {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.ui-card__hint--danger {
  color: var(--danger-strong);
}

.ui-card__empty {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
}

.facts {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.facts__row {
  display: flex;
  gap: var(--space-3);
  font-size: var(--font-size-small);
}

.facts__row dt {
  min-width: 140px;
  color: var(--text-secondary);
}

.facts__row dd {
  margin: 0;
  color: var(--text-primary);
}

.repo-ref {
  display: inline-flex;
  gap: var(--space-1);
  margin-right: var(--space-3);
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.block__label {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.block__body {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-primary);
}

.source-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.source {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-small);
}

.source__label {
  color: var(--text-primary);
}

.source__locator {
  color: var(--text-tertiary);
  font-size: var(--font-size-caption);
}

.sections {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.section {
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
}

.section__title {
  margin: 0 0 var(--space-1);
  font-size: var(--font-size-small);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.section__body {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-secondary);
  white-space: pre-wrap;
}

.diff-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.diff {
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.diff--added {
  border-left: 3px solid var(--success);
}

.diff--removed {
  border-left: 3px solid var(--danger);
}

.diff--modified {
  border-left: 3px solid var(--warning);
}

.diff--unchanged {
  border-left: 3px solid var(--border-subtle);
}

.diff__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
}

.diff__section {
  font-size: var(--font-size-small);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.diff__field {
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.diff__pair {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
}

.diff__sideLabel {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.diff__value {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-primary);
}

.diff__reason {
  margin: var(--space-2) 0 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.risk-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.risk {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.risk__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.risk__title {
  font-size: var(--font-size-small);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.risk__description {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-secondary);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
}
</style>
