<script setup lang="ts">
import { computed, nextTick, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { CheckCircle2, CircleAlert, Clock, Save, Send } from "lucide-vue-next";

import Checkbox from "@/components/ui/checkbox/Checkbox.vue";
import Input from "@/components/ui/input/Input.vue";
import Radio from "@/components/ui/radio/Radio.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import AiResponseCard from "../card/AiResponseCard.vue";
import { aiIdentity } from "../fixtures";
import { useAiInteraction } from "../useAiInteraction";
import type {
  AiEvidence,
  AiIdentity,
  CardDensity,
  FooterAction,
  InteractionStatus,
  QuestionItem,
  QuestionPayload,
  SubmitResult
} from "../types";

/**
 * Question 响应卡（08/09/10）。
 *
 * 多题一卡、混合题型（单选/多选/文本/多行/数字/日期）：
 *   - 必填校验：未通过的题目高亮并聚焦第一处，不发起提交；
 *   - Save draft 只保存草稿（本地 + 交互态），不清空已答内容；
 *   - Submit all answers 一次性提交；成功后卡片保留并转为只读答案汇总；
 *   - 提交失败保留答案，可重试。
 */
const TEXT_KINDS = new Set<QuestionItem["kind"]>([
  "text",
  "textarea",
  "number",
  "date"
]);

const props = withDefaults(
  defineProps<{
    payload: QuestionPayload;
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

const state = useAiInteraction({
  initialStatus: props.initialStatus,
  submit: async payload => {
    const result = props.submit
      ? await props.submit(payload)
      : ({ recorded: true } satisfies SubmitResult);
    return { ...result, messageKey: "ai.question.success" };
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
  saveDraft: persistDraft,
  defer,
  submit: submitInteraction,
  retry: retryInteraction
} = state;

const answers = reactive<Record<string, string | string[]>>({});
for (const question of props.payload.questions) {
  if (question.kind === "multi") {
    answers[question.id] = Array.isArray(question.defaultValue)
      ? [...question.defaultValue]
      : question.defaultValue
        ? [question.defaultValue]
        : [];
  } else {
    answers[question.id] =
      typeof question.defaultValue === "string"
        ? question.defaultValue
        : Array.isArray(question.defaultValue)
          ? (question.defaultValue[0] ?? "")
          : "";
  }
}

const uid = useId();
const errors = reactive<Record<string, string>>({});
const draftSaved = ref(false);
const submittedAt = ref("");
const formEl = ref<HTMLElement | null>(null);

const identity = computed<AiIdentity>(() => props.identity ?? aiIdentity);
const total = computed(() => props.payload.questions.length);
const answeredCount = computed(
  () => props.payload.questions.filter(isAnswered).length
);
const progressWidth = computed(() =>
  total.value
    ? `${Math.round((answeredCount.value / total.value) * 100)}%`
    : "0%"
);
const isReadonly = computed(() => submitted.value);
const submittedAtText = computed(() => {
  if (!submittedAt.value) return "";
  try {
    return new Date(submittedAt.value).toLocaleString();
  } catch {
    return submittedAt.value;
  }
});
const reference = computed(() => result.value?.reference ?? "");

const actions = computed<FooterAction[]>(() => {
  if (isReadonly.value) return [];
  return [
    {
      key: "save-draft",
      label: t("ai.question.actions.saveDraft"),
      variant: "ghost",
      icon: Save
    },
    {
      key: "defer",
      label: t("ai.question.actions.defer"),
      variant: "secondary",
      icon: Clock
    },
    {
      key: "submit",
      label: t("ai.question.actions.submit"),
      variant: "primary",
      icon: Send
    }
  ];
});

function fieldId(id: string): string {
  return `ai-question-${uid}-${id}`;
}

function isTextKind(question: QuestionItem): boolean {
  return TEXT_KINDS.has(question.kind);
}

function isAnswered(question: QuestionItem): boolean {
  const value = answers[question.id];
  if (Array.isArray(value)) return value.length > 0;
  return typeof value === "string" && value.trim().length > 0;
}

function singleValue(id: string): string {
  const value = answers[id];
  if (Array.isArray(value)) return value[0] ?? "";
  return value ?? "";
}

function multiValue(id: string): string[] {
  const value = answers[id];
  return Array.isArray(value) ? value : [];
}

function multiHas(id: string, option: string): boolean {
  return multiValue(id).includes(option);
}

function touch(): void {
  begin();
  draftSaved.value = false;
}

function setSingle(id: string, value: string): void {
  answers[id] = value;
  delete errors[id];
  touch();
}

function toggleMulti(id: string, option: string, checked: boolean): void {
  const current = multiValue(id);
  answers[id] = checked
    ? [...current, option]
    : current.filter(item => item !== option);
  delete errors[id];
  touch();
}

function snapshotAnswers(): Record<string, string | string[]> {
  const snapshot: Record<string, string | string[]> = {};
  for (const question of props.payload.questions) {
    const value = answers[question.id];
    snapshot[question.id] = Array.isArray(value) ? [...value] : value;
  }
  return snapshot;
}

function validateAnswers(): boolean {
  const missing: QuestionItem[] = [];
  for (const question of props.payload.questions) {
    delete errors[question.id];
    if (question.required && !isAnswered(question)) {
      errors[question.id] = t("ai.question.error.required", {
        label: question.label
      });
      missing.push(question);
    }
  }
  return missing.length === 0;
}

async function focusFirstInvalid(): Promise<void> {
  await nextTick();
  const host = formEl.value;
  host
    ?.querySelector<HTMLElement>(
      ".is-invalid input, .is-invalid textarea, .is-invalid button"
    )
    ?.focus();
}

function formatAnswer(question: QuestionItem): string {
  if (question.kind === "single") {
    const value = singleValue(question.id);
    return (
      question.options?.find(option => option.value === value)?.label ?? value
    );
  }
  if (question.kind === "multi") {
    return multiValue(question.id)
      .map(
        value =>
          question.options?.find(option => option.value === value)?.label ??
          value
      )
      .join(" · ");
  }
  const value = answers[question.id];
  return Array.isArray(value) ? value.join(" · ") : (value ?? "");
}

function saveDraftLocally(): void {
  const snapshot = snapshotAnswers();
  persistDraft(snapshot);
  try {
    localStorage.setItem(
      `ai.question.draft.${props.payload.groupTitle}`,
      JSON.stringify(snapshot)
    );
  } catch {
    // 本地草稿不可用时只保留组件内草稿
  }
  draftSaved.value = true;
  emit("action", "save-draft");
}

function markSubmitted(submitResult: SubmitResult): void {
  submittedAt.value = new Date().toISOString();
  emit("submitted", submitResult);
}

async function submitAll(): Promise<void> {
  if (submitting.value || isReadonly.value) return;
  if (!validateAnswers()) {
    emit("action", "validation-failed");
    await focusFirstInvalid();
    return;
  }
  emit("action", "submit");
  const submitResult = await submitInteraction({
    answers: snapshotAnswers(),
    submittedAt: new Date().toISOString()
  });
  if (submitResult) markSubmitted(submitResult);
}

async function handleRetry(): Promise<void> {
  const submitResult = await retryInteraction();
  if (submitResult) markSubmitted(submitResult);
}

function onCardAction(key: string): void {
  if (key === "save-draft") {
    saveDraftLocally();
    return;
  }
  if (key === "defer") {
    defer();
    emit("action", "defer");
    return;
  }
  if (key === "submit") {
    void submitAll();
  }
}
</script>

<template>
  <AiResponseCard
    type="question"
    :identity="identity"
    :evidence="evidence"
    :status="status"
    :title="payload.groupTitle"
    :description="t('ai.question.card.description', { count: total })"
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
      <!-- 提交成功后：只读答案汇总 -->
      <div v-if="isReadonly" class="ai-question__summary">
        <p class="ai-question__summary-title">
          {{ t("ai.question.summary.title") }}
        </p>
        <ul class="ai-question__answers">
          <li
            v-for="question in payload.questions"
            :key="question.id"
            class="ai-question__answer"
          >
            <span class="ai-question__answer-label">{{ question.label }}</span>
            <span class="ai-question__answer-value">{{
              formatAnswer(question) || t("ai.question.summary.empty")
            }}</span>
          </li>
        </ul>
        <p v-if="submittedAtText" class="ai-question__meta">
          {{ t("ai.question.summary.submittedAt", { time: submittedAtText }) }}
        </p>
        <p v-if="reference" class="ai-question__meta">
          {{ t("ai.question.summary.reference", { reference }) }}
        </p>
        <p class="ai-question__note">
          {{ t("ai.question.summary.notEditable") }}
        </p>
      </div>

      <!-- 待填写：进度 + 草稿提示 -->
      <div v-else class="ai-question">
        <div class="ai-question__progress">
          <span class="ai-question__progress-text">
            {{
              t("ai.question.progress.answered", {
                answered: answeredCount,
                total
              })
            }}
          </span>
          <span class="ai-question__progress-bar">
            <span
              class="ai-question__progress-fill"
              :style="{ width: progressWidth }"
            />
          </span>
        </div>
        <p v-if="draftSaved" class="ai-question__draft-hint" role="status">
          <CheckCircle2 :size="14" aria-hidden="true" />
          {{ t("ai.question.draft.saved") }}
        </p>
      </div>
    </template>

    <template v-if="!isReadonly" #interaction>
      <div ref="formEl" class="ai-question__form">
        <div
          v-for="question in payload.questions"
          :key="question.id"
          class="ai-question__item"
          :class="{ 'is-invalid': errors[question.id] }"
          :data-question-id="question.id"
        >
          <label
            class="ai-question__label"
            :for="isTextKind(question) ? fieldId(question.id) : undefined"
          >
            <span>{{ question.label }}</span>
            <span v-if="question.required" class="ai-question__required">{{
              t("ai.question.required")
            }}</span>
            <span v-else class="ai-question__optional">{{
              t("ai.question.optional")
            }}</span>
          </label>
          <p v-if="question.help" class="ai-question__help">
            {{ question.help }}
          </p>

          <!-- 单选：Radio 列表；无选项时退化为文本输入 -->
          <template v-if="question.kind === 'single'">
            <div
              v-if="question.options?.length"
              class="ai-question__options"
              role="radiogroup"
              :aria-label="question.label"
            >
              <div
                v-for="option in question.options ?? []"
                :key="option.value"
                class="ai-question__option"
              >
                <Radio
                  :name="`ai-question-radio-${uid}-${question.id}`"
                  :label="option.label"
                  :value="option.value"
                  :model-value="singleValue(question.id)"
                  @update:model-value="
                    value => setSingle(question.id, String(value))
                  "
                />
                <p v-if="option.description" class="ai-question__option-desc">
                  {{ option.description }}
                </p>
              </div>
            </div>
            <Input
              v-else
              :id="fieldId(question.id)"
              :model-value="singleValue(question.id)"
              :placeholder="question.placeholder"
              :invalid="Boolean(errors[question.id])"
              @update:model-value="value => setSingle(question.id, value)"
            />
          </template>

          <!-- 多选：Checkbox 列表 -->
          <div
            v-else-if="question.kind === 'multi'"
            class="ai-question__options"
            role="group"
            :aria-label="question.label"
          >
            <div
              v-for="option in question.options ?? []"
              :key="option.value"
              class="ai-question__option"
            >
              <Checkbox
                :label="option.label"
                :model-value="multiHas(question.id, option.value)"
                @update:model-value="
                  value =>
                    toggleMulti(question.id, option.value, value === true)
                "
              />
              <p v-if="option.description" class="ai-question__option-desc">
                {{ option.description }}
              </p>
            </div>
          </div>

          <!-- 多行文本 -->
          <Textarea
            v-else-if="question.kind === 'textarea'"
            :model-value="singleValue(question.id)"
            :placeholder="question.placeholder"
            :invalid="Boolean(errors[question.id])"
            @update:model-value="value => setSingle(question.id, value)"
          />

          <!-- 文本 / 数字 / 日期 -->
          <Input
            v-else
            :id="fieldId(question.id)"
            :type="
              question.kind === 'number'
                ? 'number'
                : question.kind === 'date'
                  ? 'date'
                  : 'text'
            "
            :model-value="singleValue(question.id)"
            :placeholder="question.placeholder"
            :invalid="Boolean(errors[question.id])"
            @update:model-value="value => setSingle(question.id, value)"
          />

          <p v-if="errors[question.id]" class="ai-question__error" role="alert">
            <CircleAlert :size="12" aria-hidden="true" />
            {{ errors[question.id] }}
          </p>
        </div>
      </div>
    </template>
  </AiResponseCard>
</template>

<style scoped>
.ai-question {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-question__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ai-question__progress {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.ai-question__progress-text {
  flex: none;
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-question__progress-bar {
  flex: 1;
  height: var(--space-1);
  overflow: hidden;
  background: var(--surface-raised);
  border-radius: var(--radius-full);
}

.ai-question__progress-fill {
  display: block;
  height: 100%;
  background: var(--accent);
  border-radius: var(--radius-full);
  transition: width var(--duration-fast) var(--ease-decelerate);
}

.ai-question__draft-hint {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--success-strong);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-question__item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.ai-question__item.is-invalid {
  border-color: var(--danger-title);
}

.ai-question__label {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ai-question__required {
  color: var(--danger-title);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-regular);
}

.ai-question__optional {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-regular);
}

.ai-question__help {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-question__options {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-question__option {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-height: var(--size-ai-option-min-height);
  justify-content: center;
}

.ai-question__option-desc {
  margin: 0 0 0 var(--space-6);
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-question__error {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
  color: var(--danger-title);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

/* 只读答案汇总 */
.ai-question__summary {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ai-question__summary-title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ai-question__answers {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-question__answer {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding-bottom: var(--space-2);
  border-bottom: var(--border-1) solid var(--border-subtle);
}

.ai-question__answer-label {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-question__answer-value {
  color: var(--text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  word-break: break-word;
}

.ai-question__meta {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-question__note {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

@media (prefers-reduced-motion: reduce) {
  .ai-question__progress-fill {
    transition: none;
  }
}
</style>
