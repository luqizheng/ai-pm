<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { CircleCheck, LoaderCircle, TriangleAlert } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import AiResponseCard from "../card/AiResponseCard.vue";
import { aiIdentity } from "../fixtures";
import { createMockSubmit } from "../submitAdapter";
import type {
  AiEvidence,
  AiIdentity,
  ArtifactKind,
  ArtifactPayload,
  CardDensity,
  FooterAction,
  InteractionStatus,
  SubmitResult
} from "../types";
import { useAiInteraction } from "../useAiInteraction";

/**
 * 产物卡 —— 版本信息 + Markdown 预览 + 差异切换 + 本地编辑。
 *
 * 接收草稿（accepted）与提交至既有业务流程（submitted）是两件事；
 * 任一提交流程完成后卡片转为只读，结果只写「已提交至该流程」。
 */
const props = withDefaults(
  defineProps<{
    payload: ArtifactPayload;
    identity?: AiIdentity;
    timestamp?: string;
    density?: CardDensity;
    submit?: (payload: unknown) => Promise<SubmitResult>;
    initialStatus?: InteractionStatus;
    evidence?: AiEvidence;
  }>(),
  {
    identity: () => aiIdentity,
    timestamp: "",
    density: "default",
    submit: undefined,
    initialStatus: "presented",
    evidence: undefined
  }
);

const emit = defineEmits<{
  action: [key: string];
  submitted: [result: SubmitResult];
}>();

const { t } = useI18n();

const mockSubmit = createMockSubmit({ latencyMs: 600 });
const submitAdapter = (payload: unknown) =>
  (props.submit ?? mockSubmit)(payload);

const state = useAiInteraction({
  initialStatus: props.initialStatus,
  submit: submitAdapter
});
const { status, phase, result, error, submitting, isTerminal } = state;

const workflow = useAiInteraction({ submit: submitAdapter });
const workflowPhase = workflow.phase;
const workflowResult = workflow.result;
const workflowError = workflow.error;
const workflowSubmitted = workflow.submitted;

const TYPE_KEY: Record<ArtifactKind, string> = {
  "requirement-doc": "ai.artifact.type.requirementDoc",
  "technical-design": "ai.artifact.type.technicalDesign",
  "project-plan": "ai.artifact.type.projectPlan",
  "test-plan": "ai.artifact.type.testPlan",
  "knowledge-doc": "ai.artifact.type.knowledgeDoc",
  "code-change-summary": "ai.artifact.type.codeChangeSummary"
};

const view = ref<"preview" | "diff">("preview");
const editing = ref(false);
const edited = ref(false);
const draft = ref("");
const content = ref(props.payload.contentMarkdown);
const workflowDialogOpen = ref(false);

const typeLabel = computed(() => t(TYPE_KEY[props.payload.artifactType]));
const createdAtLabel = computed(
  () => props.payload.version.createdAt?.slice(0, 10) ?? ""
);
const locked = computed(() => isTerminal.value || workflowSubmitted.value);
const accepted = computed(() => status.value === "accepted");

const actions = computed<FooterAction[]>(() => {
  if (editing.value || locked.value) return [];
  const list: FooterAction[] = [
    {
      key: "preview",
      label: t("ai.artifact.actions.preview"),
      variant: "ghost"
    },
    {
      key: "edit",
      label: t("ai.artifact.actions.edit"),
      variant: "secondary"
    },
    {
      key: "accept",
      label: t("ai.artifact.actions.accept"),
      variant: "primary"
    },
    {
      key: "reject",
      label: t("ai.artifact.actions.reject"),
      variant: "danger"
    },
    {
      key: "create-proposal",
      label: t("ai.artifact.actions.createProposal"),
      variant: "ghost"
    }
  ];
  if (props.payload.workflowLabel) {
    list.push({
      key: "submit-workflow",
      label: t("ai.artifact.actions.submitWorkflow", {
        workflow: props.payload.workflowLabel
      }),
      variant: "secondary"
    });
  }
  return list;
});

