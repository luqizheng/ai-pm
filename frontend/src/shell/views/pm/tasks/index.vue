<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { useToast } from "@/components/ui/toast";
import {
  DataTable,
  FormDialog,
  PageHeader,
  type TableColumn
} from "@/shell/components";
import ScopeSelect from "@/shell/views/pm/components/ScopeSelect.vue";
import { useUserDirectory } from "@/shell/store/user-directory";
import { translateError } from "@/shell/api";
import { useScopeStore, SCOPE_ALL } from "@/api/scope";
import { statusVariant } from "@/api/status";
import { formatDateTime } from "@/api/format";
import { createTask, listPrincipals, listTasks, updateTask } from "@/api/pm";
import type { Principal, Task } from "@/api/types";

const { t } = useI18n();
const toast = useToast();
const directory = useUserDirectory();
const scope = useScopeStore();
const principalList = ref<Principal[]>([]);

const STATUS_VALUES = [
  "backlog",
  "in_progress",
  "in_review",
  "done",
  "blocked",
  "cancelled"
];
const PRIORITY_VALUES = ["low", "normal", "high", "urgent"];

const columns: TableColumn[] = [
  { label: t("pm.columns.task"), prop: "title", slot: "task" },
  { label: t("pm.columns.status"), prop: "status", slot: "status" },
  { label: t("pm.columns.priority"), prop: "priority", slot: "priority" },
  { label: t("pm.columns.assignee"), prop: "assignee_id", slot: "assignee" },
  { label: t("pm.columns.actions"), prop: "_actions", slot: "actions" },
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

/** 编辑对话框：null = 关闭；creating 标记区分新建/编辑 */
const editing = ref<Task | null>(null);
const creating = ref(false);
const submitting = ref(false);
const form = ref({
  project_id: "",
  title: "",
  description: "",
  status: "backlog",
  priority: "normal",
  assignee_id: ""
});

const dialogOpen = computed(() => creating.value || editing.value !== null);
const titleMissing = computed(
  () =>
    (creating.value || editing.value !== null) && form.value.title.trim() === ""
);
const projectMissing = computed(
  () => creating.value && form.value.project_id === ""
);

/** 当前页按更新时间倒序（mock 与后端约定按 created_at 排序） */
const sortedRows = computed(() =>
  [...rows.value].sort((a, b) => b.updated_at.localeCompare(a.updated_at))
);

const statusOptions = computed(() =>
  STATUS_VALUES.map(value => ({ value, label: statusText(value) }))
);
const priorityOptions = computed(() =>
  PRIORITY_VALUES.map(value => ({ value, label: statusText(value) }))
);
const assigneeOptions = computed(() => [
  { value: "", label: t("pm.unassigned") },
  ...principalList.value.map(principal => ({
    value: principal.id,
    label: principal.display_name
  }))
]);
const projectOptions = computed(() =>
  scope.projects.map(project => ({
    value: project.id,
    label: `${project.code} · ${project.name}`
  }))
);

function statusText(value: unknown): string {
  const key = typeof value === "string" ? value : String(value ?? "");
  return t(`pm.status.${key}`, key);
}

function displayName(id: unknown): string {
  if (typeof id !== "string" || id === "") return t("pm.unassigned");
  return directory.names[id] ?? id;
}

/** 壳 DataTable 插槽 row 是 Record<string, unknown>，按列约定收窄 */
function asTask(row: Record<string, unknown>): Task {
  return row as unknown as Task;
}

function openCreate() {
  creating.value = true;
  editing.value = null;
  form.value = {
    project_id:
      scope.currentProjectId !== SCOPE_ALL
        ? scope.currentProjectId
        : (scope.projects[0]?.id ?? ""),
    title: "",
    description: "",
    status: "backlog",
    priority: "normal",
    assignee_id: ""
  };
}

function openEdit(row: Task) {
  creating.value = false;
  editing.value = row;
  form.value = {
    project_id: row.project_id,
    title: row.title,
    description: row.description ?? "",
    status: row.status,
    priority: row.priority,
    assignee_id: row.assignee_id ?? ""
  };
}

function closeDialog() {
  creating.value = false;
  editing.value = null;
}

async function load() {
  loading.value = true;
  try {
    await Promise.all([
      directory.ensureLoaded(),
      scope.ensureLoaded().catch(() => undefined),
      listPrincipals({ page: 1, size: 200 })
        .then(result => {
          principalList.value = result.items ?? [];
        })
        .catch(() => undefined)
    ]);
    const result = await listTasks({
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

async function submit() {
  if (titleMissing.value || projectMissing.value) return;
  submitting.value = true;
  try {
    if (creating.value) {
      await createTask({
        project_id: form.value.project_id,
        title: form.value.title.trim(),
        description: form.value.description.trim() || null,
        priority: form.value.priority,
        assignee_id: form.value.assignee_id || null
      });
      toast.toast({ variant: "success", title: t("pm.tasks.created") });
    } else if (editing.value) {
      await updateTask(editing.value.id, {
        revision: editing.value.revision,
        title: form.value.title.trim(),
        description: form.value.description.trim() || null,
        status: form.value.status,
        priority: form.value.priority,
        assignee_id: form.value.assignee_id || null
      });
      toast.toast({ variant: "success", title: t("pm.tasks.updated") });
    }
    closeDialog();
    await load();
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("pm.tasks.failed"),
      description: translateError(String(error))
    });
  } finally {
    submitting.value = false;
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
      :title="t('pm.tasks.pageTitle')"
      :subtitle="t('pm.tasks.subtitle')"
    >
      <template #actions>
        <Button size="sm" @click="openCreate">{{ t("pm.tasks.new") }}</Button>
        <ScopeSelect />
      </template>
    </PageHeader>

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
      <template #actions="{ row }">
        <Button size="sm" variant="ghost" @click="openEdit(asTask(row))">
          {{ t("pm.tasks.edit") }}
        </Button>
      </template>
      <template #updatedAt="{ row }">{{
        formatDateTime(row.updated_at)
      }}</template>
    </DataTable>

    <FormDialog
      v-model:open="dialogOpen"
      :title="creating ? t('pm.tasks.newTitle') : t('pm.tasks.editTitle')"
      :submitting="submitting"
      :submit-label="t('common.save')"
      @update:open="
        value => {
          if (!value) closeDialog();
        }
      "
      @submit="submit"
    >
      <div v-if="creating" class="field">
        <p class="field__label">{{ t("pm.columns.project") }}</p>
        <Select
          v-model="form.project_id"
          :options="projectOptions"
          :placeholder="t('pm.scope.all')"
          :invalid="projectMissing"
          aria-label="project"
        />
        <p v-if="projectMissing" class="field__hint">
          {{ t("pm.tasks.projectRequired") }}
        </p>
      </div>
      <div class="field">
        <p class="field__label">{{ t("pm.columns.task") }}</p>
        <Input
          v-model="form.title"
          :placeholder="t('pm.tasks.titlePlaceholder')"
          :invalid="titleMissing"
        />
        <p v-if="titleMissing" class="field__hint">
          {{ t("pm.tasks.titleRequired") }}
        </p>
      </div>
      <div class="field">
        <p class="field__label">{{ t("pm.tasks.descLabel") }}</p>
        <Textarea v-model="form.description" :rows="3" />
      </div>
      <div class="field-row">
        <div class="field">
          <p class="field__label">{{ t("pm.columns.status") }}</p>
          <Select
            v-model="form.status"
            :options="statusOptions"
            aria-label="status"
          />
        </div>
        <div class="field">
          <p class="field__label">{{ t("pm.columns.priority") }}</p>
          <Select
            v-model="form.priority"
            :options="priorityOptions"
            aria-label="priority"
          />
        </div>
        <div class="field">
          <p class="field__label">{{ t("pm.columns.assignee") }}</p>
          <Select
            v-model="form.assignee_id"
            :options="assigneeOptions"
            :placeholder="t('pm.unassigned')"
            aria-label="assignee"
          />
        </div>
      </div>
    </FormDialog>
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

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1;
}

.field__label {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.field__hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--danger-strong);
}

.field-row {
  display: flex;
  gap: var(--space-3);
}
</style>
