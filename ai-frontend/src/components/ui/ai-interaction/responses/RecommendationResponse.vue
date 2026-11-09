<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { CircleCheck, TriangleAlert } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import { resolveAttachmentUrl } from "@/api/base/attachments";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import AiResponseCard from "../card/AiResponseCard.vue";
import { defaultAiIdentity } from "../identity";
import { createMockSubmit } from "../submitAdapter";
import type {
  AiEvidence,
  AiIdentity,
  CardDensity,
  FooterAction,
  InteractionStatus,
  RecommendationPayload,
  RecommendationRisk,
  SubmitResult
} from "../types";
import { useAiInteraction } from "../useAiInteraction";

/**
 * 建议卡 —— 展示建议、依据、收益与风险，并把「用户选择」和「业务生效」分开。
 *
 * accept 只记录选择（accepted），文案绝不宣称已生效；
 * reject 可补原因后提交；modify / discuss 只上报动作，交上层处理。
 */
const props = withDefaults(
  defineProps<{
    payload: RecommendationPayload;
    identity?: AiIdentity;
    timestamp?: string;
    density?: CardDensity;
    submit?: (payload: unknown, action?: string) => Promise<SubmitResult>;
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

const state = useAiInteraction({
  initialStatus: props.initialStatus,
  submit: (payload, action) => (props.submit ?? mockSubmit)(payload, action)
});
const { status, phase, result, error, submitting, isTerminal } = state;

const rejecting = ref(false);
const rejectReason = ref("");
const rejectedReason = ref("");

const title = computed(
  () => props.payload.recommendedAction || t("ai.recommendation.title")
);

const actions = computed<FooterAction[]>(() => {
  if (isTerminal.value || rejecting.value) return [];
  return [
    {
      key: "accept",
      label: t("ai.recommendation.actions.accept"),
      variant: "primary"
    },
    {
      key: "reject",
      label: t("ai.recommendation.actions.reject"),
      variant: "danger"
    },
    {
      key: "modify",
      label: t("ai.recommendation.actions.modify"),
      variant: "secondary"
    },
    {
      key: "discuss",
      label: t("ai.recommendation.actions.discuss"),
      variant: "ghost"
    }
  ];
});

const accepted = computed(() => status.value === "accepted");

function severityTone(severity: RecommendationRisk["severity"]) {
  return severity === "high" ? "danger" : "warning";
}

function severityLabel(severity: RecommendationRisk["severity"]) {
  return t(`ai.recommendation.severity.${severity}`);
}

async function accept() {
  state.begin();
  const res = await state.accept({
    decision: "accept",
    recommendedAction: props.payload.recommendedAction,
    summary: props.payload.summary
  });
  if (res) emit("submitted", res);
}

function openReject() {
  state.begin();
  rejecting.value = true;
}

function cancelReject() {
  rejecting.value = false;
  rejectReason.value = "";
}

async function confirmReject() {
  const reason = rejectReason.value.trim();
  const res = await state.reject({
    decision: "reject",
    recommendedAction: props.payload.recommendedAction,
    reason: reason || undefined
  });
  if (!res) return;
  rejectedReason.value = reason;
  rejecting.value = false;
  emit("submitted", res);
}

async function retry() {
  const res = await state.retry();
  if (res) emit("submitted", res);
}

function onAction(key: string) {
  if (key === "accept") {
    void accept();
    return;
  }
  if (key === "reject") {
    openReject();
    return;
  }
  state.begin();
  emit("action", key);
}
</script>

<template>
  <AiResponseCard
    type="recommendation"
    :identity="props.identity"
    :evidence="evidence"
    :status="status"
    :title="title"
    :description="props.payload.summary"
    :timestamp="props.timestamp"
    :density="props.density"
    :actions="actions"
    :submitting="submitting"
    :submission-phase="phase"
    :submission-result="result"
    :submission-error="error"
    @action="onAction"
    @retry="retry"
  >
    <section class="ai-reco__explanation">
      <MarkdownViewer
        :model-value="props.payload.explanationMarkdown"
        :resolve-url="resolveAttachmentUrl"
      />
    </section>

    <section v-if="props.payload.benefits.length" class="ai-reco__block">
      <h4 class="ai-reco__label">{{ t("ai.recommendation.benefits") }}</h4>
      <ul class="ai-reco__list">
        <li
          v-for="(benefit, index) in props.payload.benefits"
          :key="index"
          class="ai-reco__item is-benefit"
        >
          <CircleCheck :size="14" aria-hidden="true" />
          <span class="ai-reco__item-label">{{ benefit.label }}</span>
          <span v-if="benefit.detail" class="ai-reco__item-detail">
            {{ benefit.detail }}
          </span>
        </li>
      </ul>
    </section>

    <section v-if="props.payload.risks.length" class="ai-reco__block">
      <h4 class="ai-reco__label">{{ t("ai.recommendation.risks") }}</h4>
      <ul class="ai-reco__list">
        <li
          v-for="(risk, index) in props.payload.risks"
          :key="index"
          class="ai-reco__item"
          :class="`is-${severityTone(risk.severity)}`"
        >
          <TriangleAlert :size="14" aria-hidden="true" />
          <span class="ai-reco__item-label">{{ risk.label }}</span>
          <span class="ai-reco__severity">{{
            severityLabel(risk.severity)
          }}</span>
          <span v-if="risk.detail" class="ai-reco__item-detail">
            {{ risk.detail }}
          </span>
        </li>
      </ul>
    </section>

    <div v-if="rejecting" class="ai-reco__reject">
      <p class="ai-reco__reject-label">
        {{ t("ai.recommendation.reject.reasonLabel") }}
      </p>
      <Textarea
        v-model="rejectReason"
        :rows="3"
        :placeholder="t('ai.recommendation.reject.reasonPlaceholder')"
      />
      <div class="ai-reco__reject-actions">
        <Button variant="secondary" size="sm" @click="cancelReject">
          {{ t("ai.recommendation.reject.cancel") }}
        </Button>
        <Button variant="danger" size="sm" @click="confirmReject">
          {{ t("ai.recommendation.reject.confirm") }}
        </Button>
      </div>
    </div>

    <div
      v-else-if="isTerminal"
      class="ai-reco__summary"
      :class="accepted ? 'is-accepted' : 'is-rejected'"
    >
      <p class="ai-reco__summary-title">
        {{
          accepted
            ? t("ai.recommendation.accepted.title")
            : t("ai.recommendation.rejected.title")
        }}
      </p>
      <p class="ai-reco__summary-text">{{ props.payload.summary }}</p>
      <p v-if="accepted" class="ai-reco__summary-note">
        {{ t("ai.recommendation.accepted.note") }}
      </p>
      <p v-else-if="rejectedReason" class="ai-reco__summary-note">
        {{ t("ai.recommendation.rejected.reason") }}：{{ rejectedReason }}
      </p>
    </div>
  </AiResponseCard>
</template>

<style scoped>
.ai-reco__explanation {
  margin-bottom: var(--space-2);
}

.ai-reco__block {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-3);
  padding-top: var(--space-2);
  border-top: var(--border-1) solid var(--ws-border);
}

.ai-reco__label {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.ai-reco__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-reco__item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  padding: var(--space-2);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
  border-left: var(--size-indicator) solid var(--ws-border);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-reco__item.is-benefit {
  border-left-color: var(--ws-color-success);
}

.ai-reco__item.is-benefit > svg {
  flex: none;
  color: var(--ws-color-success-text);
}

.ai-reco__item.is-warning {
  border-left-color: var(--ws-color-warning);
}

.ai-reco__item.is-warning > svg {
  flex: none;
  color: var(--ws-color-warning-text);
}

.ai-reco__item.is-danger {
  border-left-color: var(--ws-color-danger);
}

.ai-reco__item.is-danger > svg {
  flex: none;
  color: var(--ws-color-danger-text);
}

.ai-reco__item-label {
  color: var(--ws-text-primary);
  font-weight: var(--weight-medium);
}

.ai-reco__severity {
  padding: 0 var(--space-1);
  color: var(--ws-text-secondary);
  background: var(--ws-surface);
  border-radius: var(--radius-xs);
  font-size: var(--font-size-micro);
}

.ai-reco__item.is-warning .ai-reco__severity {
  color: var(--ws-color-warning-text);
}

.ai-reco__item.is-danger .ai-reco__severity {
  color: var(--ws-color-danger-text);
}

.ai-reco__item-detail {
  flex-basis: 100%;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.ai-reco__reject {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-top: var(--space-3);
  padding-top: var(--space-3);
  border-top: var(--border-1) solid var(--ws-border);
}

.ai-reco__reject-label {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.ai-reco__reject-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

.ai-reco__summary {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-3);
  padding: var(--space-2) var(--space-3);
  background: var(--ws-surface);
  border-top: var(--border-1) solid var(--ws-border);
  border-left: var(--size-indicator) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.ai-reco__summary.is-accepted {
  border-left-color: var(--ws-color-success);
}

.ai-reco__summary.is-rejected {
  border-left-color: var(--ws-color-danger);
}

.ai-reco__summary-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.ai-reco__summary.is-accepted .ai-reco__summary-title {
  color: var(--ws-color-success-text);
}

.ai-reco__summary.is-rejected .ai-reco__summary-title {
  color: var(--ws-color-danger-text);
}

.ai-reco__summary-text {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-reco__summary-note {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

@media (prefers-reduced-motion: reduce) {
  .ai-reco__item {
    transition: none;
  }
}
</style>
