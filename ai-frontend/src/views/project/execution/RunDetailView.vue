<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ArrowLeft, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import {
  getTaskRun,
  listRunCommands,
  listRunCommits,
  listRunContinuations,
  listRunEvents,
  type RunCommit,
  type RunContinuation,
  type RunEvent,
  type TaskRunDetail,
  type WorkerCommand
} from "@/api/execution-plans";
import ExecutionFrame from "./ExecutionFrame.vue";
import {
  commandTone,
  formatDateTime,
  formatDuration,
  runLabel,
  runTone,
  taskTone,
  verdictTone
} from "./execution-contract";

/**
 * 执行详情（S05-02，接真）。
 *
 * 数据：Run 详情 + 事件时间线 + Worker 命令 + Continuation 链 + 逐仓交付。
 *
 * **完成三态分离**（本页的核心）：Run 状态（Agent 声称）、review verdict
 * （AI-PM 检查）、Task 状态（人工验收）三者分列，不合并成一个「状态」。
 * 时间线只呈现结构化事件，不是原始日志。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const runId = computed(() => String(route.params.runId ?? ""));
const detail = ref<TaskRunDetail | null>(null);
const events = ref<RunEvent[]>([]);
const commands = ref<WorkerCommand[]>([]);
const continuations = ref<RunContinuation[]>([]);
const commits = ref<RunCommit[]>([]);
const loading = ref(true);
const loadError = ref(false);

const kindLabels = computed(() => ({
  delivery: t("s05.execution.runKind.delivery"),
  review: t("s05.execution.runKind.review")
}));

async function load() {
  if (!runId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    detail.value = await getTaskRun(runId.value);
    const [eventList, commandList, continuationList, commitList] =
      await Promise.all([
        listRunEvents(runId.value),
        listRunCommands(runId.value),
        listRunContinuations(runId.value),
        listRunCommits(runId.value)
      ]);
    events.value = eventList;
    commands.value = commandList;
    continuations.value = continuationList;
    commits.value = commitList;
  } catch {
    loadError.value = true;
    detail.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(runId, load);

function goBack() {
  void router.push("/project/execution");
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s05.execution.module') },
      { label: t('s05.execution.page.centerTitle'), to: '/project/execution' }
    ]"
    :title="detail?.task_title ?? t('s05.execution.page.runDetailTitle')"
    :description="
      detail
        ? runLabel(detail.run_kind, detail.attempt_no, kindLabels)
        : t('s05.execution.common.loading')
    "
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="rd-back"
        @click="goBack"
      >
        <ArrowLeft :size="14" />
        {{ t("s05.execution.common.back") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="rd-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s05.execution.common.refresh") }}
      </Button>
    </template>

    <ExecutionFrame>
      <Alert
        v-if="loadError"
        variant="danger"
        size="sm"
        :title="t('s05.execution.common.errorTitle')"
      />

      <EmptyState
        v-else-if="!loading && !detail"
        :title="t('s05.execution.runDetail.notFoundTitle')"
        :description="t('s05.execution.runDetail.notFoundHint')"
      />

      <template v-else-if="detail">
        <!-- 完成三态分离：Run / Review / Task 分列 -->
        <Card class="rd-tri" data-testid="rd-tri-state">
          <template #header>{{
            t("s05.execution.runDetail.triStateTitle")
          }}</template>
          <dl class="rd-tri__list">
            <div class="rd-tri__row">
              <dt>{{ t("s05.execution.runDetail.triRun") }}</dt>
              <dd>
                <Badge :variant="runTone(detail.status)" size="sm">
                  {{
                    t(`s05.execution.runStatus.${detail.status}`, detail.status)
                  }}
                </Badge>
                <span v-if="detail.outcome" class="rd-meta">{{
                  detail.outcome
                }}</span>
              </dd>
            </div>
            <div class="rd-tri__row">
              <dt>{{ t("s05.execution.runDetail.triReview") }}</dt>
              <dd>
                <Badge
                  v-if="detail.review_verdict"
                  :variant="verdictTone(detail.review_verdict)"
                  size="sm"
                >
                  {{
                    t(
                      `s05.execution.verdict.${detail.review_verdict}`,
                      detail.review_verdict
                    )
                  }}
                </Badge>
                <span v-else class="rd-meta">
                  {{ t("s05.execution.runDetail.notReviewed") }}
                </span>
              </dd>
            </div>
            <div class="rd-tri__row">
              <dt>{{ t("s05.execution.runDetail.triTask") }}</dt>
              <dd>
                <Badge :variant="taskTone(detail.status)" size="sm">
                  {{
                    t(
                      `s05.execution.taskStatus.${detail.status}`,
                      detail.status
                    )
                  }}
                </Badge>
                <span class="rd-meta">{{
                  t("s05.execution.runDetail.taskStateHint")
                }}</span>
              </dd>
            </div>
          </dl>
        </Card>

        <Card class="rd-section" data-testid="rd-meta">
          <template #header>{{
            t("s05.execution.runDetail.metaTitle")
          }}</template>
          <dl class="rd-meta-list">
            <div class="rd-meta-row">
              <dt>{{ t("s05.execution.runDetail.metaStage") }}</dt>
              <dd>
                {{ detail.stage ?? t("s05.execution.common.notAvailable") }}
              </dd>
            </div>
            <div class="rd-meta-row">
              <dt>{{ t("s05.execution.runDetail.metaExecutor") }}</dt>
              <dd>
                {{
                  detail.executor_kind ?? t("s05.execution.common.notAvailable")
                }}
              </dd>
            </div>
            <div class="rd-meta-row">
              <dt>{{ t("s05.execution.runDetail.metaDuration") }}</dt>
              <dd>
                {{
                  formatDuration(detail.started_at, detail.finished_at) ||
                  t("s05.execution.common.notAvailable")
                }}
              </dd>
            </div>
            <div class="rd-meta-row">
              <dt>{{ t("s05.execution.runDetail.metaStarted") }}</dt>
              <dd>{{ formatDateTime(detail.started_at) }}</dd>
            </div>
            <div class="rd-meta-row">
              <dt>{{ t("s05.execution.runDetail.metaDeadline") }}</dt>
              <dd>{{ formatDateTime(detail.deadline_at) }}</dd>
            </div>
            <div class="rd-meta-row">
              <dt>{{ t("s05.execution.runDetail.metaIntegration") }}</dt>
              <dd>{{ detail.integration_status }}</dd>
            </div>
            <div v-if="detail.release_reason" class="rd-meta-row">
              <dt>{{ t("s05.execution.runDetail.metaRelease") }}</dt>
              <dd>{{ detail.release_reason }}</dd>
            </div>
            <div v-if="detail.failure_category" class="rd-meta-row">
              <dt>{{ t("s05.execution.runDetail.metaFailure") }}</dt>
              <dd>
                {{ detail.failure_category }} · {{ detail.failure_reason }}
              </dd>
            </div>
          </dl>
        </Card>

        <Card class="rd-section" data-testid="rd-repos">
          <template #header>{{
            t("s05.execution.runDetail.reposTitle")
          }}</template>
          <p v-if="detail.repositories.length === 0" class="rd-empty">
            {{ t("s05.execution.runDetail.reposEmpty") }}
          </p>
          <ul v-else class="rd-list">
            <li
              v-for="repo in detail.repositories"
              :key="repo.repository_id"
              class="rd-row"
            >
              <span class="rd-row__title">{{ repo.repository_name }}</span>
              <span class="rd-meta"
                >{{ repo.base_ref }}@{{ repo.base_commit }}</span
              >
              <Badge
                :variant="repo.write_access ? 'warning' : 'neutral'"
                size="sm"
              >
                {{
                  repo.write_access
                    ? t("s05.execution.runDetail.writeAccess")
                    : t("s05.execution.runDetail.readOnly")
                }}
              </Badge>
              <span class="rd-meta">{{ repo.workspace_path }}</span>
            </li>
          </ul>
        </Card>

        <Card class="rd-section" data-testid="rd-events">
          <template #header>
            {{
              t("s05.execution.runDetail.eventsTitle", { count: events.length })
            }}
          </template>
          <p v-if="events.length === 0" class="rd-empty">
            {{ t("s05.execution.runDetail.eventsEmpty") }}
          </p>
          <ol v-else class="rd-events">
            <li
              v-for="event in events"
              :key="event.id"
              class="rd-event"
              :data-testid="`rd-event-${event.event_type}`"
            >
              <span class="rd-meta">{{
                formatDateTime(event.occurred_at)
              }}</span>
              <span class="rd-event__type">
                {{
                  t(
                    `s05.execution.eventType.${event.event_type}`,
                    event.event_type
                  )
                }}
              </span>
              <span class="rd-meta">gen {{ event.lease_generation }}</span>
            </li>
          </ol>
        </Card>

        <Card class="rd-section" data-testid="rd-commands">
          <template #header>
            {{
              t("s05.execution.runDetail.commandsTitle", {
                count: commands.length
              })
            }}
          </template>
          <p v-if="commands.length === 0" class="rd-empty">
            {{ t("s05.execution.runDetail.commandsEmpty") }}
          </p>
          <ul v-else class="rd-list">
            <li
              v-for="command in commands"
              :key="command.id"
              class="rd-row"
              :data-testid="`rd-command-${command.status}`"
            >
              <Badge :variant="commandTone(command.status)" size="sm">
                {{
                  t(
                    `s05.execution.commandStatus.${command.status}`,
                    command.status
                  )
                }}
              </Badge>
              <span class="rd-row__title">{{ command.command_type }}</span>
              <span class="rd-meta">{{ command.worker_id }}</span>
              <span class="rd-meta">{{
                formatDateTime(command.issued_at)
              }}</span>
              <span v-if="command.last_error" class="rd-meta">
                {{ command.last_error }}
              </span>
            </li>
          </ul>
        </Card>

        <Card class="rd-section" data-testid="rd-continuations">
          <template #header>
            {{
              t("s05.execution.runDetail.continuationsTitle", {
                count: continuations.length
              })
            }}
          </template>
          <p v-if="continuations.length === 0" class="rd-empty">
            {{ t("s05.execution.runDetail.continuationsEmpty") }}
          </p>
          <ul v-else class="rd-list">
            <li
              v-for="row in continuations"
              :key="row.id"
              class="rd-row"
              :data-testid="`rd-continuation-${row.generation}`"
            >
              <span class="rd-row__title">
                {{
                  t("s05.execution.runDetail.generation", { n: row.generation })
                }}
              </span>
              <Badge variant="neutral" size="sm">
                {{
                  t(
                    `s05.execution.continuationStatus.${row.status}`,
                    row.status
                  )
                }}
              </Badge>
              <span class="rd-meta">{{ formatDateTime(row.created_at) }}</span>
            </li>
          </ul>
        </Card>

        <Card class="rd-section" data-testid="rd-commits">
          <template #header>
            {{
              t("s05.execution.runDetail.commitsTitle", {
                count: commits.length
              })
            }}
          </template>
          <p v-if="commits.length === 0" class="rd-empty">
            {{ t("s05.execution.runDetail.commitsEmpty") }}
          </p>
          <ul v-else class="rd-list">
            <li
              v-for="commit in commits"
              :key="commit.id"
              class="rd-row"
              :data-testid="`rd-commit-${commit.commit_sha}`"
            >
              <span class="rd-row__title">{{ commit.repository_name }}</span>
              <span class="rd-mono">{{ commit.commit_sha.slice(0, 10) }}</span>
              <span v-if="commit.branch" class="rd-meta">{{
                commit.branch
              }}</span>
              <span class="rd-meta">
                {{
                  t("s05.execution.runDetail.changedFiles", {
                    n: commit.changed_files
                  })
                }}
              </span>
              <span v-if="commit.commit_author" class="rd-meta">
                {{ commit.commit_author }}
              </span>
            </li>
          </ul>
        </Card>
      </template>
    </ExecutionFrame>
  </PageFrame>
</template>

<style scoped>
.rd-section,
.rd-tri {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rd-tri__list,
.rd-meta-list,
.rd-list,
.rd-events {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rd-tri__row,
.rd-meta-row {
  display: grid;
  grid-template-columns: minmax(100px, 180px) 1fr;
  gap: var(--space-3);
  font-size: var(--font-size-caption);
}

.rd-tri__row dt,
.rd-meta-row dt {
  color: var(--ws-text-secondary);
}

.rd-tri__row dd,
.rd-meta-row dd {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
}

.rd-row,
.rd-event {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.rd-row__title {
  flex: 1;
  min-width: 120px;
  color: var(--ws-text-primary);
}

.rd-event__type {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

.rd-meta {
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.rd-mono {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-primary);
}

.rd-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
