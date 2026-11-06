<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { Component } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  AlertTriangle,
  ChevronRight,
  CircleCheck,
  ClipboardList,
  GitBranch,
  Plus,
  Search
} from "lucide-vue-next";
import { PageFrame } from "@/components/shell";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import IconButton from "@/components/ui/icon-button/IconButton.vue";
import ListItem from "@/components/ui/list-item/ListItem.vue";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";
import { StatCard, type StatCardTone } from "@/components/ui/stat-card";

import {
  loadPersonalActivity,
  loadPersonalContext,
  loadPersonalInbox,
  loadPersonalProjects,
  type ActivityEntry,
  type AssignedTask,
  type HumanAction,
  type PersonalContext,
  type PersonalProject
} from "@/api/personal-home";

import ActivityEntryView from "./components/ActivityEntry.vue";
import AssignedTaskItem from "./components/AssignedTaskItem.vue";
import HumanActionItem from "./components/HumanActionItem.vue";
import ProjectCard from "./components/ProjectCard.vue";
import SectionState from "./components/SectionState.vue";

/**
 * 个人主页（Personal Home）—— 登录后的默认落地页。
 *
 * 结构（高保真稿 §3）：
 *   Row1  My Overview（三张 StatCard）
 *   Row2  My Inbox（60%） + My Projects（40%）
 *   Row3  Recent Activity（横向）
 *
 * 组件策略（AGENTS.md §三.11）：卡片/徽标/按钮/列表行全部组合公共 ui 组件
 * （Card / StatCard / Badge / Button / ListItem / StatusIndicator / EmptyState /
 * Alert / Skeleton），页内只保留纯布局样式；列表卡用 `pm-card--flush` 把
 * Card body 清零内边距，让行交互面（ListItem hover）通到卡片边缘。
 *
 * 状态处理（高保真稿 §8）：三个区块**各自独立**加载，
 * 因此「Inbox 失败」不会拖垮「我的项目」，失败也绝不渲染成"没有数据"。
 */
type Status = "loading" | "ready" | "error" | "forbidden";

/** Row1 概览单张 StatCard 的数据形状（icon + tone 决定语义色） */
type OverviewStat = {
  label: string;
  value: string;
  description: string;
  icon: Component;
  tone: StatCardTone;
};

const router = useRouter();
const { t } = useI18n();

const pageStatus = ref<Status>("loading");
const pageNote = ref("");

const inboxStatus = ref<Status>("loading");
const projectsStatus = ref<Status>("loading");
const activityStatus = ref<Status>("loading");

const context = ref<PersonalContext | null>(null);
const humanActions = ref<HumanAction[]>([]);
const assignedTasks = ref<AssignedTask[]>([]);
const projects = ref<PersonalProject[]>([]);
const archivedProjects = ref<PersonalProject[]>([]);
const activity = ref<ActivityEntry[]>([]);

const archivedOpen = ref(false);

function statusOf(error: unknown): Status {
  const status = (error as { status?: number } | null)?.status;
  return status === 403 ? "forbidden" : "error";
}

function noteOf(error: unknown): string {
  const code = (error as { code?: string } | null)?.code;
  const status = (error as { status?: number } | null)?.status;
  if (code) return `${code} · ${status ?? ""}`.trim();
  return status ? `HTTP ${status}` : "";
}

/** 区块独立加载：互不阻塞，失败各自表达 */
async function load() {
  pageStatus.value = "loading";
  inboxStatus.value = "loading";
  projectsStatus.value = "loading";
  activityStatus.value = "loading";
  pageNote.value = "";

  let ctx: PersonalContext;
  try {
    ctx = await loadPersonalContext();
  } catch (error) {
    context.value = null;
    pageNote.value = noteOf(error);
    pageStatus.value = statusOf(error);
    return;
  }
  context.value = ctx;
  pageStatus.value = "ready";

  void loadPersonalInbox(ctx)
    .then(section => {
      humanActions.value = section.humanActions;
      assignedTasks.value = section.assignedTasks;
      inboxStatus.value = "ready";
    })
    .catch(error => {
      inboxStatus.value = statusOf(error);
    });

  void loadPersonalProjects(ctx)
    .then(section => {
      projects.value = section.projects;
      archivedProjects.value = section.archivedProjects;
      projectsStatus.value = "ready";
    })
    .catch(error => {
      projectsStatus.value = statusOf(error);
    });

  void loadPersonalActivity(ctx)
    .then(items => {
      activity.value = items;
      activityStatus.value = "ready";
    })
    .catch(error => {
      activityStatus.value = statusOf(error);
    });
}

