<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Ban,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  FileCode2,
  ListTree,
  MessageCircleQuestion,
  PencilLine,
  RotateCcw,
  TriangleAlert,
  XCircle
} from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import type { BadgeVariant } from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Table from "@/components/ui/table/Table.vue";
import TableBody from "@/components/ui/table/TableBody.vue";
import TableCell from "@/components/ui/table/TableCell.vue";
import TableHead from "@/components/ui/table/TableHead.vue";
import TableHeader from "@/components/ui/table/TableHeader.vue";
import TableRow from "@/components/ui/table/TableRow.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import AiResponseCard from "../card/AiResponseCard.vue";
import { defaultAiIdentity } from "../identity";
import { createMockSubmit } from "../submitAdapter";
import { useAiInteraction } from "../useAiInteraction";
import type {
  AiEvidence,
  AiIdentity,
  CardDensity,
  ExecutionLogEntry,
  ExecutionResultPayload,
  ExecutionState,
  FooterAction,
  InteractionStatus,
  SubmitResult
} from "../types";

/**
 * Execution Result 响应卡（15/16）。
 *
 * 展示 Agent / 任务 / 执行状态 / 变更文件 / 日志 / 告警 / 失败信息：
 *   - partial / failed 用语义色 + 图标 + 文字三通道区分；
 *   - Retry 只在 failure.retryable 时出现，且提交经 props.submit 或 mock 适配器；
 *   - 请求修改 / 接收结果都只代表「已记录用户动作」，不篡改执行结果。
 */
const STATE_ICON: Record<ExecutionState, typeof CheckCircle2> = {
  succeeded: CheckCircle2,
  partial: TriangleAlert,
  failed: XCircle,
  cancelled: Ban
};

const FILE_STATUS_VARIANT: Record<
  ExecutionResultPayload["changedFiles"][number]["status"],
  BadgeVariant
> = {
  modified: "info",
  added: "success",
  deleted: "danger"
};

const MESSAGE_KEY: Record<string, string> = {
  accept: "ai.executionResult.success.accepted",
  "request-changes": "ai.executionResult.success.changesRequested",
  retry: "ai.executionResult.success.retrySubmitted"
};

