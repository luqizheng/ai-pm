<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  BookOpen,
  Compass,
  MessageSquareText,
  MessagesSquare,
  Sparkles,
  Target
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type {
  AnalysisFinding,
  SourceItem
} from "../../requirements/requirement-entry";
import type { ReviewProposal } from "../review-contract";
import ComparisonCard from "./ComparisonCard.vue";
import DecisionPanel from "./DecisionPanel.vue";

/**
 * 评审详情（S02-UI-03 §4，右栏 65%）。
 *
 * A. 提案需求（标题/描述/类型/优先级/验收标准 + 提案编号与目标需求标注）
 * B. 来源证据（原始输入 / 发现结果 / 支撑文档——引用块与结构化提案区分）
 * C. AI 分析（完备性/歧义/重复/潜在冲突；带「AI 产出」outline 徽标，绝不表示为人类决策）
 * D. 与既有需求的并排比较（ComparisonCard）
 * 底部 = 人工决策面板（DecisionPanel）。
 */
interface Props {
  proposal: ReviewProposal | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "confirm"): void;
  (e: "changes"): void;
  (e: "reject"): void;
}>();

const { t } = useI18n();

const sourceIcons: Record<string, Component> = {
  user_request: MessageSquareText,
  discovery: Compass,
  knowledge: BookOpen,
  discussion: MessagesSquare,
  objective: Target
};

const sourceKindLabel = (kind: string) =>
  ({
    user_request: t("pm.requirements.sourceUserRequest"),
    discovery: t("pm.requirements.sourceDiscovery"),
    knowledge: t("pm.requirements.sourceKnowledge"),
    discussion: t("pm.requirements.sourceDiscussion"),
    objective: t("pm.requirements.sourceObjective")
  })[kind] ?? kind;

const analysisKindLabel = (kind: AnalysisFinding["kind"]) =>
  ({
    completeness: t("pm.requirements.analysisCompleteness"),
    ambiguity: t("pm.requirements.analysisAmbiguity"),
    duplication: t("pm.requirements.analysisDuplication"),
    conflict: t("pm.requirements.analysisConflict")
  })[kind];

const analysisKindVariant = (kind: AnalysisFinding["kind"]) =>
  kind === "conflict" ? "danger" : kind === "duplication" ? "warning" : "info";

const typeLabel = computed(() =>
  props.proposal?.type === "functional"
    ? t("pm.requirements.typeFunctional")
    : t("pm.requirements.typeNonFunctional")
);

const priorityLabel = computed(
  () =>
    (
      ({
        high: t("pm.requirements.priorityHigh"),
        medium: t("pm.requirements.priorityMedium"),
        low: t("pm.requirements.priorityLow")
      }) as const
    )[props.proposal?.priority ?? "medium"]
);

const targetLabel = computed(() =>
  props.proposal?.requirement_code
    ? t("pm.requirementReview.proposalRevision", {
        code: props.proposal.requirement_code
      })
    : t("pm.requirementReview.proposalNew")
);
</script>

