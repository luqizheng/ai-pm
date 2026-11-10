<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { Check, Pencil, RefreshCw, SearchX } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import PageFrame from "@/components/shell/PageFrame.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import {
  decideCognitionItem,
  listProjectKnowledge,
  loadCognitionOverview,
  startCognitionRun,
  type CognitionDecision,
  type CognitionDomain,
  type CognitionItem,
  type CognitionOverview,
  type ProjectKnowledge
} from "@/api/cognition";

/**
 * S01-04 · AI-PM 认知（Project Cognition）。
 *
 * 定位：项目级长期认知引擎（设计稿 V1.1，五裁决 Q1~Q5 已收编）。
 * 页面三区：Runs（历史 + 人工启动）/ Review（逐项确认，AI 提案 ≠ 正式知识）/
 * Knowledge（已确认项目知识，带确认人/时间）。
 *
 * V1 后端契约：Run 由人类启动后以结构化事实派生条目（pm-api pm_cognition.rs）；
 * 真模型编排由 ai-pm-worker 后续切片接管，本页不虚构分析输出。
 */

const { t } = useI18n();
const route = useRoute();
const scope = useScopeStore();

const overview = ref<CognitionOverview | null>(null);
const knowledge = ref<ProjectKnowledge[]>([]);
const loading = ref(true);
const errorMessage = ref("");
const starting = ref(false);
const decidingId = ref("");
const editDraft = ref("");
const editingId = ref("");

const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  if (scope.currentProjectId && scope.currentProjectId !== SCOPE_ALL) {
    return scope.currentProjectId;
  }
  return "";
});

const proposedItems = computed(
  () => overview.value?.items.filter(item => item.state === "proposed") ?? []
);

const domainTag = (domain: CognitionDomain) =>
  t(`s01.cognition.domain.${domain}`);