const props = withDefaults(
  defineProps<{
    payload: ExecutionResultPayload;
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

const fallbackSubmit = createMockSubmit({ latencyMs: 600, failTimes: 0 });

const messageKeyFor = (payload: unknown): string => {
  const kind =
    payload && typeof payload === "object" && "kind" in payload
      ? String((payload as { kind?: unknown }).kind ?? "")
      : "";
  return MESSAGE_KEY[kind] ?? "ai.executionResult.success.accepted";
};

const state = useAiInteraction({
  initialStatus: props.initialStatus,
  submit: async (payload, action) => {
    const result = props.submit
      ? await props.submit(payload, action)
      : await fallbackSubmit(payload);
    return { ...result, messageKey: messageKeyFor(payload) };
  }
});

const {
  status,
  phase,
  result,
  error,
  submitting,
  submitted,
  isTerminal,
  submit: submitInteraction,
  accept,
  retry: retryInteraction
} = state;

const detailsOpen = ref(false);
const logsOpen = ref(false);
const changesOpen = ref(false);
const changeReason = ref("");
const changeError = ref("");
const retryRequested = ref(false);

const identity = computed<AiIdentity>(
  () => props.identity ?? defaultAiIdentity
);
const executionState = computed(() => props.payload.executionState);
const stateVariant = computed<BadgeVariant>(() => {
  const map: Record<ExecutionState, BadgeVariant> = {
    succeeded: "success",
    partial: "warning",
    failed: "danger",
    cancelled: "neutral"
  };
  return map[executionState.value];
});
const stateLabel = computed(() =>
  t(`ai.executionResult.state.${executionState.value}`)
);
const stateIcon = computed(() => STATE_ICON[executionState.value]);
const isSettled = computed(() => submitted.value || isTerminal.value);
const canRetry = computed(
  () =>
    props.payload.failure?.retryable === true &&
    !retryRequested.value &&
    !isSettled.value
);
const hasLogs = computed(() => props.payload.logs.length > 0);

const actions = computed<FooterAction[]>(() => {
  const list: FooterAction[] = [
    {
      key: "toggle-details",
      label: detailsOpen.value
        ? t("ai.executionResult.actions.hideDetails")
        : t("ai.executionResult.actions.details"),
      variant: "ghost",
      icon: detailsOpen.value ? ChevronUp : ListTree
    }
  ];
  if (isSettled.value) return list;

  if (canRetry.value) {
    list.push({
      key: "retry",
      label: t("ai.executionResult.actions.retry"),
      variant: "secondary",
      icon: RotateCcw
    });
  }
  list.push(
    {
      key: "request-changes",
      label: t("ai.executionResult.actions.requestChanges"),
      variant: "secondary",
      icon: PencilLine
    },
    {
      key: "ask-explanation",
      label: t("ai.executionResult.actions.askExplanation"),
      variant: "ghost",
      icon: MessageCircleQuestion
    },
    {
      key: "accept",
      label: t("ai.executionResult.actions.accept"),
      variant: "primary",
      icon: Check
    }
  );
  return list;
});

function toggleDetails(): void {
  detailsOpen.value = !detailsOpen.value;
  if (detailsOpen.value) logsOpen.value = true;
  emit("action", detailsOpen.value ? "review-details" : "hide-details");
}

async function acceptResult(): Promise<void> {
  emit("action", "accept");
  const submitResult = await accept({
    kind: "accept",
    taskRef: props.payload.taskRef,
    executionState: executionState.value
  });
  if (submitResult) emit("submitted", submitResult);
}

async function retryExecution(): Promise<void> {
  if (!canRetry.value) return;
  emit("action", "retry");
  const submitResult = await submitInteraction({
    kind: "retry",
    taskRef: props.payload.taskRef
  });
  if (submitResult) {
    retryRequested.value = true;
    emit("submitted", submitResult);
  }
}

function toggleChanges(): void {
  changesOpen.value = !changesOpen.value;
  changeError.value = "";
  emit(
    "action",
    changesOpen.value ? "request-changes" : "cancel-request-changes"
  );
}

async function submitChanges(): Promise<void> {
  const reason = changeReason.value.trim();
  if (!reason) {
    changeError.value = t("ai.executionResult.requestChanges.required");
    return;
  }
  changeError.value = "";
  const submitResult = await submitInteraction({
    kind: "request-changes",
    reason
  });
  if (submitResult) {
    changesOpen.value = false;
    emit("submitted", submitResult);
  }
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
  if (key === "accept") {
    void acceptResult();
    return;
  }
  if (key === "retry") {
    void retryExecution();
    return;
  }
  if (key === "request-changes") {
    toggleChanges();
    return;
  }
  if (key === "ask-explanation") {
    emit("action", "ask-explanation");
  }
}

function logEntryClass(entry: ExecutionLogEntry): string {
  return `is-${entry.level}`;
}
</script>

<template>
  <AiResponseCard
    type="execution-result"
    :identity="identity"
    :evidence="evidence"
    :status="status"
    :title="t('ai.executionResult.card.title')"
    :description="t('ai.executionResult.card.description')"
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
      <div
        class="ai-execution"
        :data-execution-state="executionState"
        :class="`is-${executionState}`"
      >
        <dl class="ai-execution__meta">
          <div class="ai-execution__meta-row">
            <dt>{{ t("ai.executionResult.agent") }}</dt>
            <dd>{{ payload.agentName }}</dd>
          </div>
          <div class="ai-execution__meta-row">
            <dt>{{ t("ai.executionResult.taskRef") }}</dt>
            <dd class="ai-execution__mono">{{ payload.taskRef }}</dd>
          </div>
          <div class="ai-execution__meta-row">
            <dt>{{ t("ai.executionResult.state.label") }}</dt>
            <dd>
              <span class="ai-execution__state" :class="`is-${executionState}`">
                <component :is="stateIcon" :size="14" aria-hidden="true" />
                {{ stateLabel }}
              </span>
            </dd>
          </div>
        </dl>

        <p class="ai-execution__summary">{{ payload.summary }}</p>

        <section class="ai-execution__section">
          <p class="ai-execution__section-title">
            {{ t("ai.executionResult.changedFiles.title") }}
          </p>
          <p
            v-if="payload.changedFiles.length === 0"
            class="ai-execution__muted"
          >
            {{ t("ai.executionResult.changedFiles.empty") }}
          </p>
          <Table v-else :caption="t('ai.executionResult.changedFiles.title')">
            <TableHead>
              <TableRow>
                <TableHeader>{{
                  t("ai.executionResult.changedFiles.path")
                }}</TableHeader>
                <TableHeader>{{
                  t("ai.executionResult.changedFiles.fileStatus")
                }}</TableHeader>
                <TableHeader>{{
                  t("ai.executionResult.changedFiles.changes")
                }}</TableHeader>
              </TableRow>
            </TableHead>
            <TableBody>
              <TableRow v-for="file in payload.changedFiles" :key="file.path">
                <TableCell>
                  <span class="ai-execution__mono">{{ file.path }}</span>
                </TableCell>
                <TableCell>
                  <Badge :variant="FILE_STATUS_VARIANT[file.status]" size="sm">
                    {{ t(`ai.executionResult.fileStatus.${file.status}`) }}
                  </Badge>
                </TableCell>
                <TableCell numeric>
                  <span class="ai-execution__additions"
                    >+{{ file.additions }}</span
                  >
                  <span class="ai-execution__deletions"
                    >-{{ file.deletions }}</span
                  >
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </section>

        <section v-if="payload.warnings?.length" class="ai-execution__section">
          <p class="ai-execution__section-title">
            {{ t("ai.executionResult.warnings.title") }}
          </p>
          <ul class="ai-execution__warnings">
            <li
              v-for="(warning, index) in payload.warnings"
              :key="`warn-${index}`"
            >
              <TriangleAlert
                class="ai-execution__warning-icon"
                :size="14"
                aria-hidden="true"
              />
              {{ warning }}
            </li>
          </ul>
        </section>

        <section
          v-if="payload.failure"
          class="ai-execution__failure"
          role="alert"
        >
          <p class="ai-execution__section-title">
            {{ t("ai.executionResult.failure.title") }}
          </p>
          <dl class="ai-execution__failure-list">
            <div class="ai-execution__failure-row">
              <dt>{{ t("ai.executionResult.failure.code") }}</dt>
              <dd class="ai-execution__mono">{{ payload.failure.code }}</dd>
            </div>
            <div class="ai-execution__failure-row">
              <dt>{{ t("ai.executionResult.failure.retryable") }}</dt>
              <dd>
                {{
                  payload.failure.retryable
                    ? t("ai.executionResult.failure.retryable")
                    : t("ai.executionResult.failure.notRetryable")
                }}
              </dd>
            </div>
          </dl>
          <p class="ai-execution__failure-message">
            {{ payload.failure.message }}
          </p>
        </section>

        <p v-if="retryRequested" class="ai-execution__retry-note" role="status">
          {{ t("ai.executionResult.retryNote") }}
        </p>

        <section v-if="detailsOpen" class="ai-execution__details">
          <p class="ai-execution__section-title">
            {{ t("ai.executionResult.details.title") }}
          </p>

          <div v-if="payload.diff" class="ai-execution__diff">
            <p class="ai-execution__diff-title">
              {{ t("ai.executionResult.details.diff") }}
              <span class="ai-execution__mono">{{ payload.diff.path }}</span>
            </p>
            <pre class="ai-execution__diff-body"><code><template
              v-for="(hunk, hunkIndex) in payload.diff.hunks"
              :key="`hunk-${hunkIndex}`"
            ><span class="ai-execution__diff-hunk">{{ hunk.header }}
</span><template
              v-for="(line, lineIndex) in hunk.lines"
              :key="`line-${lineIndex}`"
            ><span class="ai-execution__diff-line" :class="`is-${line.kind}`">{{ line.text }}
</span></template></template></code></pre>
          </div>

          <div class="ai-execution__log">
            <div class="ai-execution__log-head">
              <span class="ai-execution__section-title">{{
                t("ai.executionResult.log.title")
              }}</span>
              <button
                type="button"
                class="ai-execution__log-toggle"
                :aria-expanded="logsOpen"
                @click="logsOpen = !logsOpen"
              >
                {{
                  logsOpen
                    ? t("ai.executionResult.log.hide")
                    : t("ai.executionResult.log.show")
                }}
                <component
                  :is="logsOpen ? ChevronUp : ChevronDown"
                  :size="14"
                  aria-hidden="true"
                />
              </button>
            </div>
            <p v-if="!hasLogs" class="ai-execution__muted">
              {{ t("ai.executionResult.log.empty") }}
            </p>
            <pre v-else-if="logsOpen" class="ai-execution__log-body"><code><span
              v-for="(entry, index) in payload.logs"
              :key="`log-${index}`"
              class="ai-execution__log-line"
              :class="logEntryClass(entry)"
            >{{ entry.time }}  {{ entry.text }}
</span></code></pre>
          </div>
        </section>

        <section v-if="changesOpen" class="ai-execution__changes">
          <p class="ai-execution__section-title">
            {{ t("ai.executionResult.requestChanges.title") }}
          </p>
          <Textarea
            :model-value="changeReason"
            :placeholder="t('ai.executionResult.requestChanges.placeholder')"
            :invalid="Boolean(changeError)"
            size="sm"
            @update:model-value="value => (changeReason = value)"
          />
          <p v-if="changeError" class="ai-execution__error" role="alert">
            {{ changeError }}
          </p>
          <div class="ai-execution__changes-actions">
            <Button variant="ghost" size="sm" @click="toggleChanges">
              {{ t("ai.executionResult.requestChanges.cancel") }}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              :loading="submitting"
              @click="submitChanges"
            >
              <FileCode2 :size="14" aria-hidden="true" />
              {{ t("ai.executionResult.requestChanges.submit") }}
            </Button>
          </div>
        </section>
      </div>
    </template>
  </AiResponseCard>
</template>

<style scoped>
.ai-execution {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ai-execution__meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
}

.ai-execution__meta-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--space-2);
  align-items: baseline;
}

