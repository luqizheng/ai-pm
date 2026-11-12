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
  getRequirementImpact,
  type RequirementImpact,
  type RequirementImpactRow
} from "@/api/requirement-read";

/**
 * 需求变更影响分析页（S02-UI-07，接真）。
 *
 * 数据 = GET /v1/pm/requirements/{requirement_id}/impact。
 *
 * 关键语义：**只列事实，不下结论**——谁绑定了本需求（设计 / 任务）、
 * 绑的是哪一版、是否已被新版本取代。是否重做、是否改基线由人裁决。
 *
 * 诚实缺位：AI 影响分析段、影响等级（direct/indirect/potential）、
 * 受影响发现/文档清单——后端无对应读面，本页不造这些结论。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const requirementId = computed(() => String(route.params.requirementId ?? ""));
const impact = ref<RequirementImpact | null>(null);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!requirementId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    impact.value = await getRequirementImpact(requirementId.value);
  } catch {
    loadError.value = true;
    impact.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(requirementId, load);

const designs = computed(() => impact.value?.designs ?? []);
const tasks = computed(() => impact.value?.tasks ?? []);
const staleCount = computed(() => impact.value?.stale_binding_count ?? 0);

function versionLabel(row: RequirementImpactRow): string {
  if (!row.bound_version) return t("pm.requirementImpact.noVersion");
  return t("pm.requirementImpact.versionBadge", { version: row.bound_version });
}

function goBack() {
  void router.push(`/project/requirements/${requirementId.value}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('pm.requirements.pageTitle') },
      {
        label: impact?.code ?? t('pm.requirementImpact.pageTitle'),
        to: `/project/requirements/${requirementId}`
      }
    ]"
    :title="t('pm.requirementImpact.pageTitle')"
    :description="impact?.title ?? t('pm.requirementImpact.pageDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="ri-back"
        @click="goBack"
      >
        <ArrowLeft :size="14" />
        {{ t("pm.requirementImpact.back") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="ri-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("pm.requirementImpact.refresh") }}
      </Button>
    </template>

    <Alert
      v-if="loadError"
      variant="danger"
      size="sm"
      :title="t('pm.requirementImpact.errorTitle')"
    />

    <EmptyState
      v-else-if="!loading && !impact"
      :title="t('pm.requirementImpact.notFoundTitle')"
      :description="t('pm.requirementImpact.notFoundHint')"
    />

    <template v-else-if="impact">
      <Alert
        v-if="staleCount > 0"
        variant="warning"
        size="sm"
        :title="t('pm.requirementImpact.staleTitle')"
      >
        {{ t("pm.requirementImpact.staleHint", { count: staleCount }) }}
      </Alert>

      <div class="ri-stats" data-testid="ri-stats">
        <span class="ri-stat">
          <span class="ri-stat__label">
            {{ t("pm.requirementImpact.statCurrent") }}
          </span>
          <span class="ri-stat__value">
            {{
              impact.current_version
                ? t("pm.requirementImpact.versionBadge", {
                    version: impact.current_version
                  })
                : t("pm.requirementImpact.noVersion")
            }}
          </span>
        </span>
        <span class="ri-stat">
          <span class="ri-stat__label">
            {{ t("pm.requirementImpact.statDesigns") }}
          </span>
          <span class="ri-stat__value">{{ designs.length }}</span>
        </span>
        <span class="ri-stat">
          <span class="ri-stat__label">
            {{ t("pm.requirementImpact.statTasks") }}
          </span>
          <span class="ri-stat__value">{{ tasks.length }}</span>
        </span>
        <span class="ri-stat">
          <span class="ri-stat__label">
            {{ t("pm.requirementImpact.statStale") }}
          </span>
          <span
            class="ri-stat__value"
            :class="{ 'is-warning': staleCount > 0 }"
          >
            {{ staleCount }}
          </span>
        </span>
      </div>

      <Card class="ri-section" data-testid="ri-designs">
        <template #header>
          {{ t("pm.requirementImpact.designsTitle") }}
        </template>
        <p v-if="designs.length === 0" class="ri-empty">
          {{ t("pm.requirementImpact.noDesigns") }}
        </p>
        <ul v-else class="ri-list">
          <li
            v-for="row in designs"
            :key="row.target_id"
            class="ri-row"
            :data-testid="`ri-design-${row.code}`"
          >
            <span class="ri-row__code">{{ row.code }}</span>
            <span class="ri-row__title">{{ row.title }}</span>
            <span class="ri-row__meta">{{ versionLabel(row) }}</span>
            <Badge
              :variant="row.is_current === false ? 'warning' : 'success'"
              size="sm"
            >
              {{
                row.is_current === false
                  ? t("pm.requirementImpact.stale")
                  : t("pm.requirementImpact.current")
              }}
            </Badge>
            <Badge v-if="row.target_status" variant="neutral" size="sm">
              {{
                t(
                  `pm.requirementImpact.taskState.${row.target_status}`,
                  row.target_status
                )
              }}
            </Badge>
          </li>
        </ul>
      </Card>

      <Card class="ri-section" data-testid="ri-tasks">
        <template #header>
          {{ t("pm.requirementImpact.tasksTitle") }}
        </template>
        <p v-if="tasks.length === 0" class="ri-empty">
          {{ t("pm.requirementImpact.noTasks") }}
        </p>
        <ul v-else class="ri-list">
          <li
            v-for="row in tasks"
            :key="row.target_id"
            class="ri-row"
            :data-testid="`ri-task-${row.target_id.slice(0, 8)}`"
          >
            <span class="ri-row__title">{{ row.title }}</span>
            <span class="ri-row__meta">{{ versionLabel(row) }}</span>
            <Badge
              :variant="row.is_current === false ? 'warning' : 'success'"
              size="sm"
            >
              {{
                row.is_current === false
                  ? t("pm.requirementImpact.stale")
                  : t("pm.requirementImpact.current")
              }}
            </Badge>
            <Badge v-if="row.target_status" variant="neutral" size="sm">
              {{
                t(
                  `pm.requirementImpact.taskState.${row.target_status}`,
                  row.target_status
                )
              }}
            </Badge>
          </li>
        </ul>
      </Card>
    </template>
  </PageFrame>
</template>

<style scoped>
.ri-stats {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-lg);
}

.ri-stat {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.ri-stat__label {
  color: var(--ws-text-secondary);
}

.ri-stat__value {
  font-variant-numeric: tabular-nums;
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.ri-stat__value.is-warning {
  color: var(--ws-color-warning-text);
}

.ri-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ri-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.ri-row__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-primary);
}

.ri-row__title {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

.ri-row__meta {
  color: var(--ws-text-secondary);
}

.ri-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
