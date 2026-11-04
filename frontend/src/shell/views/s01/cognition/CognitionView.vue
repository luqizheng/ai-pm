<script setup lang="ts">
/**
 * S01-04 · AI-PM 认知。
 *
 * UX 依据：`docs/ui/s01/s01-04-ai-pm-cognition-ux-v1-1.md`
 *
 * 硬语义：
 *   - Cognition（AI 认知结果）≠ Knowledge（人类确认后的正式知识）。
 *   - 逐项确认：每个 Cognition Item 独立处理，不要求整个 Run 一次性确认。
 *   - Run 完成 ≠ 全部认知已确认。
 *   - 未确认的 Cognition 不得默认进入 Agent Context。
 *   - Knowledge 的变更只能通过变更提案由人类决定。
 */
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/shell/components";
import { Tabs, type TabItem } from "@/components/ui/tabs";
import { useConfirm } from "@/shell/composables/useConfirm";
import { formatDateTime } from "@/api/format";
import {
  decideCognitionItem,
  getAgentContext,
  getCognitionPreparation,
  listCognitionItems,
  listCognitionRuns,
  listProjectKnowledge,
  setCoreKnowledge,
  startCognitionRun
} from "@/api/initialization";
import type {
  AgentContext,
  CognitionDomain,
  CognitionItem,
  CognitionPreparation,
  CognitionRun,
  ProjectKnowledge
} from "@/api/types/initialization";

const { t } = useI18n();
const route = useRoute();
const toast = useToast();
const confirm = useConfirm();

const projectId = computed(() => String(route.params.projectId ?? ""));

const tab = ref<"overview" | "runs" | "review" | "knowledge" | "context">(
  "overview"
);

const tabs = computed<TabItem[]>(() => [
  { value: "overview", label: t("s01.initialization.cognition.overview") },
  { value: "runs", label: t("s01.initialization.cognition.runs") },
  { value: "review", label: t("s01.initialization.cognition.review") },
  { value: "knowledge", label: t("s01.initialization.cognition.knowledge") },
  { value: "context", label: t("s01.initialization.cognition.agentContext") }
]);

const preparation = ref<CognitionPreparation | null>(null);
const runs = ref<CognitionRun[]>([]);
const items = ref<CognitionItem[]>([]);
const knowledge = ref<ProjectKnowledge[]>([]);
const agentContext = ref<AgentContext | null>(null);
const loading = ref(false);
const busy = ref(false);

/** reka-ui Select 不接受空字符串作为 item value，用哨兵表示「全部领域」。 */
const ALL_DOMAINS = "__all__";

const DOMAINS: CognitionDomain[] = [
  "project_identity",
  "repository_understanding",
  "domain_understanding",
  "architecture_understanding",
  "development_convention",
  "project_evolution"
];

const domainFilter = ref<string>(ALL_DOMAINS);
const itemStatusFilter = ref<string>("proposed");

const domainOptions = computed(() => [
  { value: ALL_DOMAINS, label: t("s01.initialization.common.empty") },
  ...DOMAINS.map(value => ({
    value,
    label: t(`s01.initialization.cognition.domain.${value}`)
  }))
]);

const itemStatusOptions = computed(() => [
  {
    value: "proposed",
    label: t("s01.initialization.cognition.itemStatus.proposed")
  },
  {
    value: "confirmed",
    label: t("s01.initialization.cognition.itemStatus.confirmed")
  },
  {
    value: "rejected",
    label: t("s01.initialization.cognition.itemStatus.rejected")
  },
  {
    value: "investigation_required",
    label: t("s01.initialization.cognition.itemStatus.investigation_required")
  },
  {
    value: "deferred",
    label: t("s01.initialization.cognition.itemStatus.deferred")
  }
]);

const pendingItems = computed(
  () => items.value.filter(item => item.status === "proposed").length
);

const activeKnowledge = computed(
  () => knowledge.value.filter(item => item.status === "active").length
);

const coreKnowledge = computed(() =>
  knowledge.value.filter(item => item.is_core)
);

