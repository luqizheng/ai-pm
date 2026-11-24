<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import { useDesignProject } from "@/views/project/design/use-design-project";
import { getProjectGoal, type ProjectGoalView } from "@/api/goal";
import { useI18n } from "vue-i18n";
import { PageFrame } from "@/components/shell";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";

import CommandCenterHeader from "./components/CommandCenterHeader.vue";
import ProjectHealthSection from "./components/ProjectHealthSection.vue";
import InitializationCard from "./components/InitializationCard.vue";
import GoalCard from "./components/GoalCard.vue";
import AttentionPanel from "./components/AttentionPanel.vue";
import TaskOverviewCard from "./components/TaskOverviewCard.vue";
import AiAgentsOverviewCard from "./components/AiAgentsOverviewCard.vue";
import RunHistoryCard from "./components/RunHistoryCard.vue";
import DeliverablesCard from "./components/DeliverablesCard.vue";
import ActivityTimelineCard from "./components/ActivityTimelineCard.vue";
import KnowledgeSnapshotCard from "./components/KnowledgeSnapshotCard.vue";
import MembersCard from "./components/MembersCard.vue";
import type { CommandCenterData, Translate } from "./command-center-data";

/**
 * 项目总控台（Project Command Center，Pixso 3:7089 Default State）。
 *
 * 组装策略：PageFrame 页框（§三.12）不带 title——设计稿页头 richer
 * （状态/可见性 chip + 元信息行），由 CommandCenterHeader 承担（同通知中心先例）。
 * 区块顺序 = 设计稿六段：页头 → 健康度 → 需关注 → 任务/Agents 双栏 →
 * 运行/交付物双栏 → 动态 + 右栏（知识快照/成员）。
 * 数据来源：发布构建读 `/command-center` 真实聚合读模型；界面文案走
 * s01.command-center 命名空间。仅 VITE_USE_MOCK=1（本地 dev / 单测）动态加载
 * fixtures 设计稿样本，发布产物不含演示数据（build/release-guard.ts）。
 */
type CommandCenterLoader = (projectId: string) => Promise<CommandCenterData>;

interface Props {
  /** 数据加载器（测试注入；缺省按 VITE_USE_MOCK 选真实接口或 fixtures） */
  loader?: CommandCenterLoader;
}

const props = defineProps<Props>();

const router = useRouter();
const route = useRoute();
// B06-D5：项目上下文 = URL query 优先，回退壳 scope store（顶栏已选项目）。
// 此前只读 query——左导航「项目总览」不带参数，顶栏明明已选仍报「请先选项目」。
const { projectId: storeProjectId } = useDesignProject();
const { t } = useI18n();
const translate: Translate = (key, params) => t(key, params ?? {});

const mockPreview = import.meta.env.VITE_USE_MOCK === "1";
const data = ref<CommandCenterData | null>(null);
/** 项目正式 Goal（元数据 + 知识文档正文）；null = 加载中或未选项目 */
const goal = ref<ProjectGoalView | null>(null);
const phase = ref<"loading" | "ready" | "empty" | "error">("loading");

function currentProjectId(): string {
  const query = route.query.project_id;
  if (typeof query === "string" && query) return query;
  return storeProjectId.value;
}

async function defaultLoader(projectId: string): Promise<CommandCenterData> {
  if (mockPreview) {
    const { commandCenterDataFixture } = await import("./fixtures");
    return commandCenterDataFixture(translate);
  }
  const { loadCommandCenterData } = await import("./real-data");
  return loadCommandCenterData(projectId, translate);
}

async function load() {
  const projectId = currentProjectId();
  if (!mockPreview && !projectId && !props.loader) {
    data.value = null;
    phase.value = "empty";
    return;
  }

  phase.value = "loading";
  // Goal 与总控台并行拉取：Goal 读失败不应把整页打成 error——它是一张
  // 独立的卡，失败时卡内显示自己的状态（missing / body_unavailable）。
  void loadGoal(projectId);
  try {
    const loader = props.loader ?? defaultLoader;
    data.value = await loader(projectId);
    phase.value = "ready";
  } catch {
    data.value = null;
    phase.value = "error";
  }
}

