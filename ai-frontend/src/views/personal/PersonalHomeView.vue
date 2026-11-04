<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { CircleCheck, Plus, Search } from "lucide-vue-next";
import { PageHeader } from "@/components/shell";
import Button from "@/components/ui/button/Button.vue";
import IconButton from "@/components/ui/icon-button/IconButton.vue";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";

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
import MetricTile from "./components/MetricTile.vue";
import ProjectCard from "./components/ProjectCard.vue";
import SectionState from "./components/SectionState.vue";

/**
 * 个人主页（Personal Home）—— 登录后的默认落地页。
 *
 * 结构（高保真稿 §3）：
 *   Row1  My Overview（三指标横条）
 *   Row2  My Inbox（60%） + My Projects（40%）
 *   Row3  Recent Activity（横向）
 *
 * 状态处理（高保真稿 §8）：三个区块**各自独立**加载，
 * 因此「Inbox 失败」不会拖垮「我的项目」，失败也绝不渲染成"没有数据"。
 */
type Status = "loading" | "ready" | "error" | "forbidden";

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
 * Create Project —— 复用既有项目创建流程。
 * ⚠ S01-02 的创建流程尚未实现（页面层仍是占位），此处先落到项目清单页，
 * 待创建页就位后改为直接打开创建视图。
 */
function createProject() {
  void router.push("/projects");
}

function openObject(path: string) {
  void router.push(path);
}
</script>

<template>
  <div class="pm-home">
    <PageHeader :title="greeting" :description="subtitle">
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
    </PageHeader>

    <!-- 首次登录：没有任何项目 -->
    <section
      v-if="pageStatus === 'ready' && isEmpty"
      class="pm-card pm-home__welcome"
    >
      <SectionState
        variant="empty"
        :title="t('pm.personalHome.state.firstLogin.title')"
        :description="t('pm.personalHome.state.firstLogin.description')"
        :action-label="t('pm.personalHome.action.createProject')"
        @action="createProject"
      />
    </section>

    <!-- 整页失败 -->
    <section v-else-if="pageStatus === 'error'" class="pm-card">
      <SectionState
        variant="error"
        :title="t('pm.personalHome.state.error.page.title')"
        :description="t('pm.personalHome.state.error.page.description')"
        :note="pageNote"
        @action="load"
      />
    </section>

    <!-- 无访问权限 -->
    <section v-else-if="pageStatus === 'forbidden'" class="pm-card">
      <SectionState
        variant="forbidden"
        :title="t('pm.personalHome.state.forbidden.title')"
        :description="t('pm.personalHome.state.forbidden.description')"
        :action-label="t('pm.personalHome.action.createProject')"
        @action="createProject"
      />
    </section>

    <!-- 加载中：骨架屏按真实布局形状占位 -->
    <template v-else-if="pageStatus === 'loading'">
      <div class="pm-overview" aria-busy="true">
        <div v-for="index in 3" :key="index" class="pm-overview__skeleton">
          <Skeleton width="10px" height="10px" tone="base" />
          <div class="pm-overview__skeleton-body">
            <Skeleton width="38px" height="20px" tone="strong" />
            <Skeleton width="96px" height="12px" tone="base" />
            <Skeleton width="128px" height="10px" tone="base" />
          </div>
        </div>
      </div>
      <div class="pm-rows">
        <section class="pm-card" aria-busy="true">
          <div class="pm-card__head">
            <h2 class="pm-card__title">
              {{ t("pm.personalHome.inbox.title") }}
            </h2>
          </div>
          <div v-for="index in 3" :key="index" class="pm-skeleton-row">
            <Skeleton width="62%" height="13px" tone="base" />
            <Skeleton width="44%" height="10px" tone="base" />
          </div>
        </section>
        <section class="pm-card" aria-busy="true">
          <div class="pm-card__head">
            <h2 class="pm-card__title">
              {{ t("pm.personalHome.projects.title") }}
            </h2>
          </div>
          <div v-for="index in 3" :key="index" class="pm-skeleton-row">
            <Skeleton width="58%" height="13px" tone="base" />
            <Skeleton width="88%" height="12px" tone="base" />
          </div>
        </section>
      </div>
    </template>

    <!-- 正常态 -->
    <template v-else>
      <!-- Row 1 · My Overview -->
      <div
        class="pm-overview"
        role="group"
        :aria-label="t('pm.personalHome.overview.title')"
      >
        <MetricTile
          :label="t('pm.personalHome.overview.needsAttention')"
          :value="inboxStatus === 'ready' ? humanActions.length : null"
          :hint="
            inboxStatus === 'ready'
              ? t('pm.personalHome.overview.needsAttentionHint')
              : t('pm.personalHome.overview.unavailable')
          "
          tone="warning"
        />
        <MetricTile
          :label="t('pm.personalHome.overview.assignedToMe')"
          :value="inboxStatus === 'ready' ? assignedTasks.length : null"
          :hint="
            inboxStatus === 'ready'
              ? t('pm.personalHome.overview.assignedToMeHint')
              : t('pm.personalHome.overview.unavailable')
          "
          tone="info"
        />
        <MetricTile
          :label="t('pm.personalHome.overview.activeProjects')"
          :value="projectsStatus === 'ready' ? projects.length : null"
          :hint="
            projectsStatus === 'ready'
              ? t('pm.personalHome.overview.activeProjectsHint')
              : t('pm.personalHome.overview.unavailable')
          "
          tone="accent"
        />
      </div>

      <!-- Row 2 · My Inbox + My Projects -->
      <div class="pm-rows">
        <section class="pm-card" :aria-label="t('pm.personalHome.inbox.title')">
          <div class="pm-card__head">
            <h2 class="pm-card__title">
              {{ t("pm.personalHome.inbox.title") }}
              <span class="pm-card__count">{{ inboxTotal }}</span>
            </h2>
            <button
              type="button"
              class="pm-link"
              @click="router.push('/inbox')"
            >
              {{ t("pm.personalHome.action.viewAll") }}
            </button>
          </div>

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
                <span class="pm-card__count">{{ humanActions.length }}</span>
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
                <span class="pm-card__count">{{ assignedTasks.length }}</span>
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
        </section>

        <section
          class="pm-card"
          :aria-label="t('pm.personalHome.projects.title')"
        >
          <div class="pm-card__head">
            <h2 class="pm-card__title">
              {{ t("pm.personalHome.projects.title") }}
              <span class="pm-card__count">{{ projects.length }}</span>
            </h2>
            <button
              type="button"
              class="pm-link"
              @click="router.push('/projects')"
            >
              {{ t("pm.personalHome.action.viewAll") }}
            </button>
          </div>

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

            <button
              type="button"
              class="pm-folder"
              :aria-expanded="archivedOpen"
              @click="archivedOpen = !archivedOpen"
            >
              <span
                class="pm-folder__chevron"
                :data-open="archivedOpen"
                aria-hidden="true"
                >›</span
              >
              <span>{{ t("pm.personalHome.projects.archived") }}</span>
              <span class="pm-card__count">{{ archivedProjects.length }}</span>
            </button>
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
        </section>
      </div>

      <!-- Row 3 · Recent Activity -->
      <section
        class="pm-card pm-activity"
        :aria-label="t('pm.personalHome.activity.title')"
      >
        <div class="pm-card__head">
          <h2 class="pm-card__title">
            {{ t("pm.personalHome.activity.title") }}
          </h2>
          <button
            type="button"
            class="pm-link"
            @click="router.push('/projects')"
          >
            {{ t("pm.personalHome.action.viewAll") }}
          </button>
        </div>

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
      </section>
    </template>
  </div>
