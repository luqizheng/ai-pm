<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { translateError } from "@/api/base";
import {
  fetchProjectRisks,
  fetchWaitingQueue,
  type RiskRow,
  type WaitingRow
} from "@/api/collaboration";
import { listTasks } from "@/api/pm";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 G — Exceptions & recovery（Figma 259:700，Recovery 页签）。
 *
 * 「分层恢复」：重大风险升级卡（open 风险=人类决策通道 → 决策中心）+
 * 常规恢复段（非决策类 waiting 行 + blocked 任务=AI 限次自主恢复范围，
 * 无需人工动作）+ 重试上限语义（PA-06-06 §8.2：达上限记录原因并暂停）。
 * 右栏 Impact & recommendation（当前影响/建议/你的权威/恢复上限）。
 *
 * 诚实缺位：Run 级 attempt/retry 计数无轻量读面——常规恢复卡呈现 waiting
 * 原因与受阻任务（真实数据），不编造「Attempt 2 of 3」计数；Inspect→S05
 * 人工介入页（真实通道），Escalate→决策中心。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);
const risks = ref<RiskRow[]>([]);
const routineRows = ref<
  Array<{ taskId: string; title: string; reason: string }>
>([]);
const pendingDecisionCount = ref(0);

const topRisk = computed(() => risks.value[0] ?? null);

const WAITING_REASONS = new Set([
  "needs_info",
  "external_dependency",
  "no_executor_available"
]);

async function load(): Promise<void> {
  if (!hasProject.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const id = projectId.value;
    const [riskRows, tasks, waiting] = await Promise.all([
      fetchProjectRisks(id),
      listTasks({ project_id: id, page: 1, size: 200 }),
      fetchWaitingQueue(id).catch(() => [] as WaitingRow[])
    ]);
    risks.value = riskRows.filter(risk => risk.status === "open");
    routineRows.value = waiting
      .filter(
        row =>
          WAITING_REASONS.has(row.reason) || row.reason === "needs_approval"
      )
      .map(row => ({
        taskId: row.task_id,
        title: row.task_title,
        reason: row.reason
      }));
    const blocked = (tasks.items ?? []).filter(
      task => task.status === "blocked"
    );
    for (const task of blocked) {
      if (!routineRows.value.some(row => row.taskId === task.id)) {
        routineRows.value.push({
          taskId: task.id,
          title: `${task.code} · ${task.title}`,
          reason: "blocked"
        });
      }
    }
    pendingDecisionCount.value = risks.value.length;
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

function reviewRisk(): void {
  void router.push("/project/collaboration/decisions");
}

function keepPaused(): void {
  // 保持暂停=无动作语义（恢复须同等授权）；按钮即声明，不执行任何变更
  void reviewRisk;
}

function inspectRecovery(): void {
  void router.push("/project/execution/intervention");
}

function escalate(): void {
  void router.push("/project/collaboration/decisions");
}

onMounted(load);
watch(projectId, load);
</script>

<template>
  <CollaborationFrame>
    <ScreenFeedback
      v-if="loading"
      state="loading"
      :title="$t('pa06.recovery.loadingEyebrow')"
    />
    <ScreenFeedback
      v-else-if="error"
      state="error"
      :description="error"
      @retry="load"
    />
    <template v-else>
      <template v-if="!hasProject">
        <ScreenFeedback state="empty" :title="$t('pa06.frame.noProject')" />
      </template>
      <ScreenFeedback
        v-else-if="risks.length === 0 && routineRows.length === 0"
        state="empty"
        :title="$t('pa06.recovery.emptyEyebrow')"
        :description="$t('pa06.recovery.emptyHint')"
      />
      <CollabLayout v-else>
        <div class="rc-head">
          <span class="rc-eyebrow">
            {{
              $t("pa06.recovery.eyebrow", {
                decisions: risks.length,
                auto: routineRows.length
              })
            }}
          </span>
        </div>

        <!-- 重大风险升级（人类决策通道） -->
        <div
          v-if="topRisk"
          class="rc-card rc-card--major"
          data-testid="rc-major"
        >
          <Badge variant="accent">
            {{ $t("pa06.recovery.decisionBadge") }}
          </Badge>
          <h2 class="rc-card__title">{{ topRisk.title }}</h2>
          <p class="rc-card__severity">
            {{
              $t("pa06.recovery.severityLine", {
                impact: topRisk.impact,
                probability: topRisk.probability
              })
            }}
          </p>
          <p class="rc-card__desc">
            {{ topRisk.description || topRisk.title }}
          </p>
          <div class="rc-actions">
            <Button variant="primary" size="sm" @click="reviewRisk">
              {{ $t("pa06.recovery.reviewRequest") }}
            </Button>
          </div>
        </div>

        <!-- 常规恢复（无需人工动作） -->
        <span class="rc-eyebrow">{{ $t("pa06.recovery.routineEyebrow") }}</span>
        <div
          v-if="routineRows.length > 0"
          class="rc-card"
          data-testid="rc-routine"
        >
          <Badge variant="neutral">
            {{ $t("pa06.recovery.autoBadge", { count: routineRows.length }) }}
          </Badge>
          <div
            v-for="row in routineRows"
            :key="row.taskId"
            class="rc-routine"
            :data-testid="`rc-routine-${row.taskId}`"
          >
            <p class="rc-routine__title">{{ row.title }}</p>
            <p class="rc-routine__meta">{{ row.reason }}</p>
          </div>
        </div>
        <p v-else class="rc-routine__meta" data-testid="rc-routine-empty">
          {{ $t("pa06.recovery.noRoutine") }}
        </p>

        <p class="rc-note">{{ $t("pa06.recovery.limitNote") }}</p>

        <div class="rc-actions">
          <Button variant="secondary" size="sm" @click="inspectRecovery">
            {{ $t("pa06.recovery.inspectRecovery") }}
          </Button>
          <Button variant="secondary" size="sm" @click="escalate">
            {{ $t("pa06.recovery.escalate") }}
          </Button>
        </div>

        <template #context>
          <CollabContextPanel :title="$t('pa06.recovery.ctxTitle')">
            <CollabContextSection :label="$t('pa06.recovery.ctxImpact')">
              {{ $t("pa06.recovery.ctxImpactText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.recovery.ctxRecommends')">
              {{ $t("pa06.recovery.ctxRecommendsText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.recovery.ctxAuthority')">
              {{ $t("pa06.recovery.ctxAuthorityText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.recovery.ctxLimit')">
              {{ $t("pa06.recovery.ctxLimitText") }}
            </CollabContextSection>
            <p class="rc-ctx-note">{{ $t("pa06.recovery.ctxNote") }}</p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.rc-eyebrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.rc-head {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rc-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.rc-card__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: 1.375rem;
  font-weight: var(--weight-semibold);
  letter-spacing: -0.018em;
  color: var(--text-primary);
}

.rc-card__severity {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--ws-color-danger-text);
}

.rc-card__desc {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.rc-routine {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.rc-routine__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rc-routine__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.rc-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.rc-actions {
  display: flex;
  gap: var(--space-2);
}

.rc-ctx-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: 1.45;
  color: var(--text-secondary);
}
</style>