onMounted(load);

/* ------------------------------- 派生数据 ------------------------------- */

const displayName = computed(() => context.value?.me.display_name ?? "");

const greeting = computed(() => {
  const hour = new Date().getHours();
  const slot = hour < 12 ? "morning" : hour < 18 ? "afternoon" : "evening";
  return t(`pm.personalHome.greeting.${slot}`, { name: displayName.value });
});

/** 无任何项目时副标题不做"今日待办"的承诺 */
const subtitle = computed(() => {
  if (pageStatus.value === "ready" && isEmpty.value) {
    return t("pm.personalHome.subtitle.firstLogin");
  }
  return t("pm.personalHome.subtitle.default");
});

const isEmpty = computed(
  () =>
    projectsStatus.value === "ready" &&
    projects.value.length === 0 &&
    archivedProjects.value.length === 0
);

const inboxTotal = computed(
  () => humanActions.value.length + assignedTasks.value.length
);

/** Row1 三张指标卡（需要关注 / 分配给我 / 参与的项目），加载态与正常态共用。
 * 区块失败时数值给「—」、口径给「暂不可用」，绝不渲染成 0（失败 ≠ 空）。 */
const overviewStats = computed<OverviewStat[]>(() => {
  const inboxReady = inboxStatus.value === "ready";
  const projectsReady = projectsStatus.value === "ready";
  return [
    {
      label: t("pm.personalHome.overview.needsAttention"),
      value: inboxReady ? String(humanActions.value.length) : "—",
      description: inboxReady
        ? t("pm.personalHome.overview.needsAttentionHint")
        : t("pm.personalHome.overview.unavailable"),
      icon: AlertTriangle,
      tone: "warning"
    },
    {
      label: t("pm.personalHome.overview.assignedToMe"),
      value: inboxReady ? String(assignedTasks.value.length) : "—",
      description: inboxReady
        ? t("pm.personalHome.overview.assignedToMeHint")
        : t("pm.personalHome.overview.unavailable"),
      icon: ClipboardList,
      tone: "info"
    },
    {
      label: t("pm.personalHome.overview.activeProjects"),
      value: projectsReady ? String(projects.value.length) : "—",
      description: projectsReady
        ? t("pm.personalHome.overview.activeProjectsHint")
        : t("pm.personalHome.overview.unavailable"),
      icon: GitBranch,
      tone: "accent"
    }
  ];
});

/** 我的待办数：人工事项 + 分配任务 */
const pendingByProject = computed(() => {
  const counts = new Map<string, number>();
  for (const item of [...humanActions.value, ...assignedTasks.value]) {
    counts.set(item.projectId, (counts.get(item.projectId) ?? 0) + 1);
  }
  return counts;
});

const attentionProjects = computed(
  () =>
    new Set(
      humanActions.value
        .filter(item => item.due.state === "overdue")
        .map(item => item.projectId)
    )
);

const allCaughtUp = computed(
  () =>
    inboxStatus.value === "ready" &&
    humanActions.value.length === 0 &&
    assignedTasks.value.length === 0
);

/* ------------------------------- 交互 ------------------------------- */

function pendingOf(projectId: string): number {
  return pendingByProject.value.get(projectId) ?? 0;
}

function attentionOf(projectId: string): boolean {
  return attentionProjects.value.has(projectId);
}

/** 打开全局搜索面板（壳级 ⌘/Ctrl+K） */
function focusGlobalSearch() {
  window.dispatchEvent(
    new KeyboardEvent("keydown", { key: "k", metaKey: true, bubbles: true })
  );
}

/**
 * Create Project —— 进入 S01-02 项目创建页（/project/create）。
 * 创建成功后由创建页自动带入项目初始化总览（S01-02 §7 裁决 Q3）。
 */
function createProject() {
  void router.push("/project/create");
}

function openObject(path: string) {
  void router.push(path);
}
</script>