function resetLocalState() {
  content.value = props.payload.contentMarkdown;
  view.value = "preview";
  editing.value = false;
  edited.value = false;
  draft.value = "";
}

watch(() => props.payload, resetLocalState);

function startEdit() {
  if (locked.value) return;
  draft.value = content.value;
  editing.value = true;
  view.value = "preview";
  state.begin();
}

function cancelEdit() {
  editing.value = false;
  draft.value = "";
}

function saveEdit() {
  content.value = draft.value;
  edited.value = true;
  editing.value = false;
  draft.value = "";
  state.saveDraft(content.value);
}

async function acceptDraft() {
  state.begin();
  const res = await state.accept({
    decision: "accept",
    artifactType: props.payload.artifactType,
    version: props.payload.version.label,
    edited: edited.value
  });
  if (res) emit("submitted", res);
}

async function rejectArtifact() {
  state.begin();
  const res = await state.reject({
    decision: "reject",
    artifactType: props.payload.artifactType,
    version: props.payload.version.label
  });
  if (res) emit("submitted", res);
}

function openWorkflowDialog() {
  workflowDialogOpen.value = true;
}

async function confirmWorkflowSubmit() {
  workflowDialogOpen.value = false;
  const res = await workflow.submit({
    artifactType: props.payload.artifactType,
    version: props.payload.version.label,
    workflow: props.payload.workflowLabel,
    content: content.value
  });
  if (res) emit("submitted", res);
}

async function retryWorkflow() {
  const res = await workflow.retry();
  if (res) emit("submitted", res);
}

async function retry() {
  const res = await state.retry();
  if (res) emit("submitted", res);
}

function onAction(key: string) {
  if (key === "preview") {
    view.value = "preview";
    editing.value = false;
    return;
  }
  if (key === "edit") {
    startEdit();
    return;
  }
  if (key === "accept") {
    void acceptDraft();
    return;
  }
  if (key === "reject") {
    void rejectArtifact();
    return;
  }
  if (key === "submit-workflow") {
    openWorkflowDialog();
    return;
  }
  emit("action", key);
}

onBeforeUnmount(() => {
  editing.value = false;
});
</script>

