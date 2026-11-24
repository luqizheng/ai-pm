<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import { PageFrame } from "@/components/shell";
import {
  answerDiscoveryQuestion,
  confirmDiscoveryFinding,
  getDiscoverySession,
  listDiscoverySessions,
  transitionDiscoverySession,
  type DiscoveryFinding,
  type DiscoveryQuestion,
  type DiscoverySession,
  type DiscoverySessionDetail
} from "@/api/discovery";
import { useDesignProject } from "@/views/project/design/use-design-project";

/**
 * 代码发现与人工确认（S01-I2，接真）。
 *
 * 数据 = discovery sessions（列表 + 详情）。
 *
 * 治理红线（S01-I2）：**AI 只能产出发现（finding）并要求澄清，人类负责
 * 确认或否决**——`verification_status` 的每一次流转都是人工动作，页面不
 * 提供「自动确认」。
 *
 * 本页做**人工确认/答复/会话处理**三件事：
 * - 确认/否决 finding（真实命令 confirmDiscoveryFinding）；
 * - 回答阻塞问题（真实命令 answerDiscoveryQuestion）；
 * - 处理会话状态（B03-01：REVIEW_REQUIRED 退回/完结、NEEDS_USER_INPUT 完结，
 *   真实命令 transitionDiscoverySession——没有这一步，等待复核的会话就是死信）。
 * 会话创建（startDiscoverySession）在此只给入口说明，不做隐式启动。
 */
const { t } = useI18n();
const { projectId, hasProject } = useDesignProject();

const sessions = ref<DiscoverySession[]>([]);
const selectedId = ref("");
const detail = ref<DiscoverySessionDetail | null>(null);
const loading = ref(true);
const detailLoading = ref(false);
const loadError = ref(false);
const acting = ref(false);
const actionError = ref("");

async function load() {
  if (!hasProject.value) {
    sessions.value = [];
    detail.value = null;
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    const result = await listDiscoverySessions(projectId.value);
    sessions.value = result.items ?? [];
    if (!sessions.value.some(item => item.id === selectedId.value)) {
      selectedId.value = sessions.value[0]?.id ?? "";
    }
    await loadDetail();
  } catch {
    loadError.value = true;
    sessions.value = [];
  } finally {
    loading.value = false;
  }
}

async function loadDetail() {
  if (!selectedId.value) {
    detail.value = null;
    return;
  }
  detailLoading.value = true;
  try {
    detail.value = await getDiscoverySession(projectId.value, selectedId.value);
  } catch {
    detail.value = null;
  } finally {
    detailLoading.value = false;
  }
}

onMounted(load);
watch(projectId, load);
watch(selectedId, loadDetail);

const findings = computed(() => detail.value?.findings ?? []);
const questions = computed(() => detail.value?.questions ?? []);

/** 待人工确认的发现（verification_status 非 confirmed/rejected）。 */
/**
 * 待人工确认的发现。
 *
 * 库级 CHECK 是**大写**四值：PENDING / CONFIRMED / REJECTED / CONFLICT。
 * 只有 PENDING 与 CONFLICT（冲突待裁决）需要人工动作；CONFIRMED / REJECTED
 * 是已定案状态。
 */
const pendingFindings = computed(() =>
  findings.value.filter(
    item =>
      item.verification_status === "PENDING" ||
      item.verification_status === "CONFLICT"
  )
);

/** 未答复的阻塞问题。 */
const openQuestions = computed(() =>
  questions.value.filter(item => item.status === "open")
);

async function decideFinding(
  finding: DiscoveryFinding,
  status: "CONFIRMED" | "REJECTED"
) {
  if (!selectedId.value) return;
  acting.value = true;
  actionError.value = "";
  try {
    await confirmDiscoveryFinding(
      projectId.value,
      selectedId.value,
      finding.id,
      {
        status,
        answer:
          status === "CONFIRMED" ? "confirmed by human" : "rejected by human"
      }
    );
    await loadDetail();
  } catch {
    actionError.value = t("s01.discovery.actionFailed");
  } finally {
    acting.value = false;
  }
}

// ──────────────────────────────────── 会话人工处理（B03-01 死信修复）
//
// REVIEW_REQUIRED / NEEDS_USER_INPUT 的会话在编排扫描集之外（或等用户输入）：
// 没有人工退回/完结入口时，chat 续接进来的消息永远无人消费。动作本身走
// 既有状态机（非法跃迁服务端 409）。

/** 会话处理动作进行中（防重复提交）。 */
const transitioning = ref(false);
const transitionError = ref("");

const sessionStatus = computed(() => detail.value?.session.status ?? "");
const awaitingReview = computed(() => sessionStatus.value === "REVIEW_REQUIRED");
const needsUserInput = computed(() => sessionStatus.value === "NEEDS_USER_INPUT");

// ── 阻塞问题答复（B02-01：页面内答复入口）────────────────────────
// 编排器只在 chat 线程的答复被消费；此前页面只能看不能答（answerNote 明示
// 「入口未开放」），闭环断在 UI。此处直连既有命令 answerDiscoveryQuestion，
// 答复入库后由下一轮编排消费（与 chat 线程答复同一管线）。

