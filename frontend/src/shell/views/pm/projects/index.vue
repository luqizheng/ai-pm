<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import { DataTable, PageHeader, type TableColumn } from "@/shell/components";
import { statusVariant } from "@/api/status";
import { formatDateTime } from "@/api/format";
import { listProjects } from "@/api/pm";
import { listCustomers } from "@/api/customers";
import type { Customer, Project } from "@/api/types";

const { t } = useI18n();
const toast = useToast();
const router = useRouter();

/** reka-ui 的 Select 不接受空字符串作为 item value，故用哨兵值表示「全部客户」。 */
const ALL_CUSTOMERS = "all";

const columns = computed<TableColumn[]>(() => [
  { label: t("pm.columns.customer"), prop: "customer_name", slot: "customer" },
  { label: t("pm.columns.code"), prop: "code" },
  { label: t("pm.columns.name"), prop: "name", slot: "name" },
  { label: t("pm.columns.status"), prop: "status", slot: "status" },
  { label: t("pm.columns.revision"), prop: "revision", numeric: true },
  {
    label: t("pm.columns.updatedAt"),
    prop: "updated_at",
    slot: "updatedAt",
    numeric: true
  },
  {
    label: t("pm.columns.actions"),
    prop: "_actions",
    slot: "actions"
  }
]);

const rows = ref<Project[]>([]);
const customers = ref<Customer[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const customerId = ref<string>(ALL_CUSTOMERS);

/** 未归类客户（0017 之前的隐式客户）用提示前缀，避免被误读成真实客户名。 */
function customerLabel(customer: Customer): string {
  const label = customer.short_name || customer.name;
  return customer.status === "placeholder"
    ? `${t("pm.customer.unassigned")} · ${label}`
    : label;
}

const customerOptions = computed(() => [
  { value: ALL_CUSTOMERS, label: t("pm.customer.all") },
  ...customers.value.map(item => ({ value: item.id, label: customerLabel(item) }))
]);

function statusText(value: unknown): string {
  const key = typeof value === "string" ? value : String(value ?? "");
  return t(`pm.status.${key}`, key);
}

async function loadCustomers() {
  try {
    const result = await listCustomers({ size: 200 });
    customers.value = result.items ?? [];
  } catch {
    customers.value = [];
  }
}

async function load() {
  loading.value = true;
  try {
    const result = await listProjects({
      page: page.value,
      size: pageSize,
      customer_id: customerId.value === ALL_CUSTOMERS ? undefined : customerId.value
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

function filterByCustomer(value: string) {
  customerId.value = value || ALL_CUSTOMERS;
  page.value = 1;
  load();
}

function openRepositories(row: Record<string, unknown>) {
  router.push(`/projects/${String(row.id)}/repositories`);
}

onMounted(() => {
  loadCustomers();
  load();
});
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('pm.projects.pageTitle')"
      :subtitle="t('pm.projects.subtitle')"
    />

    <div class="filters">
      <Button variant="secondary" @click="router.push('/customers')">
        {{ t("pm.customer.pageTitle") }}
      </Button>
      <Select
        :model-value="customerId"
        :options="customerOptions"
        :aria-label="t('pm.columns.customer')"
        @update:model-value="filterByCustomer(String($event ?? ''))"
      />
    </div>

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
      <template #customer="{ row }">
        <span v-if="row.customer_status === 'placeholder'" class="cell-hint">
          {{ t("pm.customer.unassigned") }} · {{ row.customer_name }}
        </span>
        <span v-else>{{ row.customer_name }}</span>
      </template>
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
      <template #actions="{ row }">
        <Button size="sm" variant="ghost" @click="openRepositories(row)">
          {{ t("pm.repositories.pageTitle") }}
        </Button>
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

.filters {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-3);
  max-width: 420px;
  margin-left: auto;
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