<template>
  <section class="rrd" :aria-label="t('pm.requirementReview.detailTitle')">
    <EmptyState
      v-if="!props.proposal"
      :title="t('pm.requirementReview.detailEmptyTitle')"
      :description="t('pm.requirementReview.detailEmptyDesc')"
    />

    <template v-else>
      <article
        class="rrd__card"
        :aria-label="t('pm.requirementReview.proposalTitle')"
      >
        <div class="rrd__card-head">
          <h4 class="rrd__card-title">
            {{ t("pm.requirementReview.proposalTitle") }}
          </h4>
          <Badge variant="accent" size="sm" outline>
            {{ targetLabel }}
          </Badge>
        </div>
        <div class="rrd__identity">
          <span class="rrd__code">{{ props.proposal.code }}</span>
          <h3 class="rrd__name">{{ props.proposal.title }}</h3>
        </div>
        <dl class="rrd__facts">
          <div>
            <dt>{{ t("pm.requirementReview.proposalId") }}</dt>
            <dd class="rrd__mono">{{ props.proposal.code }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirements.detailType") }}</dt>
            <dd>{{ typeLabel }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirements.detailPriority") }}</dt>
            <dd>{{ priorityLabel }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirementReview.submittedAt") }}</dt>
            <dd>{{ props.proposal.submitted_at }}</dd>
          </div>
        </dl>
        <p class="rrd__desc">{{ props.proposal.description }}</p>
        <div class="rrd__criteria">
          <span class="rrd__criteria-label">
            {{ t("pm.requirementDetail.acceptTitle") }}
          </span>
          <ol class="rrd__criteria-list">
            <li
              v-for="criterion in props.proposal.acceptance_criteria"
              :key="criterion.id"
            >
              {{ criterion.statement }}
            </li>
          </ol>
          <p
            v-if="props.proposal.acceptance_criteria.length === 0"
            class="rrd__empty"
          >
            {{ t("pm.requirements.criteriaEmpty") }}
          </p>
        </div>
      </article>

      <article
        class="rrd__card"
        :aria-label="t('pm.requirementReview.evidenceTitle')"
      >
        <h4 class="rrd__card-title">
          {{ t("pm.requirementReview.evidenceTitle") }}
        </h4>
        <ul v-if="props.proposal.sources.length > 0" class="rrd__sources">
          <li
            v-for="source in props.proposal.sources"
            :key="source.id"
            class="rrd__source"
          >
            <component
              :is="sourceIcons[source.kind]"
              class="rrd__source-icon"
              aria-hidden="true"
            />
            <div class="rrd__source-body">
              <div class="rrd__source-head">
                <span class="rrd__source-kind">{{
                  sourceKindLabel(source.kind)
                }}</span>
                <span class="rrd__source-title">{{ source.title }}</span>
                <span v-if="source.ref" class="rrd__source-ref">{{
                  source.ref
                }}</span>
              </div>
              <blockquote class="rrd__source-quote">
                {{ source.detail }}
              </blockquote>
            </div>
          </li>
        </ul>
        <p v-else class="rrd__empty">{{ t("pm.requirements.sourcesEmpty") }}</p>
      </article>

      <article
        class="rrd__card"
        :aria-label="t('pm.requirementReview.analysisTitle')"
      >
        <div class="rrd__card-head">
          <h4 class="rrd__card-title">
            {{ t("pm.requirementReview.analysisTitle") }}
          </h4>
          <span class="rrd__ai-badge">
            <Sparkles aria-hidden="true" />
            <Badge variant="accent" size="sm" outline>
              {{ t("pm.requirementReview.analysisBadge") }}
            </Badge>
          </span>
        </div>
        <ul v-if="props.proposal.analysis.length > 0" class="rrd__findings">
          <li
            v-for="finding in props.proposal.analysis"
            :key="finding.id"
            class="rrd__finding"
          >
            <div class="rrd__finding-head">
              <Badge
                :variant="analysisKindVariant(finding.kind)"
                size="sm"
                :outline="finding.kind !== 'conflict'"
              >
                {{ analysisKindLabel(finding.kind) }}
              </Badge>
              <Badge variant="neutral" size="sm">
                {{
                  t(
                    `pm.requirements.severity${finding.severity.charAt(0).toUpperCase()}${finding.severity.slice(1)}`
                  )
                }}
              </Badge>
            </div>
            <p class="rrd__finding-body">{{ finding.body }}</p>
          </li>
        </ul>
        <p v-else class="rrd__empty">
          {{ t("pm.requirements.analysisEmpty") }}
        </p>
      </article>

      <ComparisonCard
        :existing-code="props.proposal.comparison.existing_code"
        :blocks="props.proposal.comparison.blocks"
      />

      <DecisionPanel
        :proposal="props.proposal"
        @confirm="emit('confirm')"
        @changes="emit('changes')"
        @reject="emit('reject')"
      />
    </template>
  </section>
</template>

<style scoped>
.rrd {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.rrd__card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rrd__card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.rrd__card-title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rrd__ai-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  color: var(--accent);
}

.rrd__ai-badge svg {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

.rrd__identity {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.rrd__code {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rrd__name {
  margin: 0;
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rrd__facts {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.rrd__facts dt {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rrd__facts dd {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.rrd__mono {
  font-family: var(--font-mono);
}

.rrd__desc {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.rrd__criteria {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rrd__criteria-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rrd__criteria-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding-left: var(--space-5);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rrd__criteria-list li::marker {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.rrd__empty {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}

.rrd__sources {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rrd__source {
  display: flex;
  gap: var(--space-2);
}

.rrd__source-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  margin-top: 2px;
  color: var(--text-secondary);
}

.rrd__source-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.rrd__source-head {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rrd__source-kind {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
}

.rrd__source-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.rrd__source-ref {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.rrd__source-quote {
  margin: 0;
  padding: var(--space-1) var(--space-3);
  border-left: var(--border-2) solid var(--border-default);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rrd__findings {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rrd__finding {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.rrd__finding-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rrd__finding-body {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

@media (max-width: 1100px) {
  .rrd__facts {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
