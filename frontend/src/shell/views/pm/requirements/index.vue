<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { useScopeStore } from "@/api/scope";
import { statusVariant } from "@/api/status";
import { formatDate } from "@/api/format";
import {
  approveRevision,
  createRevision,
  listRevisions,
  listRequirements
} from "@/api/pm";
import type { Requirement, RequirementRevision } from "@/api/types";

const { t } = useI18n();
const toast = useToast();
const scope = useScopeStore();
const directory = useUserDirectory();

const columns: TableColumn[] = [
  { label: t("pm.columns.code"), prop: "code" },
  { label: t("pm.columns.title"), prop: "title" },
  { label: t("pm.columns.status"), prop: "status", slot: "status" },
  { label: t("pm.columns.rowRevision"), prop: "row_revision", numeric: true },
  { label: t("pm.columns.actions"), prop: "_actions", slot: "actions" },
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

/** 修订对话框状态：null = 关闭 */
const historyOf = ref<Requirement | null>(null);
const revisions = ref<RequirementRevision[]>([]);
const revisionsLoading = ref(false);
const newMarkdown = ref("");
const submitting = ref(false);
const approvingVersion = ref<number | null>(null);

const historyOpen = computed(() => historyOf.value !== null);

function statusText(value: unknown): string {
  const key = typeof value === "string" ? value : String(value ?? "");
  return t(`pm.status.${key}`, key);
}

/** 壳 DataTable 插槽 row 是 Record<string, unknown>，按列约定收窄 */
function asRequirement(row: Record<string, unknown>): Requirement {
  return row as unknown as Requirement;
}

async function load() {
  loading.value = true;
  try {
    const result = await listRequirements({
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

async function loadRevisions(requirementId: string) {
  revisionsLoading.value = true;
  try {
    const result = await listRevisions(requirementId);
    revisions.value = result.items ?? [];
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("pm.loadFailed"),
      description: String(error)
    });
  } finally {
    revisionsLoading.value = false;
  }
}

function openHistory(row: Requirement) {
  historyOf.value = row;
  newMarkdown.value = "";
  void loadRevisions(row.id);
}

async function submitRevision() {
  const target = historyOf.value;
  const markdown = newMarkdown.value.trim();
  if (!target || markdown === "") return;
  submitting.value = true;
  try {
    await createRevision(target.id, markdown);
    toast.toast({ variant: "success", title: t("pm.revisions.created") });
    newMarkdown.value = "";
    await loadRevisions(target.id);
    await load();
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("pm.revisions.failed"),
      description: translateError(String(error))
    });
  } finally {
    submitting.value = false;
  }
}

async function approve(revision: RequirementRevision) {
  if (!historyOf.value) return;
  approvingVersion.value = revision.version;
  try {
    await approveRevision(historyOf.value.id, revision.id);
    toast.toast({ variant: "success", title: t("pm.revisions.approved") });
    await loadRevisions(historyOf.value.id);
    await load();
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("pm.revisions.failed"),
      description: translateError(String(error))
    });
  } finally {
    approvingVersion.value = null;
  }
}

watch(
  () => scope.currentProjectId,
  () => {
    page.value = 1;
    load();
  }
);

onMounted(() => {
  load();
  void directory.ensureLoaded().catch(() => undefined);
});
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('pm.requirements.pageTitle')"
      :subtitle="t('pm.requirements.subtitle')"
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
      <template #actions="{ row }">
        <Button
          size="sm"
          variant="ghost"
          @click="openHistory(asRequirement(row))"
        >
          {{ t("pm.revisions.open") }}
        </Button>
      </template>
      <template #createdAt="{ row }">{{ formatDate(row.created_at) }}</template>
    </DataTable>

    <FormDialog
      v-model:open="historyOpen"
      :title="
        historyOf
          ? `${t('pm.revisions.title')} · ${historyOf.code}`
          : t('pm.revisions.title')
      "
      :submitting="submitting"
      :submit-label="t('pm.revisions.newSubmit')"
      size="lg"
      @update:open="
        value => {
          if (!value) historyOf = null;
        }
      "
      @submit="submitRevision"
    >
      <p v-if="revisionsLoading" class="rev-loading">
        {{ t("pm.overview.empty") }}
      </p>
      <ul v-else-if="revisions.length > 0" class="rev-list">
        <li
          v-for="revision in revisions"
          :key="revision.id"
          class="rev-list__item"
        >
          <div class="rev-list__main">
            <span class="rev-list__version">
              v{{ revision.version }}
              <Badge :variant="statusVariant(revision.state)">
                {{ statusText(revision.state) }}
              </Badge>
            </span>
            <span class="rev-list__meta">
              {{ formatDate(revision.created_at) }} ·
              {{ revision.content_hash.slice(0, 12) }}
            </span>
          </div>
          <Button
            v-if="revision.state === 'draft'"
            size="sm"
            :disabled="approvingVersion !== null"
            @click="approve(revision)"
          >
            {{ t("pm.revisions.approve") }}
          </Button>
        </li>
      </ul>
      <p v-else class="rev-loading">{{ t("pm.revisions.empty") }}</p>

      <p class="rev-new-label">{{ t("pm.revisions.newLabel") }}</p>
      <Textarea
        v-model="newMarkdown"
        :placeholder="t('pm.revisions.newPlaceholder')"
        :rows="5"
      />
    </FormDialog>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.rev-loading {
  margin: var(--space-4) 0;
  text-align: center;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.rev-list {
  margin: 0;
  padding: 0;
  list-style: none;
  max-height: 240px;
  overflow-y: auto;
}

.rev-list__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.rev-list__item:last-child {
  border-bottom: none;
}

.rev-list__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.rev-list__version {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-mono);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.rev-list__meta {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.rev-new-label {
  margin: var(--space-3) 0 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}
</style>
