<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useToast } from "@/components/ui/toast";
import {
  answerDiscoveryQuestion,
  confirmDiscoveryFinding,
  deferDiscoveryQuestion,
  getDiscoverySession,
  listDiscoverySessions,
  startDiscoverySession,
  transitionDiscoverySession
} from "@/api/discovery";
import type { DiscoveryFinding, DiscoverySessionDetail } from "@/api/discovery";

const props = defineProps<{
  projectId: string;
  importId?: string;
}>();

const { t } = useI18n();
const toast = useToast();

const detail = ref<DiscoverySessionDetail | null>(null);
const busy = ref(false);
const ready = ref(false);
const answerDrafts = ref<Record<string, string>>({});
const choices = ref<Record<string, string>>({});
const notes = ref<Record<string, string>>({});

const CHOICE_STATUS: Record<string, string> = {
  confirmed: "CONFIRMED",
  resolved: "CONFIRMED",
  rejected: "REJECTED",
  unsure: "PENDING"
};
const CHOICES = ["confirmed", "resolved", "rejected", "unsure"] as const;

const KIND_KEYS: Record<string, string> = {
  observed: "kindObserved",
  user_confirmed: "kindUserConfirmed",
  speculation: "kindSpeculation",
  contradicted: "kindContradicted"
};
const VERIFICATION_KEYS: Record<string, string> = {
  PENDING: "verificationPending",
  CONFIRMED: "verificationConfirmed",
  REJECTED: "verificationRejected",
  CONFLICT: "verificationConflict"
};
const SESSION_KEYS: Record<string, string> = {
  IN_PROGRESS: "sessionInProgress",
  NEEDS_USER_INPUT: "sessionNeedsUserInput",
  REVIEW_REQUIRED: "sessionReviewRequired",
  COMPLETED: "sessionCompleted",
  PAUSED: "sessionPaused"
};

const openQuestions = computed(
  () => detail.value?.questions.filter(item => item.status === "open") ?? []
);
const decidedQuestions = computed(
  () => detail.value?.questions.filter(item => item.status !== "open") ?? []
);
const pendingFindings = computed(
  () =>
    detail.value?.findings.filter(item => item.verification_status === "PENDING") ?? []
);
const decidedFindings = computed(
  () =>
    detail.value?.findings.filter(item => item.verification_status !== "PENDING") ?? []
);
const statusVariant = computed(() => {
  const status = detail.value?.session.status;
  if (status === "COMPLETED") return "success" as const;
  if (status === "NEEDS_USER_INPUT") return "warning" as const;
  return "info" as const;
});

function kindLabel(kind: string) {
  const key = KIND_KEYS[kind];
  return key ? t(`pm.discovery.${key}`) : kind;
}
function verificationLabel(status: string) {
  const key = VERIFICATION_KEYS[status];
  return key ? t(`pm.discovery.${key}`) : status;
}
function sessionLabel(status: string) {
  const key = SESSION_KEYS[status];
  return key ? t(`pm.discovery.${key}`) : status;
}

async function reload(sessionId: string) {
  detail.value = await getDiscoverySession(props.projectId, sessionId);
}

async function loadExisting() {
  const list = await listDiscoverySessions(props.projectId);
  const existing = list.items[0];
  if (existing) {
    await reload(existing.id);
  }
}

onMounted(async () => {
  try {
    await loadExisting();
  } catch {
    detail.value = null;
  } finally {
    ready.value = true;
  }
});

async function start() {
  busy.value = true;
  try {
    const outcome = await startDiscoverySession(
      props.projectId,
      props.importId || undefined
    );
    await reload(outcome.session.id);
    toast.toast({ variant: "success", title: t("pm.discovery.started") });
  } catch {
    toast.toast({
      variant: "danger",
      title: t("pm.discovery.actionFailed")
    });
  } finally {
    busy.value = false;
  }
}

