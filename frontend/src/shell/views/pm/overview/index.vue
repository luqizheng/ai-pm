<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/shell/components";
import { statusVariant } from "@/api/status";
import { formatDate } from "@/api/format";
import {
  listApprovals,
  listPrincipals,
  listProjects,
  listRequirements,
  listTasks
} from "@/api/pm";
import type { ApprovalRequest, Task } from "@/api/types";

const { t } = useI18n();
const toast = useToast();

const loading = ref(false);
const projectCount = ref(0);
const requirementCount = ref(0);
const taskCount = ref(0);
const recentTasks = ref<Task[]>([]);
const pendingApprovals = ref<ApprovalRequest[]>([]);
const principalNames = ref<Record<string, string>>({});

const stats = computed(() => [
  { label: t("pm.overview.statProjects"), value: projectCount.value },
  { label: t("pm.overview.statRequirements"), value: requirementCount.value },
  { label: t("pm.overview.statTasks"), value: taskCount.value },
  {
    label: t("pm.overview.statApprovals"),
    value: pendingApprovals.value.length
  }
]);

function displayName(id: string | null): string {
  if (!id) return t("pm.unassigned");
  return principalNames.value[id] ?? id;
}

function statusText(value: unknown): string {
  const key = typeof value === "string" ? value : String(value ?? "");
  return t(`pm.status.${key}`, key);
}

async function load() {
  loading.value = true;
  try {
    const [projects, requirements, tasks, approvals, principals] =
      await Promise.all([
        listProjects({ page: 1, size: 200 }),
        listRequirements({ page: 1, size: 200 }),
        listTasks({ page: 1, size: 200 }),
        listApprovals({ page: 1, size: 200 }),
        listPrincipals({ page: 1, size: 200 })
      ]);
    projectCount.value = projects.total;
    requirementCount.value = requirements.total;
    taskCount.value = tasks.total;
    recentTasks.value = [...tasks.items]
      .sort((a, b) => b.updated_at.localeCompare(a.updated_at))
      .slice(0, 6);
    pendingApprovals.value = approvals.items
      .filter(item => item.status === "pending")
      .slice(0, 5);
    const names: Record<string, string> = {};
    for (const principal of principals.items) {
      names[principal.id] = principal.display_name;
    }
    principalNames.value = names;
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("pm.loadFailed"),
      description: String(error)
    });
  } finally {
    loading.value = false;
  }
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('pm.overview.pageTitle')"
      :subtitle="t('pm.overview.scopeAll')"
    />

    <div class="stat-grid">
      <Card v-for="stat in stats" :key="stat.label" class="stat-card">
        <p class="stat-card__label">{{ stat.label }}</p>
        <p class="stat-card__value">{{ stat.value }}</p>
      </Card>
    </div>

    <div class="panel-grid">
      <Card class="panel">
        <p class="panel__title">{{ t("pm.overview.recentTasks") }}</p>
        <p v-if="recentTasks.length === 0" class="panel__empty">
          {{ t("pm.overview.empty") }}
        </p>
        <ul v-else class="row-list">
          <li v-for="task in recentTasks" :key="task.id" class="row-list__item">
            <div class="row-list__main">
              <span class="row-list__text">{{ task.title }}</span>
              <span class="row-list__meta">
                {{ displayName(task.assignee_id) }} ·
                {{ formatDate(task.updated_at) }}
              </span>
            </div>
            <Badge :variant="statusVariant(task.status)">
              {{ statusText(task.status) }}
            </Badge>
          </li>
        </ul>
      </Card>

      <Card class="panel">
        <p class="panel__title">{{ t("pm.overview.pendingApprovals") }}</p>
        <p v-if="pendingApprovals.length === 0" class="panel__empty">
          {{ t("pm.overview.queueEmpty") }} ——
          {{ t("pm.overview.queueEmptyHint") }}
        </p>
        <ul v-else class="row-list">
          <li
            v-for="approval in pendingApprovals"
            :key="approval.id"
            class="row-list__item"
          >
            <div class="row-list__main">
              <span class="row-list__mono">{{ approval.proposal_id }}</span>
              <span class="row-list__meta">
                {{ approval.required_role }} ·
                {{ formatDate(approval.created_at) }}
              </span>
            </div>
            <Badge :variant="statusVariant(approval.status)">
              {{ statusText(approval.status) }}
            </Badge>
          </li>
        </ul>
      </Card>
    </div>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.stat-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-4);
}

.stat-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4);
}

.stat-card__label {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.stat-card__value {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.panel-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
}

.panel__title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.panel__empty {
  margin: var(--space-6) 0;
  text-align: center;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.row-list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.row-list__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.row-list__item:last-child {
  border-bottom: none;
}

.row-list__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.row-list__text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-primary);
}

.row-list__mono {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.row-list__meta {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

@media (max-width: 960px) {
  .stat-grid,
  .panel-grid {
    grid-template-columns: 1fr;
  }
}
</style>
