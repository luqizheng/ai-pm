<script setup lang="ts">
import { computed, ref } from "vue";
import {
  ChevronDown,
  ChevronRight,
  Lock,
  Repeat,
  Settings2
} from "lucide-vue-next";

import {
  isNavV2Allowed,
  type NavV2Domain,
  type NavV2Page,
  type NavV2Role
} from "./model";

/**
 * NavV2Sidebar —— Main Navigation V2.0 三级导航交互原型（仅设计目录用）。
 *
 * L1 业务域（常驻）→ L2 功能模块（可折叠）→ L3 功能页（可点选）。
 * 多组同时展开；父级在子页激活时保持展开；角色域整体隐藏、角色页受限态。
 * 分组结构 = 间距节奏 + 图标瓦片 + 子树缩进导轨（无分割线、无卡片堆砌）。
 * 折叠态 = 60px 图标栏（未读圆点 + 原生 title 提示）。
 * 项目上下文：project 传入时顶部渲染项目头（名称 / 状态 / 切换 / 操作）。
 */
const props = withDefaults(
  defineProps<{
    domains: NavV2Domain[];
    role?: NavV2Role;
    collapsed?: boolean;
    project?: { name: string; status: string };
    initialActive?: string;
    initialExpanded?: string[];
    initialExpandedModules?: string[];
  }>(),
  {
    role: "member",
    collapsed: false,
    project: undefined,
    initialActive: undefined,
    initialExpanded: () => [],
    initialExpandedModules: () => []
  }
);

const emit = defineEmits<{
  navigate: [
    trail: { domainId: string; moduleId: string; pageId: string; label: string }
  ];
}>();

const activePage = ref(props.initialActive);
const expandedDomains = ref(new Set(props.initialExpanded));
const expandedModules = ref(new Set(props.initialExpandedModules));

const visibleDomains = computed(() =>
  props.domains.filter(domain => isNavV2Allowed(domain, props.role))
);

function isDomainExpanded(domain: NavV2Domain) {
  return expandedDomains.value.has(domain.id);
}

function isModuleExpanded(moduleId: string) {
  return expandedModules.value.has(moduleId);
}

function toggleDomain(domain: NavV2Domain) {
  if (props.collapsed) return;
  const next = new Set(expandedDomains.value);
  if (!next.delete(domain.id)) next.add(domain.id);
  expandedDomains.value = next;
}

function toggleModule(moduleId: string) {
  const next = new Set(expandedModules.value);
  if (!next.delete(moduleId)) next.add(moduleId);
  expandedModules.value = next;
}

/** 页是否可点：角色不满足 = 受限态（不导航） */
function isPageRestricted(page: NavV2Page) {
  return !isNavV2Allowed(page, props.role);
}

/** 域 / 模块内是否含激活页（父级保持高亮） */
function containsActive(domain: NavV2Domain): boolean {
  return domain.modules.some(module =>
    module.pages.some(page => page.id === activePage.value)
  );
}

function onSelect(domain: NavV2Domain, moduleId: string, page: NavV2Page) {
  if (isPageRestricted(page)) return;
  activePage.value = page.id;
  emit("navigate", {
    domainId: domain.id,
    moduleId,
    pageId: page.id,
    label: page.label
  });
}

/** 域徽标合计（折叠态折叠为圆点） */
function domainBadge(domain: NavV2Domain): number {
  return domain.modules.reduce(
    (sum, module) =>
      sum +
      module.pages.reduce((pageSum, page) => pageSum + (page.badge ?? 0), 0),
    0
  );
}
</script>

