<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@/components/ui/toast";
import { DataTable, PageHeader, type TableColumn } from "@/shell/components";
import ScopeSelect from "@/shell/views/pm/components/ScopeSelect.vue";
import { useUserDirectory } from "@/shell/store/user-directory";
import { useScopeStore } from "@/api/scope";
import { formatDateTime } from "@/api/format";
import { listAuditLogs } from "@/api/pm";
import type { AuditLog } from "@/api/types";

const { t } = useI18n();
const toast = useToast();
const directory = useUserDirectory();
const scope = useScopeStore();

const columns: TableColumn[] = [
  {
    label: t("pm.columns.createdAt"),
    prop: "created_at",
    slot: "createdAt",
    numeric: true
  },
  { label: t("pm.columns.actor"), prop: "actor_id", slot: "actor" },
  { label: t("pm.columns.action"), prop: "action", slot: "action" },
  { label: t("pm.columns.target"), prop: "target_type", slot: "target" }
];

const rows = ref<AuditLog[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);

function displayName(id: unknown): string {
  if (typeof id !== "string") return "—";
  return directory.names[id] ?? id;
}

async function load() {
  loading.value = true;
  try {
    await directory.ensureLoaded();
    const result = await listAuditLogs({
      project_id: scope.projectIdParam,
      page: page.value,
      size: pageSize
    });
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

watch(
  () => scope.currentProjectId,
  () => {
    page.value = 1;
    load();
  }
);

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('pm.audit.pageTitle')"
      :subtitle="t('pm.audit.subtitle')"
    >
      <template #actions>
        <ScopeSelect />
      </template>
    </PageHeader>

    <DataTable
      :columns="columns"
      :rows="rows"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('pm.audit.pageTitle')"
      :empty-text="t('pm.audit.emptyHint')"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #createdAt="{ row }">{{
        formatDateTime(row.created_at)
      }}</template>
      <template #actor="{ row }">{{ displayName(row.actor_id) }}</template>
      <template #action="{ row }">
        <span class="mono">{{ row.action }}</span>
      </template>
      <template #target="{ row }">
        <span>{{ row.target_type }}</span>
        <span v-if="row.target_id" class="mono cell-hint">{{
          row.target_id
        }}</span>
      </template>
    </DataTable>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.mono {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

.cell-hint {
  margin-left: var(--space-2);
  color: var(--text-secondary);
}
</style>
