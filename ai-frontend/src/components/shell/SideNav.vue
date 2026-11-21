<script setup lang="ts">
import { computed, ref, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import {
  ChevronDown,
  ChevronRight,
  Keyboard,
  PanelLeftClose,
  PanelLeftOpen,
  Search
} from "lucide-vue-next";
import { useRoute } from "vue-router";

import IconTooltip from "./IconTooltip.vue";

import { NavigationGroup } from "@/components/ui/navigation-group";
import { SidebarItem } from "@/components/ui/sidebar-item";
import {
  isNavChildActive,
  isNavItemActive,
  mainNavGroups,
  type NavGroup,
  type NavItem
} from "@/router/navigation";
import { useShellStore } from "@/stores/shell";
import {
  useReviewCenterStore,
  REVIEW_CENTER_PATH
} from "@/stores/reviewCenter";

/**
 * 左导航（Figma「三层」画布 157:3603 · Global Navigation — Level 3 Active）。
 *
 * L2 = 业务域分组扁平列表（mainNavGroups 单源，顺序即视觉顺序）：
 *   全局 → 项目总览 → 开发 → 知识 → 环境 → 管理。
 *   四个项目域组（projectScoped）仅在选中项目时渲染
 *   （shell.currentProjectId 单源，R2-D4：无项目不渲染、不伪造上下文）；
 *   管理组默认折叠（Figma ADMINISTRATION Collapsed 态）。
 * 项目/团队上下文切换只在顶栏（"CONTEXT SELECTOR · NOT A NAVIGATION
 * LEVEL"），侧栏不内嵌切换器；分组间分隔线按 Figma 位置：全局组后、
 * 环境组与管理组之间。
 *
 * 层级统一由 DS 组件组合（NavigationGroup 13:33391 + SidebarItem 13:33329）。
 * 两态（Figma 7-3454 展开 232px / 7:3724 折叠 60px）：
 *   - 展开：分组可折叠、条目带文案、开发环境带子菜单
 *   - 折叠：分组标题行隐藏为纯图标、组间分隔线、未读项圆点，悬停出 Tooltip
 */

const emit = defineEmits<{ "open-search": [] }>();

const { t } = useI18n();
const route = useRoute();
const shell = useShellStore();
const reviewCenter = useReviewCenterStore();

// 徽标数据：随当前项目拉聚合（R09；静默进行，不阻塞导航渲染）
watch(
  () => shell.currentProjectId,
  pid => {
    if (pid) void reviewCenter.ensureLoaded(pid);
  },
  { immediate: true }
);

/** 各分组折叠状态；管理组默认折叠（Figma ADMINISTRATION Collapsed） */
const collapsedGroups = ref<Record<string, boolean>>(
  Object.fromEntries(
    mainNavGroups
      .filter(group => group.defaultCollapsed)
      .map(group => [group.id, true])
  )
);

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

/**
 * 渲染列表：项目域组按当前项目条件渲染（保持 mainNavGroups 顺序）。
 */
const renderedGroups = computed<NavGroup[]>(() =>
  mainNavGroups.filter(
    group => !group.projectScoped || shell.currentProjectId !== null
  )
);

/** 分组分隔线（Figma：全局组后、环境组与管理组之间） */
function showDivider(index: number, group: NavGroup) {
  return index > 0 && (index === 1 || group.id === "administration");
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
 * 动态徽标数：知识评审（人工审核中心）= 项目级聚合待处理数（R09，
 * 与指挥中心面板同源）。聚合不可用或为 0 时不显示——不伪造数字（V1.1 §6.3）。
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
</script>

<template>
  <nav
    class="shell-sidenav"
    :class="{ 'is-collapsed': shell.sidebarCollapsed }"
    :aria-label="t('shell.app.side.label')"
    :data-state="shell.sidebarState"
  >
    <!-- 导航控制行：WORKSPACE 标识 + 折叠/展开开关（Figma：panel-left-close/open） -->
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
        <!-- 分组分隔线（Figma：全局组后 / 管理组前） -->
        <div
          v-if="showDivider(index, group)"
          class="shell-sidenav__divider"
          aria-hidden="true"
        />

        <NavigationGroup
          :title="t(group.labelKey)"
          :headerless="shell.sidebarCollapsed"
          :collapsed="groupCollapsed(group)"
          @update:collapsed="value => onGroupToggle(group, value)"
        >
          <template v-for="item in group.items" :key="item.path">
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

            <!-- 开发环境子菜单（Devices/Executors/Runners），父项激活时呈现 -->
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

          <!-- 全局搜索动作项（Figma GLOBAL 组末条）：无路由，打开 ⌘K 命令面板。
               仅全局组渲染——动作项挂在分组 v-for 内，缺组级守卫会在每组重复 -->
          <template v-if="group.id === 'global'">
            <IconTooltip
              v-if="shell.sidebarCollapsed"
              :content="t('shell.app.side.item.globalSearch')"
              side="right"
            >
              <SidebarItem
                :label="t('shell.app.side.item.globalSearch')"
                :icon="Search"
                collapsed
                :aria-label="t('shell.app.side.item.globalSearch')"
                @click="emit('open-search')"
              />
            </IconTooltip>
            <SidebarItem
              v-else
              :label="t('shell.app.side.item.globalSearch')"
              :icon="Search"
              @click="emit('open-search')"
            />
          </template>
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

/* 组分隔线（Figma 分组边界） */
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
</style>