async function submitAnswer(questionId: string) {
  const answer = (answerDrafts.value[questionId] ?? "").trim();
  if (!answer || !detail.value) return;
  busy.value = true;
  try {
    await answerDiscoveryQuestion(
      props.projectId,
      detail.value.session.id,
      questionId,
      answer
    );
    answerDrafts.value[questionId] = "";
    await reload(detail.value.session.id);
  } catch {
    toast.toast({ variant: "danger", title: t("pm.discovery.actionFailed") });
  } finally {
    busy.value = false;
  }
}

async function defer(questionId: string) {
  if (!detail.value) return;
  busy.value = true;
  try {
    await deferDiscoveryQuestion(
      props.projectId,
      detail.value.session.id,
      questionId
    );
    await reload(detail.value.session.id);
  } catch {
    toast.toast({ variant: "danger", title: t("pm.discovery.actionFailed") });
  } finally {
    busy.value = false;
  }
}

async function confirmFinding(finding: DiscoveryFinding) {
  const choice = choices.value[finding.id];
  if (!choice || !detail.value) return;
  const note = (notes.value[finding.id] ?? "").trim();
  busy.value = true;
  try {
    const label = t(`pm.discovery.choice${choice.charAt(0).toUpperCase()}${choice.slice(1)}`);
    await confirmDiscoveryFinding(
      props.projectId,
      detail.value.session.id,
      finding.id,
      {
        status: CHOICE_STATUS[choice],
        answer: note ? `${label}：${note}` : label,
        confirmed_finding:
          choice === "resolved"
            ? note || t("pm.discovery.resolvedDefault")
            : undefined
      }
    );
    await reload(detail.value.session.id);
  } catch {
    toast.toast({ variant: "danger", title: t("pm.discovery.actionFailed") });
  } finally {
    busy.value = false;
  }
}