<template>
  <AiResponseCard
    type="artifact"
    :identity="props.identity"
    :evidence="evidence"
    :status="status"
    :title="typeLabel"
    :timestamp="props.timestamp"
    :density="props.density"
    :actions="actions"
    :submitting="submitting"
    :submission-phase="phase"
    :submission-result="result"
    :submission-error="error"
    :footer-wrap="true"
    @action="onAction"
    @retry="retry"
  >
    <template #summary>
      <div class="ai-artifact__meta">
        <Badge variant="info" size="sm">{{ typeLabel }}</Badge>
        <span class="ai-artifact__version">
          {{ t("ai.artifact.version") }} {{ props.payload.version.label }}
        </span>
        <span v-if="props.payload.version.author" class="ai-artifact__author">
          {{ t("ai.artifact.author") }} {{ props.payload.version.author }}
        </span>
        <time
          v-if="createdAtLabel"
          class="ai-artifact__time"
          :datetime="props.payload.version.createdAt"
        >
          {{ createdAtLabel }}
        </time>
        <Badge v-if="edited" variant="warning" size="sm">
          {{ t("ai.artifact.edited") }}
        </Badge>
        <Badge v-if="locked" variant="neutral" size="sm">
          {{ t("ai.artifact.locked") }}
        </Badge>
      </div>
    </template>

    <div v-if="!editing && props.payload.diff" class="ai-artifact__views">
      <button
        type="button"
        class="ai-artifact__view-btn"
        :class="{ 'is-active': view === 'preview' }"
        :aria-pressed="view === 'preview'"
        @click="view = 'preview'"
      >
        {{ t("ai.artifact.preview") }}
      </button>
      <button
        type="button"
        class="ai-artifact__view-btn"
        :class="{ 'is-active': view === 'diff' }"
        :aria-pressed="view === 'diff'"
        @click="view = 'diff'"
      >
        {{ t("ai.artifact.diff") }}
      </button>
    </div>

    <section v-if="editing" class="ai-artifact__editor">
      <Textarea
        v-model="draft"
        :rows="10"
        :placeholder="t('ai.artifact.editPlaceholder')"
      />
      <div class="ai-artifact__edit-actions">
        <Button variant="secondary" size="sm" @click="cancelEdit">
          {{ t("ai.artifact.cancel") }}
        </Button>
        <Button variant="primary" size="sm" @click="saveEdit">
          {{ t("ai.artifact.save") }}
        </Button>
      </div>
    </section>

    <section
      v-else-if="view === 'diff'"
      class="ai-artifact__diff"
      :aria-label="t('ai.artifact.diff')"
    >
      <template v-if="props.payload.diff">
        <p class="ai-artifact__diff-path">
          <span class="ai-artifact__diff-path-label">{{
            t("ai.artifact.diffPath")
          }}</span>
          <span>{{ props.payload.diff.path }}</span>
        </p>
        <div
          v-for="(hunk, hunkIndex) in props.payload.diff.hunks"
          :key="hunkIndex"
          class="ai-artifact__hunk"
        >
          <p class="ai-artifact__hunk-header">{{ hunk.header }}</p>
          <p
            v-for="(line, lineIndex) in hunk.lines"
            :key="lineIndex"
            class="ai-artifact__diff-line"
            :class="`is-${line.kind}`"
          >
            {{ line.text }}
          </p>
        </div>
      </template>
      <EmptyState v-else :title="t('ai.artifact.diffEmpty')" size="sm" />
    </section>

    <section v-else class="ai-artifact__preview">
      <MarkdownViewer :model-value="content" />
    </section>

    <div
      v-if="isTerminal"
      class="ai-artifact__result"
      :class="accepted ? 'is-accepted' : 'is-rejected'"
    >
      <p class="ai-artifact__result-title">
        {{
          accepted
            ? t("ai.artifact.accepted.title")
            : t("ai.artifact.rejected.title")
        }}
      </p>
      <p v-if="accepted" class="ai-artifact__result-note">
        {{ t("ai.artifact.accepted.note") }}
      </p>
    </div>

    <div v-if="workflowPhase !== 'idle'" class="ai-artifact__workflow">
      <p class="ai-artifact__label">
        {{ t("ai.artifact.workflow.resultTitle") }}
      </p>
      <div
        class="ai-artifact__workflow-status"
        :class="`is-${workflowPhase}`"
        role="status"
        aria-live="polite"
      >
        <LoaderCircle
          v-if="workflowPhase === 'submitting'"
          class="ai-artifact__spinner"
          :size="14"
          aria-hidden="true"
        />
        <CircleCheck
          v-else-if="workflowPhase === 'success'"
          :size="14"
          aria-hidden="true"
        />
        <TriangleAlert v-else :size="14" aria-hidden="true" />

        <span v-if="workflowPhase === 'submitting'">
          {{ t("ai.card.submission.submitting") }}
        </span>
        <span v-else-if="workflowPhase === 'success'">
          {{
            t("ai.artifact.workflow.submitted", {
              workflow: props.payload.workflowLabel
            })
          }}
          <span v-if="workflowResult?.reference" class="ai-artifact__reference">
            {{
              t("ai.card.submission.reference", {
                reference: workflowResult.reference
              })
            }}
          </span>
        </span>
        <span v-else>
          {{ workflowError || t("ai.card.submission.failed") }}
        </span>

        <Button
          v-if="workflowPhase === 'error'"
          class="ai-artifact__retry"
          variant="secondary"
          size="sm"
          @click="retryWorkflow"
        >
          {{ t("ai.card.submission.retry") }}
        </Button>
      </div>
    </div>

    <ConfirmationDialog
      :open="workflowDialogOpen"
      :title="t('ai.artifact.workflow.dialogTitle')"
      :description="t('ai.artifact.workflow.dialogDescription')"
      :confirm-label="t('ai.artifact.workflow.confirm')"
      :cancel-label="t('ai.artifact.workflow.cancel')"
      @update:open="workflowDialogOpen = $event"
      @confirm="confirmWorkflowSubmit"
      @cancel="workflowDialogOpen = false"
    />
  </AiResponseCard>