</template>

<style scoped>
.pm-home {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5) var(--space-6) var(--space-6);
}

.pm-home__welcome {
  max-width: 560px;
  margin: var(--space-8) auto 0;
}

/* ------------------------------- 概览 ------------------------------- */

.pm-overview {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow: hidden;
}

.pm-overview__skeleton {
  display: flex;
  gap: var(--space-3);
  padding: var(--space-4);
  border-left: var(--border-1) solid var(--ws-border);
}

.pm-overview__skeleton:first-child {
  border-left: 0;
}

.pm-overview__skeleton-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
}

/* -------------------------------- 栅格 -------------------------------- */

.pm-rows {
  display: grid;
  grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr);
  gap: var(--space-4);
  align-items: start;
}

/* -------------------------------- 卡片 -------------------------------- */

.pm-card {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.pm-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.pm-card__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
  letter-spacing: -0.005em;
}

.pm-card__count {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  background: var(--ws-elevated);
  border-radius: var(--radius-full);
  padding: 1px 6px;
}

.pm-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.pm-link:hover {
  color: var(--ws-text-primary);
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

.pm-folder {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  text-align: left;
  padding: 9px var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.pm-folder:hover {
  background: var(--ws-hover-veil);
}

.pm-folder__chevron {
  display: inline-flex;
  transition: transform var(--duration-fast) var(--ease-standard);
}

.pm-folder__chevron[data-open="true"] {
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
  gap: 10px;
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
  gap: 8px;
  padding: var(--space-3) var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
}

/* ---------------------------- 自适应 ---------------------------- */

@media (max-width: 1120px) {
  .pm-rows {
    grid-template-columns: 1fr;
  }

  .pm-activity-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 860px) {
  .pm-overview {
    grid-template-columns: 1fr;
  }

  .pm-activity-grid {
    grid-template-columns: 1fr;
  }
}
</style>
