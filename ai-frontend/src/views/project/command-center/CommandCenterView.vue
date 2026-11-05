<script setup lang="ts">
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { PageFrame } from "@/components/shell";

import CommandCenterHeader from "./components/CommandCenterHeader.vue";
import ProjectHealthSection from "./components/ProjectHealthSection.vue";
import AttentionPanel from "./components/AttentionPanel.vue";
import TaskOverviewCard from "./components/TaskOverviewCard.vue";
import AiAgentsOverviewCard from "./components/AiAgentsOverviewCard.vue";
import RunHistoryCard from "./components/RunHistoryCard.vue";
import DeliverablesCard from "./components/DeliverablesCard.vue";
import ActivityTimelineCard from "./components/ActivityTimelineCard.vue";
import KnowledgeSnapshotCard from "./components/KnowledgeSnapshotCard.vue";
import MembersCard from "./components/MembersCard.vue";

import {
  activityFixture,
  agentMetricsFixture,
  agentRowsFixture,
  attentionFixture,
  deliverableRowsFixture,
  healthFixture,
  knowledgeStatsFixture,
  membersFixture,
  projectHeaderFixture,
  runEntriesFixture,
  taskStatsFixture
} from "./fixtures";

/**
 * 项目总控台（Project Command Center，Pixso 3:7089 Default State）。
 *
 * 组装策略：PageFrame 页框（§三.12）不带 title——设计稿页头 richer
 * （状态/可见性 chip + 元信息行），由 CommandCenterHeader 承担（同通知中心先例）。
 * 区块顺序 = 设计稿六段：页头 → 健康度 → 需关注 → 任务/Agents 双栏 →
 * 运行/交付物双栏 → 动态 + 右栏（知识快照/成员）。
 * 数据 = fixtures（mock 先行）；界面文案走 s01.command-center 命名空间。
 * 动作路由：已有业务路由直接下钻；未落地入口暂不接线（组件事件保留）。
 */
const router = useRouter();
const { t } = useI18n();

function openRequirements() {
  void router.push("/project/requirements");
}

function openExecution() {
  void router.push("/project/execution");
}

function openKnowledge() {
  void router.push("/project/knowledge");
}
</script>

<template>
  <PageFrame>
    <CommandCenterHeader
      :breadcrumb-label="t('s01.command-center.breadcrumbProjects')"
      breadcrumb-to="/projects"
      :project-name="projectHeaderFixture.name"
      :status-label="t('s01.command-center.statusNormal')"
      status-tone="success"
      :visibility-label="t('s01.command-center.visibilityOrgOnly')"
      :description="projectHeaderFixture.description"
      :meta-entries="[
        {
          label: t('s01.command-center.ownerLabel'),
          value: projectHeaderFixture.ownerValue
        },
        {
          label: t('s01.command-center.createdLabel'),
          value: projectHeaderFixture.createdValue
        }
      ]"
      :settings-label="t('s01.command-center.settings')"
      :settings-button-label="t('s01.command-center.settings')"
    />

    <ProjectHealthSection
      :title="t('s01.command-center.healthTitle')"
      :meta="t('s01.command-center.healthMeta', { time: '09:42' })"
      :action-label="t('s01.command-center.healthAction')"
      :metrics="healthFixture"
    />

    <AttentionPanel
      :title="t('s01.command-center.attentionTitle')"
      :pending-label="
        t('s01.command-center.attentionPending', {
          count: attentionFixture.pendingCount
        })
      "
      :counts="[
        {
          label: t('s01.command-center.attentionUrgent'),
          count: attentionFixture.urgentCount,
          tone: 'danger'
        },
        {
          label: t('s01.command-center.attentionNotify'),
          count: attentionFixture.notifyCount,
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
      :items="attentionFixture.items"
    />

    <div class="cc-grid cc-grid--half">
      <TaskOverviewCard
        :title="t('s01.command-center.taskTitle')"
        :total-label="t('s01.command-center.taskTotal', { count: 24 })"
        :view-all-label="t('s01.command-center.taskViewAll')"
        :stats="taskStatsFixture"
        @view-all="openExecution"
      />
      <AiAgentsOverviewCard
        :title="t('s01.command-center.agentsTitle')"
        :meta="t('s01.command-center.agentsMeta')"
        :action-label="t('s01.command-center.agentsAction')"
        :metrics="agentMetricsFixture"
        :column-labels="{
          agent: t('s01.command-center.agentsColAgent'),
          work: t('s01.command-center.agentsColWork'),
          status: t('s01.command-center.agentsColStatus'),
          progress: t('s01.command-center.agentsColProgress'),
          duration: t('s01.command-center.agentsColDuration')
        }"
        :rows="agentRowsFixture"
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
        :entries="runEntriesFixture"
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
        :rows="deliverableRowsFixture"
        :footnote="t('s01.command-center.delivFootnote')"
        @action="openRequirements"
      />
    </div>

    <div class="cc-grid cc-grid--rail">
      <ActivityTimelineCard
        :title="t('s01.command-center.activityTitle')"
        :meta="t('s01.command-center.activityMeta')"
        :action-label="t('s01.command-center.activityAction')"
        :items="activityFixture"
      />
      <div class="cc-rail">
        <KnowledgeSnapshotCard
          :title="t('s01.command-center.knowledgeTitle')"
          :meta="t('s01.command-center.knowledgeMeta')"
          :action-label="t('s01.command-center.knowledgeAction')"
          :stats="knowledgeStatsFixture"
          :footnote="t('s01.command-center.knowledgeFootnote')"
          @action="openKnowledge"
        />
        <MembersCard
          :title="t('s01.command-center.membersTitle')"
          :meta="t('s01.command-center.membersMeta', { humans: 8, agents: 4 })"
          :members="membersFixture"
        />
      </div>
    </div>
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