<template>
  <nav
    class="navv2"
    :class="{ 'is-collapsed': collapsed }"
    aria-label="PM-AI 导航 V2 原型"
  >
    <!-- 项目上下文头（§5）：名称 / 状态 / 切换器 / 项目级操作 -->
    <div v-if="project && !collapsed" class="navv2__project">
      <div class="navv2__project-name">
        <span class="navv2__project-title">{{ project.name }}</span>
        <span class="navv2__project-status">
          <span class="navv2__status-dot" aria-hidden="true" />
          {{ project.status }}
        </span>
      </div>
      <div class="navv2__project-actions">
        <button
          type="button"
          class="navv2__iconbtn"
          aria-label="切换项目"
          title="切换项目"
        >
          <Repeat :size="13" />
        </button>
        <button
          type="button"
          class="navv2__iconbtn"
          aria-label="项目操作"
          title="项目操作"
        >
          <Settings2 :size="13" />
        </button>
      </div>
    </div>

    <template v-for="(domain, index) in visibleDomains" :key="domain.id">
      <!-- L1 业务域（组间用间距节奏，不用分割线） -->
      <button
        type="button"
        class="navv2__l1"
        :class="{
          'is-first': index === 0,
          'is-open': !collapsed && isDomainExpanded(domain),
          'is-active': containsActive(domain)
        }"
        :title="collapsed ? domain.label : undefined"
        :aria-expanded="collapsed ? undefined : isDomainExpanded(domain)"
        @click="toggleDomain(domain)"
      >
        <span class="navv2__l1-icon">
          <component :is="domain.icon" :size="15" />
          <span
            v-if="collapsed && domainBadge(domain) > 0"
            class="navv2__dot"
            aria-hidden="true"
          />
        </span>
        <span v-if="!collapsed" class="navv2__l1-label">{{
          domain.label
        }}</span>
        <span
          v-if="!collapsed && domainBadge(domain) > 0"
          class="navv2__badge"
          >{{ domainBadge(domain) }}</span
        >
        <ChevronDown
          v-if="!collapsed"
          :size="12"
          class="navv2__chevron"
          aria-hidden="true"
        />
      </button>

      <!-- L2 功能模块 → L3 功能页（子树缩进导轨承载体） -->
      <div v-if="!collapsed && isDomainExpanded(domain)" class="navv2__subtree">
        <template v-for="module in domain.modules" :key="module.id">
          <button
            v-if="module.label"
            type="button"
            class="navv2__l2"
            :aria-expanded="isModuleExpanded(module.id)"
            @click="toggleModule(module.id)"
          >
            <component
              :is="isModuleExpanded(module.id) ? ChevronDown : ChevronRight"
              :size="10"
              class="navv2__chevron"
              aria-hidden="true"
            />
            <span class="navv2__l2-label">{{ module.label }}</span>
          </button>

          <div
            v-if="!module.label || isModuleExpanded(module.id)"
            class="navv2__l3list"
            :class="{ 'is-direct': !module.label }"
          >
            <button
              v-for="page in module.pages"
              :key="page.id"
              type="button"
              class="navv2__l3"
              :class="{
                'is-active': page.id === activePage,
                'is-restricted': isPageRestricted(page)
              }"
              :title="isPageRestricted(page) ? '需要管理员权限' : undefined"
              @click="onSelect(domain, module.id, page)"
            >
              <span class="navv2__l3-label">{{ page.label }}</span>
              <Lock
                v-if="isPageRestricted(page)"
                :size="11"
                class="navv2__lock"
                aria-hidden="true"
              />
              <span v-else-if="page.planned" class="navv2__planned">规划</span>
              <span v-else-if="page.badge" class="navv2__badge">{{
                page.badge
              }}</span>
            </button>
            <p v-if="module.pages.length === 0" class="navv2__empty">
              暂无页面
            </p>
          </div>
        </template>
      </div>
    </template>
  </nav>
</template>

<style scoped>
.navv2 {
  display: flex;
  flex-direction: column;
  width: 264px;
  height: 100%;
  padding: var(--space-3) var(--space-2) var(--space-3);
  overflow-y: auto;
  background: var(--ws-sidebar);
  border-right: var(--border-1) solid var(--ws-border);
}

/* 折叠态：60px 纯图标栏 */
.navv2.is-collapsed {
  width: 60px;
  padding: var(--space-3) var(--space-1) var(--space-3);
  align-items: center;
}

/* ===== 项目上下文头 ===== */
.navv2__project {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  height: 44px;
  margin-bottom: var(--space-3);
  padding: 0 var(--space-2);
  background: var(--accent-surface);
  border-radius: var(--radius-sm);
}

