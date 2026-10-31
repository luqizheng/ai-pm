<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
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
import { useConfirm } from "@/shell/composables/useConfirm";
import { useScopeStore } from "@/api/scope";
import { formatDateTime } from "@/api/format";
import {
  createRepository,
  deleteRepository,
  deleteSshKey,
  generateSshKey,
  getSshKey,
  listRepositories,
  updateRepository,
  type RepositoryPayload
} from "@/api/repositories";
import type { Repository, RepositoryRole, SshKey } from "@/api/types";

const { t } = useI18n();
const route = useRoute();
const toast = useToast();
const confirm = useConfirm();
const scope = useScopeStore();

const projectId = computed(() => String(route.params.projectId ?? ""));
const project = computed(
  () => scope.projects.find(item => item.id === projectId.value) ?? null
);
const projectLabel = computed(() =>
  project.value
    ? `${project.value.code} · ${project.value.name}`
    : projectId.value
);
const subtitle = computed(
  () => `${projectLabel.value} · ${t("pm.repositories.subtitle")}`
);

const rows = ref<Repository[]>([]);
const loading = ref(false);
const sshKey = ref<SshKey | null>(null);
const keyLoading = ref(false);
const keyBusy = ref(false);

const ROLE_VALUES: RepositoryRole[] = ["main", "docs", "service", "infra"];
const roleOptions = computed(() =>
  ROLE_VALUES.map(value => ({
    value,
    label: t(`pm.repositories.role.${value}`)
  }))
);

const columns = computed<TableColumn[]>(() => [
  { label: t("pm.repositories.name"), prop: "name", slot: "name" },
  { label: t("pm.repositories.url"), prop: "url", slot: "url" },
  { label: t("pm.repositories.roleLabel"), prop: "role", slot: "role" },
  {
    label: t("pm.repositories.defaultBranch"),
    prop: "default_branch",
    slot: "branch"
  },
  {
    label: t("pm.repositories.updatedAt"),
    prop: "updated_at",
    slot: "updatedAt",
    numeric: true
  },
  { label: t("pm.columns.actions"), prop: "_actions", slot: "actions" }
]);

const creating = ref(false);
const editing = ref<Repository | null>(null);
const submitting = ref(false);
const form = ref({
  name: "",
  url: "",
  role: "main" as RepositoryRole,
  default_branch: "",
  description: ""
});

const dialogOpen = computed(() => creating.value || editing.value !== null);
const nameMissing = computed(
  () => dialogOpen.value && form.value.name.trim() === ""
);
const urlMissing = computed(
  () => dialogOpen.value && form.value.url.trim() === ""
);

function roleVariant(value: unknown): "info" | "warning" | "neutral" {
  if (value === "main") return "info";
  if (value === "service") return "warning";
  return "neutral";
}

/** DataTable 插槽 row 是 Record<string, unknown>，按列约定收窄。 */
function asRepository(row: Record<string, unknown>): Repository {
  return row as unknown as Repository;
}

function failure(error: unknown) {
  toast.toast({
    variant: "danger",
    title: t("pm.repositories.failed"),
    description: String(error)
  });
}

async function load() {
  loading.value = true;
  try {
    const result = await listRepositories(projectId.value);
    rows.value = result.items ?? [];
  } catch (error) {
    failure(error);
  } finally {
    loading.value = false;
  }
}

/** 未生成时后端 404 —— 这里统一按「无密钥」渲染，不弹错误。 */
async function loadKey() {
  keyLoading.value = true;
  try {
    sshKey.value = await getSshKey(projectId.value);
  } catch {
    sshKey.value = null;
  } finally {
    keyLoading.value = false;
  }
}

async function copyKey() {
  if (!sshKey.value) return;
  try {
    await navigator.clipboard.writeText(sshKey.value.public_key);
    toast.toast({
      variant: "success",
      title: t("pm.repositories.copied")
    });
  } catch {
    toast.toast({
      variant: "danger",
      title: t("pm.repositories.copyFailed")
    });
  }
}

