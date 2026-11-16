<script setup lang="ts">
import { computed, ref, watch } from "vue";
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
 * 左导航（AI R&D Workspace Application Shell §2B + Figma「04 — Global Navigation」）。
 *
 * 三种导航上下文分列四组，不混成一张扁平菜单：
 *   personal / workspace —— 全局与个人上下文
 *   modules —— 当前项目下的业务模块（带项目上下文头）
 *   admin —— 管理
 *
 * 两态（Figma 7-3454 展开 232px / 7:3724 折叠 60px）：
 *   - 展开：分组可折叠、条目带文案、Repository 带二级上下文子菜单、底部键盘提示
 *   - 折叠：纯图标、组间加分隔线、未读项带圆点，悬停出 Tooltip
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

/** 展开态下各分组的折叠状态（key = 分组 id，默认全部展开） */
const collapsedGroups = ref<Record<string, boolean>>({});

const currentPath = computed(() => route.path);

function isGroupCollapsed(group: NavGroup) {
  return collapsedGroups.value[group.id] === true;
}

function toggleGroup(group: NavGroup) {
  collapsedGroups.value = {
    ...collapsedGroups.value,
    [group.id]: !isGroupCollapsed(group)
  };
}

/** 组 → 导航区域映射（R2-D1 区域排序规则，见 navigation.ts NAV_REGION_ORDER）。 */
const REGION_BY_GROUP_ID: Record<string, NavRegionId> = {
  personal: "personal",
  workspace: "team",
  projectHome: "project",
  modules: "project",
  projectSettings: "project",
  admin: "adminLegacy"
};

/**
 * 渲染队列（V1.2 §6.1，区域顺序 = NAV_REGION_ORDER）：
 * Personal → Team → Current Project（有当前项目时：总控台/业务模块/项目设置，
 * R2-D4 条件渲染）→ 遗留管理入口。无当前项目时整个项目区不渲染，
 * 不出现灰色禁用的空项目菜单、不伪造默认项目上下文。
 */
const renderedGroups = computed<NavGroup[]>(() => {
  const groups: NavGroup[] = [
    ...globalNavConfig.filter(
      group => group.id === "personal" || group.id === "workspace"
    )
  ];
  if (shell.currentProjectId) {
    groups.push(...projectNavConfig, settingsNavConfig);
  }
  groups.push(...globalNavConfig.filter(group => group.id === "admin"));
  return groups;
});

/**
 * 分组分隔线可见性（V1.2 §6.1：展开态区域边界各一条；折叠态：除首组外组间全加）。
 */
