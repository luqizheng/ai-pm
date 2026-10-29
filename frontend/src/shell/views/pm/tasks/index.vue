<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import { DataTable, PageHeader, type TableColumn } from "@/shell/components";
import { useUserDirectory } from "@/shell/store/user-directory";
import { statusVariant } from "@/api/status";
import { formatDateTime } from "@/api/format";
import { listTasks } from "@/api/pm";
import type { Task } from "@/api/types";

const { t } = useI18n();
const toast = useToast();
const directory = useUserDirectory();

const columns: TableColumn[] = [
  { label: t("pm.columns.task"), prop: "title", slot: "task" },
  { label: t("pm.columns.status"), prop: "status", slot: "status" },
  { label: t("pm.columns.priority"), prop: "priority", slot: "priority" },
  { label: t("pm.columns.assignee"), prop: "assignee_id", slot: "assignee" },
  {
    label: t("pm.columns.updatedAt"),
    prop: "updated_at",
    slot: "updatedAt",
    numeric: true
  }
];

const rows = ref<Task[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);

/** 当前页按更新时间倒序（mock 与后端约定按 created_at 排序） */
const sortedRows = computed(() =>
  [...rows.value].sort((a, b) => b.updated_at.localeCompare(a.updated_at))
);

function statusText(value: unknown): string {
  const key = typeof value === "string" ? value : String(value ?? "");
  return t(`pm.status.${key}`, key);
}

function displayName(id: unknown): string {
  if (typeof id !== "string" || id === "") return t("pm.unassigned");
  return directory.names[id] ?? id;
}

async function load() {
  loading.value = true;
  try {
    await directory.ensureLoaded();
    const result = await listTasks({ page: page.value, size: pageSize });
    rows.value = result.items ?? [];
    total.value = result.total ?? 0;
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
      :title="t('pm.tasks.pageTitle')"
      :subtitle="t('pm.tasks.subtitle')"
    />

    <DataTable
      :columns="columns"
      :rows="sortedRows"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('pm.tasks.pageTitle')"
      :empty-text="t('pm.tasks.emptyHint')"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #task="{ row }">
        <div class="cell-stack">
          <span>{{ row.title }}</span>
          <span v-if="row.description" class="cell-hint">{{
            row.description
          }}</span>
        </div>
      </template>
      <template #status="{ row }">
        <Badge :variant="statusVariant(row.status)">{{
          statusText(row.status)
        }}</Badge>
      </template>
      <template #priority="{ row }">
        <Badge :variant="statusVariant(row.priority)">{{
          statusText(row.priority)
        }}</Badge>
      </template>
      <template #assignee="{ row }">{{
        displayName(row.assignee_id)
      }}</template>
      <template #updatedAt="{ row }">{{
        formatDateTime(row.updated_at)
      }}</template>
    </DataTable>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.cell-stack {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.cell-hint {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}
</style>