.ai-execution__meta-row dt {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-execution__meta-row dd {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  word-break: break-word;
}

.ai-execution__mono {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

/* 执行状态：语义色 + 图标 + 文字三通道 */
.ai-execution__state {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0 var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-body);
  background: var(--surface-raised);
  border: var(--border-1) solid currentcolor;
  border-radius: var(--radius-xs);
}

.ai-execution__state.is-succeeded {
  color: var(--success-strong);
}

.ai-execution__state.is-partial {
  color: var(--warning-strong);
}

.ai-execution__state.is-failed {
  color: var(--danger-title);
}

.ai-execution__state.is-cancelled {
  color: var(--text-secondary);
}

.ai-execution__summary {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.ai-execution__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-execution__section-title {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  line-height: var(--line-height-micro);
}

.ai-execution__muted {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-execution__additions {
  color: var(--success-strong);
}

.ai-execution__deletions {
  margin-left: var(--space-2);
  color: var(--danger-title);
}

.ai-execution__warnings {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-execution__warnings li {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-execution__warning-icon {
  flex: none;
  margin-top: var(--space-1);
  color: var(--warning-strong);
}

.ai-execution__failure {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--danger-surface);
  border: var(--border-1) solid var(--danger-border);
  border-radius: var(--radius-sm);
}

.ai-execution__failure .ai-execution__section-title {
  color: var(--danger-title);
}

.ai-execution__failure-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
}

.ai-execution__failure-row {
  display: flex;
  gap: var(--space-2);
  align-items: baseline;
}

.ai-execution__failure-row dt {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-execution__failure-row dd {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-execution__failure-message {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-execution__retry-note {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-execution__details {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.ai-execution__diff-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-execution__diff-body {
  max-height: var(--size-ai-log-max-height);
  margin: var(--space-2) 0 0;
  padding: var(--space-2);
  overflow: auto;
  background: var(--surface-sunken);
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--size-ai-diff-line);
}

.ai-execution__diff-hunk {
  display: block;
  color: var(--info-strong);
}

.ai-execution__diff-line {
  display: block;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text-secondary);
}

.ai-execution__diff-line.is-add {
  color: var(--success-strong);
}

.ai-execution__diff-line.is-remove {
  color: var(--danger-title);
}

.ai-execution__log {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-execution__log-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.ai-execution__log-toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  background: transparent;
  border: none;
  cursor: pointer;
}

.ai-execution__log-toggle:hover {
  color: var(--text-primary);
}

.ai-execution__log-toggle:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.ai-execution__log-body {
  max-height: var(--size-ai-log-max-height);
  margin: 0;
  padding: var(--space-2);
  overflow: auto;
  background: var(--surface-sunken);
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--size-ai-diff-line);
}

.ai-execution__log-line {
  display: block;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text-secondary);
}

.ai-execution__log-line.is-warn {
  color: var(--warning-strong);
}

.ai-execution__log-line.is-error {
  color: var(--danger-title);
}

.ai-execution__changes {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.ai-execution__error {
  margin: 0;
  color: var(--danger-title);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-execution__changes-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

@media (prefers-reduced-motion: reduce) {
  .ai-execution * {
    transition: none;
  }
}
</style>