<template>
  <PageFrame :title="greeting" :description="subtitle">
    <template #actions>
      <IconButton
        :label="t('pm.personalHome.action.search')"
        variant="secondary"
        @click="focusGlobalSearch"
      >
        <Search :size="16" aria-hidden="true" />
      </IconButton>
      <Button variant="primary" @click="createProject">
        <Plus :size="14" aria-hidden="true" />
        {{ t("pm.personalHome.action.createProject") }}
      </Button>
    </template>

    <!-- 首次登录：没有任何项目 -->
    <Card v-if="pageStatus === 'ready' && isEmpty" class="pm-home__welcome">
      <SectionState
        variant="empty"
        :title="t('pm.personalHome.state.firstLogin.title')"
        :description="t('pm.personalHome.state.firstLogin.description')"
        :action-label="t('pm.personalHome.action.createProject')"
        @action="createProject"
      />
    </Card>

    <!-- 整页失败 -->
    <Card v-else-if="pageStatus === 'error'">
      <SectionState
        variant="error"
        :title="t('pm.personalHome.state.error.page.title')"
        :description="t('pm.personalHome.state.error.page.description')"
        :note="pageNote"
        @action="load"
      />
    </Card>

    <!-- 无访问权限 -->
    <Card v-else-if="pageStatus === 'forbidden'">
      <SectionState
        variant="forbidden"
        :title="t('pm.personalHome.state.forbidden.title')"
        :description="t('pm.personalHome.state.forbidden.description')"
        :action-label="t('pm.personalHome.action.createProject')"
        @action="createProject"
      />
    </Card>

    <!-- 加载中：骨架屏按真实布局形状占位（概览直接用 StatCard loading 态） -->
    <template v-else-if="pageStatus === 'loading'">
      <div class="pm-overview" aria-busy="true">
        <StatCard
          v-for="stat in overviewStats"
          :key="stat.label"
          v-bind="stat"
          loading
        />
      </div>
      <div class="pm-rows">
        <Card class="pm-card--flush">
          <template #header>
            <div class="pm-card-head">
              <h2 class="pm-card-head__title">
                {{ t("pm.personalHome.inbox.title") }}
              </h2>
            </div>
          </template>
          <div class="pm-skeleton-list" aria-busy="true">
            <div v-for="index in 4" :key="index" class="pm-skeleton-row">
              <Skeleton width="58%" height="13px" tone="base" />
              <Skeleton width="40%" height="10px" tone="base" />
            </div>
          </div>
        </Card>
        <Card class="pm-card--flush">
          <template #header>
            <div class="pm-card-head">
              <h2 class="pm-card-head__title">
                {{ t("pm.personalHome.projects.title") }}
              </h2>
            </div>
          </template>
          <div class="pm-skeleton-list" aria-busy="true">
            <div v-for="index in 3" :key="index" class="pm-skeleton-row">
              <Skeleton width="58%" height="13px" tone="base" />
              <Skeleton width="88%" height="12px" tone="base" />
            </div>
          </div>
        </Card>
      </div>
    </template>

    <!-- 正常态 -->
    <template v-else>
      <!-- Row 1 · My Overview：三张 StatCard 独立成卡 -->
      <div
        class="pm-overview"
        role="group"
        :aria-label="t('pm.personalHome.overview.title')"
      >
        <StatCard
          v-for="stat in overviewStats"
          :key="stat.label"
          v-bind="stat"
        />
      </div>

      <!-- Row 2 · My Inbox + My Projects -->
      <div class="pm-rows">
        <Card class="pm-card--flush">
          <template #header>
            <div class="pm-card-head">
              <h2 class="pm-card-head__title">
                {{ t("pm.personalHome.inbox.title") }}
                <Badge variant="count" size="sm">{{ inboxTotal }}</Badge>
              </h2>
              <Button variant="ghost" size="sm" @click="router.push('/inbox')">
                {{ t("pm.personalHome.action.viewAll") }}
              </Button>
            </div>
          </template>

          <SectionState
            v-if="inboxStatus === 'error'"
            variant="error"
            :title="t('pm.personalHome.state.error.inbox.title')"
            :description="t('pm.personalHome.state.error.inbox.description')"
            @action="load"
          />

          <div
            v-else-if="inboxStatus === 'loading'"
            class="pm-skeleton-list"
            aria-busy="true"
          >
            <div v-for="index in 4" :key="index" class="pm-skeleton-row">
              <Skeleton width="58%" height="13px" tone="base" />
              <Skeleton width="40%" height="10px" tone="base" />
            </div>
          </div>

          <SectionState
            v-else-if="allCaughtUp"
            variant="empty"
            :icon="CircleCheck"
            :title="t('pm.personalHome.inbox.empty.title')"
            :description="t('pm.personalHome.inbox.empty.description')"
          />

          <template v-else>
            <div class="pm-section">
              <div class="pm-section__head">
                <span class="pm-section__title">
                  {{ t("pm.personalHome.inbox.humanActions") }}
                </span>
                <Badge variant="count" size="sm">
                  {{ humanActions.length }}
                </Badge>
                <span class="pm-section__note">
                  {{ t("pm.personalHome.inbox.humanActionsNote") }}
                </span>
              </div>
              <HumanActionItem
                v-for="item in humanActions"
                :key="item.id"
                :kind="item.kind"
                :title="item.title"
                :subject="item.subject"
                :object-type="item.objectType"
                :project-name="item.projectName"
                :due="item.due"
                @open="openObject(item.objectPath)"
              />
              <p v-if="!humanActions.length" class="pm-section__empty">
                {{ t("pm.personalHome.inbox.humanActionsEmpty") }}
              </p>
            </div>

            <div class="pm-divider" role="separator" />

            <div class="pm-section">
              <div class="pm-section__head">
                <span class="pm-section__title">
                  {{ t("pm.personalHome.inbox.assignedTasks") }}
                </span>
                <Badge variant="count" size="sm">
                  {{ assignedTasks.length }}
                </Badge>
                <span class="pm-section__note">
                  {{ t("pm.personalHome.inbox.assignedTasksNote") }}
                </span>
              </div>
              <AssignedTaskItem
                v-for="item in assignedTasks"
                :key="item.id"
                :title="item.title"
                :subject="item.subject"
                :project-name="item.projectName"
                :status="item.status"
                :priority="item.priority"
                :due="item.due"
                @open="openObject(item.objectPath)"
              />
              <p v-if="!assignedTasks.length" class="pm-section__empty">
                {{ t("pm.personalHome.inbox.assignedTasksEmpty") }}
              </p>
            </div>
          </template>
        </Card>

        <Card class="pm-card--flush">
          <template #header>
            <div class="pm-card-head">
              <h2 class="pm-card-head__title">
                {{ t("pm.personalHome.projects.title") }}
                <Badge variant="count" size="sm">{{ projects.length }}</Badge>
              </h2>
              <Button
                variant="ghost"
                size="sm"
                @click="router.push('/projects')"
              >
                {{ t("pm.personalHome.action.viewAll") }}
              </Button>
            </div>
          </template>

          <SectionState
            v-if="projectsStatus === 'error'"
            variant="error"
            :title="t('pm.personalHome.state.error.projects.title')"
            :description="t('pm.personalHome.state.error.projects.description')"
            @action="load"
          />

          <div
            v-else-if="projectsStatus === 'loading'"
            class="pm-skeleton-list"
            aria-busy="true"
          >
            <div v-for="index in 3" :key="index" class="pm-skeleton-row">
              <Skeleton width="58%" height="13px" tone="base" />
              <Skeleton width="88%" height="12px" tone="base" />
            </div>
          </div>

          <template v-else>
            <ProjectCard
              v-for="project in projects"
              :key="project.id"
              :name="project.name"
              :code="project.code"
              :description="project.description"
              :status="project.status"
              :role="project.role"
              :pending-count="pendingOf(project.id)"
              :last-activity-at="project.lastActivityAt"
              :attention="attentionOf(project.id)"
              @open="openObject(project.objectPath)"
            />
            <p v-if="!projects.length" class="pm-section__empty">
              {{ t("pm.personalHome.projects.empty") }}
            </p>

            <ListItem
              v-if="archivedProjects.length"
              class="pm-folder"
              :class="{ 'is-open': archivedOpen }"
              :title="t('pm.personalHome.projects.archived')"
              :icon="ChevronRight"
              :aria-expanded="archivedOpen ? 'true' : 'false'"
              @click="archivedOpen = !archivedOpen"
            >
              <template #trailing>
                <Badge variant="count" size="sm">
                  {{ archivedProjects.length }}
                </Badge>
              </template>
            </ListItem>
            <template v-if="archivedOpen">
              <ProjectCard
                v-for="project in archivedProjects"
                :key="project.id"
                :name="project.name"
                :code="project.code"
                :description="project.description"
                :status="project.status"
                :role="project.role"
                :pending-count="pendingOf(project.id)"
                :last-activity-at="project.lastActivityAt"
                :attention="attentionOf(project.id)"
                @open="openObject(project.objectPath)"
              />
            </template>
          </template>
        </Card>
      </div>

      <!-- Row 3 · Recent Activity -->
      <Card class="pm-card--flush">
        <template #header>
          <div class="pm-card-head">
            <h2 class="pm-card-head__title">
              {{ t("pm.personalHome.activity.title") }}
            </h2>
            <Button variant="ghost" size="sm" @click="router.push('/projects')">
              {{ t("pm.personalHome.action.viewAll") }}
            </Button>
          </div>
        </template>

        <SectionState
          v-if="activityStatus === 'error'"
          variant="error"
          :title="t('pm.personalHome.state.error.activity.title')"
          :description="t('pm.personalHome.state.error.activity.description')"
          @action="load"
        />

        <div
          v-else-if="activityStatus === 'loading'"
          class="pm-activity-grid"
          aria-busy="true"
        >
          <div v-for="index in 4" :key="index" class="pm-activity-skeleton">
            <Skeleton width="120px" height="20px" tone="base" />
            <Skeleton width="92%" height="10px" tone="base" />
            <Skeleton width="64%" height="10px" tone="base" />
          </div>
        </div>

        <div v-else-if="!activity.length" class="pm-section">
          <p class="pm-section__empty">
            {{ t("pm.personalHome.activity.empty") }}
          </p>
        </div>

        <div v-else class="pm-activity-grid">
          <ActivityEntryView
            v-for="entry in activity"
            :key="entry.id"
            :actor-kind="entry.actorKind"
            :actor-name="entry.actorName"
            :event-kind="entry.eventKind"
            :action="entry.action"
            :target-type="entry.targetType"
            :project-name="entry.projectName"
            :object-ref="entry.objectRef"
            :occurred-at="entry.occurredAt"
            @open="
              openObject(
                `/project/command-center?project_id=${entry.projectId}`
              )
            "
          />
        </div>
      </Card>
    </template>
  </PageFrame>
