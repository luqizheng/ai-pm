<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  BookOpen,
  ExternalLink,
  History,
  MessageSquareText,
  MessagesSquare,
  Pencil,
  ScanSearch,
  ShieldCheck,
  Target,
  XCircle
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import type { BadgeVariant } from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Input from "@/components/ui/input/Input.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import type { RequirementEntry } from "../requirement-entry";

/**
 * 需求详情面板（S02-UI-01 §6.2 + §8）。
 *
 * 五节：Overview / Acceptance Criteria / Sources & Traceability /
 * Revision History / Related Items；原始来源材料用引用块与结构化需求视觉区分。
 * 操作按状态显隐（§8）：不适用不展示；人类确认/拒绝是显式操作（弹窗在页根编排）。
 * editing = 行内编辑表单态（draft 才允许编辑）。
 */
interface Props {
  entry: RequirementEntry;
  editing: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (
    e: "action",
    action:
      | "edit"
      | "propose"
      | "confirm"
      | "reject"
      | "resolve"
      | "history"
      | "open"
  ): void;
  /**
   * 保存编辑 = 起草/更新一份正文修订（S02-I1 语义：后端仅支持正文修订，
   * title/type/priority 创建后不可改）。payload 为修订正文 + 一句话变更描述。
   */
  (e: "save", payload: { markdown: string; summary: string }): void;
  (e: "cancel"): void;
}>();

const { t } = useI18n();

const sourceIcons: Record<string, Component> = {
  user_request: MessageSquareText,
  discovery: ScanSearch,
  knowledge: BookOpen,
  discussion: MessagesSquare,
  objective: Target
};

const form = reactive({
  summary: "",
  markdown: ""
});

watch(
  () => [props.entry.id, props.editing] as const,
  ([, editing]) => {
    if (!editing) return;
    form.summary = "";
    form.markdown = props.entry.body_markdown ?? props.entry.description;
  },
  { immediate: true }
);

const typeLabel = computed(() =>
  props.entry.type === "functional"
    ? t("pm.requirements.typeFunctional")
    : t("pm.requirements.typeNonFunctional")
);

const priorityLabel = computed(
  () =>
    ({
      high: t("pm.requirements.priorityHigh"),
      medium: t("pm.requirements.priorityMedium"),
      low: t("pm.requirements.priorityLow")
    })[props.entry.priority]
);

const statusLabel = computed(
  () =>
    ({
      draft: t("pm.requirements.statusDraft"),
      pending_confirmation: t("pm.requirements.statusPending"),
      confirmed: t("pm.requirements.statusConfirmed"),
      conflict: t("pm.requirements.statusConflict")
    })[props.entry.status]
);

const statusTone = computed<StatusIndicatorTone>(
  () =>
    (
      ({
        draft: "neutral",
        pending_confirmation: "warning",
        confirmed: "success",
        conflict: "danger"
      }) as const
    )[props.entry.status]
);

const revisionStateLabel = (state: string) =>
  ({
    draft: t("pm.requirements.revStateDraft"),
    pending_confirmation: t("pm.requirements.revStatePending"),
    approved: t("pm.requirements.revStateApproved"),
    rejected: t("pm.requirements.revStateRejected"),
    superseded: t("pm.requirements.revStateSuperseded")
  })[state] ?? state;

const revisionStateVariant = (state: string): BadgeVariant =>
  (
    ({
      draft: "neutral",
      pending_confirmation: "warning",
      approved: "success",
      rejected: "danger",
      superseded: "neutral"
    }) as const
  )[state] ?? "neutral";

const latestRevisions = computed(() => props.entry.revisions.slice(0, 3));

const canEdit = computed(() => props.entry.status === "draft");
const canPropose = computed(
  () => props.entry.status === "draft" || props.entry.status === "confirmed"
);
const canConfirm = computed(
  () => props.entry.status === "pending_confirmation"
);
const canResolve = computed(() => props.entry.status === "conflict");

const sourceKindLabel = (kind: string) =>
  ({
    user_request: t("pm.requirements.sourceUserRequest"),
    discovery: t("pm.requirements.sourceDiscovery"),
    knowledge: t("pm.requirements.sourceKnowledge"),
    discussion: t("pm.requirements.sourceDiscussion"),
    objective: t("pm.requirements.sourceObjective"),
    external_feedback: t("pm.requirements.sourceExternalFeedback"),
    intake: t("pm.requirements.sourceIntake"),
    import: t("pm.requirements.sourceImport")
  })[kind] ?? kind;

