<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { FileText, Link2 } from "lucide-vue-next";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import {
  fetchExecutionLogs,
  fetchWorkItemDetail,
  type ExecutionLog,
  type WorkItemDetail
} from "@/api/feeds";
import { translateError } from "@/api/base";
import { useRelativeTime } from "@/views/personal/useRelativeTime";

/**
 * 统一工作项详情（PA-05 UI 屏 04 + 屏 06 并入）。
 *
 * 「Activity records describe the past. Work Items describe the present.」
 * —— 详情呈现当前注册行（状态/关联/活动史）；Timeline 页签=业务活动，
 * Logs 页签=Execution logs（task_run_event，仅 task 事项，§8.2 粒度规则：
 * 详细执行过程不混入 Timeline）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const { relative } = useRelativeTime();

const loading = ref(true);
const error = ref<string | null>(null);
const detail = ref<WorkItemDetail | null>(null);
const logs = ref<ExecutionLog[]>([]);
/** 06 页签：timeline | logs */
const tab = ref<"timeline" | "logs">("timeline");

const itemId = computed(() => String(route.params.itemId));
const scopeLabel = computed(() => {
  const projectId = detail.value?.item.project_id;
  return projectId
    ? t("pm.feeds.detailScopeProject")
    : t("pm.feeds.detailScopeProduct");
});

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    detail.value = await fetchWorkItemDetail(itemId.value);
    if (detail.value.item.target_type === "task") {
      logs.value = await fetchExecutionLogs(itemId.value, 50, 0);
    }
  } catch (err) {
    error.value = translateError(err);
  } finally {
    loading.value = false;
  }
}

function openActivity(activityId: string): void {
  void router.push(`/feeds/activities/${activityId}`);
}

function statusVariant(status: string): "neutral" | "accent" | "warning" {
  if (status === "done") return "accent";
  if (status === "blocked") return "warning";
  return "neutral";
}

watch(itemId, () => void load());
onMounted(load);
</script>

<template>
  <PageFrame
    :title="detail?.item.title ?? $t('pm.feeds.detailTitle')"
    :breadcrumbs="[
      { label: $t('shell.app.side.item.feeds'), to: '/feeds' },
      { label: $t('pm.feeds.detailTitle') }
    ]"
  >
    <p v-if="error" class="wd-error">{{ error }}</p>

    <EmptyState
      v-if="!loading && !detail"
      :title="$t('pm.feeds.detailEmpty')"
    />

    <template v-else-if="detail">
      <!-- 徽标行（屏 04）：type + 统一状态 + scope -->
      <div class="wd-badges" data-testid="work-item-badges">
        <Badge variant="neutral">{{ detail.item.target_type }}</Badge>
        <Badge :variant="statusVariant(detail.item.status)">
          {{ detail.item.status }}
        </Badge>
        <span class="wd-scope">{{ scopeLabel }}</span>
      </div>

      <!-- Relationships（屏 04 关联卡） -->
      <section class="wd-card" data-testid="work-item-relations">
        <h2 class="wd-card__title">{{ $t("pm.feeds.relationsTitle") }}</h2>
        <p v-if="detail.relations.length === 0" class="wd-card__empty">
          {{ $t("pm.feeds.relationsEmpty") }}
        </p>
        <div
          v-for="relation in detail.relations"
          :key="relation.id"
          class="wd-relation"
          :data-testid="`relation-${relation.id}`"
        >
          <Link2 :size="14" aria-hidden="true" />
          <span class="wd-relation__type">
            {{
              relation.direction === "outgoing"
                ? relation.relation_type
                : $t("pm.feeds.relationIncoming", {
                    type: relation.relation_type
                  })
            }}
          </span>
          <button
            type="button"
            class="wd-relation__link"
            @click="$router.push(`/feeds/work-items/${relation.other_item_id}`)"
          >
            <FileText :size="13" aria-hidden="true" />
            {{ relation.other_type }} · {{ relation.other_title }}
          </button>
          <Badge variant="neutral">{{ relation.other_status }}</Badge>
        </div>
      </section>

      <!-- 06：Timeline | Execution logs 页签 -->
      <div class="wd-tabs" data-testid="work-item-tabs">
        <button
          type="button"
          class="wd-tabs__item"
          :class="{ 'is-active': tab === 'timeline' }"
          @click="tab = 'timeline'"
        >
          {{ $t("pm.feeds.tabTimeline") }}
        </button>
        <button
          v-if="detail.item.target_type === 'task'"
          type="button"
          class="wd-tabs__item"
          :class="{ 'is-active': tab === 'logs' }"
          @click="tab = 'logs'"
        >
          {{ $t("pm.feeds.tabLogs") }}
        </button>
      </div>

      <!-- Timeline（业务活动；打开 → 屏 05） -->
      <template v-if="tab === 'timeline'">
        <p v-if="detail.activities.length === 0" class="wd-empty">
          {{ $t("pm.feeds.activityEmpty") }}
        </p>
        <div
          v-for="row in detail.activities"
          :key="row.id"
          class="wd-activity"
          :class="{ 'is-unread': row.read_at === null }"
        >
          <span class="wd-activity__main">
            {{ row.summary }}
          </span>
          <Badge variant="neutral">{{ row.activity_type }}</Badge>
          <span class="wd-activity__time">{{ relative(row.created_at) }}</span>
          <Button
            variant="secondary"
            size="sm"
            :data-test="`open-activity-${row.id}`"
            @click="openActivity(row.id)"
          >
            {{ $t("pm.feeds.openActivity") }}
          </Button>
        </div>
      </template>

      <!-- Execution logs（task_run_event；§8.2 不混入 Timeline） -->
      <template v-else>
        <p v-if="logs.length === 0" class="wd-empty">
          {{ $t("pm.feeds.logsEmpty") }}
        </p>
        <div
          v-for="logRow in logs"
          :key="logRow.id"
          class="wd-log"
          :data-testid="`exec-log-${logRow.id}`"
        >
          <span class="wd-log__time">{{
            new Date(logRow.occurred_at).toLocaleTimeString()
          }}</span>
          <Badge variant="neutral">{{ logRow.event_type }}</Badge>
          <span class="wd-log__run">run {{ logRow.run_id.slice(0, 8) }}</span>
        </div>
      </template>
    </template>
  </PageFrame>
</template>

<style scoped>
.wd-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.wd-badges {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.wd-scope {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.wd-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.wd-card__title {
  margin: 0 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.wd-card__empty,
.wd-empty {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.wd-relation {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.wd-relation:last-child {
  border-bottom: none;
}

.wd-relation__type {
  min-width: 9rem;
  color: var(--text-secondary);
}

.wd-relation__link {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  flex: 1;
  border: none;
  background: transparent;
  padding: 0;
  color: var(--ws-accent);
  cursor: pointer;
  text-align: left;
}

.wd-tabs {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: 1px solid var(--ws-border);
}

.wd-tabs__item {
  padding: 0 var(--space-3);
  height: 2.25rem;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
  cursor: pointer;
}

.wd-tabs__item.is-active {
  border-bottom-color: var(--ws-accent);
  color: var(--text-primary);
}

.wd-activity {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.wd-activity.is-unread .wd-activity__main {
  font-weight: var(--weight-semibold);
}

.wd-activity__main {
  flex: 1;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.wd-activity__time {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.wd-log {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-mono, monospace);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}
</style>