</template>

<style scoped>
.pm-home__welcome {
  max-width: 560px;
  margin: var(--space-8) auto 0;
}

/* 列表卡：清掉 Card body 内边距，让 ListItem 交互面通到卡片边缘
 *（SectionState / EmptyState 自带内边距，不受影响） */
.pm-card--flush :deep(.ui-card__body) {
  padding: 0;
}

/* ---------------------------- 卡片头 ---------------------------- */

.pm-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.pm-card-head__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font: inherit;
}

/* ------------------------------- 概览 ------------------------------- */

/* StatCard 自带 surface 底 + 边框，独立成卡，无需外层 Card 包裹 */
.pm-overview {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

/* -------------------------------- 栅格 -------------------------------- */

.pm-rows {
  display: grid;
  grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr);
  gap: var(--space-4);
  align-items: start;
}

/* ------------------------------ 分区 ------------------------------ */

.pm-section {
  padding: var(--space-3) 0 var(--space-1);
}

.pm-section__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-4) var(--space-2);
}

.pm-section__title {
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
  text-transform: uppercase;
  color: var(--ws-text-secondary);
}

.pm-section__note {
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.pm-section__empty {
  margin: 0;
  padding: var(--space-3) var(--space-4) var(--space-4);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.pm-divider {
  height: var(--border-1);
  background: var(--ws-border);
  margin: var(--space-2) 0;
}

/* ---------------------------- 折叠分组 ---------------------------- */

.pm-folder :deep(.ui-list-item__icon) {
  transition: transform var(--duration-fast) var(--ease-standard);
}

.pm-folder.is-open :deep(.ui-list-item__icon) {
  transform: rotate(90deg);
}

/* ---------------------------- 活动流 ---------------------------- */

.pm-activity-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
}

.pm-activity-skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-left: var(--border-1) solid var(--ws-border);
}

.pm-activity-skeleton:first-child {
  border-left: 0;
}

/* ---------------------------- 骨架屏 ---------------------------- */

.pm-skeleton-list {
  padding: var(--space-2) 0;
}

.pm-skeleton-row {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
}

/* ---------------------------- 自适应 ---------------------------- */

@media (max-width: 1100px) {
  .pm-rows {
    grid-template-columns: 1fr;
  }

  .pm-activity-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 880px) {
  .pm-overview {
    grid-template-columns: 1fr;
  }

  .pm-activity-grid {
    grid-template-columns: 1fr;
  }
}
</style>