/** 批量只选无冲突条目（UX §6.2：冲突条目不得混入普通批量确认）。 */
const selectableIds = computed(() =>
  items.value
    .filter(item => item.status === "proposed" && !item.has_conflict)
    .map(item => item.id)
);

const selectedIds = ref<string[]>([]);

function isSelected(id: string) {
  return selectedIds.value.includes(id);
}

function toggleSelect(id: string) {
  const index = selectedIds.value.indexOf(id);
  if (index >= 0) {
    selectedIds.value.splice(index, 1);
  } else {
    selectedIds.value.push(id);
  }
}

function sourceVariant(
  type: string
): "success" | "info" | "warning" | "neutral" {
  if (type === "code_fact" || type === "git_commit") return "success";
  if (type === "doc_fact") return "info";
  return "warning";
}

function failure(error: unknown) {
  toast.toast({
    variant: "danger",
    title: t("s01.initialization.common.operationFailed"),
    description: String(error)
  });
}

async function load() {
  loading.value = true;
  try {
    const [prep, runResult, itemResult, knowledgeResult] = await Promise.all([
      getCognitionPreparation(projectId.value),
      listCognitionRuns(projectId.value, { size: 50 }),
      listCognitionItems(projectId.value, {
        status: itemStatusFilter.value || undefined,
        domain:
          domainFilter.value === ALL_DOMAINS ? undefined : domainFilter.value,
        size: 100
      }),
      listProjectKnowledge(projectId.value, { size: 100 })
    ]);
    preparation.value = prep;
    runs.value = runResult.items ?? [];
    items.value = itemResult.items ?? [];
    knowledge.value = knowledgeResult.items ?? [];
  } catch (error) {
    failure(error);
  } finally {
    loading.value = false;
  }
}

async function loadContext() {
  try {
    agentContext.value = await getAgentContext(projectId.value);
  } catch (error) {
    failure(error);
  }
}

