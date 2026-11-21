<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { translateError } from "@/api/base";
import { listApprovals } from "@/api/pm";
import { getProjectGoal } from "@/api/goal";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 H — Authorization & autonomy（Figma 259:807，Authorization 页签）。
 *
 * 「可见边界内的自主」：已确认目标卡（goal vN）+ 两列边界清单（AI 可自主 /
 * 须人类确认——PA-06-01~06 基线文案的 UI 呈现，非运行时状态）+ 运营约束 +
 * 待决授权请求卡（pending 红线审批 → 决策中心）。右栏 = Human-owned
 * authority + 审批历史（已决 approval_request，真实数据）。
 *
 * 诚实缺位：后端无版本化授权实体——「vN is active」呈现为「红线策略生效」；
 * 授权历史用已决审批记录，不编造版本时间线。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);
const goalLine = ref<string | null>(null);
const goalRevision = ref<number | null>(null);
const pendingApprovals = ref<Array<{ id: string; required_role: string }>>([]);
const decidedApprovals = ref<
  Array<{
    id: string;
    status: string;
    decision_reason: string | null;
    decided_at: string | null;
    required_role: string;
  }>
>([]);

const AUTONOMOUS_ITEMS = [
  "autonomous1",
  "autonomous2",
  "autonomous3",
  "autonomous4",
  "autonomous5",
  "autonomous6"
] as const;

const CONFIRM_ITEMS = [
  "confirm1",
  "confirm2",
  "confirm3",
  "confirm4",
  "confirm5",
  "confirm6"
] as const;

function historyLine(row: (typeof decidedApprovals.value)[number]): string {
  const time = row.decided_at ? new Date(row.decided_at).toLocaleString() : "—";
  const reason = row.decision_reason ?? "";
  return `${time} · ${row.status}\n${reason}`;
}