async function complete() {
  if (!detail.value) return;
  busy.value = true;
  try {
    await transitionDiscoverySession(
      props.projectId,
      detail.value.session.id,
      "COMPLETED"
    );
    await reload(detail.value.session.id);
  } catch {
    toast.toast({ variant: "danger", title: t("pm.discovery.actionFailed") });
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section class="discovery" data-testid="discovery-panel">
    <div class="discovery__head">
      <h3 class="discovery__title">{{ t("pm.discovery.title") }}</h3>
      <Badge v-if="detail" :variant="statusVariant">
        {{ sessionLabel(detail.session.status) }}
      </Badge>
    </div>
    <p class="discovery__subtitle">{{ t("pm.discovery.subtitle") }}</p>

    <div v-if="!ready" class="discovery__hint">{{ t("pm.import.loadFailed") }}</div>

    <template v-else-if="!detail">
      <Button :loading="busy" @click="start">
        {{ t("pm.discovery.start") }}
      </Button>
    </template>

    <template v-else>
      <section v-if="openQuestions.length" class="discovery__group">
        <h4 class="discovery__group-title">{{ t("pm.discovery.questions") }}</h4>
        <article
          v-for="question in openQuestions"
          :key="question.id"
          class="card"
        >
          <p class="card__content">{{ question.question }}</p>
          <p v-if="question.why_blocking" class="card__hint">
            {{ question.why_blocking }}
          </p>
          <div class="card__row">
            <Input
              v-model="answerDrafts[question.id]"
              :placeholder="t('pm.discovery.answerPlaceholder')"
            />
            <Button :disabled="busy" @click="submitAnswer(question.id)">
              {{ t("pm.discovery.submitAnswer") }}
            </Button>
            <Button variant="secondary" :disabled="busy" @click="defer(question.id)">
              {{ t("pm.discovery.defer") }}
            </Button>
          </div>
        </article>
      </section>

      <section v-if="pendingFindings.length" class="discovery__group">
        <h4 class="discovery__group-title">{{ t("pm.discovery.findings") }}</h4>
        <article
          v-for="finding in pendingFindings"
          :key="finding.id"
          class="card"
        >
          <header class="card__head">
            <Badge variant="warning">{{ kindLabel(finding.kind) }}</Badge>
            <span class="card__confidence" v-if="finding.confidence !== null">
              {{ t("pm.discovery.confidence") }}
              {{ Math.round(finding.confidence * 100) }}%
            </span>
          </header>
          <p class="card__content">{{ finding.content }}</p>
          <div v-if="finding.evidence.length" class="evidence">
            <p class="evidence__title">{{ t("pm.discovery.evidence") }}</p>
            <p v-for="(item, index) in finding.evidence" :key="index" class="evidence__item">
              <code>{{ item.file }}</code>
              <span v-if="item.description"> — {{ item.description }}</span>
            </p>
          </div>
          <fieldset class="choices">
            <legend class="visually-hidden">{{ t("pm.discovery.findings") }}</legend>
            <label v-for="choice in CHOICES" :key="choice" class="choices__item">
              <input
                v-model="choices[finding.id]"
                type="radio"
                :name="`finding-${finding.id}`"
                :value="choice"
              />
              <span>{{ t(`pm.discovery.choice${choice.charAt(0).toUpperCase()}${choice.slice(1)}`) }}</span>
            </label>
          </fieldset>
          <div class="card__row">
            <Input
              v-model="notes[finding.id]"
              :placeholder="t('pm.discovery.notePlaceholder')"
            />
            <Button :disabled="busy || !choices[finding.id]" @click="confirmFinding(finding)">
              {{ t("pm.discovery.confirm") }}
            </Button>
          </div>
        </article>
      </section>

      <section v-if="decidedFindings.length" class="discovery__group">
        <article v-for="finding in decidedFindings" :key="finding.id" class="card card--done">
          <header class="card__head">
            <Badge :variant="finding.verification_status === 'CONFIRMED' ? 'success' : 'neutral'">
              {{ verificationLabel(finding.verification_status) }}
            </Badge>
            <span class="card__kind">{{ kindLabel(finding.kind) }}</span>
          </header>
          <p class="card__content">{{ finding.content }}</p>
        </article>
      </section>

      <section v-if="decidedQuestions.length" class="discovery__group">
        <article v-for="question in decidedQuestions" :key="question.id" class="card card--done">
          <header class="card__head">
            <Badge :variant="question.status === 'answered' ? 'success' : 'neutral'">
              {{ question.status === "answered" ? t("pm.discovery.statusAnswered") : t("pm.discovery.statusDeferred") }}
            </Badge>
          </header>
          <p class="card__content">{{ question.question }}</p>
          <p v-for="answer in question.answers" :key="answer.id" class="card__hint">
            {{ answer.answer }}
          </p>
        </article>
      </section>

      <div
        v-if="detail.session.status === 'IN_PROGRESS' && !openQuestions.length && !pendingFindings.length"
        class="discovery__actions"
      >
        <Button :disabled="busy" @click="complete">
          {{ t("pm.discovery.complete") }}
        </Button>
      </div>
    </template>
  </section>
</template>

<style scoped>
.discovery {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.discovery__head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.discovery__title {
  margin: 0;
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.discovery__subtitle {
  margin: 0;
  color: var(--text-secondary);
}

.discovery__hint {
  color: var(--text-secondary);
}

.discovery__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.discovery__group-title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.discovery__actions {
  display: flex;
  justify-content: flex-end;
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.card--done {
  opacity: 0.85;
}

.card__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.card__kind {
  color: var(--text-secondary);
}

.card__confidence {
  color: var(--text-secondary);
}

.card__content {
  margin: 0;
  color: var(--text-primary);
}

.card__hint {
  margin: 0;
  color: var(--text-secondary);
}

.card__row {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}

.card__row > :first-child {
  flex: 1;
}

.evidence {
  padding: var(--space-3);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
}

.evidence__title {
  margin: 0 0 var(--space-1);
  color: var(--text-secondary);
}

.evidence__item {
  margin: 0;
  color: var(--text-primary);
}

.choices {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  border: 0;
}

.choices__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--text-primary);
}

.choices__item input {
  accent-color: var(--accent);
}

.choices__item input:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
}

@media (prefers-reduced-motion: reduce) {
  .discovery *,
  .card {
    transition: none;
  }
}
</style>