async function loadGoal(projectId: string) {
  if (!projectId) {
    goal.value = null;
    return;
  }
  try {
    goal.value = await getProjectGoal(projectId);
  } catch {
    goal.value = null;
  }
}

onMounted(load);
watch(
  () => currentProjectId(),
  () => void load()
);

onBeforeUnmount(() => {
  data.value = null;
});

function openRequirements() {
  void router.push("/project/requirements");
}

function openExecution() {
  void router.push("/project/execution");
}

function openKnowledge() {
  void router.push("/project/knowledge");
}

/** 项目设置（V1.0 稿统一入口）：Git 仓库 / Runners 等配置中心。
 *  携带当前项目上下文（?project_id=），设置各节自动继承、不再重复选择项目。 */
function openSettings() {
  const projectId = route.query.project_id;
  void router.push({
    path: "/project/settings",
    query:
      typeof projectId === "string" && projectId
        ? { project_id: projectId }
        : {}
  });
}
</script>

<template>
  <PageFrame>
    <Skeleton
      v-if="phase === 'loading'"
      width="100%"
      height="var(--space-12)"
    />

    <EmptyState
      v-else-if="phase === 'empty'"
      :title="t('s01.command-center.selectProject')"
    />

    <EmptyState
      v-else-if="phase === 'error'"
      :title="t('pm.commandCenter.loadFailed')"
      :description="t('pm.commandCenter.unavailable')"
    />

    <template v-else-if="data">
      <CommandCenterHeader
        :breadcrumb-label="t('s01.command-center.breadcrumbProjects')"
        breadcrumb-to="/projects"
        :project-name="data.header.name"
        :status-label="data.header.statusLabel"
        :status-tone="data.header.statusTone"
        :visibility-label="data.header.visibilityLabel"
        :description="data.header.description"
        :meta-entries="data.header.metaEntries"
        :settings-label="t('s01.command-center.settings')"
        :settings-button-label="t('s01.command-center.settings')"
        @settings="openSettings"
      />

      <GoalCard
        :title="t('s01.command-center.goalTitle')"
        :labels="{
          revision: t('s01.command-center.goalRevision'),
          publishedAt: t('s01.command-center.goalPublishedAt'),
          commit: t('s01.command-center.goalCommit')
        }"
        :states="{
          missingTitle: t('s01.command-center.goalMissingTitle'),
          missingHint: t('s01.command-center.goalMissingHint'),
          bodyUnavailableTitle: t(
            's01.command-center.goalBodyUnavailableTitle'
          ),
          bodyUnavailableHint: t('s01.command-center.goalBodyUnavailableHint')
        }"
        :badges="{
          ready: t('s01.command-center.goalBadgeReady'),
          missing: t('s01.command-center.goalBadgeMissing'),
          bodyUnavailable: t('s01.command-center.goalBadgeBodyUnavailable')
        }"
        :data="goal"
      />

      <InitializationCard
        :title="t('s01.command-center.initTitle')"
        :progress-label="
          t('s01.command-center.initProgress', {
            done: data.initialization?.requiredDone ?? 0,
            total: data.initialization?.requiredTotal ?? 0
          })
        "
        :completion-labels="{
          complete: t('s01.command-center.initComplete'),
          incomplete: t('s01.command-center.initIncomplete'),
          notDeterminable: t('s01.command-center.initNotDeterminable')
        }"
        :required-label="t('s01.command-center.initRequired')"
        :data="data.initialization"
        :project-id="currentProjectId()"
      />

      <ProjectHealthSection
        :title="t('s01.command-center.healthTitle')"
        :meta="
          t('s01.command-center.healthMeta', { time: data.healthMetaTime })
        "
        :action-label="t('s01.command-center.healthAction')"
        :metrics="data.health"
      />

      <AttentionPanel
        :title="t('s01.command-center.attentionTitle')"
        :pending-label="
          t('s01.command-center.attentionPending', {
            count: data.attention.pendingCount
          })
        "
        :counts="[
          {
            label: t('s01.command-center.attentionUrgent'),
            count: data.attention.urgentCount,
            tone: 'danger'
          },
          {
            label: t('s01.command-center.attentionNotify'),
            count: data.attention.notifyCount,
            tone: 'warning'
          }
        ]"
        :action-label="t('s01.command-center.attentionInbox')"
        :column-labels="{
          item: t('s01.command-center.attentionColItem'),
          category: t('s01.command-center.attentionColCategory'),
          object: t('s01.command-center.attentionColObject'),
          status: t('s01.command-center.attentionColStatus'),
          owner: t('s01.command-center.attentionColOwner'),
          goto: t('s01.command-center.attentionColGoto')
        }"
        :items="data.attention.items"
      />

      <div class="cc-grid cc-grid--half">
        <TaskOverviewCard
          :title="t('s01.command-center.taskTitle')"
          :total-label="
            t('s01.command-center.taskTotal', { count: data.taskTotal })
          "
          :view-all-label="t('s01.command-center.taskViewAll')"
          :stats="data.taskStats"
          @view-all="openExecution"
        />
        <AiAgentsOverviewCard
          :title="t('s01.command-center.agentsTitle')"
          :meta="t('s01.command-center.agentsMeta')"
          :action-label="t('s01.command-center.agentsAction')"
          :metrics="data.agentMetrics"
          :column-labels="{
            agent: t('s01.command-center.agentsColAgent'),
            work: t('s01.command-center.agentsColWork'),
            status: t('s01.command-center.agentsColStatus'),
            progress: t('s01.command-center.agentsColProgress'),
            duration: t('s01.command-center.agentsColDuration')
          }"
          :rows="data.agentRows"
        />
      </div>

      <div class="cc-grid cc-grid--half">
        <RunHistoryCard
          :title="t('s01.command-center.runsTitle')"
          :meta="t('s01.command-center.runsMeta')"
          :action-label="t('s01.command-center.runsAction')"
          :column-labels="{
            run: t('s01.command-center.runsColRun'),
            agent: t('s01.command-center.runsColAgent'),
            trigger: t('s01.command-center.runsColTrigger'),
            result: t('s01.command-center.runsColResult'),
            duration: t('s01.command-center.runsColDuration')
          }"
          :entries="data.runs"
        />
        <DeliverablesCard
          :title="t('s01.command-center.delivTitle')"
          :meta="t('s01.command-center.delivMeta')"
          :action-label="t('s01.command-center.delivAction')"
          :column-labels="{
            requirement: t('s01.command-center.delivColRequirement'),
            acceptance: t('s01.command-center.delivColAcceptance'),
            ai: t('s01.command-center.delivColAi'),
            deliverable: t('s01.command-center.delivColDeliverable')
          }"
          :rows="data.deliverables"
          :footnote="t('s01.command-center.delivFootnote')"
          @action="openRequirements"
        />
      </div>

      <div class="cc-grid cc-grid--rail">
        <ActivityTimelineCard
          :title="t('s01.command-center.activityTitle')"
          :meta="t('s01.command-center.activityMeta')"
          :action-label="t('s01.command-center.activityAction')"
          :items="data.activity"
        />
        <div class="cc-rail">
          <!-- 脚注不接 i18n 演示文案（AGENTS 规则 4：发布态禁止演示数据）；
               真实同步信息由数据面下发后再以 :footnote 传入 -->
          <KnowledgeSnapshotCard
            :title="t('s01.command-center.knowledgeTitle')"
            :meta="t('s01.command-center.knowledgeMeta')"
            :action-label="t('s01.command-center.knowledgeAction')"
            :stats="data.knowledge"
            @action="openKnowledge"
          />
          <MembersCard
            :title="t('s01.command-center.membersTitle')"
            :meta="
              t('s01.command-center.membersMeta', {
                humans: data.memberHumanCount,
                agents: data.memberAgentCount
              })
            "
            :members="data.members"
          />
        </div>
      </div>
    </template>
  </PageFrame>
</template>

<style scoped>
.cc-grid {
  display: grid;
  gap: var(--space-4);
  min-width: 0;
}

.cc-grid--half {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.cc-grid--rail {
  grid-template-columns: minmax(0, 1fr) 264px;
}

.cc-rail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

@media (max-width: 1100px) {
  .cc-grid--half,
  .cc-grid--rail {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