async function generateKey() {
  if (sshKey.value) {
    const ok = await confirm.confirm({
      title: t("pm.repositories.regenerate"),
      description: t("pm.repositories.regenerateConfirm"),
      confirmLabel: t("pm.repositories.regenerate"),
      danger: true
    });
    if (!ok) return;
  }
  keyBusy.value = true;
  try {
    sshKey.value = await generateSshKey(projectId.value);
    toast.toast({
      variant: "success",
      title: t("pm.repositories.keyGenerated")
    });
  } catch (error) {
    failure(error);
  } finally {
    keyBusy.value = false;
  }
}

async function removeKey() {
  const ok = await confirm.confirm({
    title: t("pm.repositories.deleteKey"),
    description: t("pm.repositories.deleteKeyConfirm"),
    confirmLabel: t("pm.repositories.deleteKey"),
    danger: true
  });
  if (!ok) return;
  keyBusy.value = true;
  try {
    await deleteSshKey(projectId.value);
    sshKey.value = null;
    toast.toast({
      variant: "success",
      title: t("pm.repositories.keyDeleted")
    });
  } catch (error) {
    failure(error);
  } finally {
    keyBusy.value = false;
  }
}

function openCreate() {
  creating.value = true;
  editing.value = null;
  form.value = {
    name: "",
    url: "",
    role: "main",
    default_branch: "",
    description: ""
  };
}

function openEdit(row: Repository) {
  creating.value = false;
  editing.value = row;
  form.value = {
    name: row.name,
    url: row.url,
    role: row.role,
    default_branch: row.default_branch ?? "",
    description: row.description ?? ""
  };
}

function closeDialog() {
  creating.value = false;
  editing.value = null;
}

async function remove(row: Repository) {
  const ok = await confirm.confirm({
    title: t("pm.repositories.deleteTitle"),
    description: t("pm.repositories.deleteConfirm", { name: row.name }),
    confirmLabel: t("pm.repositories.delete"),
    danger: true
  });
  if (!ok) return;
  try {
    await deleteRepository(row.id);
    toast.toast({
      variant: "success",
      title: t("pm.repositories.deleted")
    });
    await load();
  } catch (error) {
    failure(error);
  }
}

async function submit() {
  if (nameMissing.value || urlMissing.value) return;
  const payload: RepositoryPayload = {
    name: form.value.name.trim(),
    url: form.value.url.trim(),
    role: form.value.role,
    default_branch: form.value.default_branch.trim() || null,
    description: form.value.description.trim() || null
  };
  submitting.value = true;
  try {
    if (creating.value) {
      await createRepository(projectId.value, payload);
      toast.toast({
        variant: "success",
        title: t("pm.repositories.created")
      });
    } else if (editing.value) {
      await updateRepository(editing.value.id, {
        ...payload,
        revision: editing.value.revision
      });
      toast.toast({
        variant: "success",
        title: t("pm.repositories.updated")
      });
    }
    closeDialog();
    await load();
  } catch (error) {
    failure(error);
  } finally {
    submitting.value = false;
  }
}

onMounted(async () => {
  await scope.ensureLoaded().catch(() => undefined);
  await Promise.all([load(), loadKey()]);
});
</script>

