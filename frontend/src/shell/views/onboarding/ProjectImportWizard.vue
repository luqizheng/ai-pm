<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/shell/components";
import { createProject } from "@/api/projects";
import { listCustomers } from "@/api/customers";
import {
  createRepository,
  createSshKey,
  listRepositories,
  verifyRepository
} from "@/api/repositories";
import {
  applyImport,
  cancelImport,
  getImport,
  startImport
} from "@/api/imports";
import DiscoveryPanel from "./DiscoveryPanel.vue";
import type {
  Customer,
  ProjectDraft,
  ProjectImport,
  ProposedImport,
  Repository,
  RepositoryRole,
  SshKey
} from "@/api/types";

const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const toast = useToast();

const step = ref(0);
const busy = ref(false);
const projectId = ref("");
const form = ref({ code: "", name: "", description: "", customer_id: "none" });

/** 客户主数据（0017）。建项时选已有客户，缺省则落为「未归类」客户。 */
const NONE_CUSTOMER = "none";
const customers = ref<Customer[]>([]);
const customerOptions = computed(() => [
  { value: NONE_CUSTOMER, label: t("pm.customer.none") },
  ...customers.value.map(item => ({
    value: item.id,
    label:
      item.status === "placeholder"
        ? `${t("pm.customer.unassigned")} · ${item.short_name || item.name}`
        : item.short_name || item.name
  }))
]);

async function loadCustomers() {
  try {
    const result = await listCustomers({ size: 200, status: "active" });
    customers.value = result.items ?? [];
  } catch {
    customers.value = [];
  }
}

const repos = ref<Repository[]>([]);
const sshKey = ref<SshKey | null>(null);
const keyBusy = ref(false);
const repoForm = ref<{ name: string; url: string; role: RepositoryRole }>({
  name: "",
  url: "",
  role: "main"
});

const importId = ref("");
const detail = ref<ProjectImport | null>(null);
let timer: ReturnType<typeof setInterval> | null = null;

const selectedMilestones = ref<string[]>([]);
const selectedRequirements = ref<string[]>([]);
const selectedTasks = ref<string[]>([]);
const writeDocuments = ref(true);
const proposed = ref<ProposedImport | null>(null);

const steps = computed(() => [
  t("pm.import.stepBasics"),
  t("pm.import.stepRepositories"),
  t("pm.import.stepAnalysis"),
  t("pm.import.stepReview")
]);

const canNextBasics = computed(
  () => form.value.code.trim() !== "" && form.value.name.trim() !== ""
);
const verifiedCount = computed(
  () => repos.value.filter(item => item.status === "verified").length
);
const draft = computed<ProjectDraft | null>(() => detail.value?.draft ?? null);
const roleOptions = computed(() =>
  (["main", "docs", "service", "infra"] as RepositoryRole[]).map(value => ({
    value,
    label: t(`pm.repositories.role.${value}`)
  }))
);

const stageIndex = computed(() => {
  const status = detail.value?.status;
  if (!status) return 0;
  if (status === "queued") return 0;
  if (status === "cloning") return 1;
  if (status === "reading" || status === "analyzing") return 2;
  return 3;
});

const analysisFailed = computed(
  () =>
    detail.value?.status === "failed" || detail.value?.status === "cancelled"
);

function failure(error: unknown, title: string) {
  toast.toast({ variant: "danger", title, description: String(error) });
}

async function loadRepos() {
  const result = await listRepositories(projectId.value);
  repos.value = result.items ?? [];
}

async function ensureKey() {
  keyBusy.value = true;
  try {
    sshKey.value = await createSshKey(projectId.value);
  } catch (error) {
    failure(error, t("pm.import.repoFailed"));
  } finally {
    keyBusy.value = false;
  }
}

async function submitBasics() {
  if (!canNextBasics.value || busy.value) return;
  busy.value = true;
  try {
    const project = await createProject({
      code: form.value.code.trim(),
      name: form.value.name.trim(),
      description: form.value.description.trim(),
      // 未选客户时不下发该字段，交由后端走兼容路径（落 placeholder 客户）。
      ...(form.value.customer_id && form.value.customer_id !== NONE_CUSTOMER
        ? { customer_id: form.value.customer_id }
        : {})
    });
    projectId.value = project.id;
    await Promise.all([loadRepos(), ensureKey()]);
    step.value = 1;
    toast.toast({ variant: "success", title: t("pm.import.projectCreated") });
  } catch (error) {
    failure(error, t("pm.import.loadFailed"));
  } finally {
    busy.value = false;
  }
}