function showDivider(group: NavGroup, index: number) {
  if (shell.sidebarCollapsed) return index > 0;
  const prev = renderedGroups.value[index - 1];
  if (!prev) return false;
  return REGION_BY_GROUP_ID[prev.id] !== REGION_BY_GROUP_ID[group.id];
}

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
      <template v-for="(group, index) in renderedGroups" :key="group.id">
        <!-- 分组分隔线（V1.2 §6.1：区域边界；折叠态：除首组外组间） -->
        <div
          v-if="showDivider(group, index)"
          class="shell-sidenav__divider"
          aria-hidden="true"
        />

        <section class="shell-sidenav__group">
          <!-- 分组标题（折叠态隐藏，仅保留图标与分隔线；headerless 组无标题行） -->
          <button
            v-if="!shell.sidebarCollapsed && !group.headerless"
            type="button"
            class="shell-sidenav__group-toggle"
            :aria-expanded="!isGroupCollapsed(group)"
            @click="toggleGroup(group)"
          >
            <ChevronDown
              v-if="!isGroupCollapsed(group)"
              :size="10"
              class="shell-sidenav__chevron"
            />
            <ChevronRight v-else :size="10" class="shell-sidenav__chevron" />
            <span class="shell-sidenav__group-label">{{
              t(group.labelKey)
            }}</span>
            <span v-if="group.badge" class="shell-sidenav__group-badge">{{
              group.badge
            }}</span>
          </button>

          <!-- 分组内容（折叠态下标题隐藏，条目仍全部展示为图标）
               用 v-if 而非 v-show：收起的分组必须真正离开 DOM，
               否则其链接仍留在键盘 Tab 顺序与无障碍树中。 -->
          <div
            v-if="shell.sidebarCollapsed || !isGroupCollapsed(group)"
            class="shell-sidenav__items"
          >
            <!-- 紧凑 Project Switcher（R2-D3：复用现有切换守卫链
                 requestProjectSwitch；折叠态收紧为图标入口，项目名识别由
                 顶栏项目芯片承载。旧版大尺寸项目头不恢复——展开过大已裁撤） -->
            <div
              v-if="group.id === 'projectHome'"
              class="shell-sidenav__project"
            >
              <ProjectSwitcher
                variant="header"
                searchable
                :projects="shell.projects"
                :current-id="shell.currentProjectId"
                :switching-to-id="shell.switchingProjectId"
                @select="onSelectProject"
              />
            </div>

            <template v-for="item in group.items" :key="item.path">
              <IconTooltip
                v-if="shell.sidebarCollapsed"
                :content="itemLabel(item)"
                side="right"
              >
                <RouterLink
                  class="shell-sidenav__item is-collapsed"
                  :class="{ 'is-active': isActive(item) }"
                  :to="item.path"
                  :aria-label="itemLabel(item)"
                  :aria-current="isActive(item) ? 'page' : undefined"
                >
                  <component :is="item.icon" :size="16" aria-hidden="true" />
                  <span
                    v-if="hasUnreadDot(item)"
                    class="shell-sidenav__dot"
                    aria-hidden="true"
                  />
                </RouterLink>
              </IconTooltip>

              <RouterLink
                v-else
                class="shell-sidenav__item"
                :class="{ 'is-active': isActive(item) }"
                :to="item.path"
                :aria-current="isActive(item) ? 'page' : undefined"
              >
                <span class="shell-sidenav__active-bar" aria-hidden="true" />
                <component :is="item.icon" :size="16" aria-hidden="true" />
                <span class="shell-sidenav__item-label">{{
                  t(item.labelKey)
                }}</span>
                <span v-if="item.planned" class="shell-sidenav__planned">{{
                  t("shell.app.side.plannedBadge")
                }}</span>
                <span v-if="hasUnreadDot(item)" class="shell-sidenav__badge">
                  {{ itemBadge(item) }}
                </span>
                <ChevronDown
                  v-if="item.expandable && item.children"
                  :size="12"
                  class="shell-sidenav__chevron shell-sidenav__chevron--end"
                  aria-hidden="true"
                />
                <ChevronRight
                  v-else-if="item.expandable"
                  :size="12"
                  class="shell-sidenav__chevron shell-sidenav__chevron--end"
                  aria-hidden="true"
                />
              </RouterLink>

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
          </div>
        </section>
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

.shell-sidenav__group {
  display: flex;
  flex-direction: column;
}

.shell-sidenav__group-toggle {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-xs);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-nano);
  font-weight: var(--weight-medium);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: none;
}

.shell-sidenav__group-toggle:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.shell-sidenav__group-label {
  letter-spacing: var(--tracking-caption);
}

.shell-sidenav__group-badge {
  margin-left: auto;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.shell-sidenav__chevron {
  flex: none;
}

.shell-sidenav__chevron--end {
  margin-left: auto;
}

.shell-sidenav__items {
  display: flex;
  flex-direction: column;
}

/* 项目标记行 */
.shell-sidenav__project {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
  padding: 0 var(--space-3);
}

.shell-sidenav__project-dot {
  flex: none;
  width: var(--space-1);
  height: var(--space-1);
  background: var(--ws-accent);
  border-radius: var(--radius-xs);
}

.shell-sidenav__project-name {
  overflow: hidden;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 导航条目 */
.shell-sidenav__item {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-row-height);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  text-decoration: none;
  border-radius: var(--radius-sm);
}

.shell-sidenav__item:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.shell-sidenav__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.shell-sidenav__item.is-active {
  color: var(--ws-text-primary);
  font-weight: var(--weight-medium);
  background: var(--ws-color-accent-surface);
}

/* 激活指示条：2×16 accent（Figma 展开态左侧竖条） */
.shell-sidenav__active-bar {
  position: absolute;
  top: 50%;
  left: 0;
  width: var(--size-indicator);
  height: var(--space-4);
  background: var(--ws-accent);
  border-radius: var(--radius-xs);
  transform: translateY(-50%);
  opacity: 0;
}

.shell-sidenav__item.is-active .shell-sidenav__active-bar {
  opacity: 1;
}

.shell-sidenav__item-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shell-sidenav__badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: var(--size-badge-sm);
  min-width: 18px;
  margin-left: auto;
  padding: 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

/* 折叠态：纯图标方钮 */
.shell-sidenav__item.is-collapsed {
  justify-content: center;
  height: var(--ws-row-height);
  padding: 0;
}

.shell-sidenav__item.is-collapsed .shell-sidenav__active-bar {
  top: var(--space-2);
  height: var(--space-4);
  transform: none;
}

.shell-sidenav__dot {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  width: var(--space-1);
  height: var(--space-1);
  background: var(--ws-accent);
  border-radius: var(--radius-full);
}

/* 组分隔线（折叠态） */
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
  .shell-sidenav__item,
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
