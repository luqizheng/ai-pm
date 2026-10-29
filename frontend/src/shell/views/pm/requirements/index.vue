<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import { DataTable, PageHeader, type TableColumn } from "@/shell/components";
import { statusVariant } from "@/api/status";
import { formatDate } from "@/api/format";
import { listRequirements } from "@/api/pm";
import type { Requirement } from "@/api/types";

const { t } = useI18n();
const toast = useToast();

const columns: TableColumn[] = [
  { label: t("pm.columns.code"), prop: "code" },
  { label: t("pm.columns.title"), prop: "title" },
  { label: t("pm.columns.status"), prop: "status", slot: "status" },
  { label: t("pm.columns.rowRevision"), prop: "row_revision", numeric: true },
  {
    label: t("pm.columns.createdAt"),
    prop: "created_at",
    slot: "createdAt",
    numeric: true
  }
];

const rows = ref<Requirement[]>([]);
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
    const result = await listRequirements({ page: page.value, size: pageSize });
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
      :title="t('pm.requirements.pageTitle')"
      :subtitle="t('pm.requirements.subtitle')"
    />

    <DataTable
      :columns="columns"
      :rows="rows"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('pm.requirements.pageTitle')"
      :empty-text="t('pm.requirements.emptyHint')"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #status="{ row }">
        <Badge :variant="statusVariant(row.status)">{{
          statusText(row.status)
        }}</Badge>
      </template>
      <template #createdAt="{ row }">{{ formatDate(row.created_at) }}</template>
    </DataTable>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}
</style>
