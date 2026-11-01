<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import {
  DataTable,
  FormDialog,
  PageHeader,
  type TableColumn
} from "@/shell/components";
import {
  createCustomer,
  listCustomers,
  updateCustomer
} from "@/api/customers";
import { formatDateTime } from "@/api/format";
import { statusVariant } from "@/api/status";
import type { Customer } from "@/api/types";

const { t } = useI18n();
const toast = useToast();

const columns = computed<TableColumn[]>(() => [
  { label: t("pm.customer.colName"), prop: "name", slot: "name" },
  { label: t("pm.customer.colCode"), prop: "code" },
  { label: t("pm.customer.colProjects"), prop: "project_count", numeric: true },
  { label: t("pm.customer.colStatus"), prop: "status", slot: "status" },
  { label: t("pm.columns.updatedAt"), prop: "updated_at", slot: "updatedAt" },
  { label: t("pm.columns.actions"), prop: "_actions", slot: "actions" }
]);

const rows = ref<Customer[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");

const editorOpen = ref(false);
const submitting = ref(false);
const editing = ref<Customer | null>(null);
const form = ref({
  code: "",
  name: "",
  short_name: "",
  industry: "",
  notes: "",
  status: "active"
});

const isPlaceholder = computed(() => editing.value?.status === "placeholder");

const statusOptions = computed(() => [
  { value: "active", label: t("pm.customer.statusActive") },
  { value: "archived", label: t("pm.customer.statusArchived") }
]);

function statusLabel(value: string): string {
  if (value === "placeholder") return t("pm.customer.statusPlaceholder");
  return value === "archived"
    ? t("pm.customer.statusArchived")
    : t("pm.customer.statusActive");
}

async function load() {
  loading.value = true;
  try {
    const result = await listCustomers({
      q: keyword.value.trim() || undefined,
      page: page.value,
      size: pageSize
    });
    rows.value = result.items ?? [];
    total.value = result.total ?? 0;
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("pm.customer.loadFailed"),
      description: String(error)
    });
  } finally {
    loading.value = false;
  }
}

function search() {
  page.value = 1;
  load();
}

function openCreate() {
  editing.value = null;
  form.value = {
    code: "",
    name: "",
    short_name: "",
    industry: "",
    notes: "",
    status: "active"
  };
  editorOpen.value = true;
}

function openEdit(row: Record<string, unknown>) {
  const customer = row as unknown as Customer;
  editing.value = customer;
  form.value = {
    code: customer.code ?? "",
    name: customer.name,
    short_name: customer.short_name ?? "",
    industry: customer.industry ?? "",
    notes: customer.notes ?? "",
    status: customer.status === "placeholder" ? "active" : customer.status
  };
  editorOpen.value = true;
}

async function submit() {
  const code = form.value.code.trim();
  const name = form.value.name.trim();
  if (!code) {
    toast.toast({ variant: "danger", title: t("pm.customer.codeRequired") });
    return;
  }
  if (!name) {
    toast.toast({ variant: "danger", title: t("pm.customer.nameRequired") });
    return;
  }

  submitting.value = true;
  try {
    if (editing.value) {
      await updateCustomer(editing.value.id, {
        code,
        name,
        short_name: form.value.short_name.trim(),
        industry: form.value.industry.trim(),
        notes: form.value.notes.trim(),
        status: form.value.status as "active" | "archived"
      });
      toast.toast({ variant: "success", title: t("pm.customer.updated") });
    } else {
      await createCustomer({
        code,
        name,
        short_name: form.value.short_name.trim() || undefined,
        industry: form.value.industry.trim() || undefined,
        notes: form.value.notes.trim() || undefined
      });
      toast.toast({ variant: "success", title: t("pm.customer.created") });
    }
    editorOpen.value = false;
    await load();
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("pm.customer.loadFailed"),
      description: String(error)
    });
  } finally {
    submitting.value = false;
  }
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('pm.customer.pageTitle')"
      :subtitle="t('pm.customer.subtitle')"
    />

    <div class="toolbar">
      <Input
        v-model="keyword"
        :placeholder="t('pm.customer.searchPlaceholder')"
        @keyup.enter="search"
      />
      <Button variant="secondary" @click="search">{{
        t("pm.customer.search")
      }}</Button>
      <Button @click="openCreate">{{ t("pm.customer.new") }}</Button>
    </div>

    <DataTable
      :columns="columns"
      :rows="rows"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('pm.customer.pageTitle')"
      :empty-text="t('pm.customer.emptyHint')"
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
          <span v-if="row.short_name" class="cell-hint">{{
            row.short_name
          }}</span>
        </div>
      </template>
      <template #status="{ row }">
        <Badge :variant="statusVariant(row.status)">{{
          statusLabel(String(row.status))
        }}</Badge>
      </template>
      <template #updatedAt="{ row }">{{
        formatDateTime(row.updated_at)
      }}</template>
      <template #actions="{ row }">
        <Button size="sm" variant="ghost" @click="openEdit(row)">
          {{ t("pm.customer.editTitle") }}
        </Button>
      </template>
    </DataTable>

    <FormDialog
      v-model:open="editorOpen"
      :title="editing ? t('pm.customer.editTitle') : t('pm.customer.newTitle')"
      :submitting="submitting"
      :submit-label="t('system.common.save')"
      @submit="submit"
      @cancel="editorOpen = false"
    >
      <p v-if="isPlaceholder" class="hint">{{ t("pm.customer.selectHint") }}</p>
      <label class="field">
        <span class="field__label">{{ t("pm.customer.code") }}</span>
        <Input
          v-model="form.code"
          :placeholder="t('pm.customer.codePlaceholder')"
          required
        />
      </label>
      <label class="field">
        <span class="field__label">{{ t("pm.customer.name") }}</span>
        <Input
          v-model="form.name"
          :placeholder="t('pm.customer.namePlaceholder')"
          required
        />
      </label>
      <label class="field">
        <span class="field__label">{{ t("pm.customer.shortName") }}</span>
        <Input
          v-model="form.short_name"
          :placeholder="t('pm.customer.shortNamePlaceholder')"
        />
      </label>
      <label class="field">
        <span class="field__label">{{ t("pm.customer.industry") }}</span>
        <Input
          v-model="form.industry"
          :placeholder="t('pm.customer.industryPlaceholder')"
        />
      </label>
      <label class="field">
        <span class="field__label">{{ t("pm.customer.notes") }}</span>
        <Input v-model="form.notes" />
      </label>
      <label v-if="editing" class="field">
        <span class="field__label">{{ t("pm.customer.colStatus") }}</span>
        <Select
          v-model="form.status"
          :options="statusOptions"
          :aria-label="t('pm.customer.colStatus')"
        />
      </label>
    </FormDialog>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  max-width: 560px;
  margin-left: auto;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-bottom: var(--space-3);
}

.field__label {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
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

.hint {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}
</style>