.navv2__project-name {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.navv2__project-title {
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-medium);
  color: var(--ws-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.navv2__project-status {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

/* 项目运行状态点（accent，静态不闪） */
.navv2__status-dot {
  width: 5px;
  height: 5px;
  flex: none;
  background: var(--ws-accent);
  border-radius: var(--radius-full);
}

.navv2__project-actions {
  display: flex;
  gap: var(--space-1);
  flex: none;
}

.navv2__iconbtn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-xs);
}

.navv2__iconbtn:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

/* ===== L1 业务域：间距节奏 + 图标瓦片（替代分割线） ===== */
.navv2__l1 {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: 40px;
  margin-top: var(--space-3);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-medium);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.navv2__l1.is-first {
  margin-top: 0;
}

.navv2__l1:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.navv2__l1.is-active,
.navv2__l1.is-open {
  color: var(--ws-text-primary);
}

.navv2__l1:focus-visible,
.navv2__l2:focus-visible,
.navv2__l3:focus-visible,
.navv2__iconbtn:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 图标瓦片：给域一个轻量视觉锚点（无描边、无渐变） */
.navv2__l1-icon {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: 24px;
  height: 24px;
  color: var(--ws-text-secondary);
  background: var(--ws-elevated);
  border-radius: var(--radius-sm);
  transition:
    color 120ms ease,
    background-color 120ms ease;
}

.navv2__l1:hover .navv2__l1-icon {
  color: var(--ws-text-primary);
}

.navv2__l1.is-open .navv2__l1-icon,
.navv2__l1.is-active .navv2__l1-icon {
  color: var(--ws-accent);
  background: var(--accent-surface);
}

.navv2.is-collapsed .navv2__l1 {
  width: 40px;
  height: 40px;
  justify-content: center;
  padding: 0;
}

.navv2.is-collapsed .navv2__l1.is-active {
  color: var(--ws-accent);
  background: var(--accent-surface);
}

.navv2__l1-label {
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.navv2__chevron {
  flex: none;
  color: var(--ws-text-secondary);
  transition: transform 140ms ease;
}

/* L1 收起时指向右、展开时指向下（微交互） */
.navv2__l1 .navv2__chevron {
  transform: rotate(-90deg);
}

.navv2__l1.is-open .navv2__chevron {
  transform: rotate(0deg);
}

/* ===== 子树：L2 模块 + L3 页，左缘树形导轨 ===== */
.navv2__subtree {
  position: relative;
  display: flex;
  flex-direction: column;
  margin: 2px 0 var(--space-1) 19px;
  padding-left: var(--space-2);
  animation: navv2-reveal 140ms ease;
}

/* 树形导轨：1px 竖线替代分割线的结构提示 */
.navv2__subtree::before {
  content: "";
  position: absolute;
  top: 4px;
  bottom: 6px;
  left: 0;
  width: var(--border-1);
  background: var(--ws-border);
  border-radius: var(--radius-full);
}

/* ===== L2 功能模块 ===== */
.navv2__l2 {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  width: 100%;
  height: 30px;
  padding: 0 var(--space-2) 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-xs);
}

.navv2__l2:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.navv2__l2-label {
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ===== L3 功能页 ===== */
.navv2__l3list {
  display: flex;
  flex-direction: column;
  padding-left: var(--space-4);
}

.navv2__l3list.is-direct {
  padding-left: var(--space-1);
}

.navv2__l3 {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: 28px;
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-xs);
}

.navv2__l3:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

/* 激活页：accent 药丸 + 导轨上的 accent 段（压在 ::before 竖线上） */
.navv2__l3.is-active {
  color: var(--ws-accent);
  font-weight: var(--weight-medium);
  background: var(--accent-surface);
}

.navv2__l3.is-active::before {
  content: "";
  position: absolute;
  top: 4px;
  bottom: 4px;
  /* 对齐到子树导轨：行起点 -（l3list 缩进 16 + subtree 缩进 8） */
  left: calc(-1 * var(--space-4) - var(--space-2));
  width: 2px;
  background: var(--ws-accent);
  border-radius: var(--radius-full);
}

/* 直属页（无模块层）的激活条对齐到导轨 */
.navv2__l3list.is-direct .navv2__l3.is-active::before {
  left: calc(-1 * var(--space-2) - var(--space-1));
}

.navv2__l3.is-restricted {
  color: var(--ws-text-disabled);
  cursor: not-allowed;
}

.navv2__l3.is-restricted:hover {
  color: var(--ws-text-disabled);
  background: transparent;
}

.navv2__l3-label {
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.navv2__lock {
  flex: none;
}

/* ===== 徽标 / 规划 / 圆点 / 空态 ===== */
.navv2__badge {
  flex: none;
  padding: 0 5px;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  line-height: 15px;
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-full);
}

.navv2__l1.is-active .navv2__badge,
.navv2__l3.is-active .navv2__badge {
  color: var(--ws-accent);
  background: transparent;
  border-color: var(--ws-accent);
}

.navv2__planned {
  flex: none;
  padding: 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.navv2__dot {
  position: absolute;
  top: -2px;
  right: -3px;
  width: 6px;
  height: 6px;
  background: var(--ws-accent);
  border-radius: var(--radius-full);
}

/* 空模块：虚线占位（区别于可点条目） */
.navv2__empty {
  display: flex;
  align-items: center;
  margin: 2px 0;
  padding: var(--space-1) var(--space-2);
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
  background: transparent;
  border: var(--border-1) dashed var(--ws-border);
  border-radius: var(--radius-xs);
}

/* 子树展开微动效（位移 2px，尊重 reduced-motion） */
@keyframes navv2-reveal {
  from {
    opacity: 0;
    transform: translateY(-2px);
  }

  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .navv2__chevron,
  .navv2__l1-icon {
    transition: none;
  }

  .navv2__subtree {
    animation: none;
  }
}
</style>