const relatedKindLabel = (kind: string) =>
  ({
    requirement: t("pm.requirements.relatedRequirement"),
    objective: t("pm.requirements.relatedObjective"),
    task: t("pm.requirements.relatedTask")
  })[kind] ?? kind;

/** 验收标准承载词表（Q-13 扩枚举）与裁决词表（Q-14）标签。 */
const criterionKindLabel = (kind: string) =>
  ({
    manual: t("pm.requirements.critKindManual"),
    automated_test: t("pm.requirements.critKindAutomatedTest"),
    artifact_exists: t("pm.requirements.critKindArtifactExists"),
    review: t("pm.requirements.critKindReview"),
    performance: t("pm.requirements.critKindPerformance"),
    security: t("pm.requirements.critKindSecurity")
  })[kind] ?? kind;

const criterionVerdictLabel = (verdict: string) =>
  ({
    pending: t("pm.requirements.critVerdictPending"),
    passed: t("pm.requirements.critVerdictPassed"),
    failed: t("pm.requirements.critVerdictFailed")
  })[verdict] ?? verdict;

function onSave() {
  emit("save", {
    markdown: form.markdown.trim(),
    summary: form.summary.trim()
  });
}
</script>

<template>
  <article class="req-detail">
    <header class="req-detail__head">
      <div class="req-detail__title-group">
        <span class="req-detail__code">{{ entry.code }}</span>
        <h3 class="req-detail__title">{{ entry.title }}</h3>
      </div>
      <div class="req-detail__status">
        <StatusIndicator :status="statusTone" size="sm" />
        <span>{{ statusLabel }}</span>
      </div>
    </header>

    <div v-if="!editing" class="req-detail__actions">
      <Button
        v-if="canEdit"
        variant="secondary"
        size="sm"
        @click="emit('action', 'edit')"
      >
        <Pencil aria-hidden="true" />
        {{ t("pm.requirements.actionEdit") }}
      </Button>
      <Button
        v-if="canPropose"
        variant="secondary"
        size="sm"
        @click="emit('action', 'propose')"
      >
        {{ t("pm.requirements.actionPropose") }}
      </Button>
      <Button
        v-if="canConfirm"
        variant="primary"
        size="sm"
        @click="emit('action', 'confirm')"
      >
        <ShieldCheck aria-hidden="true" />
        {{ t("pm.requirements.actionConfirm") }}
      </Button>
      <Button
        v-if="canConfirm"
        variant="secondary"
        size="sm"
        @click="emit('action', 'reject')"
      >
        <XCircle aria-hidden="true" />
        {{ t("pm.requirements.actionReject") }}
      </Button>
      <Button
        v-if="canResolve"
        variant="primary"
        size="sm"
        @click="emit('action', 'resolve')"
      >
        {{ t("pm.requirements.actionResolve") }}
      </Button>
      <Button variant="ghost" size="sm" @click="emit('action', 'history')">
        <History aria-hidden="true" />
        {{ t("pm.requirements.actionHistory") }}
      </Button>
      <Button variant="ghost" size="sm" @click="emit('action', 'open')">
        <ExternalLink aria-hidden="true" />
        {{ t("pm.requirementDetail.openFull") }}
      </Button>
    </div>

    <form v-if="editing" class="req-detail__form" @submit.prevent="onSave">
      <div class="req-detail__field-row">
        <label class="req-detail__field">
          <span class="req-detail__field-label">{{
            t("pm.requirements.editTitleField")
          }}</span>
          <Input :model-value="entry.title" size="sm" disabled />
          <span class="req-detail__field-hint">{{
            t("pm.requirements.editImmutableHint")
          }}</span>
        </label>
      </div>
      <div class="req-detail__field-row">
        <label class="req-detail__field">
          <span class="req-detail__field-label">{{
            t("pm.requirements.detailType")
          }}</span>
          <Input :model-value="typeLabel" size="sm" disabled />
        </label>
        <label class="req-detail__field">
          <span class="req-detail__field-label">{{
            t("pm.requirements.detailPriority")
          }}</span>
          <Input :model-value="priorityLabel" size="sm" disabled />
        </label>
      </div>
      <label class="req-detail__field">
        <span class="req-detail__field-label">{{
          t("pm.requirements.editSummary")
        }}</span>
        <Input
          v-model="form.summary"
          size="sm"
          :placeholder="t('pm.requirements.editSummaryPlaceholder')"
        />
      </label>
      <label class="req-detail__field">
        <span class="req-detail__field-label">{{
          t("pm.requirements.editBody")
        }}</span>
        <Textarea v-model="form.markdown" size="sm" :rows="12" required />
        <span class="req-detail__field-hint">{{
          t("pm.requirements.editBodyHint")
        }}</span>
      </label>
      <div class="req-detail__form-actions">
        <Button
          variant="primary"
          size="sm"
          type="button"
          :disabled="form.markdown.trim().length === 0"
          @click="onSave"
        >
          {{ t("pm.requirements.editSave") }}
        </Button>
        <Button variant="ghost" size="sm" type="button" @click="emit('cancel')">
          {{ t("pm.requirements.editCancel") }}
        </Button>
      </div>
    </form>

    <template v-else>
      <section
        class="req-detail__section"
        :aria-label="t('pm.requirements.detailOverview')"
      >
        <h4 class="req-detail__section-title">
          {{ t("pm.requirements.detailOverview") }}
        </h4>
        <dl class="req-detail__facts">
          <div>
            <dt>{{ t("pm.requirements.detailType") }}</dt>
            <dd>{{ typeLabel }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirements.detailPriority") }}</dt>
            <dd>{{ priorityLabel }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirements.detailUpdated") }}</dt>
            <dd>{{ entry.updated_at }}</dd>
          </div>
          <div>
            <dt>{{ t("pm.requirements.detailSourceLabel") }}</dt>
            <dd>{{ entry.source_label }}</dd>
          </div>
        </dl>
        <p v-if="entry.description" class="req-detail__desc">
          {{ entry.description }}
        </p>
        <pre v-if="entry.body_markdown" class="req-detail__body">{{
          entry.body_markdown
        }}</pre>
      </section>

      <section
        class="req-detail__section"
        :aria-label="t('pm.requirements.detailAcceptance')"
      >
        <h4 class="req-detail__section-title">
          {{ t("pm.requirements.detailAcceptance") }}
        </h4>
        <ol class="req-detail__criteria">
          <li
            v-for="criterion in entry.acceptance_criteria"
            :key="criterion.id"
            class="req-detail__criterion"
          >
            <span class="req-detail__criterion-text">{{
              criterion.statement
            }}</span>
            <Badge
              v-if="criterion.verification_kind"
              variant="neutral"
              size="sm"
              outline
            >
              {{ criterionKindLabel(criterion.verification_kind) }}
            </Badge>
            <Badge
              v-if="criterion.verdict && criterion.verdict !== 'pending'"
              :variant="criterion.verdict === 'passed' ? 'success' : 'danger'"
              size="sm"
            >
              {{ criterionVerdictLabel(criterion.verdict) }}
            </Badge>
          </li>
        </ol>
        <p
          v-if="entry.acceptance_criteria.length === 0"
          class="req-detail__empty"
        >
          {{ t("pm.requirements.criteriaEmpty") }}
        </p>
      </section>

      <section
        class="req-detail__section"
        :aria-label="t('pm.requirements.detailSources')"
      >
        <h4 class="req-detail__section-title">
          {{ t("pm.requirements.detailSources") }}
        </h4>
        <ul class="req-detail__sources">
          <li
            v-for="source in entry.sources"
            :key="source.id"
            class="req-detail__source"
          >
            <component
              :is="sourceIcons[source.kind] ?? BookOpen"
              class="req-detail__source-icon"
              aria-hidden="true"
            />
            <div class="req-detail__source-body">
              <div class="req-detail__source-head">
                <span class="req-detail__source-kind">{{
                  sourceKindLabel(source.kind)
                }}</span>
                <span class="req-detail__source-title">{{ source.title }}</span>
                <span v-if="source.ref" class="req-detail__source-ref">{{
                  source.ref
                }}</span>
              </div>
              <blockquote class="req-detail__source-quote">
                {{ source.detail }}
              </blockquote>
            </div>
          </li>
        </ul>
        <p v-if="entry.sources.length === 0" class="req-detail__empty">
          {{ t("pm.requirements.sourcesEmpty") }}
        </p>
      </section>

      <section
        class="req-detail__section"
        :aria-label="t('pm.requirements.detailRevisions')"
      >
        <div class="req-detail__section-head">
          <h4 class="req-detail__section-title">
            {{ t("pm.requirements.detailRevisions") }}
          </h4>
          <button
            type="button"
            class="req-detail__history-link"
            @click="emit('action', 'history')"
          >
            {{ t("pm.requirements.revisionViewHistory") }}
          </button>
        </div>
        <ul class="req-detail__revisions">
          <li
            v-for="revision in latestRevisions"
            :key="revision.id"
            class="req-detail__revision"
          >
            <span class="req-detail__revision-version">
              {{
                t("pm.requirements.revisionVersion", {
                  version: revision.version
                })
              }}
            </span>
            <Badge :variant="revisionStateVariant(revision.state)" size="sm">
              {{ revisionStateLabel(revision.state) }}
            </Badge>
            <span class="req-detail__revision-summary">{{
              revision.summary
            }}</span>
            <span class="req-detail__revision-meta">
              {{ revision.author }} · {{ revision.created_at }}
            </span>
          </li>
        </ul>
      </section>

      <section
        class="req-detail__section"
        :aria-label="t('pm.requirements.detailRelated')"
      >
        <h4 class="req-detail__section-title">
          {{ t("pm.requirements.detailRelated") }}
        </h4>
        <ul v-if="entry.related.length > 0" class="req-detail__related">
          <li
            v-for="item in entry.related"
            :key="`${item.kind}-${item.code}`"
            class="req-detail__related-item"
          >
            <span class="req-detail__related-kind">{{
              relatedKindLabel(item.kind)
            }}</span>
            <span class="req-detail__related-code">{{ item.code }}</span>
            <span class="req-detail__related-title">{{ item.title }}</span>
          </li>
        </ul>
        <p v-else class="req-detail__empty">
          {{ t("pm.requirements.relatedEmpty") }}
        </p>
      </section>
    </template>
  </article>
</template>

<style scoped>
.req-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.req-detail__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.req-detail__title-group {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.req-detail__code {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.req-detail__title {
  margin: 0;
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.req-detail__status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  white-space: nowrap;
}

.req-detail__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.req-detail__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.req-detail__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.req-detail__field-row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.req-detail__field-label {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.req-detail__field-hint {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.req-detail__form-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.req-detail__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-top: var(--space-3);
  border-top: var(--hairline) solid var(--border-default);
}

.req-detail__section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.req-detail__section-title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.req-detail__history-link {
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
  cursor: pointer;
}

.req-detail__history-link:hover {
  color: var(--accent-hover);
}

.req-detail__history-link:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.req-detail__facts {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.req-detail__facts dt {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-detail__facts dd {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.req-detail__desc {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.req-detail__body {
  margin: 0;
  padding: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 18rem;
  overflow: auto;
}

.req-detail__criteria {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding-left: var(--space-5);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.req-detail__criteria li::marker {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.req-detail__criterion {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
  min-width: 0;
}

.req-detail__criterion-text {
  min-width: 0;
  word-break: break-word;
}

.req-detail__sources {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.req-detail__source {
  display: flex;
  gap: var(--space-2);
  min-width: 0;
}

.req-detail__source-icon {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  flex: none;
  margin-top: 2px;
  color: var(--text-secondary);
}

.req-detail__source-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.req-detail__source-head {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.req-detail__source-kind {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
}

.req-detail__source-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.req-detail__source-ref {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

/* 原始材料引用块：左侧竖线 + 次文字，与结构化需求正文视觉区分 */
.req-detail__source-quote {
  margin: 0;
  padding: var(--space-1) var(--space-3);
  border-left: var(--border-2) solid var(--border-default);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.req-detail__revisions {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.req-detail__revision {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.req-detail__revision-version {
  font-family: var(--font-mono);
  color: var(--text-primary);
}

.req-detail__revision-summary {
  color: var(--text-primary);
  min-width: 0;
}

.req-detail__revision-meta {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.req-detail__related {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.req-detail__related-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.req-detail__related-kind {
  flex: none;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-detail__related-code {
  font-family: var(--font-mono);
  color: var(--text-primary);
}

.req-detail__related-title {
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.req-detail__empty {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}

@media (max-width: 1100px) {
  .req-detail__facts {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
