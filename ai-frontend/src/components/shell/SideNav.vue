<script setup lang="ts">
import { computed, ref, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import {
  ChevronDown,
  ChevronRight,
  Keyboard,
  PanelLeftClose,
  PanelLeftOpen
} from "lucide-vue-next";
import { useRoute } from "vue-router";

import IconTooltip from "./IconTooltip.vue";
import ProjectSwitcher from "./ProjectSwitcher.vue";

import { NavigationGroup } from "@/components/ui/navigation-group";
import { SidebarItem } from "@/components/ui/sidebar-item";
import {
  globalNavConfig,
  isNavChildActive,
  isNavItemActive,
  projectNavConfig,
  settingsNavConfig,
  type NavGroup,
  type NavItem,
  type NavRegionId
} from "@/router/navigation";
import { useShellStore } from "@/stores/shell";
import { useAssistantStore } from "@/stores/assistant";
import {
  useReviewCenterStore,
  REVIEW_CENTER_PATH
} from "@/stores/reviewCenter";

/**
 * 左导航（AI R&D Workspace Application Shell §2B + Figma「03/04/05」）。
 *
 * 一级 = 三个导航上下文（不混成一张扁平菜单）：
 *   personal —— 个人
 *   team     —— 工作区
 *   project  —— 当前项目（有当前项目时渲染）
 * 遗留管理入口（NAV-0103 待核验）作为末尾一级组原样保留。
 *
 * 层级统一由 DS 组件组合（NavigationGroup 13:33391 + SidebarItem 13:33329）：
 *   一级 NavigationGroup = 上下文；二级 NavigationGroup = 上下文内分区
 *   （项目总控台 / 项目业务模块 / 项目设置）。
 *
 * 两态（Figma 7-3454 展开 232px / 7:3724 折叠 60px）：
 *   - 展开：一级/二级分组可折叠、条目带文案、Repository 带上下文子菜单
 *   - 折叠：所有分组行隐藏为纯图标、组间加分隔线、未读项带圆点，悬停出 Tooltip
 */

const { t } = useI18n();
const route = useRoute();
const shell = useShellStore();
const assistant = useAssistantStore();
const reviewCenter = useReviewCenterStore();

// 徽标数据：随当前项目拉聚合（R09；静默进行，不阻塞导航渲染）
watch(
  () => shell.currentProjectId,
  pid => {
    if (pid) void reviewCenter.ensureLoaded(pid);
  },
  { immediate: true }
);

/** 各分组（一级上下文与二级分区共用）的折叠状态，默认全部展开 */
const collapsedGroups = ref<Record<string, boolean>>({});

const currentPath = computed(() => route.path);

function isGroupCollapsed(group: { id: string }) {
  return collapsedGroups.value[group.id] === true;
}

/**
 * 侧栏折叠态下强制展开分组（此时分组标题行隐藏，条目全部以图标呈现，
 * 与折叠态视觉规格一致）。
 */
function groupCollapsed(group: { id: string }) {
  return shell.sidebarCollapsed ? false : isGroupCollapsed(group);
}

function onGroupToggle(group: { id: string }, collapsed: boolean) {
  collapsedGroups.value = {
    ...collapsedGroups.value,
    [group.id]: collapsed
  };
}

/** 一级导航上下文（对应 NAV_REGION_ORDER，渲染顺序即视觉顺序） */
interface NavContextView {
  id: NavRegionId;
  labelKey: string;
  badge?: number;
  sections: NavGroup[];
}

/**
 * 渲染树：Personal → Team → Current Project（有当前项目时）→ 遗留管理入口。
 *
 * 每个上下文内部按分区拆列（二级 NavigationGroup）；无独立分区的上下文
 * （个人/工作区/管理）用 headerless 分区承载条目，避免出现重复的组标题。
 */
const renderedContexts = computed<NavContextView[]>(() => {
  const contexts: NavContextView[] = [];

  const personal = globalNavConfig.find(group => group.id === "personal");
  if (personal) {
    contexts.push({
      id: "personal",
      labelKey: "shell.app.side.group.personal",
      sections: [{ ...personal, id: "personalItems", headerless: true }]
    });
  }

  const workspace = globalNavConfig.find(group => group.id === "workspace");
  if (workspace) {
    contexts.push({
      id: "team",
      labelKey: "shell.app.side.group.workspace",
      sections: [{ ...workspace, id: "workspaceItems", headerless: true }]
    });
  }

  // 无当前项目时整个项目区不渲染：不出现灰色禁用的空项目菜单、
  // 不伪造默认项目上下文（R2-D4）。
  if (shell.currentProjectId) {
    const sections = projectNavConfig.map(group =>
      group.id === "projectHome" ? { ...group, headerless: true } : group
    );
    sections.push(settingsNavConfig);
    contexts.push({
      id: "project",
      labelKey: "shell.app.side.group.project",
      sections
    });
  }

  const admin = globalNavConfig.find(group => group.id === "admin");
  if (admin) {
    contexts.push({
      id: "adminLegacy",
      labelKey: admin.labelKey,
      badge: admin.badge,
      sections: [{ ...admin, id: "adminItems", headerless: true }]
    });
  }

  return contexts;
});

function isActive(item: NavItem) {
  if (isNavItemActive(item, currentPath.value)) return true;
  // 子路由前缀兜底：跨父前缀的子项（如 devenv 下的 Runners
  // /project/settings/runners）命中时父项同样激活，保证子菜单可见。
  return (
    item.children?.some(
      child =>
        currentPath.value === child.path ||
        currentPath.value.startsWith(`${child.path}/`)
    ) ?? false
  );
}

/**
 * 动态徽标数：人工审核中心 = 项目级聚合待处理数（R09，与指挥中心面板同源）。
 * 聚合不可用或为 0 时不显示——不伪造数字（V1.1 §6.3）。
 */
function itemBadge(item: NavItem): number | undefined {
  if (item.path === REVIEW_CENTER_PATH) {
    const total = reviewCenter.totalCount;
    return total !== null && total > 0 ? total : undefined;
  }
  return item.badge;
}

/** 该项是否带未读圆点（Figma：收件箱未读、AI 执行中心未读） */
function hasUnreadDot(item: NavItem) {
  const badge = itemBadge(item);
  return badge !== undefined && badge > 0;
}

/** 折叠态下 Tooltip 的文案（图标按钮的可访问名称） */
function itemLabel(item: NavItem) {
  return t(item.labelKey);
}

/** SidebarItem 的接线参数（两态共用一个数据源，避免分支漂移） */
function itemBindings(item: NavItem): {
  label: string;
  icon: Component;
  to: string;
  collapsed: boolean;
  state: "active" | "default";
  badge: string | number | undefined;
  badgeType: "count" | "status";
} {
  const badge = itemBadge(item);
  const planned = item.planned === true;
  return {
    label: itemLabel(item),
    icon: item.icon,
    to: item.path,
    collapsed: shell.sidebarCollapsed,
    state: isActive(item) ? "active" : "default",
    // PLANNED 占位项用状态徽标呈现规划标识（不伪装已上线）
    badge: badge ?? (planned ? t("shell.app.side.plannedBadge") : undefined),
    badgeType: badge === undefined && planned ? "status" : "count"
  };
}

/**
 * 切换项目（Figma「07 — Project Switching」）。
 *
 * 两阶段：store 进入 switching 态（不显示旧项目数据），
 * 新上下文就绪后才落地新项目；目标目的地保持不变
 * （同一目的地在新项目下解析，不可达时回落到项目概览）。
 */
async function onSelectProject(id: string) {
  // 切项目守卫：有活跃会话时先确认（G12 · DR-C6）
  shell.requestProjectSwitch(id, assistant.activeConversationId !== null);
}
</script>

<template>
  <nav
    class="shell-sidenav"
    :class="{ 'is-collapsed': shell.sidebarCollapsed }"
    :aria-label="t('shell.app.side.label')"
    :data-state="shell.sidebarState"
  >
    <!-- 导航控制行：折叠/展开开关（Figma：panel-left-close / panel-left-open） -->
    <div class="shell-sidenav__control">
      <template v-if="!shell.sidebarCollapsed">
        <span class="shell-sidenav__control-label">WORKSPACE</span>
      </template>
      <button
        type="button"
        class="shell-sidenav__control-button"
        :aria-label="
          shell.sidebarCollapsed
            ? t('shell.app.side.expand')
            : t('shell.app.side.collapse')
        "
        :aria-expanded="!shell.sidebarCollapsed"
        @click="shell.toggleSidebar()"
      >
        <PanelLeftOpen v-if="shell.sidebarCollapsed" :size="16" />
        <PanelLeftClose v-else :size="16" />
      </button>
    </div>

    <div class="shell-sidenav__groups">
      <template v-for="(context, index) in renderedContexts" :key="context.id">
        <!-- 上下文分隔线（V1.2 §6.1：区域边界） -->
        <div
          v-if="index > 0"
          class="shell-sidenav__divider"
          aria-hidden="true"
        />

        <!-- 一级：导航上下文（个人 / 工作区 / 当前项目 / 管理） -->
        <NavigationGroup
          :title="t(context.labelKey)"
          :badge="context.badge"
          :headerless="shell.sidebarCollapsed"
          :collapsed="groupCollapsed(context)"
          @update:collapsed="value => onGroupToggle(context, value)"
        >
          <!-- 紧凑 Project Switcher（R2-D3：复用现有切换守卫链
               requestProjectSwitch；折叠态收紧为图标入口，项目名识别由
               顶栏项目芯片承载。旧版大尺寸项目头不恢复——展开过大已裁撤） -->
          <div v-if="context.id === 'project'" class="shell-sidenav__project">
            <ProjectSwitcher
              variant="header"
              searchable
              :projects="shell.projects"
              :current-id="shell.currentProjectId"
              :switching-to-id="shell.switchingProjectId"
              @select="onSelectProject"
            />
          </div>

          <!-- 二级：上下文内分区（项目总控台 / 项目业务模块 / 项目设置） -->
          <NavigationGroup
            v-for="section in context.sections"
            :key="section.id"
            :title="t(section.labelKey)"
            :headerless="shell.sidebarCollapsed || section.headerless"
            :collapsed="groupCollapsed(section)"
            @update:collapsed="value => onGroupToggle(section, value)"
          >
            <template v-for="item in section.items" :key="item.path">
              <IconTooltip
                v-if="shell.sidebarCollapsed"
                :content="itemLabel(item)"
                side="right"
              >
                <SidebarItem
                  v-bind="itemBindings(item)"
                  :aria-label="itemLabel(item)"
                >
                  <template v-if="hasUnreadDot(item)" #trailing>
                    <span class="shell-sidenav__dot" aria-hidden="true" />
                  </template>
                </SidebarItem>
              </IconTooltip>

              <SidebarItem v-else v-bind="itemBindings(item)">
                <template v-if="item.expandable" #trailing>
                  <ChevronDown
                    v-if="item.children"
                    :size="12"
                    class="shell-sidenav__chevron"
                    aria-hidden="true"
                  />
                  <ChevronRight
                    v-else
                    :size="12"
                    class="shell-sidenav__chevron"
                    aria-hidden="true"
                  />
                </template>
              </SidebarItem>

              <!-- 上下文子菜单（Figma：Repository 下的 Overview / Branches） -->
              <div
                v-if="
                  !shell.sidebarCollapsed &&
                  item.children?.length &&
                  isActive(item)
                "
                class="shell-sidenav__submenu"
              >
                <RouterLink
                  v-for="child in item.children"
                  :key="child.path"
                  class="shell-sidenav__subitem"
                  :class="{
                    'is-active': isNavChildActive(child, currentPath)
                  }"
                  :to="child.path"
                >
                  {{ t(child.labelKey) }}
                  <span v-if="child.planned" class="shell-sidenav__planned">{{
                    t("shell.app.side.plannedBadge")
                  }}</span>
                </RouterLink>
              </div>
            </template>
          </NavigationGroup>
        </NavigationGroup>
      </template>
    </div>

    <!-- 底部键盘提示（展开态带文案与 ? 芯片；折叠态仅图标） -->
    <div class="shell-sidenav__footer">
      <span v-if="!shell.sidebarCollapsed" class="shell-sidenav__shortcut">
        <Keyboard :size="14" aria-hidden="true" />
        <span class="shell-sidenav__shortcut-text">{{
          t("shell.app.side.shortcuts")
        }}</span>
        <kbd class="shell-sidenav__kbd">?</kbd>
      </span>
      <IconTooltip
        v-else
        :content="t('shell.app.side.shortcutsHint')"
        side="right"
      >
        <span class="shell-sidenav__shortcut-icon" aria-hidden="true">
          <Keyboard :size="14" />
        </span>
      </IconTooltip>
    </div>
  </nav>
