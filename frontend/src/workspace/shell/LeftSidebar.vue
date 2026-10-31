<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import {
  Activity,
  Bot,
  BookOpen,
  ChevronLeft,
  FileText,
  LayoutDashboard,
  ListTree,
  Map,
  PackageCheck,
  Scale,
  Users
} from "lucide-vue-next";
import { useWorkspaceStore } from "../store";
import { demoProject } from "../fixtures";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const store = useWorkspaceStore();

interface NavItem {
  key: string;
  labelKey: string;
  icon: typeof Map;
  to: string;
}

interface NavGroup {
  key: string;
  labelKey: string;
  items: NavItem[];
}

const projectId = computed(
  () => (route.params.projectId as string | undefined) ?? ""
);

function workspaceTo(slug: string): string {
  return `/workspace/${projectId.value}/${slug}`;
}

const groups = computed<NavGroup[]>(() => [
  {
    key: "workspace",
    labelKey: "workspace.nav.groupWorkspace",
    items: [
      {
        key: "console",
        labelKey: "workspace.nav.console",
        icon: Map,
        to: workspaceTo("console")
      },
      {
        key: "team",
        labelKey: "workspace.nav.team",
        icon: Users,
        to: workspaceTo("team")
      },
      {
        key: "goals",
        labelKey: "workspace.nav.goals",
        icon: ListTree,
        to: workspaceTo("goals")
      },
      {
        key: "activity",
        labelKey: "workspace.nav.activity",
        icon: Activity,
        to: workspaceTo("activity")
      }
    ]
  },
  {
    key: "modules",
    labelKey: "workspace.nav.groupModules",
    items: [
      {
        key: "requirements",
        labelKey: "workspace.nav.requirements",
        icon: FileText,
        to: workspaceTo("requirements")
      },
      {
        key: "decisions",
        labelKey: "workspace.nav.decisions",
        icon: Scale,
        to: workspaceTo("decisions")
      },
      {
        key: "knowledge",
        labelKey: "workspace.nav.knowledge",
        icon: BookOpen,
        to: workspaceTo("knowledge")
      },
      {
        key: "artifacts",
        labelKey: "workspace.nav.artifacts",
        icon: PackageCheck,
        to: workspaceTo("artifacts")
      }
    ]
  },
  {
    key: "classic",
    labelKey: "workspace.nav.groupClassic",
    items: [
      {
        key: "classic-overview",
        labelKey: "workspace.nav.classicOverview",
        icon: LayoutDashboard,
        to: "/dashboard"
      }
    ]
  }
]);

const activeTo = computed(() => route.path);

function isActive(item: NavItem): boolean {
  return activeTo.value === item.to;
}

function onItem(item: NavItem) {
  router.push(item.to);
}
</script>

<template>
  <nav class="ws-leftnav" aria-label="workspace">
    <div v-for="group in groups" :key="group.key" class="ws-nav-group">
      <div class="ws-nav-group-label">{{ t(group.labelKey) }}</div>
      <button
        v-for="item in group.items"
        :key="item.key"
        type="button"
        class="ws-nav-item ws-focusable"
        :class="{ 'is-active': isActive(item) }"
        @click="onItem(item)"
      >
        <component :is="item.icon" :size="16" aria-hidden="true" />
        <span class="ws-nav-label">{{ t(item.labelKey) }}</span>
      </button>
    </div>

    <div class="ws-leftnav-footer">
      <div class="ws-iteration">
        <span class="ws-iteration-label">
          {{ t("workspace.service.iteration") }}
        </span>
        <span class="ws-iteration-value">{{ demoProject.iteration }}</span>
      </div>
      <div class="ws-service" data-test="service-ok">
        <span
          class="ws-service-dot"
          data-test="service-dot"
          aria-hidden="true"
        />
        <span class="ws-nav-label">{{ t("workspace.service.ok") }}</span>
      </div>
      <button
        type="button"
        class="ws-icon-btn ws-focusable"
        data-test="sidebar-toggle"
        :aria-label="
          store.sidebarCollapsed
            ? t('workspace.shell.expand')
            : t('workspace.shell.collapse')
        "
        @click="store.toggleSidebar()"
      >
        <ChevronLeft :size="16" aria-hidden="true" />
      </button>
    </div>
  </nav>
</template>

<style scoped>
.ws-leftnav {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 12px 8px;
  gap: 4px;
}

.ws-nav-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-bottom: 12px;
}

.ws-nav-group-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  padding: 8px 10px 4px;
}

.ws-nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  height: var(--ws-row-height);
  padding: 0 10px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
  cursor: pointer;
  text-align: left;
}

.ws-nav-item:hover {
  background: var(--ws-elevated);
}

/* 紫色克制：AI 紫仅用于选中态 */
.ws-nav-item.is-active {
  background: var(--ws-elevated);
  color: var(--ws-accent);
  font-weight: 600;
}

.ws-leftnav-footer {
  margin-top: auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 6px 0;
  border-top: 1px solid var(--ws-border);
}

.ws-service {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-service-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ws-color-success);
}

.ws-iteration {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 8px;
}

.ws-iteration-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-iteration-value {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
}
</style>
