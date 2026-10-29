<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import { DataTable, PageHeader, type TableColumn } from "@/shell/components";
import { statusVariant } from "@/api/status";
import { formatDateTime } from "@/api/format";
import { listProjects } from "@/api/pm";
import type { Project } from "@/api/types";

const { t } = useI18n();
const toast = useToast();

const columns: TableColumn[] = [
  { label: t("pm.columns.code"), prop: "code" },
  { label: t("pm.columns.name"), prop: "name", slot: "name" },
  { label: t("pm.columns.status"), prop: "status", slot: "status" },
  { label: t("pm.columns.revision"), prop: "revision", numeric: true },
  {
    label: t("pm.columns.updatedAt"),
    prop: "updated_at",
    slot: "updatedAt",
    numeric: true
  }
];

const rows = ref<Project[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);

function statusText(value: unknown): string {
  const key = typeof value === "string" ? value : String(value ?? "");
  return t(`pm.status.${key}`, key);
}

async function load() {
  loading.value = true;
  try {
    const result = await listProjects({ page: page.value, size: pageSize });
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
      :title="t('pm.projects.pageTitle')"
      :subtitle="t('pm.projects.subtitle')"
    />

    <DataTable
      :columns="columns"
      :rows="rows"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('pm.projects.pageTitle')"
      :empty-text="t('pm.projects.emptyHint')"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #name="{ row }">
        <div class="cell-stack">
          <span>{{ row.name }}</span>
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