/** 每个未答复问题的草稿（按 question id 键控）。 */
const answerDrafts = ref<Record<string, string>>({});
/** 正在提交答复的问题 id（防重复提交）。 */
const answeringId = ref<string | null>(null);
const answerError = ref("");

async function submitAnswer(question: DiscoveryQuestion) {
  if (!selectedId.value || answeringId.value) return;
  const answer = (answerDrafts.value[question.id] ?? "").trim();
  if (!answer) return;
  answeringId.value = question.id;
  answerError.value = "";
  try {
    await answerDiscoveryQuestion(
      projectId.value,
      selectedId.value,
      question.id,
      answer
    );
    answerDrafts.value = { ...answerDrafts.value, [question.id]: "" };
    await loadDetail();
  } catch {
    answerError.value = t("s01.discovery.answerFailed");
  } finally {
    answeringId.value = null;
  }
}

/**
 * 人工处理会话：REVIEW_REQUIRED 退回 IN_PROGRESS（编排器恢复消费未处理
 * 消息）或完结 COMPLETED；NEEDS_USER_INPUT 完结 COMPLETED。失败（含 409
 * 竞态）重拉详情对齐服务端真源。
 */
async function transitionSession(status: "IN_PROGRESS" | "COMPLETED") {
  if (!selectedId.value || transitioning.value) return;
  transitioning.value = true;
  transitionError.value = "";
  try {
    await transitionDiscoverySession(
      projectId.value,
      selectedId.value,
      status
    );
    await load();
  } catch {
    transitionError.value = t("s01.discovery.transitionConflict");
    await loadDetail();
  } finally {
    transitioning.value = false;
  }
}