async function addRepository() {
  const name = repoForm.value.name.trim();
  const url = repoForm.value.url.trim();
  if (!name || !url || !projectId.value) return;
  busy.value = true;
  try {
    await createRepository(projectId.value, {
      name,
      url,
      role: repoForm.value.role,
      default_branch: null,
      description: null
    });
    repoForm.value = { name: "", url: "", role: "main" };
    await loadRepos();
  } catch (error) {
    failure(error, t("pm.import.repoFailed"));
  } finally {
    busy.value = false;
  }
}

async function verifyRow(row: Repository) {
  busy.value = true;
  try {
    await verifyRepository(row.id);
    await loadRepos();
  } catch (error) {
    failure(error, t("pm.import.repoFailed"));
  } finally {
    busy.value = false;
  }
}

function stopPolling() {
  if (timer !== null) {
    clearInterval(timer);
    timer = null;
  }
}

function applyDefaults() {
  const value = draft.value;
  if (!value) return;
  selectedMilestones.value = value.milestones.map(item => item.key);
  selectedRequirements.value = value.requirements
    .filter(item => item.confidence !== "low")
    .map(item => item.key);
  selectedTasks.value = value.tasks
    .filter(item => item.confidence !== "low")
    .map(item => item.key);
}

async function pollOnce() {
  if (!projectId.value || !importId.value) return;
  try {
    detail.value = await getImport(projectId.value, importId.value);
    if (detail.value.status === "draft_ready") {
      stopPolling();
      applyDefaults();
      step.value = 3;
    } else if (analysisFailed.value) {
      stopPolling();
    }
  } catch (error) {
    stopPolling();
    failure(error, t("pm.import.failed"));
  }
}

function startPolling() {
  stopPolling();
  timer = setInterval(() => {
    void pollOnce();
  }, 1000);
}

async function startAnalysis() {
  if (verifiedCount.value === 0 || busy.value) return;
  busy.value = true;
  try {
    const started = await startImport(projectId.value);
    importId.value = started.import_id;
    step.value = 2;
    await pollOnce();
    if (detail.value && detail.value.status !== "draft_ready") startPolling();
  } catch (error) {
    failure(error, t("pm.import.failed"));
  } finally {
    busy.value = false;
  }
}

async function cancelAnalysis() {
  if (!projectId.value || !importId.value) return;
  try {
    await cancelImport(projectId.value, importId.value);
    stopPolling();
    await pollOnce();
  } catch (error) {
    failure(error, t("pm.import.failed"));
  }
}

async function retryAnalysis() {
  stopPolling();
  importId.value = "";
  detail.value = null;
  step.value = 1;
  await startAnalysis();
}

function toggle(
  list: "milestones" | "requirements" | "tasks",
  key: string
) {
  const target =
    list === "milestones"
      ? selectedMilestones
      : list === "requirements"
        ? selectedRequirements
        : selectedTasks;
  const index = target.value.indexOf(key);
  if (index >= 0) target.value.splice(index, 1);
  else target.value.push(key);
}

async function submitApply() {
  const value = detail.value;
  if (!value || busy.value) return;
  busy.value = true;
  try {
    proposed.value = await applyImport(projectId.value, value.id, {
      draft_revision: value.draft_revision,
      project: {
        code: form.value.code.trim(),
        name: form.value.name.trim(),
        description: form.value.description.trim()
      },
      milestone_keys: selectedMilestones.value,
      requirement_keys: selectedRequirements.value,
      task_keys: selectedTasks.value,
      write_knowledge_documents: writeDocuments.value
    });
    toast.toast({
      variant: "success",
      title: t("pm.import.proposed", {
        count: proposed.value.proposals?.length ?? 0
      })
    });
  } catch (error) {
    failure(error, t("pm.import.applyFailed"));
  } finally {
    busy.value = false;
  }
}

function goApprovals() {
  void router.push(
    `/workspace/${projectId.value}/classic/approvals`
  );
}

onMounted(async () => {
  if (typeof window !== "undefined") {
    window.addEventListener("beforeunload", stopPolling);
  }
  loadCustomers();
  const existing = String(route.query.projectId ?? "");
  if (existing) {
    projectId.value = existing;
    form.value.code = String(route.query.code ?? "");
    form.value.name = String(route.query.name ?? "");
    await Promise.all([loadRepos(), ensureKey()]).catch(() => undefined);
    step.value = 1;
  }
});

onBeforeUnmount(() => {
  stopPolling();
  if (typeof window !== "undefined") {
    window.removeEventListener("beforeunload", stopPolling);
  }
});
</script>