<template>
  <section class="ui-page">
    <PageHeader :title="t('pm.repositories.pageTitle')" :subtitle="subtitle">
      <template #actions>
        <Button size="sm" @click="openCreate">
          {{ t("pm.repositories.new") }}
        </Button>
      </template>
    </PageHeader>

    <section class="ui-card">
      <header class="ui-card__head">
        <div class="ui-card__text">
          <h2 class="ui-card__title">{{ t("pm.repositories.sshKeyTitle") }}</h2>
          <p class="ui-card__hint">{{ t("pm.repositories.sshKeyHint") }}</p>
        </div>
        <div class="ui-card__actions">
          <Button
            v-if="sshKey"
            size="sm"
            variant="secondary"
            :disabled="keyBusy"
            @click="copyKey"
          >
            {{ t("pm.repositories.copy") }}
          </Button>
          <Button size="sm" :loading="keyBusy" @click="generateKey">
            {{
              sshKey
                ? t("pm.repositories.regenerate")
                : t("pm.repositories.generate")
            }}
          </Button>
          <Button
            v-if="sshKey"
            size="sm"
            variant="ghost"
            :disabled="keyBusy"
            @click="removeKey"
          >
            {{ t("pm.repositories.deleteKey") }}
          </Button>
        </div>
      </header>

      <p v-if="!sshKey && !keyLoading" class="ui-card__empty">
        {{ t("pm.repositories.sshKeyEmpty") }}
      </p>
      <template v-else-if="sshKey">
        <pre class="key-block">{{ sshKey.public_key }}</pre>
        <p class="ui-card__meta">
          {{ t("pm.repositories.fingerprint") }}:
          <code>{{ sshKey.fingerprint }}</code>
          · {{ t("pm.repositories.createdAt") }}
          {{ formatDateTime(sshKey.created_at) }}
        </p>
      </template>
    </section>

    <DataTable
      :columns="columns"
      :rows="rows"
      :loading="loading"
      :page="1"
      :page-size="Math.max(rows.length, 10)"
      :total="rows.length"
      :caption="t('pm.repositories.pageTitle')"
      :empty-text="t('pm.repositories.emptyHint')"
    >
      <template #name="{ row }">
        <div class="cell-stack">
          <span>{{ row.name }}</span>
          <span v-if="row.description" class="cell-hint">{{
            row.description
          }}</span>
        </div>
      </template>
      <template #url="{ row }">
        <code class="cell-url">{{ row.url }}</code>
      </template>
      <template #role="{ row }">
        <Badge :variant="roleVariant(row.role)">{{
          t(`pm.repositories.role.${String(row.role)}`)
        }}</Badge>
      </template>
      <template #branch="{ row }">{{ row.default_branch || "—" }}</template>
      <template #updatedAt="{ row }">{{
        formatDateTime(row.updated_at)
      }}</template>
      <template #actions="{ row }">
        <div class="row-actions">
          <Button
            size="sm"
            variant="ghost"
            @click="openEdit(asRepository(row))"
          >
            {{ t("pm.repositories.edit") }}
          </Button>
          <Button size="sm" variant="ghost" @click="remove(asRepository(row))">
            {{ t("pm.repositories.delete") }}
          </Button>
        </div>
      </template>
    </DataTable>

    <FormDialog
      :open="dialogOpen"
      :title="
        creating
          ? t('pm.repositories.newTitle')
          : t('pm.repositories.editTitle')
      "
      :submitting="submitting"
      :submit-label="t('pm.repositories.save')"
      @update:open="
        value => {
          if (!value) closeDialog();
        }
      "
      @submit="submit"
      @cancel="closeDialog"
    >
      <div class="field">
        <p class="field__label">{{ t("pm.repositories.name") }}</p>
        <Input
          v-model="form.name"
          :placeholder="t('pm.repositories.namePlaceholder')"
          :invalid="nameMissing"
        />
        <p v-if="nameMissing" class="field__hint">
          {{ t("pm.repositories.nameRequired") }}
        </p>
      </div>
      <div class="field">
        <p class="field__label">{{ t("pm.repositories.url") }}</p>
        <Input
          v-model="form.url"
          :placeholder="t('pm.repositories.urlPlaceholder')"
          :invalid="urlMissing"
        />
        <p v-if="urlMissing" class="field__hint">
          {{ t("pm.repositories.urlRequired") }}
        </p>
        <p v-else class="field__hint field__hint--muted">
          {{ t("pm.repositories.urlHint") }}
        </p>
      </div>
      <div class="field-row">
        <div class="field">
          <p class="field__label">{{ t("pm.repositories.roleLabel") }}</p>
          <Select
            v-model="form.role"
            :options="roleOptions"
            aria-label="role"
          />
        </div>
        <div class="field">
          <p class="field__label">{{ t("pm.repositories.defaultBranch") }}</p>
          <Input
            v-model="form.default_branch"
            :placeholder="t('pm.repositories.defaultBranchPlaceholder')"
          />
        </div>
      </div>
      <div class="field">
        <p class="field__label">{{ t("pm.repositories.description") }}</p>
        <Textarea v-model="form.description" :rows="3" />
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

.ui-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.ui-card__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.ui-card__text {
  min-width: 0;
}

.ui-card__title {
  margin: 0;
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ui-card__hint {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.ui-card__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}

.ui-card__empty {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
}

.ui-card__meta {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.key-block {
  margin: 0;
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  white-space: pre-wrap;
  word-break: break-all;
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

.cell-url {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  word-break: break-all;
}

.row-actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
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

.field__hint--muted {
  color: var(--text-tertiary);
}

.field-row {
  display: flex;
  gap: var(--space-3);
}
</style>