</template>

<style scoped>
.shell-sidenav {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  width: 100%;
  height: 100%;
  padding: var(--space-3) var(--space-2);
  overflow-y: auto;
  background: var(--ws-sidebar);
  border-right: var(--border-1) solid var(--ws-border);
}

/* 导航控制行 */
.shell-sidenav__control {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: var(--size-control-sm);
  padding: 0 0 0 var(--space-3);
}

.shell-sidenav__control-label {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
}

.shell-sidenav__control-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-sidenav__control-button:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.shell-sidenav__control-button:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 分组 */
.shell-sidenav__groups {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

/* 折叠态：分组条目占满整行（DS 分组默认左缩进在图标态下不适用） */
.shell-sidenav.is-collapsed :deep(.ui-nav-group__items) {
  padding-left: 0;
}

/* 项目标记行 */
.shell-sidenav__project {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
  padding: 0 var(--space-3);
}

.shell-sidenav__chevron {
  flex: none;
  color: var(--ws-text-secondary);
}

/* 折叠态未读圆点（侧栏折叠时徽标隐藏，仅保留圆点） */
.shell-sidenav__dot {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  width: var(--space-1);
  height: var(--space-1);
  background: var(--ws-accent);
  border-radius: var(--radius-full);
}

/* 组分隔线（上下文边界） */
.shell-sidenav__divider {
  height: var(--border-1);
  margin: 0 var(--space-3);
  background: var(--ws-border);
}

/* 上下文子菜单（缩进 36px = Figma 0 8 0 36） */
.shell-sidenav__submenu {
  display: flex;
  flex-direction: column;
  padding: 0 var(--space-2) 0 calc(var(--space-8) + var(--space-1));
}

.shell-sidenav__subitem {
  display: flex;
  align-items: center;
  height: var(--size-control-compact);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  text-decoration: none;
  border-left: var(--border-1) solid var(--ws-border);
}

.shell-sidenav__subitem:hover {
  color: var(--ws-text-primary);
}

.shell-sidenav__subitem:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.shell-sidenav__subitem.is-active {
  color: var(--ws-accent);
  border-left-color: var(--ws-accent);
}

/* 底部 */
.shell-sidenav__footer {
  display: flex;
  align-items: center;
  height: var(--size-control-compact);
  padding: 0 var(--space-3);
  margin-top: auto;
  border-top: var(--border-1) solid var(--ws-border);
}

.shell-sidenav__shortcut {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.shell-sidenav__shortcut-icon {
  display: inline-flex;
  color: var(--ws-text-secondary);
}

/* 键位芯片（Figma Keyboard shortcut：elevated 底 + 1px 描边 + mono） */
.shell-sidenav__kbd {
  padding: 1px var(--space-1);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

@media (prefers-reduced-motion: reduce) {
  :deep(.ui-sidebar-item),
  .shell-sidenav__control-button {
    transition: none;
  }
}

/* 规划项标识（V1.2 §11.12：占位能力不得伪装已上线） */
.shell-sidenav__planned {
  padding: 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

/* 折叠态：项目头收紧为图标入口 */
.shell-sidenav.is-collapsed .shell-sidenav__project {
  justify-content: center;
  padding: 0;
}

.shell-sidenav.is-collapsed
  .shell-sidenav__project
  :deep(.shell-project-switcher__name),
.shell-sidenav.is-collapsed
  .shell-sidenav__project
  :deep(.shell-project-switcher__chevron) {
  display: none;
}
</style>