/** 四值 → 语义档：CONFLICT 是「冲突待裁决」，与 PENDING 同属待人工。 */
function verificationTone(
  status: string
): "success" | "danger" | "warning" | "neutral" {
  switch (status) {
    case "CONFIRMED":
      return "success";
    case "REJECTED":
      return "danger";
    case "PENDING":
    case "CONFLICT":
      return "warning";
    default:
      return "neutral";
  }
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s01.discovery.pageTitle') }]"
    :title="t('s01.discovery.pageTitle')"
    :description="t('s01.discovery.pageDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="dc-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s01.discovery.refresh") }}
      </Button>
    </template>

    <Alert
      v-if="loadError"
      variant="danger"
      size="sm"
      :title="t('s01.discovery.errorTitle')"
    />

    <Alert v-if="actionError" variant="danger" size="sm" :title="actionError" />

    <EmptyState
      v-if="!hasProject"
      :title="t('s01.discovery.noProjectTitle')"
      :description="t('s01.discovery.noProjectHint')"
    />

    <EmptyState
      v-else-if="!loading && sessions.length === 0"
      :title="t('s01.discovery.emptyTitle')"
      :description="t('s01.discovery.emptyHint')"
    />

    <div v-else class="dc-layout">
      <Card class="dc-sessions" data-testid="dc-sessions">
        <template #header>
          {{ t("s01.discovery.sessionsTitle", { count: sessions.length }) }}
        </template>
        <ul class="dc-list">
          <li v-for="item in sessions" :key="item.id">
            <button
              type="button"
              class="dc-item"
              :class="{ 'is-active': item.id === selectedId }"
              :aria-pressed="item.id === selectedId"
              :data-testid="`dc-session-${item.id}`"
              @click="selectedId = item.id"
            >
              <span class="dc-item__source">{{ item.source }}</span>
              <span class="dc-item__kind">{{ item.kind }}</span>
              <Badge variant="neutral" size="sm">{{ item.status }}</Badge>
            </button>
          </li>
        </ul>
      </Card>

      <div class="dc-main">
        <!-- B03-01：等待人工复核/输入的会话处理面板（不做 = 消息死信）。 -->
        <Card
          v-if="awaitingReview || needsUserInput"
          class="dc-section"
          data-testid="dc-session-actions"
        >
          <template #header>
            {{ t("s01.discovery.sessionActionsTitle") }}
          </template>
          <p class="dc-note">
            {{
              awaitingReview
                ? t("s01.discovery.reviewRequiredNotice")
                : t("s01.discovery.needsInputNotice")
            }}
          </p>
          <p
            v-if="detail?.session.last_round_error"
            class="dc-note"
            data-testid="dc-round-error"
          >
            {{ t("s01.discovery.lastRoundError") }}:
            {{ detail.session.last_round_error }}
          </p>
          <div class="dc-actions">
            <Button
              v-if="awaitingReview"
              variant="secondary"
              size="sm"
              :disabled="transitioning"
              data-testid="dc-resume-session"
              @click="transitionSession('IN_PROGRESS')"
            >
              {{ t("s01.discovery.resumeSession") }}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              :disabled="transitioning"
              data-testid="dc-complete-session"
              @click="transitionSession('COMPLETED')"
            >
              {{ t("s01.discovery.completeSession") }}
            </Button>
          </div>
        </Card>

        <Card class="dc-section" data-testid="dc-findings">
          <template #header>
            {{
              t("s01.discovery.findingsTitle", {
                pending: pendingFindings.length,
                total: findings.length
              })
            }}
          </template>
          <p v-if="findings.length === 0" class="dc-empty">
            {{ t("s01.discovery.findingsEmpty") }}
          </p>
          <ul v-else class="dc-list">
            <li
              v-for="finding in findings"
              :key="finding.id"
              class="dc-row"
              :data-testid="`dc-finding-${finding.id}`"
            >
              <Badge
                :variant="verificationTone(finding.verification_status)"
                size="sm"
              >
                {{
                  t(
                    `s01.discovery.verification.${finding.verification_status}`,
                    finding.verification_status
                  )
                }}
              </Badge>
              <span class="dc-row__kind">{{ finding.kind }}</span>
              <span class="dc-row__body">{{ finding.content }}</span>
              <template
                v-if="pendingFindings.some(item => item.id === finding.id)"
              >
                <Button
                  variant="primary"
                  size="sm"
                  :disabled="acting"
                  :data-testid="`dc-confirm-${finding.id}`"
                  @click="decideFinding(finding, 'CONFIRMED')"
                >
                  {{ t("s01.discovery.confirm") }}
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  :disabled="acting"
                  :data-testid="`dc-reject-${finding.id}`"
                  @click="decideFinding(finding, 'REJECTED')"
                >
                  {{ t("s01.discovery.reject") }}
                </Button>
              </template>
            </li>
          </ul>
        </Card>

        <Card class="dc-section" data-testid="dc-questions">
          <template #header>
            {{
              t("s01.discovery.questionsTitle", {
                open: openQuestions.length,
                total: questions.length
              })
            }}
          </template>
          <p v-if="questions.length === 0" class="dc-empty">
            {{ t("s01.discovery.questionsEmpty") }}
          </p>
          <ul v-else class="dc-list">
            <li
              v-for="question in questions"
              :key="question.id"
              class="dc-row"
              :data-testid="`dc-question-${question.id}`"
            >
              <Badge
                :variant="question.status === 'open' ? 'warning' : 'neutral'"
                size="sm"
              >
                {{
                  t(
                    `s01.discovery.questionStatus.${question.status}`,
                    question.status
                  )
                }}
              </Badge>
              <span class="dc-row__body">{{ question.question }}</span>
              <span v-if="question.why_blocking" class="dc-row__why">
                {{ question.why_blocking }}
              </span>
              <ul v-if="question.answers.length > 0" class="dc-answers">
                <li
                  v-for="answer in question.answers"
                  :key="answer.id"
                  class="dc-answer"
                >
                  {{ answer.answer }}
                </li>
              </ul>
              <div
                v-if="question.status === 'open'"
                class="dc-answer-form"
                :data-testid="`dc-answer-form-${question.id}`"
              >
                <Textarea
                  v-model="answerDrafts[question.id]"
                  size="sm"
                  :placeholder="t('s01.discovery.answerPlaceholder')"
                  :data-testid="`dc-answer-input-${question.id}`"
                />
                <Button
                  variant="primary"
                  size="sm"
                  :disabled="
                    answeringId !== null ||
                    !(answerDrafts[question.id] ?? '').trim()
                  "
                  :data-testid="`dc-answer-submit-${question.id}`"
                  @click="submitAnswer(question)"
                >
                  {{ t("s01.discovery.submitAnswer") }}
                </Button>
              </div>
            </li>
          </ul>
          <Alert
            v-if="answerError"
            variant="danger"
            size="sm"
            :title="answerError"
          />
          <p class="dc-note">{{ t("s01.discovery.answerNote") }}</p>
        </Card>

        <p v-if="detailLoading" class="dc-empty">
          {{ t("s01.discovery.loading") }}
        </p>
      </div>
    </div>
  </PageFrame>
</template>

<style scoped>
.dc-layout {
  display: grid;
  grid-template-columns: minmax(220px, 300px) 1fr;
  gap: var(--space-5);
  align-items: start;
}

.dc-main {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.dc-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.dc-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.dc-item {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  text-align: left;
  background: transparent;
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.dc-item:hover {
  background: var(--surface-base);
}

.dc-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.dc-item.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.dc-item__source {
  font-family: var(--font-mono);
  color: var(--ws-text-primary);
}

.dc-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.dc-row__kind {
  font-family: var(--font-mono);
  color: var(--ws-text-secondary);
}

.dc-row__body {
  flex: 1;
  min-width: 180px;
  color: var(--ws-text-primary);
}

.dc-row__why {
  flex-basis: 100%;
  color: var(--ws-text-secondary);
}

.dc-answers {
  flex-basis: 100%;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.dc-answer {
  padding-left: var(--space-3);
  color: var(--ws-text-secondary);
  border-left: var(--border-1) solid var(--ws-border);
}

.dc-answer-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  align-items: flex-start;
  padding-left: var(--space-3);
  border-left: var(--border-1) solid var(--ws-border);
}

.dc-answer-form :deep(textarea) {
  width: 100%;
}

.dc-empty,
.dc-note {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.dc-actions {
  display: flex;
  gap: var(--space-2);
}

</style>