</template>

<style scoped>
.ai-artifact__meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: var(--space-1);
}

.ai-artifact__version,
.ai-artifact__author,
.ai-artifact__time {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

.ai-artifact__version {
  font-family: var(--font-mono);
}

.ai-artifact__views {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin-bottom: var(--space-2);
  padding: var(--space-1);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

.ai-artifact__view-btn {
  height: var(--size-control-sm);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-xs);
}

.ai-artifact__view-btn:hover {
  color: var(--ws-text-primary);
}

.ai-artifact__view-btn.is-active {
  color: var(--ws-text-primary);
  background: var(--ws-surface);
}

.ai-artifact__view-btn:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.ai-artifact__preview {
  min-height: var(--size-ai-artifact-preview-height);
  max-height: var(--size-ai-artifact-preview-height);
  overflow-y: auto;
}

.ai-artifact__diff {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  max-height: var(--size-ai-artifact-preview-height);
  overflow: auto;
  padding: var(--space-2);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

.ai-artifact__diff-path {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.ai-artifact__diff-path-label {
  color: var(--ws-text-secondary);
  font-family: var(--font-ui);
}

.ai-artifact__hunk {
  display: flex;
  flex-direction: column;
  gap: 1px;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  overflow: hidden;
}

.ai-artifact__hunk-header {
  margin: 0;
  padding: 0 var(--space-2);
  min-height: var(--size-ai-diff-line);
  line-height: var(--size-ai-diff-line);
  color: var(--ws-text-secondary);
  background: var(--ws-surface);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.ai-artifact__diff-line {
  margin: 0;
  padding: 0 var(--space-2);
  min-height: var(--size-ai-diff-line);
  line-height: var(--size-ai-diff-line);
  color: var(--ws-text-secondary);
  background: var(--ws-surface);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  white-space: pre-wrap;
  word-break: break-word;
}

.ai-artifact__diff-line.is-add {
  color: var(--ws-color-success-text);
  border-left: var(--size-indicator) solid var(--ws-color-success);
}

.ai-artifact__diff-line.is-remove {
  color: var(--ws-color-danger-text);
  background: var(--ws-color-danger-surface);
  border-left: var(--size-indicator) solid var(--ws-color-danger);
}

.ai-artifact__editor {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-artifact__edit-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

.ai-artifact__result {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-3);
  padding: var(--space-2) var(--space-3);
  background: var(--ws-elevated);
  border-left: var(--size-indicator) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.ai-artifact__result.is-accepted {
  border-left-color: var(--ws-color-success);
}

.ai-artifact__result.is-rejected {
  border-left-color: var(--ws-color-danger);
}

.ai-artifact__result-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.ai-artifact__result.is-accepted .ai-artifact__result-title {
  color: var(--ws-color-success-text);
}

.ai-artifact__result.is-rejected .ai-artifact__result-title {
  color: var(--ws-color-danger-text);
}

.ai-artifact__result-note {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.ai-artifact__workflow {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-3);
  padding-top: var(--space-3);
  border-top: var(--border-1) solid var(--ws-border);
}

.ai-artifact__label {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.ai-artifact__workflow-status {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  color: var(--ws-text-secondary);
  background: var(--ws-elevated);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
}

.ai-artifact__workflow-status.is-success {
  color: var(--ws-color-success-text);
}

.ai-artifact__workflow-status.is-error {
  color: var(--ws-color-danger-text);
  background: var(--ws-color-danger-surface);
  border: var(--border-1) solid var(--ws-color-danger-border);
}

.ai-artifact__reference {
  margin-left: var(--space-1);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.ai-artifact__retry {
  margin-left: auto;
}

.ai-artifact__spinner {
  animation: ai-artifact-spin var(--duration-spinner) linear infinite;
}

@keyframes ai-artifact-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ai-artifact__spinner {
    animation: none;
  }
}
</style>