<template>
  <section class="wizard">
    <PageHeader
      :title="t('pm.import.pageTitle')"
      :subtitle="t('pm.import.subtitle')"
    />

    <ol class="steps">
      <li
        v-for="(label, index) in steps"
        :key="label"
        class="steps__item"
        :class="{
          'steps__item--active': index === step,
          'steps__item--done': index < step
        }"
      >
        <span class="steps__index">{{ index + 1 }}</span>
        <span>{{ label }}</span>
      </li>
    </ol>

    <section v-if="step === 0" class="card">
      <div class="field">
        <label class="field__label">{{ t("pm.import.code") }}</label>
        <Input
          v-model="form.code"
          :placeholder="t('pm.import.codePlaceholder')"
        />
      </div>
      <div class="field">
        <label class="field__label">{{ t("pm.import.name") }}</label>
        <Input
          v-model="form.name"
          :placeholder="t('pm.import.namePlaceholder')"
        />
      </div>
      <div class="field">
        <label class="field__label">{{ t("pm.import.description") }}</label>
        <Input v-model="form.description" />
      </div>
      <div class="field">
        <label class="field__label">{{ t("pm.customer.select") }}</label>
        <Select
          v-model="form.customer_id"
          :options="customerOptions"
          :aria-label="t('pm.customer.select')"
        />
        <p class="hint">{{ t("pm.customer.selectHint") }}</p>
      </div>
      <p v-if="!canNextBasics" class="hint">{{ t("pm.import.required") }}</p>
      <div class="actions">
        <Button :disabled="!canNextBasics" :loading="busy" @click="submitBasics">
          {{ t("pm.import.createProject") }}
        </Button>
      </div>
    </section>

    <section v-else-if="step === 1" class="card">
      <div class="key">
        <p class="key__hint">{{ t("pm.repositories.sshKeyHint") }}</p>
        <pre v-if="sshKey" class="key__block">{{ sshKey.public_key }}</pre>
        <p v-else class="hint">{{ t("pm.repositories.sshKeyEmpty") }}</p>
        <p v-if="sshKey" class="hint">
          {{ t("pm.repositories.fingerprint") }}: {{ sshKey.fingerprint }}
        </p>
      </div>

      <div class="repo-form">
        <Input
          v-model="repoForm.name"
          :placeholder="t('pm.import.repositoryName')"
        />
        <Input
          v-model="repoForm.url"
          :placeholder="t('pm.repositories.urlPlaceholder')"
        />
        <Select
          v-model="repoForm.role"
          :options="roleOptions"
          :aria-label="t('pm.import.repositoryRole')"
        />
        <Button :loading="busy" @click="addRepository">
          {{ t("pm.import.addRepository") }}
        </Button>
      </div>

      <ul class="repos">
        <li v-for="row in repos" :key="row.id" class="repos__row">
          <div class="repos__text">
            <strong>{{ row.name }}</strong>
            <code>{{ row.url }}</code>
          </div>
          <Badge
            :variant="
              row.status === 'verified'
                ? 'success'
                : row.status === 'unreachable'
                  ? 'danger'
                  : 'neutral'
            "
          >
            {{ t(`pm.import.${row.status}`) }}
          </Badge>
          <Button size="sm" variant="ghost" @click="verifyRow(row)">
            {{ t("pm.import.verify") }}
          </Button>
        </li>
      </ul>

      <p v-if="verifiedCount === 0" class="hint">
        {{ t("pm.import.noVerified") }}
      </p>
      <div class="actions">
        <Button variant="ghost" @click="step = 0">
          {{ t("pm.import.back") }}
        </Button>
        <Button :disabled="verifiedCount === 0" :loading="busy" @click="startAnalysis">
          {{ t("pm.import.startAnalysis") }}
        </Button>
      </div>
    </section>

    <section v-else-if="step === 2" class="card">
      <p class="stage">
        {{ detail ? t("pm.import.analyzing") : t("pm.import.analyzing") }}
        <span v-if="detail">· {{ detail.progress }}%</span>
      </p>
      <ol class="timeline">
        <li
          v-for="(label, index) in [
            t('pm.import.stageClone'),
            t('pm.import.stageRead'),
            t('pm.import.stageModel'),
            t('pm.import.stageDraft')
          ]"
          :key="label"
          :class="{ 'timeline__done': index <= stageIndex }"
        >
          {{ label }}
        </li>
      </ol>
      <p v-if="detail?.model_name === 'fake'" class="hint">
        {{ t("pm.import.demoModel") }}
      </p>
      <p v-if="analysisFailed" class="hint hint--danger">
        {{ t("pm.import.failed") }}: {{ detail?.error_code || "—" }}
      </p>
      <div class="actions">
        <Button variant="ghost" @click="cancelAnalysis">
          {{ t("pm.import.cancel") }}
        </Button>
        <Button v-if="analysisFailed" @click="retryAnalysis">
          {{ t("pm.import.retry") }}
        </Button>
      </div>
    </section>

    <section v-else class="card">
      <template v-if="draft">
        <div class="section">
          <h3 class="section__title">{{ t("pm.import.profile") }}</h3>
          <p class="section__body">
            {{ draft.profile.suggested_name }} · {{ draft.profile.suggested_code }}
          </p>
          <p class="hint">{{ draft.profile.summary }}</p>
        </div>

        <div class="section">
          <h3 class="section__title">{{ t("pm.import.milestones") }}</h3>
          <label v-for="item in draft.milestones" :key="item.key" class="check">
            <input
              type="checkbox"
              :checked="selectedMilestones.includes(item.key)"
              @change="toggle('milestones', item.key)"
            />
            <span>{{ item.name }}</span>
          </label>
        </div>

        <div class="section">
          <h3 class="section__title">{{ t("pm.import.requirements") }}</h3>
          <label v-for="item in draft.requirements" :key="item.key" class="check">
            <input
              type="checkbox"
              :checked="selectedRequirements.includes(item.key)"
              @change="toggle('requirements', item.key)"
            />
            <span>{{ item.code }} · {{ item.title }}</span>
            <Badge v-if="item.confidence === 'low'" variant="warning">
              {{ t("pm.import.confidenceLow") }}
            </Badge>
            <code v-for="ev in item.evidence" :key="ev.ref" class="evidence">{{
              ev.ref
            }}</code>
          </label>
        </div>

        <div class="section">
          <h3 class="section__title">{{ t("pm.import.tasks") }}</h3>
          <label v-for="item in draft.tasks" :key="item.key" class="check">
            <input
              type="checkbox"
              :checked="selectedTasks.includes(item.key)"
              @change="toggle('tasks', item.key)"
            />
            <span>{{ item.title }}</span>
            <Badge v-if="item.assignee_role" variant="info">
              {{ item.assignee_role }}
            </Badge>
            <Badge v-if="item.confidence === 'low'" variant="warning">
              {{ t("pm.import.confidenceLow") }}
            </Badge>
          </label>
        </div>

        <div v-if="draft.risks.length" class="section">
          <h3 class="section__title">{{ t("pm.import.risks") }}</h3>
          <p v-for="risk in draft.risks" :key="risk.description" class="hint">
            · {{ risk.description }}
          </p>
        </div>

        <div v-if="draft.unknowns.length" class="section">
          <h3 class="section__title">{{ t("pm.import.unknowns") }}</h3>
          <p v-for="item in draft.unknowns" :key="item.question" class="hint">
            · {{ item.question }}
          </p>
        </div>

        <DiscoveryPanel
          v-if="projectId && importId"
          :project-id="projectId"
          :import-id="importId"
        />

        <p v-if="detail?.dropped_items" class="hint hint--warning">
          {{ t("pm.import.droppedHint", { count: detail?.dropped_items ?? 0 }) }}
        </p>
        <p
          v-if="draft.coverage.repos.some(repo => repo.truncated)"
          class="hint hint--warning"
        >
          {{ t("pm.import.truncatedHint") }}
        </p>
        <label class="check">
          <input v-model="writeDocuments" type="checkbox" />
          <span>{{ t("pm.import.writeDocuments") }}</span>
        </label>

        <div class="actions">
          <Button :loading="busy" @click="submitApply">
            {{ t("pm.import.submitProposals") }}
          </Button>
          <Button v-if="proposed" variant="secondary" @click="goApprovals">
            {{ t("pm.import.goApprovals") }}
          </Button>
        </div>
      </template>
      <p v-else class="hint">{{ t("pm.import.loadFailed") }}</p>
    </section>
  </section>
</template>

<style scoped>
.wizard {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.steps {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.steps__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.steps__item--active {
  border-color: var(--accent);
  color: var(--text-primary);
}

.steps__item--done {
  color: var(--success-strong);
}

.steps__index {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.25rem;
  height: 1.25rem;
  border-radius: var(--radius-full);
  background: var(--surface-sunken);
  font-family: var(--font-mono);
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.field__label {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.hint--danger {
  color: var(--danger-strong);
}

.hint--warning {
  color: var(--warning-strong);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

.key {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.key__hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.key__block {
  margin: 0;
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  white-space: pre-wrap;
  word-break: break-all;
}

.repo-form {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-2);
  align-items: center;
}

.repos {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.repos__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.repos__text {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.timeline {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding-left: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-caption);
}

.timeline__done {
  color: var(--success-strong);
}

.stage {
  margin: 0;
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.section {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.section__title {
  margin: 0;
  font-size: var(--font-size-h4);
  font-weight: var(--weight-semibold);
}

.section__body {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-primary);
}

.check {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-small);
  color: var(--text-primary);
}

.evidence {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

@media (max-width: 900px) {
  .repo-form {
    grid-template-columns: 1fr;
  }
}
</style>