/** 人工启动：启动前展示范围、仓库与资料，用户确认后才创建 Run。 */
async function startRun() {
  const prep = preparation.value;
  if (!prep) return;
  const ok = await confirm.confirm({
    title: t("s01.initialization.cognition.startRunTitle"),
    description: [
      t("s01.initialization.cognition.startRunHint"),
      `${t("s01.initialization.cognition.startRunScope")}: ${prep.analysis_scope
        .map(d => t(`s01.initialization.cognition.domain.${d}`))
        .join(", ")}`,
      `${t("s01.initialization.cognition.startRunRepositories")}: ${
        prep.repositories.length
      }`,
      `${t("s01.initialization.cognition.startRunMaterials")}: ${prep.collected_count}`,
      `${t("s01.initialization.cognition.startRunMissing")}: ${
        prep.missing_materials.length
      }`
    ].join("\n"),
    confirmLabel: t("s01.initialization.cognition.startRunConfirm")
  });
  if (!ok) return;

  busy.value = true;
  try {
    await startCognitionRun(projectId.value, {
      scope: prep.analysis_scope,
      repository_ids: prep.repositories.map(r => r.repository_id)
    });
    toast.toast({
      variant: "success",
      title: t("s01.initialization.cognition.runStarted")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busy.value = false;
  }
}

/** 逐项处理：不提供「全部通过」，每条独立留痕。 */
async function decide(item: CognitionItem, action: string) {
  let content: string | undefined;
  let note: string | undefined;

  if (action === "edit_and_confirm") {
    content = window.prompt(
      t("s01.initialization.cognition.editContent"),
      item.content
    );
    if (content === null) return;
  }
  if (
    action === "reject" ||
    action === "defer" ||
    action === "request_investigation"
  ) {
    note = window.prompt(t("s01.initialization.cognition.decisionNote"));
    if (note === null) return;
  }

  busy.value = true;
  try {
    await decideCognitionItem(projectId.value, item.id, {
      action,
      content,
      note
    });
    const messages: Record<string, string> = {
      confirm: "s01.initialization.cognition.itemConfirmed",
      edit_and_confirm: "s01.initialization.cognition.itemConfirmed",
      reject: "s01.initialization.cognition.itemRejected",
      defer: "s01.initialization.cognition.itemDeferred",
      request_investigation: "s01.initialization.cognition.itemInvestigation"
    };
    toast.toast({
      variant: "success",
      title: t(messages[action] ?? "s01.initialization.common.operationFailed")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busy.value = false;
  }
}

/** 批量确认：逐条提交，语义与单条一致。 */
async function confirmSelected() {
  const ids = [...selectedIds.value];
  if (ids.length === 0) return;
  busy.value = true;
  try {
    for (const id of ids) {
      await decideCognitionItem(projectId.value, id, { action: "confirm" });
    }
    selectedIds.value = [];
    toast.toast({
      variant: "success",
      title: t("s01.initialization.cognition.itemConfirmed")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busy.value = false;
  }
}

async function toggleCore(item: ProjectKnowledge) {
  busy.value = true;
  try {
    await setCoreKnowledge(projectId.value, item.id, {
      is_core: !item.is_core
    });
    toast.toast({
      variant: "success",
      title: item.is_core
        ? t("s01.initialization.cognition.coreUnset")
        : t("s01.initialization.cognition.coreSet")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  await load();
  await loadContext();
});
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('s01.initialization.cognition.pageTitle')"
      :subtitle="t('s01.initialization.cognition.subtitle')"
    >
      <template #actions>
        <Button size="sm" :loading="busy" @click="startRun">
          {{ t("s01.initialization.cognition.startRun") }}
        </Button>
      </template>
    </PageHeader>

    <Tabs
      v-model="tab"
      :tabs="tabs"
      :aria-label="t('s01.initialization.cognition.pageTitle')"
    />

    <!-- 总览 -->
    <section v-if="tab === 'overview'" class="ui-page__body">
      <div class="metrics">
        <div class="metric">
          <p class="metric__label">
            {{ t("s01.initialization.cognition.activeKnowledge") }}
          </p>
          <p class="metric__value">{{ activeKnowledge }}</p>
        </div>
        <div class="metric">
          <p class="metric__label">
            {{ t("s01.initialization.cognition.pendingItems") }}
          </p>
          <p class="metric__value">{{ pendingItems }}</p>
        </div>
        <div class="metric">
          <p class="metric__label">
            {{ t("s01.initialization.cognition.coverage") }}
          </p>
          <p class="metric__value">
            {{ preparation?.repositories.length ?? 0 }}
          </p>
        </div>
        <div class="metric">
          <p class="metric__label">
            {{ t("s01.initialization.cognition.lastRun") }}
          </p>
          <p class="metric__value metric__value--sm">
            {{ runs[0] ? formatDateTime(runs[0].created_at) : "—" }}
          </p>
        </div>
      </div>

      <section class="ui-card">
        <h2 class="ui-card__title">
          {{ t("s01.initialization.cognition.preparation") }}
        </h2>

        <Alert v-if="!preparation?.project_info_complete" variant="warning">
          {{ t("s01.initialization.cognition.projectInfoIncomplete") }}
        </Alert>
        <Alert
          v-if="(preparation?.repositories.length ?? 0) === 0"
          variant="info"
        >
          {{ t("s01.initialization.cognition.noRepositoryHint") }}
        </Alert>

        <dl class="facts">
          <div class="facts__row">
            <dt>{{ t("s01.initialization.cognition.collectedCount") }}</dt>
            <dd>{{ preparation?.collected_count ?? 0 }}</dd>
          </div>
          <div class="facts__row">
            <dt>{{ t("s01.initialization.cognition.startRunScope") }}</dt>
            <dd>
              <span
                v-for="domain in preparation?.analysis_scope ?? []"
                :key="domain"
                class="chip"
              >
                {{ t(`s01.initialization.cognition.domain.${domain}`) }}
              </span>
            </dd>
          </div>
          <div v-if="preparation?.missing_materials.length" class="facts__row">
            <dt>{{ t("s01.initialization.cognition.missingMaterials") }}</dt>
            <dd>
              <span
                v-for="missing in preparation.missing_materials"
                :key="missing.code"
                class="chip chip--warn"
              >
                {{ missing.message }}
              </span>
            </dd>
          </div>
        </dl>
      </section>
    </section>

    <!-- 执行记录 -->
    <section v-else-if="tab === 'runs'" class="ui-page__body">
      <p v-if="runs.length === 0" class="ui-card__empty">
        {{ t("s01.initialization.common.empty") }}
      </p>
      <ul v-else class="run-list">
        <li v-for="run in runs" :key="run.id" class="run">
          <header class="run__head">
            <span class="run__scope">
              {{
                run.scope
                  .map(d => t(`s01.initialization.cognition.domain.${d}`))
                  .join(" · ")
              }}
            </span>
            <Badge
              :variant="
                run.status === 'completed'
                  ? 'success'
                  : run.status === 'failed'
                    ? 'danger'
                    : run.status === 'cancelled'
                      ? 'neutral'
                      : 'warning'
              "
            >
              {{ t(`s01.initialization.cognition.runStatus.${run.status}`) }}
            </Badge>
          </header>
          <dl class="facts">
            <div class="facts__row">
              <dt>{{ t("s01.initialization.cognition.runItems") }}</dt>
              <dd>{{ run.item_count }}</dd>
            </div>
            <div class="facts__row">
              <dt>{{ t("s01.initialization.cognition.runProgress") }}</dt>
              <dd>{{ run.confirmed_count }} / {{ run.item_count }}</dd>
            </div>
            <div class="facts__row">
              <dt>{{ t("s01.initialization.cognition.repositories") }}</dt>
              <dd>
                <span
                  v-for="repo in run.repository_baselines"
                  :key="repo.repository_id"
                  class="chip"
                >
                  {{ repo.repository_name }} @ {{ repo.commit_sha ?? "—" }}
                </span>
              </dd>
            </div>
          </dl>
        </li>
      </ul>
    </section>

    <!-- 逐条审核 -->
    <section v-else-if="tab === 'review'" class="ui-page__body">
      <div class="filters">
        <Select
          v-model="itemStatusFilter"
          :options="itemStatusOptions"
          :aria-label="t('s01.initialization.cognition.itemStatus.proposed')"
          @update:model-value="load"
        />
        <Select
          v-model="domainFilter"
          :options="domainOptions"
          :aria-label="
            t('s01.initialization.cognition.domain.project_identity')
          "
          @update:model-value="load"
        />
        <Button
          v-if="selectedIds.length > 0"
          size="sm"
          :loading="busy"
          @click="confirmSelected"
        >
          {{ t("s01.initialization.cognition.batchConfirm") }}
          （{{
            t("s01.initialization.cognition.batchSelected", {
              count: selectedIds.length
            })
          }}）
        </Button>
      </div>

      <p class="hint">{{ t("s01.initialization.cognition.batchHint") }}</p>

      <p v-if="items.length === 0" class="ui-card__empty">
        {{ t("s01.initialization.common.empty") }}
      </p>
      <ul v-else class="item-list">
        <li v-for="item in items" :key="item.id" class="item">
          <header class="item__head">
            <label v-if="selectableIds.includes(item.id)" class="item__check">
              <input
                type="checkbox"
                :checked="isSelected(item.id)"
                @change="toggleSelect(item.id)"
              />
            </label>
            <Badge variant="info">
              {{ t(`s01.initialization.cognition.domain.${item.domain}`) }}
            </Badge>
            <Badge
              :variant="
                item.has_conflict
                  ? 'danger'
                  : item.status === 'proposed'
                    ? 'warning'
                    : 'success'
              "
            >
              {{ t(`s01.initialization.cognition.itemStatus.${item.status}`) }}
            </Badge>
            <Badge variant="neutral">
              {{
                t(
                  `s01.initialization.cognition.evidenceType.${item.evidence_type}`
                )
              }}
            </Badge>
            <span class="item__confidence">
              {{ t("s01.initialization.cognition.itemConfidence") }}:
              {{ Math.round(item.confidence * 100) }}%
            </span>
          </header>

          <p class="item__content">{{ item.content }}</p>

          <div v-if="item.evidence.length > 0" class="item__evidence">
            <p class="item__evidenceLabel">
              {{ t("s01.initialization.cognition.itemEvidence") }}
            </p>
            <ul class="evidence-list">
              <li
                v-for="(source, index) in item.evidence"
                :key="index"
                class="evidence"
              >
                <Badge :variant="sourceVariant(source.source_type)">
                  {{
                    t(
                      `s01.initialization.cognition.evidenceType.${source.source_type}`
                    )
                  }}
                </Badge>
                <span>{{ source.label }}</span>
                <code v-if="source.locator">{{ source.locator }}</code>
              </li>
            </ul>
          </div>

          <p v-if="item.ai_rationale" class="item__rationale">
            {{ t("s01.initialization.cognition.itemRationale") }}:
            {{ item.ai_rationale }}
          </p>

          <div v-if="item.status === 'proposed'" class="item__actions">
            <Button size="sm" :disabled="busy" @click="decide(item, 'confirm')">
              {{ t("s01.initialization.cognition.confirmItem") }}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              :disabled="busy"
              @click="decide(item, 'edit_and_confirm')"
            >
              {{ t("s01.initialization.cognition.editAndConfirm") }}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              :disabled="busy"
              @click="decide(item, 'request_investigation')"
            >
              {{ t("s01.initialization.cognition.investigateItem") }}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              :disabled="busy"
              @click="decide(item, 'defer')"
            >
              {{ t("s01.initialization.cognition.deferItem") }}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              :disabled="busy"
              @click="decide(item, 'reject')"
            >
              {{ t("s01.initialization.cognition.rejectItem") }}
            </Button>
          </div>
        </li>
      </ul>
    </section>

    <!-- 正式知识 -->
    <section v-else-if="tab === 'knowledge'" class="ui-page__body">
      <p class="hint">{{ t("s01.initialization.cognition.coreHint") }}</p>
      <p v-if="knowledge.length === 0" class="ui-card__empty">
        {{ t("s01.initialization.common.empty") }}
      </p>
      <ul v-else class="item-list">
        <li v-for="item in knowledge" :key="item.id" class="item">
          <header class="item__head">
            <Badge variant="info">{{ item.category }}</Badge>
            <Badge
              :variant="
                item.status === 'active'
                  ? 'success'
                  : item.status === 'retired'
                    ? 'danger'
                    : 'neutral'
              "
            >
              {{
                t(`s01.initialization.cognition.knowledgeStatus.${item.status}`)
              }}
            </Badge>
            <Badge v-if="item.is_core" variant="info">
              {{ t("s01.initialization.cognition.contextBase") }}
            </Badge>
            <span class="item__confidence">
              {{ t("s01.initialization.cognition.knowledgeVersion") }}: v{{
                item.version
              }}
            </span>
          </header>
          <p class="item__content">{{ item.content }}</p>
          <dl class="facts">
            <div class="facts__row">
              <dt>
                {{ t("s01.initialization.cognition.knowledgeConfirmedBy") }}
              </dt>
              <dd>{{ item.confirmed_by_name ?? "—" }}</dd>
            </div>
            <div class="facts__row">
              <dt>
                {{ t("s01.initialization.cognition.knowledgeConfirmedAt") }}
              </dt>
              <dd>{{ formatDateTime(item.confirmed_at) }}</dd>
            </div>
          </dl>
          <div class="item__actions">
            <Button
              size="sm"
              variant="secondary"
              :disabled="busy"
              @click="toggleCore(item)"
            >
              {{
                item.is_core
                  ? t("s01.initialization.cognition.unsetCore")
                  : t("s01.initialization.cognition.setCore")
              }}
            </Button>
          </div>
        </li>
      </ul>
    </section>

    <!-- Agent Context -->
    <section v-else class="ui-page__body">
      <p class="hint">
        {{ t("s01.initialization.cognition.contextUnconfirmed") }}
      </p>

      <div class="context-grid">
        <section class="ui-card">
          <h3 class="ui-card__title">
            {{ t("s01.initialization.cognition.contextBase") }}
          </h3>
          <ul class="context-list">
            <li
              v-for="k in agentContext?.base_context.knowledge ?? []"
              :key="k.id"
            >
              {{ k.content }} <code>v{{ k.version }}</code>
            </li>
          </ul>
          <p class="ui-card__meta">
            {{ t("s01.initialization.cognition.coreKnowledge") }}:
            {{ coreKnowledge.length }}
          </p>
        </section>

        <section class="ui-card">
          <h3 class="ui-card__title">
            {{ t("s01.initialization.cognition.contextTask") }}
          </h3>
          <ul class="context-list">
            <li
              v-for="k in agentContext?.task_context.knowledge ?? []"
              :key="k.id"
            >
              {{ k.content }} <code>v{{ k.version }}</code>
            </li>
          </ul>
        </section>

        <section class="ui-card">
          <h3 class="ui-card__title">
            {{ t("s01.initialization.cognition.contextRepository") }}
          </h3>
          <p class="ui-card__meta">
            {{ agentContext?.repository_context.repository_id ?? "—" }} @
            {{ agentContext?.repository_context.branch ?? "—" }} @
            {{ agentContext?.repository_context.commit ?? "—" }}
          </p>
          <ul class="context-list">
            <li
              v-for="k in agentContext?.repository_context.knowledge ?? []"
              :key="k.id"
            >
              {{ k.content }} <code>v{{ k.version }}</code>
            </li>
          </ul>
        </section>

        <section class="ui-card">
          <h3 class="ui-card__title">
            {{ t("s01.initialization.cognition.contextExecution") }}
          </h3>
          <p class="ui-card__meta">
            {{ agentContext?.execution_context.run_id ?? "—" }} ·
            {{ agentContext?.execution_context.state ?? "—" }}
          </p>
          <p v-if="agentContext" class="ui-card__meta">
            {{ t("s01.initialization.cognition.contextGeneratedAt") }}:
            {{ formatDateTime(agentContext.generated_at) }}
          </p>
        </section>
      </div>
    </section>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.ui-page__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.ui-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.ui-card__title {
  margin: 0;
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ui-card__meta {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.ui-card__empty {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
}

.metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: var(--space-3);
}

.metric {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.metric__label {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.metric__value {
  margin: 0;
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.metric__value--sm {
  font-size: var(--font-size-small);
  font-weight: var(--weight-regular);
}

.filters {
  display: flex;
  gap: var(--space-3);
  align-items: center;
  flex-wrap: wrap;
}

.hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.facts {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.facts__row {
  display: flex;
  gap: var(--space-3);
  font-size: var(--font-size-small);
}

.facts__row dt {
  min-width: 140px;
  color: var(--text-secondary);
}

.facts__row dd {
  margin: 0;
  color: var(--text-primary);
}

.chip {
  display: inline-block;
  margin-right: var(--space-2);
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  background: var(--surface-sunken);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.chip--warn {
  color: var(--warning-strong);
}

.run-list,
.item-list,
.context-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.run,
.item {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
}

.run__head,
.item__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.run__scope {
  font-size: var(--font-size-small);
  color: var(--text-primary);
}

.item__check {
  display: inline-flex;
  align-items: center;
}

.item__confidence {
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.item__content {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-primary);
  white-space: pre-wrap;
}

.item__evidence {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.item__evidenceLabel {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.evidence-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.evidence {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.item__rationale {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.item__actions {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.context-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: var(--space-3);
}

.context-list li {
  font-size: var(--font-size-small);
  color: var(--text-primary);
}
</style>
