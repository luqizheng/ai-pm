<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Link2 } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import AiResponseCard from "../card/AiResponseCard.vue";
import AiSubmissionStatus from "../card/AiSubmissionStatus.vue";
import { defaultAiIdentity } from "../identity";
import { createMockSubmit } from "../submitAdapter";
import type {
  AiEvidence,
  AiIdentity,
  CardDensity,
  FooterAction,
  InteractionStatus,
  ProposalChange,
  ProposalPayload,
  SubmitResult
} from "../types";
import { useAiInteraction } from "../useAiInteraction";

/**
 * 提案卡 —— 变更前后对比、影响、关联对象与版本。
 *
 * 两条状态严格分开：
 *   - 接收提案 = 记录用户选择（accepted），不代表已批准；
 *   - 提交正式审批 = 独立提交（submitted），结果只写「已提交至正式审批流程」。
 * 因此用两个独立状态机，互不覆盖。
 */
const props = withDefaults(
  defineProps<{
    payload: ProposalPayload;
    identity?: AiIdentity;
    timestamp?: string;
    density?: CardDensity;
    submit?: (
    payload: unknown,
    action?: string
  ) => Promise<SubmitResult>;
    initialStatus?: InteractionStatus;
    evidence?: AiEvidence;
  }>(),
  {
    identity: () => defaultAiIdentity,
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
const submitAdapter = (payload: unknown, action?: string) =>
  (props.submit ?? mockSubmit)(payload, action);

const state = useAiInteraction({
  initialStatus: props.initialStatus,
  submit: submitAdapter
});
const { status, phase, result, error, submitting, isTerminal } = state;

const approval = useAiInteraction({ submit: submitAdapter });
const approvalPhase = approval.phase;
const approvalResult = approval.result;
const approvalError = approval.error;

const detailsOpen = ref(true);
const editing = ref(false);
const edited = ref(false);
const approvalDialogOpen = ref(false);

const changes = ref<ProposalChange[]>(
  props.payload.changes.map(item => ({ ...item }))
);
const impact = ref<string[]>([...props.payload.impact]);
const editChanges = ref<ProposalChange[]>([]);
const editImpact = ref("");

const approved = computed(() => approvalPhase.value === "success");

const approvalDisplayResult = computed(() => {
  const value = approvalResult.value;
  if (!value) return null;
  return { ...value, messageKey: "ai.proposal.approval.submitted" };
});

const actions = computed<FooterAction[]>(() => {
  if (editing.value) return [];
  const list: FooterAction[] = [];
  if (!isTerminal.value) {
    list.push({
      key: "review",
      label: detailsOpen.value
        ? t("ai.proposal.reviewCollapse")
        : t("ai.proposal.review"),
      variant: "ghost"
    });
    list.push({
      key: "edit",
      label: t("ai.proposal.edit"),
      variant: "secondary"
    });
    list.push({
      key: "accept",
      label: t("ai.proposal.actions.accept"),
      variant: "primary"
    });
    list.push({
      key: "reject",
      label: t("ai.proposal.actions.reject"),
      variant: "danger"
    });
  }
  if (
    props.payload.canSubmitForApproval &&
    !approved.value &&
    status.value !== "rejected"
  ) {
    list.push({
      key: "submit-approval",
      label: t("ai.proposal.actions.submitForApproval"),
      variant: isTerminal.value ? "primary" : "secondary"
    });
  }
  return list;
});

function startEdit() {
  editChanges.value = changes.value.map(item => ({ ...item }));
  editImpact.value = impact.value.join("\n");
  editing.value = true;
  state.begin();
}

function cancelEdit() {
  editing.value = false;
}

function saveEdit() {
  changes.value = editChanges.value.map(item => ({
    label: item.label,
    before: item.before.trim(),
    after: item.after.trim()
  }));
  impact.value = editImpact.value
    .split("\n")
    .map(line => line.trim())
    .filter(Boolean);
  edited.value = true;
  editing.value = false;
  state.saveDraft({ changes: changes.value, impact: impact.value });
}

async function accept() {
  state.begin();
  const res = await state.accept({
    decision: "accept",
    proposalType: props.payload.proposalType,
    version: props.payload.version,
    changes: changes.value
  });
  if (res) emit("submitted", res);
}

async function reject() {
  state.begin();
  const res = await state.reject({
    decision: "reject",
    proposalType: props.payload.proposalType,
    version: props.payload.version
  });
  if (res) emit("submitted", res);
}

function openApproval() {
  approvalDialogOpen.value = true;
}

async function confirmApproval() {
  approvalDialogOpen.value = false;
  const res = await approval.submit({
    decision: "submit-for-approval",
    proposalType: props.payload.proposalType,
    version: props.payload.version
  });
  if (res) emit("submitted", res);
}

async function retryApproval() {
  const res = await approval.retry();
  if (res) emit("submitted", res);
}

async function retry() {
  const res = await state.retry();
  if (res) emit("submitted", res);
}

function onAction(key: string) {
  if (key === "review") {
    detailsOpen.value = !detailsOpen.value;
    return;
  }
  if (key === "edit") {
    startEdit();
    return;
  }
  if (key === "accept") {
    void accept();
    return;
  }
  if (key === "reject") {
    void reject();
    return;
  }
  if (key === "submit-approval") {
    openApproval();
    return;
  }
  emit("action", key);
}
</script>

<template>
  <AiResponseCard
    type="proposal"
    :identity="props.identity"
    :evidence="evidence"
    :status="status"
    :title="t('ai.proposal.title')"
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
      <div class="ai-proposal__meta">
        <Badge variant="info" size="sm">{{ props.payload.proposalType }}</Badge>
        <span class="ai-proposal__version">
          {{ t("ai.proposal.version") }} {{ props.payload.version }}
        </span>
        <Badge v-if="edited" variant="warning" size="sm">
          {{ t("ai.proposal.edited") }}
        </Badge>
      </div>
    </template>

    <section v-if="editing" class="ai-proposal__editor">
      <div
        v-for="(change, index) in editChanges"
        :key="index"
        class="ai-proposal__edit-item"
      >
        <p class="ai-proposal__change-label">{{ change.label }}</p>
        <label class="ai-proposal__edit-field">
          <span class="ai-proposal__edit-label">{{
            t("ai.proposal.before")
          }}</span>
          <Textarea v-model="change.before" :rows="2" />
        </label>
        <label class="ai-proposal__edit-field">
          <span class="ai-proposal__edit-label">{{
            t("ai.proposal.after")
          }}</span>
          <Textarea v-model="change.after" :rows="2" />
        </label>
      </div>

      <label class="ai-proposal__edit-field">
        <span class="ai-proposal__edit-label">{{
          t("ai.proposal.impact")
        }}</span>
        <Textarea
          v-model="editImpact"
          :rows="4"
          :placeholder="t('ai.proposal.impactPlaceholder')"
        />
      </label>

      <div class="ai-proposal__edit-actions">
        <Button variant="secondary" size="sm" @click="cancelEdit">
          {{ t("ai.proposal.cancel") }}
        </Button>
        <Button variant="primary" size="sm" @click="saveEdit">
          {{ t("ai.proposal.save") }}
        </Button>
      </div>
    </section>

    <template v-else-if="detailsOpen">
      <section class="ai-proposal__section">
        <h4 class="ai-proposal__label">{{ t("ai.proposal.changes") }}</h4>
        <div class="ai-proposal__changes">
          <div
            v-for="(change, index) in changes"
            :key="index"
            class="ai-proposal__change"
          >
            <p class="ai-proposal__change-label">{{ change.label }}</p>
            <div class="ai-proposal__change-columns">
              <div class="ai-proposal__change-col is-before">
                <span class="ai-proposal__change-tag">{{
                  t("ai.proposal.before")
                }}</span>
                <p class="ai-proposal__change-text">{{ change.before }}</p>
              </div>
              <div class="ai-proposal__change-col is-after">
                <span class="ai-proposal__change-tag">{{
                  t("ai.proposal.after")
                }}</span>
                <p class="ai-proposal__change-text">{{ change.after }}</p>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section v-if="impact.length" class="ai-proposal__section">
        <h4 class="ai-proposal__label">{{ t("ai.proposal.impact") }}</h4>
        <ul class="ai-proposal__impact">
          <li v-for="(item, index) in impact" :key="index">
            {{ item }}
          </li>
        </ul>
      </section>

      <section v-if="props.payload.related.length" class="ai-proposal__section">
        <h4 class="ai-proposal__label">{{ t("ai.proposal.related") }}</h4>
        <ul class="ai-proposal__related">
          <li
            v-for="item in props.payload.related"
            :key="item.id"
            class="ai-proposal__related-item"
          >
            <Link2 :size="12" aria-hidden="true" />
            <span class="ai-proposal__related-kind">{{ item.kind }}</span>
            <span>{{ item.label }}</span>
          </li>
        </ul>
      </section>
    </template>

    <div
      v-if="isTerminal"
      class="ai-proposal__result"
      :class="status === 'accepted' ? 'is-accepted' : 'is-rejected'"
    >
      <p class="ai-proposal__result-title">
        {{
          status === "accepted"
            ? t("ai.proposal.accepted.title")
            : t("ai.proposal.rejected.title")
        }}
      </p>
      <p v-if="status === 'accepted'" class="ai-proposal__result-note">
        {{ t("ai.proposal.accepted.note") }}
      </p>
    </div>

    <div v-if="approvalPhase !== 'idle'" class="ai-proposal__approval">
      <p class="ai-proposal__label">
        {{ t("ai.proposal.approval.resultTitle") }}
      </p>
      <AiSubmissionStatus
        :phase="approvalPhase"
        :result="approvalDisplayResult"
        :error-message="approvalError"
        @retry="retryApproval"
      />
    </div>

    <ConfirmationDialog
      :open="approvalDialogOpen"
      :title="t('ai.proposal.approval.dialogTitle')"
      :description="t('ai.proposal.approval.dialogDescription')"
      :confirm-label="t('ai.proposal.approval.confirm')"
      :cancel-label="t('ai.proposal.approval.cancel')"
      @update:open="approvalDialogOpen = $event"
      @confirm="confirmApproval"
      @cancel="approvalDialogOpen = false"
    />
  </AiResponseCard>
</template>

<style scoped>
.ai-proposal__meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: var(--space-1);
}

.ai-proposal__version {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

.ai-proposal__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-top: var(--space-3);
}

.ai-proposal__section + .ai-proposal__section {
  margin-top: var(--space-2);
  border-top: var(--border-1) solid var(--ws-border);
}

.ai-proposal__label {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.ai-proposal__changes {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-proposal__change {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

.ai-proposal__change-label {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
}

.ai-proposal__change-columns {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.ai-proposal__change-col {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2);
  background: var(--ws-surface);
  border-radius: var(--radius-xs);
  border-left: var(--size-indicator) solid var(--ws-border);
}

.ai-proposal__change-col.is-before {
  border-left-color: var(--ws-color-danger);
}

.ai-proposal__change-col.is-after {
  border-left-color: var(--ws-color-success);
}

.ai-proposal__change-tag {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.ai-proposal__change-text {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  word-break: break-word;
}

.ai-proposal__impact {
  margin: 0;
  padding-left: var(--space-5);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-proposal__related {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-proposal__related-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-badge);
  padding: 0 var(--space-2);
  color: var(--ws-text-primary);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
  font-size: var(--font-size-micro);
}

.ai-proposal__related-kind {
  color: var(--ws-text-secondary);
}

.ai-proposal__editor {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding-top: var(--space-3);
}

.ai-proposal__edit-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

.ai-proposal__edit-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ai-proposal__edit-label {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.ai-proposal__edit-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

.ai-proposal__result {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-3);
  padding: var(--space-2) var(--space-3);
  background: var(--ws-elevated);
  border-left: var(--size-indicator) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.ai-proposal__result.is-accepted {
  border-left-color: var(--ws-color-success);
}

.ai-proposal__result.is-rejected {
  border-left-color: var(--ws-color-danger);
}

.ai-proposal__result-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.ai-proposal__result.is-accepted .ai-proposal__result-title {
  color: var(--ws-color-success-text);
}

.ai-proposal__result.is-rejected .ai-proposal__result-title {
  color: var(--ws-color-danger-text);
}

.ai-proposal__result-note {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.ai-proposal__approval {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-3);
  padding-top: var(--space-3);
  border-top: var(--border-1) solid var(--ws-border);
}

@media (prefers-reduced-motion: reduce) {
  .ai-proposal__change {
    transition: none;
  }
}
</style>
