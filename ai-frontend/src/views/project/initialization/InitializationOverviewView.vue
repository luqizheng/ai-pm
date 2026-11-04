<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { CircleAlert, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Descriptions from "@/components/ui/descriptions/Descriptions.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import PageFrame from "@/components/shell/PageFrame.vue";
import { useShellStore } from "@/stores/shell";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import {
  loadInitializationOverview,
  type CurrentAction,
  type HandoverOutcome,
  type HumanAction,
  type InitStage,
  type InitializationOverview,
  type StageId
} from "@/api/initialization-overview";
import InitAiContext from "./components/InitAiContext.vue";
import InitCurrentAction from "./components/InitCurrentAction.vue";
import InitHumanActions from "./components/InitHumanActions.vue";
import InitStageCard from "./components/InitStageCard.vue";

/**
 * S01-01 · 项目初始化总览（Project Initialization Overview）。
 *
 * 定位：**项目创建之后、正式交接之前**的项目级初始化总控视图。
 *
 * 数据来源：已上线的 `GET /v1/pm/projects/{project_id}/command-center` 聚合读模型。
 * 本页**不新建事实来源、不新建初始化状态机**，只做阶段化呈现与导航。
 *
 * 设计稿：docs/ui/s01/s01-01-project-initialization-overview-ux-v1.md
 */

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const scope = useScopeStore();

const overview = ref<InitializationOverview | null>(null);
const loading = ref(true);
const errorMessage = ref("");
const projectId = ref("");

/**
 * 右上下文栏：页面自带 AI 上下文栏（只读），进入本页时收起壳级右栏，
 * 离开时恢复原状；用户在壳内重新打开右栏时本页让位（不并排两栏）。
 */
let previousContextOpen: boolean | null = null;

const showRail = computed(() => !shell.contextOpen);

const summary = computed(() => overview.value?.projectSummary);
const archived = computed(() => summary.value?.archived === true);

const stages = computed(() => overview.value?.stages ?? []);

const stageTitleMap = computed<Record<StageId, string>>(() => {
  const map = {} as Record<StageId, string>;
  for (const stage of stages.value) {
    map[stage.id] = t(`s01.initialization-overview.${stage.titleKey}.title`);
  }
  return map;
});

const dependencyLabels = (stage: InitStage) =>
  stage.dependsOn.map(id => stageTitleMap.value[id]).filter(Boolean);

const handoverText = computed(() => {
  const outcome: HandoverOutcome =
    overview.value?.handoverOutcome ?? "undetermined";
  return t(`s01.initialization-overview.handover.${outcome}`);
});

const handoverHint = computed(() => {
  const outcome: HandoverOutcome =
    overview.value?.handoverOutcome ?? "undetermined";
  if (outcome === "undetermined") {
    return t("s01.initialization-overview.handover.undeterminedHint");
  }
  if (outcome === "blocked") {
    return t("s01.initialization-overview.handover.blockedHint");
  }
  return "";
});

const summaryItems = computed(() => {
  const data = summary.value;
  if (!data) return [];
  return [
    { label: t("s01.initialization-overview.project.code"), value: data.code },
    {
      label: t("s01.initialization-overview.project.lifecycle"),
      value: t(
        `s01.initialization-overview.lifecycle.${data.lifecycle}`,
        data.lifecycle
      )
    },
    {
      label: t("s01.initialization-overview.project.customer"),
      value: data.customer_name ?? "-"
    },
    {
      label: t("s01.initialization-overview.project.repositories"),
      value: t("s01.initialization-overview.project.repositoryCount", {
        total: data.repository_count,
        verified: data.verified_repository_count
      })
    },
    {
      label: t("s01.initialization-overview.project.members"),
      value: String(data.member_count)
    },
    {
      label: t("s01.initialization-overview.project.goal"),
      value: data.goal.present
        ? t("s01.initialization-overview.project.goalPresent")
        : t("s01.initialization-overview.project.goalMissing")
    }
  ];
});

/** 项目作用域解析：路由参数 → 已选项目 → 项目列表首个 */
async function resolveProjectId(): Promise<string> {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;

  if (scope.currentProjectId && scope.currentProjectId !== SCOPE_ALL) {
    return scope.currentProjectId;
  }

  try {
    await scope.ensureLoaded();
  } catch {
    return "";
  }
  return scope.projects[0]?.id ?? "";
}

async function load() {
  loading.value = true;
  errorMessage.value = "";
  try {
    projectId.value = await resolveProjectId();
    if (!projectId.value) {
      overview.value = null;
      return;
    }
    overview.value = await loadInitializationOverview(projectId.value);
  } catch (error) {
    overview.value = null;
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

function openStage(stage: InitStage) {
  if (!stage.route) return;
  router.push(stage.route);
}

function openCurrentAction(_action: CurrentAction) {
  const stage = stages.value.find(
    item => item.id === overview.value?.currentAction.stageId
  );
  if (stage?.route) router.push(stage.route);
}

function openHumanAction(action: HumanAction) {
  const routeByDomain: Record<string, string> = {
    discovery: "/project/discovery",
    knowledge: "/project/knowledge",
    delivery: "/project/execution",
    repository: "/project/repository/overview"
  };
  const target = routeByDomain[action.domain];
  if (target) router.push(target);
}

onMounted(async () => {
  previousContextOpen = shell.contextOpen;
  shell.setContextOpen(false);
  await load();
});

onBeforeUnmount(() => {
  if (previousContextOpen !== null) shell.setContextOpen(previousContextOpen);
});

watch(
  () => route.query.project_id,
  () => {
    if (route.query.project_id) load();
  }
);
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s01.initialization-overview.page.breadcrumbProject') },
      { label: t('s01.initialization-overview.page.breadcrumbOverview') }
    ]"
    :title="t('s01.initialization-overview.page.title')"
    :description="t('s01.initialization-overview.page.description')"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("s01.initialization-overview.page.reload") }}
      </Button>
      <Button variant="secondary" size="sm" @click="router.push('/dashboard')">
        {{ t("s01.initialization-overview.page.openConsole") }}
      </Button>
    </template>

    <!-- 归档只读 -->
    <Alert
      v-if="archived"
      variant="warning"
      :title="t('s01.initialization-overview.project.archivedTitle')"
    >
      {{ t("s01.initialization-overview.project.archivedHint") }}
    </Alert>

    <!-- 载入中 -->
    <div v-else-if="loading" class="init-page__loading">
      <p class="init-page__loading-text">
        {{ t("s01.initialization-overview.page.loading") }}
      </p>
    </div>

    <!-- 加载失败：失败不得被显示成「没有待办事项」 -->
    <Alert
      v-else-if="errorMessage"
      variant="danger"
      :title="t('s01.initialization-overview.page.loadFailed')"
    >
      <p class="init-page__error">
        {{ t("s01.initialization-overview.page.loadFailedHint") }}
      </p>
      <p v-if="errorMessage" class="init-page__error-code">
        {{ errorMessage }}
      </p>
      <Button variant="secondary" size="sm" @click="load">
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("s01.initialization-overview.page.retry") }}
      </Button>
    </Alert>

    <!-- 未选项目 -->
    <EmptyState
      v-else-if="!overview"
      :title="t('s01.initialization-overview.page.noProjectTitle')"
      :description="t('s01.initialization-overview.page.noProjectHint')"
      :action-label="t('s01.initialization-overview.page.reload')"
      @action="load"
    />

    <div
      v-else-if="overview"
      class="init-page__grid"
      :class="{ 'has-rail': showRail }"
    >
      <div class="init-page__main">
        <!-- ① 项目身份 -->
        <section class="init-identity" data-testid="init-identity">
          <div class="init-identity__head">
            <h2 class="init-identity__name">{{ summary?.name }}</h2>
            <Badge variant="neutral" size="sm">{{ summary?.code }}</Badge>
            <Badge
              v-if="summary"
              :variant="summary.lifecycle === 'archived' ? 'danger' : 'info'"
              size="sm"
            >
              {{
                t(
                  `s01.initialization-overview.lifecycle.${summary.lifecycle}`,
                  summary.lifecycle
                )
              }}
            </Badge>
          </div>
          <Descriptions :items="summaryItems" :columns="2" />
        </section>

        <!-- ③ 当前最重要的一件事 -->
        <InitCurrentAction
          :action="overview.currentAction"
          @open="openCurrentAction"
        />

        <!-- ② 六阶段进度 -->
        <section class="init-stages" data-testid="init-stages">
          <header class="init-stages__header">
            <h2 class="init-stages__heading">
              {{ t("s01.initialization-overview.stages.title") }}
            </h2>
            <p class="init-stages__hint">
              {{ t("s01.initialization-overview.stages.hint") }}
            </p>
          </header>

          <div class="init-stages__grid">
            <InitStageCard
              v-for="stage in stages"
              :key="stage.id"
              :stage="stage"
              :dependency-labels="dependencyLabels(stage)"
              @open="openStage"
            />
          </div>

          <!-- ⑥ 交接出口 -->
          <div class="init-handover" :data-outcome="overview.handoverOutcome">
            <div class="init-handover__body">
              <p class="init-handover__title">{{ handoverText }}</p>
              <p v-if="handoverHint" class="init-handover__hint">
                {{ handoverHint }}
              </p>
            </div>
            <Button
              variant="secondary"
              size="sm"
              :disabled="overview.handoverOutcome === 'blocked'"
              @click="router.push('/dashboard')"
            >
              {{ t("s01.initialization-overview.handover.open") }}
            </Button>
          </div>
        </section>

        <!-- ④ 待人工处理 -->
        <InitHumanActions
          :actions="overview.humanActions"
          @open="openHumanAction"
        />
      </div>

      <!-- ⑤ AI 上下文（只读） -->
      <InitAiContext
        v-if="showRail"
        class="init-page__rail"
        :items="overview.aiContext"
      />
    </div>
  </PageFrame>