async function load(): Promise<void> {
  if (!hasProject.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const id = projectId.value;
    const [approvals, goal] = await Promise.all([
      listApprovals({ project_id: id, page: 1, size: 100 }),
      getProjectGoal(id).catch(() => null)
    ]);
    const rows = approvals.items ?? [];
    pendingApprovals.value = rows
      .filter(row => row.status === "pending")
      .map(row => ({ id: row.id, required_role: row.required_role }));
    decidedApprovals.value = rows
      .filter(row => row.status !== "pending" && row.decided_at)
      .slice(0, 4)
      .map(row => ({
        id: row.id,
        status: row.status,
        decision_reason: row.decision_reason,
        decided_at: row.decided_at,
        required_role: row.required_role
      }));
    if (goal && goal.state === "ready" && goal.markdown) {
      goalLine.value = goal.markdown.split("\n").find(l => l.trim()) ?? null;
      goalRevision.value = goal.metadata?.revision_no ?? null;
    } else {
      goalLine.value = null;
      goalRevision.value = null;
    }
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

function reviewRequest(): void {
  void router.push("/project/collaboration/decisions");
}

function reviewLimits(): void {
  void router.push("/project/settings/ai/execution-policies");
}

onMounted(load);
watch(projectId, load);
</script>

<template>
  <CollaborationFrame>
    <ScreenFeedback
      v-if="loading"
      state="loading"
      :title="$t('pa06.authorization.loadingEyebrow')"
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
      <CollabLayout v-else>
        <div class="au-head">
          <span class="au-eyebrow">
            {{ $t("pa06.authorization.eyebrow") }}
          </span>
          <h2 class="au-head__title">
            {{ $t("pa06.authorization.headline") }}
          </h2>
        </div>

        <!-- 已确认目标 -->
        <div class="au-card" data-testid="au-objective">
          <span class="au-eyebrow">
            {{
              goalRevision
                ? $t("pa06.authorization.objectiveEyebrow", {
                    rev: goalRevision
                  })
                : $t("pa06.authorization.noObjective")
            }}
          </span>
          <p v-if="goalLine" class="au-objective">{{ goalLine }}</p>
          <p class="au-objective__meta">
            {{ $t("pa06.authorization.objectiveMeta") }}
          </p>
        </div>

        <!-- 边界两列：AI 可自主 / 须人类确认（PA-06 基线清单） -->
        <div class="au-two-col">
          <div class="au-card au-card--accent" data-testid="au-autonomous">
            <span class="au-eyebrow au-eyebrow--accent">
              {{ $t("pa06.authorization.autonomousTitle") }}
            </span>
            <ul class="au-list">
              <li
                v-for="key in AUTONOMOUS_ITEMS"
                :key="key"
                class="au-list__item"
              >
                ✓ {{ $t(`pa06.authorization.${key}`) }}
              </li>
            </ul>
          </div>
          <div class="au-card au-card--danger" data-testid="au-confirm">
            <span class="au-eyebrow au-eyebrow--danger">
              {{ $t("pa06.authorization.confirmTitle") }}
            </span>
            <ul class="au-list">
              <li v-for="key in CONFIRM_ITEMS" :key="key" class="au-list__item">
                • {{ $t(`pa06.authorization.${key}`) }}
              </li>
            </ul>
          </div>
        </div>

        <!-- 运营约束（基线文案） -->
        <span class="au-eyebrow">{{
          $t("pa06.authorization.constraintsEyebrow")
        }}</span>
        <p class="au-constraints" data-testid="au-constraints">
          {{ $t("pa06.authorization.constraintsText") }}
        </p>

        <!-- 待决授权请求（pending 红线审批） -->
        <div
          v-if="pendingApprovals.length > 0"
          class="au-pending"
          data-testid="au-pending"
        >
          <div>
            <p class="au-pending__title">
              {{
                $t("pa06.authorization.pendingTitle", {
                  count: pendingApprovals.length
                })
              }}
            </p>
            <p class="au-pending__meta">
              {{
                $t("pa06.authorization.pendingMeta", {
                  role: pendingApprovals[0]?.required_role ?? ""
                })
              }}
            </p>
          </div>
          <Button variant="primary" size="sm" @click="reviewRequest">
            {{ $t("pa06.authorization.reviewRequest") }}
          </Button>
        </div>

        <template #context>
          <CollabContextPanel
            :title="$t('pa06.authorization.ctxTitle')"
            :badge="$t('pa06.authorization.ctxBadge')"
          >
            <p class="au-ctx-text">
              {{ $t("pa06.authorization.ctxInheritText") }}
            </p>
            <CollabContextSection :label="$t('pa06.authorization.ctxHistory')">
              <template v-if="decidedApprovals.length > 0">
                <p
                  v-for="row in decidedApprovals"
                  :key="row.id"
                  class="au-ctx-history"
                >
                  {{ historyLine(row) }}
                </p>
              </template>
              <template v-else>
                {{ $t("pa06.authorization.ctxNoHistory") }}
              </template>
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.authorization.ctxControl')">
              {{ $t("pa06.authorization.ctxControlText") }}
            </CollabContextSection>
            <Button variant="secondary" size="sm" @click="reviewLimits">
              {{ $t("pa06.authorization.reviewLimits") }}
            </Button>
            <p class="au-ctx-note">
              {{ $t("pa06.authorization.ctxFooter") }}
            </p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.au-eyebrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.au-eyebrow--accent {
  color: var(--ws-accent);
}

.au-eyebrow--danger {
  color: var(--ws-color-danger-text);
}

.au-head {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.au-head__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: 1.375rem;
  font-weight: var(--weight-semibold);
  letter-spacing: -0.018em;
  color: var(--text-primary);
}

.au-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.au-objective {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.au-objective__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.au-two-col {
  display: flex;
  align-items: stretch;
  gap: var(--space-4);
}

.au-two-col > .au-card {
  flex: 1;
  min-width: 0;
}

.au-list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.au-list__item {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.au-constraints {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
  white-space: pre-line;
}

.au-pending {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--ws-accent) 10%, transparent);
  padding: var(--space-3) var(--space-4);
}

.au-pending__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.au-pending__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.au-ctx-text,
.au-ctx-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.au-ctx-note {
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.au-ctx-history {
  margin: 0 0 var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: 1.45;
  color: var(--text-secondary);
  white-space: pre-line;
}

@media (max-width: 900px) {
  .au-two-col {
    flex-direction: column;
  }
}
</style>