async function load() {
  loading.value = true;
  errorMessage.value = "";
  try {
    if (!projectId.value) return;
    overview.value = await loadCognitionOverview(projectId.value);
    knowledge.value = (await listProjectKnowledge(projectId.value)).items;
  } catch (error) {
    overview.value = null;
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

async function startRun() {
  if (!projectId.value || starting.value) return;
  starting.value = true;
  errorMessage.value = "";
  try {
    await startCognitionRun(projectId.value);
    await load();
  } catch (error) {
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    starting.value = false;
  }
}

function beginEdit(item: CognitionItem) {
  editingId.value = item.id;
  editDraft.value = item.content;
}

function cancelEdit() {
  editingId.value = "";
  editDraft.value = "";
}

async function decide(
  item: CognitionItem,
  action: CognitionDecision,
  content?: string
) {
  if (!projectId.value || decidingId.value) return;
  decidingId.value = item.id;
  try {
    await decideCognitionItem(projectId.value, item.id, {
      action,
      content: action === "confirm" ? content : undefined
    });
    editingId.value = "";
    editDraft.value = "";
    await load();
  } finally {
    decidingId.value = "";
  }
}

onMounted(load);

// 项目上下文变化（壳切项目 / 路由带入 project_id）必须重载——
// 该页读模型是项目级的，不重载会停留在上一个项目的数据。
watch(projectId, load);
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s01.cognition.breadcrumb.project') },
      { label: t('s01.cognition.breadcrumb.cognition') }
    ]"
    :title="t('s01.cognition.page.title')"
    :description="t('s01.cognition.page.description')"
  >
    <template #actions>
      <Button
        variant="primary"
        size="sm"
        :loading="starting"
        :disabled="!projectId"
        data-testid="cognition-start"
        @click="startRun"
      >
        <RefreshCw v-if="!starting" :size="14" aria-hidden="true" />
        {{ t("s01.cognition.action.start") }}
      </Button>
    </template>

    <!-- 加载失败：失败不得显示为空 -->
    <Alert
      v-if="errorMessage"
      variant="danger"
      :title="t('s01.cognition.state.loadFailed.title')"
    >
      {{ errorMessage }}
    </Alert>

    <div v-else-if="loading" class="cog-page__loading">
      {{ t("s01.cognition.state.loading") }}
    </div>

    <EmptyState
      v-else-if="!projectId"
      :title="t('s01.cognition.state.noProject.title')"
      :description="t('s01.cognition.state.noProject.description')"
    />

    <template v-else-if="overview">
      <!-- ① 概览统计 -->
      <section class="cog-page__stats" data-testid="cognition-stats">
        <div class="cog-page__stat">
          <b>{{ overview.knowledge_active }}</b>
          <span>{{ t("s01.cognition.stat.knowledge") }}</span>
        </div>
        <div class="cog-page__stat">
          <b>{{ overview.proposed }}</b>
          <span>{{ t("s01.cognition.stat.proposed") }}</span>
        </div>
        <div class="cog-page__stat">
          <b>{{ overview.runs.length }}</b>
          <span>{{ t("s01.cognition.stat.runs") }}</span>
        </div>
      </section>

      <!-- ② 认知 Run 历史（每次执行独立保留，§10.2） -->
      <section class="cog-page__card">
        <h2>{{ t("s01.cognition.runs.title") }}</h2>
        <p v-if="!overview.runs.length" class="cog-page__hint">
          {{ t("s01.cognition.runs.empty") }}
        </p>
        <ul v-else class="cog-page__runs">
          <li
            v-for="run in overview.runs"
            :key="run.id"
            data-testid="cognition-run"
          >
            <Badge
              :variant="run.status === 'completed' ? 'info' : 'neutral'"
              size="sm"
            >
              {{ t(`s01.cognition.runs.status.${run.status}`) }}
            </Badge>
            <span class="cog-page__run-meta">
              {{
                t("s01.cognition.runs.meta", {
                  confirmed: run.confirmed_count,
                  total: run.item_count
                })
              }}
              · {{ new Date(run.created_at).toLocaleString() }}
            </span>
          </li>
        </ul>
      </section>

      <!-- ③ 逐项审核（Q3-B：每条独立确认，AI 提案不自动成为知识） -->
      <section class="cog-page__card">
        <h2>{{ t("s01.cognition.review.title") }}</h2>
        <p class="cog-page__hint">
          {{ t("s01.cognition.review.hint") }}
        </p>
        <EmptyState
          v-if="!overview.items.length"
          :title="t('s01.cognition.review.empty.title')"
          :description="t('s01.cognition.review.empty.description')"
        />
        <ul v-else class="cog-page__items">
          <li
            v-for="item in overview.items"
            :key="item.id"
            class="cog-page__item"
            :data-state="item.state"
            data-testid="cognition-item"
          >
            <div class="cog-page__item-head">
              <Badge variant="neutral" size="sm">{{
                domainTag(item.domain)
              }}</Badge>
              <Badge variant="neutral" size="sm">
                {{ t(`s01.cognition.confidence.${item.confidence}`) }}
              </Badge>
              <Badge
                :variant="
                  item.state === 'confirmed'
                    ? 'info'
                    : item.state === 'rejected'
                      ? 'danger'
                      : 'neutral'
                "
                size="sm"
              >
                {{ t(`s01.cognition.itemState.${item.state}`) }}
              </Badge>
            </div>
            <template v-if="editingId === item.id">
              <Textarea
                v-model="editDraft"
                :label="t('s01.cognition.review.editLabel')"
              />
              <div class="cog-page__item-actions">
                <Button
                  variant="primary"
                  size="sm"
                  :disabled="decidingId === item.id || !editDraft.trim()"
                  :data-testid="`cognition-confirm-edited-${item.id}`"
                  @click="decide(item, 'confirm', editDraft.trim())"
                >
                  <Check :size="14" aria-hidden="true" />
                  {{ t("s01.cognition.action.confirmEdited") }}
                </Button>
                <Button variant="ghost" size="sm" @click="cancelEdit">
                  {{ t("s01.cognition.action.cancelEdit") }}
                </Button>
              </div>
            </template>
            <template v-else>
              <p class="cog-page__item-content">{{ item.content }}</p>
              <p class="cog-page__item-evidence">
                {{ t("s01.cognition.review.evidence") }}:
                {{ String(item.evidence?.type ?? "—") }}
              </p>
              <div
                v-if="item.state === 'proposed'"
                class="cog-page__item-actions"
              >
                <Button
                  variant="primary"
                  size="sm"
                  :loading="decidingId === item.id"
                  :data-testid="`cognition-confirm-${item.id}`"
                  @click="decide(item, 'confirm')"
                >
                  <Check :size="14" aria-hidden="true" />
                  {{ t("s01.cognition.action.confirm") }}
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  :disabled="Boolean(decidingId)"
                  @click="beginEdit(item)"
                >
                  <Pencil :size="14" aria-hidden="true" />
                  {{ t("s01.cognition.action.editConfirm") }}
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  :disabled="Boolean(decidingId)"
                  @click="decide(item, 'reject')"
                >
                  <SearchX :size="14" aria-hidden="true" />
                  {{ t("s01.cognition.action.reject") }}
                </Button>
              </div>
            </template>
          </li>
        </ul>
      </section>

      <!-- ④ 已确认知识（正式项目依据） -->
      <section class="cog-page__card">
        <h2>{{ t("s01.cognition.knowledge.title") }}</h2>
        <p v-if="!knowledge.length" class="cog-page__hint">
          {{ t("s01.cognition.knowledge.empty") }}
        </p>
        <ul v-else class="cog-page__items">
          <li
            v-for="entry in knowledge"
            :key="entry.id"
            class="cog-page__item"
            data-testid="cognition-knowledge"
          >
            <div class="cog-page__item-head">
              <Badge variant="neutral" size="sm">{{
                domainTag(entry.domain)
              }}</Badge>
              <Badge variant="info" size="sm">v{{ entry.version }}</Badge>
            </div>
            <p class="cog-page__item-content">{{ entry.content }}</p>
          </li>
        </ul>
      </section>
    </template>
  </PageFrame>
</template>

<style scoped>
.cog-page__loading {
  padding: var(--space-10);
  color: var(--ws-text-secondary);
}

.cog-page__stats {
  display: flex;
  gap: var(--space-8);
}

.cog-page__stat {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.cog-page__stat b {
  color: var(--ws-text-primary);
  font-size: var(--font-size-h4);
}

.cog-page__stat span {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.cog-page__card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.cog-page__card h2 {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.cog-page__hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.cog-page__runs,
.cog-page__items {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.cog-page__runs li {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.cog-page__run-meta {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.cog-page__item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  background: var(--ws-canvas);
}

.cog-page__item[data-state="rejected"] {
  opacity: 0.6;
}

.cog-page__item-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.cog-page__item-content {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.cog-page__item-evidence {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.cog-page__item-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}
</style>
