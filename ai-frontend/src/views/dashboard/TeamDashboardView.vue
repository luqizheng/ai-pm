<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Building2, Users, FolderKanban, Cpu, Mail } from "lucide-vue-next";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { StatCard } from "@/components/ui/stat-card";
import { listProjects } from "@/api/pm";
import type { Project } from "@/api/types";
import {
  listTeamInvitations,
  listTeamMembers,
  type TeamInvitation,
  type TeamMember
} from "@/api/team";
import { useTeamStore } from "@/stores/team";

/**
 * Team Dashboard（/dashboard）—— **组织入口**，不是项目工作台（UI 细则 #1）：
 * 只呈现团队级事实（档案 / 成员概览 / 项目入口 / 邀请摘要 / AI 配置入口），
 * 项目执行指标一律在项目总控台（/project/command-center）。成员可读；
 * 邀请摘要仅 admin（角色即权限：无权限区块直接不渲染）。
 */
const { t } = useI18n();
const router = useRouter();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const members = ref<TeamMember[]>([]);
const invitations = ref<TeamInvitation[]>([]);
const projects = ref<Project[]>([]);

const team = computed(() => teamStore.currentTeam);
const isAdmin = computed(() => teamStore.isTeamAdmin);

const roleCounts = computed(() => {
  const counts = { owner: 0, admin: 0, member: 0, inactive: 0 };
  for (const member of members.value) {
    if (!member.is_active) counts.inactive += 1;
    counts[member.role] += 1;
  }
  return counts;
});

const pendingInvitations = computed(() =>
  invitations.value.filter(item => item.status === "PENDING")
);

onMounted(async () => {
  loading.value = true;
  error.value = null;
  try {
    await teamStore.load();
    const tenantId = teamStore.context?.tenant_id;
    const jobs: Array<Promise<unknown>> = [
      listProjects({ page: 1, size: 100 }).then(res => {
        projects.value = res.items ?? [];
      })
    ];
    if (tenantId) {
      jobs.push(
        listTeamMembers(tenantId).then(rows => {
          members.value = rows;
        })
      );
      if (isAdmin.value) {
        jobs.push(
          listTeamInvitations(tenantId).then(rows => {
            invitations.value = rows;
          })
        );
      }
    }
    await Promise.all(jobs);
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;
  }
});

function openProject(project: Project): void {
  void router.push({
    path: "/project/command-center",
    query: { project_id: project.id }
  });
}

function createProject(): void {
  void router.push("/project/create");
}

function openMembers(): void {
  void router.push("/admin/members");
}

function openInvitations(): void {
  void router.push("/admin/members");
}

function openAiProviders(): void {
  void router.push("/settings/team/ai-providers");
}
</script>

<template>
  <PageFrame
    :title="t('team.dashboard.title', { team: team?.name ?? '' })"
    :description="t('team.dashboard.description')"
  >
    <template #actions>
      <Button
        variant="primary"
        size="sm"
        data-testid="team-dashboard-create-project"
        @click="createProject"
      >
        {{ t("team.dashboard.createProject") }}
      </Button>
    </template>

    <Alert
      v-if="error"
      variant="danger"
      :title="t('team.dashboard.loadFailed')"
    >
      {{ error }}
    </Alert>

    <!-- 团队档案 -->
    <section class="profile" data-testid="team-dashboard-profile">
      <span class="profile__avatar" aria-hidden="true">
        <Building2 :size="20" />
      </span>
      <div class="profile__body">
        <p class="profile__name">{{ team?.name }}</p>
        <p class="profile__meta">
          <span v-if="team?.slug" class="profile__slug">{{ team.slug }}</span>
          <span>{{
            t("team.dashboard.roleLine", {
              role: teamStore.teamRole.toUpperCase()
            })
          }}</span>
        </p>
      </div>
      <Button
        variant="secondary"
        size="sm"
        data-testid="team-dashboard-settings"
        @click="openMembers"
      >
        {{ t("team.dashboard.manageMembers") }}
      </Button>
    </section>

    <!-- 成员概览：团队级计数（非项目执行指标） -->
    <section class="section">
      <div class="section__head">
        <h2 class="section__title">
          <Users :size="15" aria-hidden="true" />
          {{ t("team.dashboard.membersTitle") }}
        </h2>
        <Button variant="tertiary" size="sm" @click="openMembers">
          {{ t("team.dashboard.viewAll") }}
        </Button>
      </div>
      <div class="stats">
        <StatCard
          :label="t('team.dashboard.statOwners')"
          :value="String(roleCounts.owner)"
        />
        <StatCard
          :label="t('team.dashboard.statAdmins')"
          :value="String(roleCounts.admin)"
        />
        <StatCard
          :label="t('team.dashboard.statMembers')"
          :value="String(roleCounts.member)"
        />
        <StatCard
          :label="t('team.dashboard.statInactive')"
          :value="String(roleCounts.inactive)"
        />
      </div>
    </section>

    <!-- 团队项目：入口列表（执行指标在项目总控台） -->
    <section class="section">
      <div class="section__head">
        <h2 class="section__title">
          <FolderKanban :size="15" aria-hidden="true" />
          {{ t("team.dashboard.projectsTitle") }}
        </h2>
      </div>
      <EmptyState
        v-if="!loading && projects.length === 0"
        :title="t('team.dashboard.projectsEmpty')"
        :description="t('team.dashboard.projectsEmptyHint')"
      />
      <ul v-else class="projects" data-testid="team-dashboard-projects">
        <li v-for="project in projects" :key="project.id">
          <button type="button" class="project" @click="openProject(project)">
            <span class="project__code">{{ project.code }}</span>
            <span class="project__name">{{ project.name }}</span>
            <Badge variant="neutral">{{ project.status }}</Badge>
          </button>
        </li>
      </ul>
    </section>

    <!-- 邀请摘要（admin only）+ AI 配置入口 -->
    <section class="section cards">
      <button
        v-if="isAdmin"
        type="button"
        class="card"
        data-testid="team-dashboard-invitations"
        @click="openInvitations"
      >
        <Mail :size="16" aria-hidden="true" />
        <span class="card__title">{{
          t("team.dashboard.invitationsTitle")
        }}</span>
        <Badge variant="count">{{ pendingInvitations.length }}</Badge>
      </button>
      <button
        type="button"
        class="card"
        data-testid="team-dashboard-ai"
        @click="openAiProviders"
      >
        <Cpu :size="16" aria-hidden="true" />
        <span class="card__title">{{ t("team.dashboard.aiTitle") }}</span>
        <Badge variant="neutral">{{ t("team.dashboard.open") }}</Badge>
      </button>
    </section>
  </PageFrame>
</template>

<style scoped>
.profile {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-4) var(--space-5);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.profile__avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-lg);
  height: var(--size-control-lg);
  border-radius: var(--radius-md);
  background: var(--accent-surface);
  color: var(--accent);
  flex-shrink: 0;
}

.profile__body {
  flex: 1;
  min-width: 0;
}

.profile__name {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.profile__meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin: var(--space-1) 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.profile__slug {
  font-family: var(--font-mono, monospace);
}

.section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.section__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.section__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
}

.projects {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.project {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  cursor: pointer;
  text-align: left;
}

.project:hover {
  border-color: var(--accent);
}

.project__code {
  font-family: var(--font-mono, monospace);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.project__name {
  flex: 1;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.cards {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.card {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
  cursor: pointer;
  text-align: left;
  color: var(--text-secondary);
}

.card:hover {
  border-color: var(--accent);
}

.card__title {
  flex: 1;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}
</style>