</template>

<style scoped>
.init-page__grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: var(--space-5);
  align-items: start;
}

/* 页面自带 AI 上下文栏时，右侧固定 320px（与壳级右栏同宽） */
.init-page__grid.has-rail {
  grid-template-columns: minmax(0, 1fr) var(--ws-context-width);
}

.init-page__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  min-width: 0;
}

.init-page__rail {
  position: sticky;
  top: var(--space-6);
}

.init-page__loading {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: var(--size-row);
  padding: var(--space-10);
}

.init-page__loading-text {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

.init-page__error {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.init-page__error-code {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-disabled);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

/* 项目身份区 */
.init-identity {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.init-identity__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.init-identity__name {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 六阶段 */
.init-stages {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.init-stages__header {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
}

.init-stages__heading {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.init-stages__hint {
  margin: 0;
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
}

.init-stages__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: var(--space-3);
}

/* 交接出口 */
.init-handover {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.init-handover__body {
  flex: 1;
  min-width: 0;
}

.init-handover__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-medium);
}

.init-handover__hint {
  margin: var(--space-1) 0 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.init-handover.is-blocked {
  box-shadow: inset var(--size-indicator) 0 0 var(--ws-color-danger);
}

/* 宽屏（≥1600px）才把 AI 上下文栏并排到主区右侧 */
/* 窄屏：右栏下沉到主区末尾（设计系统 §四：长内容建议隐藏右栏，由用户决定） */
@media (max-width: 1180px) {
  .init-page__grid.has-rail {
    grid-template-columns: minmax(0, 1fr);
  }

  .init-page__rail {
    position: static;
  }
}
</style>
