<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Check, CheckCircle2, Sparkles, X } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import Checkbox from "@/components/ui/checkbox/Checkbox.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import AiResponseCard from "../card/AiResponseCard.vue";
import { defaultAiIdentity } from "../identity";
import { useAiInteraction } from "../useAiInteraction";
import type {
  AiEvidence,
  AiIdentity,
  CardDensity,
  DecisionOption,
  DecisionPayload,
  FooterAction,
  InteractionStatus,
  SubmitResult
} from "../types";

/**
 * Decision 响应卡（11/12）。
 *
 * 「AI 推荐」与「你的选择」是两条独立的视觉通道：
 *   - AI 推荐 = accent 描边徽标，只作建议；
 *   - 你的选择 = success 徽标 + 选中描边；
 * 提交必须勾选「以本人身份提交」并经 ConfirmationDialog 二次确认，
 * 成功后只读记录决策人 = 人类、时间与决策记录号，AI 不得成为决策人。
 */
const props = withDefaults(
  defineProps<{
    payload: DecisionPayload;
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

const state = useAiInteraction({
  initialStatus: props.initialStatus,
  submit: async (payload, action) => {
    const result = props.submit
      ? await props.submit(payload, action)
      : ({ recorded: true } satisfies SubmitResult);
    return { ...result, messageKey: "ai.decision.success" };
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
  resolve,
  retry: retryInteraction
} = state;

const selectedId = ref("");
const comment = ref("");
const confirmed = ref(false);
const confirmHint = ref(false);
const dialogOpen = ref(false);
const submittedAt = ref("");
const uid = useId();
const commentId = `ai-decision-comment-${uid}`;

const identity = computed<AiIdentity>(
  () => props.identity ?? defaultAiIdentity
);
const selectedOption = computed<DecisionOption | undefined>(() =>
  props.payload.options.find(option => option.id === selectedId.value)
);
const canConfirm = computed(() => Boolean(selectedId.value) && confirmed.value);
const isReadonly = computed(() => submitted.value);
const recordReference = computed(() => result.value?.reference ?? "");
const submittedAtText = computed(() => {
  if (!submittedAt.value) return "";
  try {
    return new Date(submittedAt.value).toLocaleString();
  } catch {
    return submittedAt.value;
  }
});
const identityName = computed(() => identity.value.name);

const actions = computed<FooterAction[]>(() => {
  if (isReadonly.value) return [];
  return [
    {
      key: "submit",
      label: t("ai.decision.actions.submit"),
      variant: "primary",
      icon: Check
    }
  ];
});

function selectOption(optionId: string): void {
  selectedId.value = optionId;
  confirmHint.value = false;
  begin();
  emit("action", `select:${optionId}`);
}

function requestSubmit(): void {
  if (isReadonly.value || submitting.value) return;
  if (!canConfirm.value) {
    confirmHint.value = true;
    emit("action", "confirmation-required");
    return;
  }
  dialogOpen.value = true;
  emit("action", "submit-intent");
}

function markSubmitted(submitResult: SubmitResult): void {
  submittedAt.value = new Date().toISOString();
  emit("submitted", submitResult);
}

async function confirmDecision(): Promise<void> {
  dialogOpen.value = false;
  emit("action", "submit");
  const decision = {
    optionId: selectedId.value,
    optionLabel: selectedOption.value?.label ?? "",
    comment: comment.value.trim() || undefined,
    confirmedAsSelf: true
  };
  const submitResult = await resolve(decision);
  if (submitResult) markSubmitted(submitResult);
}

async function handleRetry(): Promise<void> {
  const submitResult = await retryInteraction();
  if (submitResult) markSubmitted(submitResult);
}

function onCardAction(key: string): void {
  if (key === "submit") requestSubmit();
}
</script>

<template>
  <AiResponseCard
    type="decision"
    :identity="identity"
    :evidence="evidence"
    :status="status"
    :title="t('ai.decision.card.title')"
    :description="t('ai.decision.card.description')"
    :timestamp="timestamp"
    :density="density"
    :actions="actions"
    :submitting="submitting"
    :submission-phase="phase"
    :submission-result="result"
    :submission-error="error"
    @action="onCardAction"
    @retry="handleRetry"
  >
    <template #default>
      <!-- 已记录：只读决策摘要 -->
      <div v-if="isReadonly" class="ai-decision__record">
        <p class="ai-decision__record-title">
          <CheckCircle2 :size="14" aria-hidden="true" />
          {{ t("ai.decision.record.title") }}
        </p>
        <dl class="ai-decision__record-list">
          <div class="ai-decision__record-row">
            <dt>{{ t("ai.decision.record.chosen") }}</dt>
            <dd>
              {{ selectedOption?.label ?? t("ai.decision.record.none") }}
            </dd>
          </div>
          <div v-if="comment.trim()" class="ai-decision__record-row">
            <dt>{{ t("ai.decision.record.comment") }}</dt>
            <dd>{{ comment }}</dd>
          </div>
          <div class="ai-decision__record-row">
            <dt>{{ t("ai.decision.record.decidedBy") }}</dt>
            <dd>{{ t("ai.decision.record.decidedByHuman") }}</dd>
          </div>
          <div class="ai-decision__record-row">
            <dt>{{ t("ai.decision.record.decidedAt") }}</dt>
            <dd>{{ submittedAtText }}</dd>
          </div>
          <div class="ai-decision__record-row">
            <dt>{{ t("ai.decision.record.reference") }}</dt>
            <dd class="ai-decision__record-reference">
              {{ recordReference || t("ai.decision.record.none") }}
            </dd>
          </div>
        </dl>
        <p class="ai-decision__note">
          {{ t("ai.decision.record.note", { name: identityName }) }}
        </p>
      </div>

      <!-- 待决策：背景 / 影响 / 选项 / 补充意见 / 身份确认 -->
      <div v-else class="ai-decision">
        <section class="ai-decision__section">
          <p class="ai-decision__label">{{ t("ai.decision.background") }}</p>
          <p class="ai-decision__text">{{ payload.background }}</p>
        </section>

        <section class="ai-decision__section">
          <p class="ai-decision__label">{{ t("ai.decision.impact") }}</p>
          <ul class="ai-decision__impact">
            <li
              v-for="(item, index) in payload.impact"
              :key="`impact-${index}`"
            >
              {{ item }}
            </li>
          </ul>
        </section>

        <section class="ai-decision__section">
          <p class="ai-decision__label">{{ t("ai.decision.options") }}</p>
          <div
            class="ai-decision__options"
            role="radiogroup"
            :aria-label="t('ai.decision.options')"
          >
            <button
              v-for="option in payload.options"
              :key="option.id"
              type="button"
              class="ai-decision__option"
              :class="{
                'is-recommended': option.recommended,
                'is-selected': option.id === selectedId
              }"
              role="radio"
              :aria-checked="option.id === selectedId"
              @click="selectOption(option.id)"
            >
              <span class="ai-decision__option-head">
                <span class="ai-decision__option-label">{{
                  option.label
                }}</span>
                <Badge
                  v-if="option.recommended"
                  variant="accent"
                  outline
                  size="sm"
                >
                  <Sparkles :size="10" aria-hidden="true" />
                  {{ t("ai.decision.recommended") }}
                </Badge>
                <Badge
                  v-if="option.id === selectedId"
                  variant="success"
                  outline
                  size="sm"
                >
                  <Check :size="10" aria-hidden="true" />
                  {{ t("ai.decision.selected") }}
                </Badge>
              </span>
              <span v-if="option.summary" class="ai-decision__option-summary">
                {{ option.summary }}
              </span>
              <span
                v-if="option.pros?.length"
                class="ai-decision__option-group"
              >
                <span class="ai-decision__option-group-title">{{
                  t("ai.decision.pros")
                }}</span>
                <span class="ai-decision__list">
                  <span
                    v-for="(pro, index) in option.pros"
                    :key="`pro-${index}`"
                    class="ai-decision__list-item"
                  >
                    <Check
                      class="ai-decision__list-icon is-pro"
                      :size="12"
                      aria-hidden="true"
                    />
                    {{ pro }}
                  </span>
                </span>
              </span>
              <span
                v-if="option.cons?.length"
                class="ai-decision__option-group"
              >
                <span class="ai-decision__option-group-title">{{
                  t("ai.decision.cons")
                }}</span>
                <span class="ai-decision__list">
                  <span
                    v-for="(con, index) in option.cons"
                    :key="`con-${index}`"
                    class="ai-decision__list-item"
                  >
                    <X
                      class="ai-decision__list-icon is-con"
                      :size="12"
                      aria-hidden="true"
                    />
                    {{ con }}
                  </span>
                </span>
              </span>
            </button>
          </div>
          <p class="ai-decision__note">{{ t("ai.decision.note") }}</p>
        </section>

        <section
          v-if="payload.allowComment !== false"
          class="ai-decision__section"
        >
          <label class="ai-decision__label" :for="commentId">{{
            t("ai.decision.comment")
          }}</label>
          <Textarea
            :id="commentId"
            :model-value="comment"
            :placeholder="t('ai.decision.commentPlaceholder')"
            size="sm"
            @update:model-value="value => (comment = value)"
          />
        </section>

        <section class="ai-decision__confirm">
          <Checkbox
            class="ai-decision__confirm-checkbox"
            :model-value="confirmed"
            :label="t('ai.decision.confirm.label')"
            @update:model-value="
              value => {
                confirmed = value === true;
                confirmHint = false;
              }
            "
          />
          <p v-if="confirmHint" class="ai-decision__error" role="alert">
            {{ t("ai.decision.confirm.requiredHint") }}
          </p>
        </section>
      </div>
    </template>
  </AiResponseCard>

  <ConfirmationDialog
    :open="dialogOpen"
    :title="t('ai.decision.dialog.title')"
    :description="
      t('ai.decision.dialog.description', {
        option: selectedOption?.label ?? ''
      })
    "
    :confirm-label="t('ai.decision.dialog.confirm')"
    :cancel-label="t('ai.decision.dialog.cancel')"
    @confirm="confirmDecision"
    @cancel="dialogOpen = false"
  />
</template>

<style scoped>
.ai-decision {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.ai-decision__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-decision__label {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  line-height: var(--line-height-micro);
}

.ai-decision__text {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.ai-decision__impact {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding-left: var(--space-4);
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-decision__options {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-decision__option {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
  width: 100%;
  min-height: var(--size-ai-option-min-height);
  padding: var(--space-3);
  text-align: left;
  font-family: var(--font-ui);
  color: var(--text-primary);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate),
    background-color var(--duration-fast) var(--ease-decelerate);
}

.ai-decision__option:hover {
  border-color: var(--border-hover-strong);
}

.ai-decision__option:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* AI 推荐：accent 描边（建议通道，不等于已选择） */
.ai-decision__option.is-recommended {
  background: var(--accent-surface);
  border-color: var(--accent);
}

/* 你的选择：success 徽标 + 内描边（决策通道） */
.ai-decision__option.is-selected {
  box-shadow: inset 0 0 0 var(--border-1) var(--accent);
  border-color: var(--accent);
}

.ai-decision__option-head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.ai-decision__option-label {
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  line-height: var(--line-height-body);
}

.ai-decision__option-summary {
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-decision__option-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ai-decision__option-group-title {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-decision__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ai-decision__list-item {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-decision__list-icon.is-pro {
  color: var(--success-strong);
}

.ai-decision__list-icon.is-con {
  color: var(--danger-title);
}

.ai-decision__note {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-decision__confirm {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-sunken);
  border: var(--border-1) solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.ai-decision__confirm-checkbox {
  align-items: flex-start;
}

.ai-decision__error {
  margin: 0;
  color: var(--danger-title);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

/* 只读决策记录 */
.ai-decision__record {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--accent-surface);
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-sm);
}

.ai-decision__record-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--accent);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  line-height: var(--line-height-body);
}

.ai-decision__record-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.ai-decision__record-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--space-2);
  align-items: baseline;
}

.ai-decision__record-row dt {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-decision__record-row dd {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  word-break: break-word;
}

.ai-decision__record-reference {
  font-family: var(--font-mono);
}

@media (prefers-reduced-motion: reduce) {
  .ai-decision__option {
    transition: none;
  }
}
</style>
