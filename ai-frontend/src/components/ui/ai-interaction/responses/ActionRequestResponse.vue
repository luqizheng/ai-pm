<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Check,
  ChevronDown,
  ChevronUp,
  CircleAlert,
  ShieldCheck,
  TriangleAlert
} from "lucide-vue-next";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import AiResponseCard from "../card/AiResponseCard.vue";
import { defaultAiIdentity } from "../identity";
import { useAiInteraction } from "../useAiInteraction";
import type {
  ActionRequestPayload,
  AiEvidence,
  AiIdentity,
  CardDensity,
  FooterAction,
  InteractionStatus,
  SubmitResult
} from "../types";

/**
 * Action Request 响应卡（14）。
 *
 * 红线：后端 submit resolve 之前不展示任何成功态；成功文案只写
 * 「操作已确认并提交执行」+ 记录号 + 「以执行记录为准」，不声称已执行完成。
 * consequential = true 时先经 ConfirmationDialog 二次确认。
 */
const RISK_ICON = {
  low: ShieldCheck,
  medium: CircleAlert,
  high: TriangleAlert
} as const;

const props = withDefaults(
  defineProps<{
    payload: ActionRequestPayload;
    identity?: AiIdentity;
    timestamp?: string;
    density?: CardDensity;
    submit?: (payload: unknown) => Promise<SubmitResult>;
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

const state = useAiInteraction({
  initialStatus: props.initialStatus,
  submit: async payload => {
    const result = props.submit
      ? await props.submit(payload)
      : ({ recorded: true } satisfies SubmitResult);
    return { ...result, messageKey: "ai.actionRequest.success" };
  }
});

const {
  status,
  phase,
  result,
  error,
  submitting,
  submitted,
  begin,
  submit: submitInteraction,
  retry: retryInteraction
} = state;

const detailsOpen = ref(false);
const dialogOpen = ref(false);

const identity = computed<AiIdentity>(
  () => props.identity ?? defaultAiIdentity
);
const isReadonly = computed(() => submitted.value);
const riskLabel = computed(() =>
  t(`ai.actionRequest.risk.${props.payload.riskLevel}`)
);
const riskIcon = computed(() => RISK_ICON[props.payload.riskLevel]);
const riskHint = computed(() =>
  props.payload.riskLevel === "high"
    ? t("ai.actionRequest.riskHint.high")
    : t("ai.actionRequest.riskHint.default")
);

const actions = computed<FooterAction[]>(() => {
  const list: FooterAction[] = [
    {
      key: "toggle-details",
      label: detailsOpen.value
        ? t("ai.actionRequest.actions.hide")
        : t("ai.actionRequest.actions.review"),
      variant: "ghost",
      icon: detailsOpen.value ? ChevronUp : ChevronDown
    }
  ];
  if (isReadonly.value) return list;
  list.push(
    {
      key: "cancel",
      label: t("ai.actionRequest.actions.cancel"),
      variant: "secondary"
    },
    {
      key: "confirm",
      label: t("ai.actionRequest.actions.confirm"),
      variant: props.payload.riskLevel === "high" ? "danger" : "primary",
      icon: Check
    }
  );
  return list;
});

function toggleDetails(): void {
  detailsOpen.value = !detailsOpen.value;
  emit("action", detailsOpen.value ? "review-details" : "hide-details");
}

function requestConfirm(): void {
  if (isReadonly.value || submitting.value) return;
  begin();
  emit("action", "confirm-intent");
  if (props.payload.consequential) {
    dialogOpen.value = true;
    return;
  }
  void runSubmit();
}

async function runSubmit(): Promise<void> {
  dialogOpen.value = false;
  emit("action", "confirm");
  const submitResult = await submitInteraction({
    action: props.payload.action,
    target: props.payload.target,
    riskLevel: props.payload.riskLevel
  });
  if (submitResult) emit("submitted", submitResult);
}

async function handleRetry(): Promise<void> {
  const submitResult = await retryInteraction();
  if (submitResult) emit("submitted", submitResult);
}

function onCardAction(key: string): void {
  if (key === "toggle-details") {
    toggleDetails();
    return;
  }
  if (key === "cancel") {
    emit("action", "cancel");
    return;
  }
  if (key === "confirm") requestConfirm();
}
</script>

<template>
  <AiResponseCard
    type="action-request"
    :identity="identity"
    :evidence="evidence"
    :status="status"
    :title="payload.action"
    :description="t('ai.actionRequest.card.description')"
    :timestamp="timestamp"
    :density="density"
    :actions="actions"
    :submitting="submitting"
    :submission-phase="phase"
    :submission-result="result"
    :submission-error="error"
    footer-wrap
    @action="onCardAction"
    @retry="handleRetry"
  >
    <template #default>
      <div class="ai-action">
        <dl class="ai-action__meta">
          <div class="ai-action__meta-row">
            <dt>{{ t("ai.actionRequest.target") }}</dt>
            <dd>{{ payload.target }}</dd>
          </div>
          <div class="ai-action__meta-row">
            <dt>{{ t("ai.actionRequest.permission") }}</dt>
            <dd>{{ payload.permission }}</dd>
          </div>
          <div class="ai-action__meta-row">
            <dt>{{ t("ai.actionRequest.risk.label") }}</dt>
            <dd>
              <span class="ai-action__risk" :class="`is-${payload.riskLevel}`">
                <component :is="riskIcon" :size="14" aria-hidden="true" />
                {{ riskLabel }}
              </span>
            </dd>
          </div>
        </dl>

        <p class="ai-action__impact-summary">
          {{
            t("ai.actionRequest.impactCount", {
              count: payload.impact.length
            })
          }}
        </p>

        <div class="ai-action__statement">
          <p class="ai-action__statement-label">
            {{ t("ai.actionRequest.statement") }}
          </p>
          <p class="ai-action__statement-text">{{ payload.statement }}</p>
        </div>

        <p class="ai-action__hint">{{ riskHint }}</p>

        <section v-if="detailsOpen" class="ai-action__details">
          <p class="ai-action__details-title">
            {{ t("ai.actionRequest.details.title") }}
          </p>
          <dl class="ai-action__details-list">
            <div class="ai-action__details-row">
              <dt>{{ t("ai.actionRequest.details.action") }}</dt>
              <dd>{{ payload.action }}</dd>
            </div>
            <div class="ai-action__details-row">
              <dt>{{ t("ai.actionRequest.details.impact") }}</dt>
              <dd>
                <ul class="ai-action__impact">
                  <li
                    v-for="(item, index) in payload.impact"
                    :key="`impact-${index}`"
                  >
                    {{ item }}
                  </li>
                </ul>
              </dd>
            </div>
          </dl>
        </section>

        <p v-if="submitted" class="ai-action__success-note">
          {{ t("ai.actionRequest.successNote") }}
        </p>
      </div>
    </template>
  </AiResponseCard>

  <ConfirmationDialog
    :open="dialogOpen"
    :title="t('ai.actionRequest.dialog.title')"
    :description="
      t('ai.actionRequest.dialog.description', {
        action: payload.action,
        target: payload.target
      })
    "
    :confirm-label="t('ai.actionRequest.dialog.confirm')"
    :cancel-label="t('ai.actionRequest.dialog.cancel')"
    :variant="payload.riskLevel === 'high' ? 'danger' : 'default'"
    :icon="TriangleAlert"
    @confirm="runSubmit"
    @cancel="dialogOpen = false"
  />
</template>

<style scoped>
.ai-action {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ai-action__meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.ai-action__meta-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--space-2);
  align-items: baseline;
}

.ai-action__meta-row dt {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-action__meta-row dd {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  word-break: break-word;
}

/* 风险等级：语义色 + 文字，不用颜色单独承载含义 */
.ai-action__risk {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0 var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  background: var(--surface-raised);
  border: var(--border-1) solid currentcolor;
  border-radius: var(--radius-xs);
}

.ai-action__risk.is-low {
  color: var(--success-strong);
}

.ai-action__risk.is-medium {
  color: var(--warning-strong);
}

.ai-action__risk.is-high {
  color: var(--danger-title);
}

.ai-action__impact-summary {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-action__statement {
  padding: var(--space-3);
  background: var(--surface-sunken);
  border: var(--border-1) solid var(--border-subtle);
  border-left: var(--size-indicator) solid var(--accent);
  border-radius: var(--radius-xs);
}

.ai-action__statement-label {
  margin: 0 0 var(--space-1);
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-action__statement-text {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-action__hint {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-action__details {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.ai-action__details-title {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  line-height: var(--line-height-micro);
}

.ai-action__details-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.ai-action__details-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--space-2);
  align-items: baseline;
}

.ai-action__details-row dt {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-action__details-row dd {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  word-break: break-word;
}

.ai-action__impact {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding-left: var(--space-4);
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-action__success-note {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}
</style>
