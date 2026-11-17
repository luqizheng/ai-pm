<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Bell, Search } from "lucide-vue-next";

import DesignLayout from "../DesignLayout.vue";
import DesignFrame from "../design-kit/DesignFrame.vue";
import Specimen from "../design-kit/Specimen.vue";

import NavV2Sidebar from "./nav-v2/NavV2Sidebar.vue";
import {
  findNavV2Trail,
  navV2Domains,
  navV2ProjectDomains,
  type NavV2Domain
} from "./nav-v2/model";

/**
 * /design/navigation/v2 —— Main Navigation V2.0 三级导航原型。
 *
 * L1 业务域 → L2 功能模块 → L3 功能页；六板覆盖 §8 交付物：
 * 默认态（交互）/ 多组展开 + L3 激活 / 项目上下文 / 折叠侧栏 /
 * 角色可见性对照 / 组件库 specimen。原型数据见 ./nav-v2/model.ts，
 * 非生产单源（业务落地仍走 src/router/navigation.ts + locales）。
 */
const { t } = useI18n();

/* ===== Board 01：默认态（交互，面包屑联动） ===== */
const activePage = ref("/v2/execution/tasks");
const trail = computed(
  () => findNavV2Trail(navV2Domains, activePage.value) ?? null
);

function onNavigate(pageId: string) {
  activePage.value = pageId;
}

/* ===== 组件库 specimen 用单域演示数据 ===== */
function oneDomain(
  overrides: Partial<NavV2Domain> & { id: string; label: string }
): NavV2Domain[] {
  return [
    {
      icon: overrides.icon ?? (navV2Domains[1] as NavV2Domain).icon,
      modules: overrides.modules ?? [],
      ...overrides
    }
  ];
}

const l1Demo = oneDomain({ id: "d1", label: "项目管理" });
const l1OpenDemo = oneDomain({
  id: "d2",
  label: "项目管理",
  modules: [
    {
      id: "m1",
      label: "项目执行",
      pages: [{ id: "/v2/x/tasks", label: "任务管理" }]
    }
  ]
});
const emptyModuleDemo = oneDomain({
  id: "d3",
  label: "团队知识",
  modules: [{ id: "m-empty", label: "知识组织", pages: [] }]
});
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.navigation.v2.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.navigation.v2.subtitle") }}
      </p>
    </template>

    <!-- Board 01 — Default State（交互原型） -->
    <DesignFrame
      label="Board 01 — Main Navigation / Default State（可交互）"
      note="L1 点击展开/收起（多组可同开）；L2 点击展开页列表；L3 点击打开页面，父级保持展开，面包屑随轨迹联动。默认展开：项目管理 → 项目执行 → 任务管理激活。"
      width="full"
    >
      <div class="v2shell">
        <header class="v2shell__top">
          <ol class="v2shell__crumbs">
            <template v-if="trail">
              <li>{{ trail.domain.label }}</li>
              <li v-if="trail.module.label">{{ trail.module.label }}</li>
              <li class="is-current">{{ trail.page.label }}</li>
            </template>
          </ol>
          <div class="v2shell__search">
            <Search :size="13" aria-hidden="true" />
            <span>全局搜索…</span>
          </div>
          <div class="v2shell__top-right">
            <Bell :size="15" aria-hidden="true" />
            <span class="v2shell__avatar" aria-hidden="true">U</span>
          </div>
        </header>
        <div class="v2shell__body">
          <NavV2Sidebar
            :domains="navV2Domains"
            :initial-active="activePage"
            :initial-expanded="['project', 'personal']"
            :initial-expanded-modules="['project-execution']"
            @navigate="nav => onNavigate(nav.pageId)"
          />
          <main class="v2shell__content">
            <h3 class="v2shell__page-title">
              {{ trail?.page.label ?? "未选中页面" }}
            </h3>
            <p class="v2shell__page-path">
              {{ trail?.domain.label ?? "—"
              }}<template v-if="trail?.module.label">
                / {{ trail.module.label }}</template
              >
              / {{ trail?.page.label ?? "—" }}
            </p>
            <div class="v2shell__placeholder">
              页面内容占位（设计原型不承载业务数据）
            </div>
          </main>
        </div>
      </div>
    </DesignFrame>

    <!-- Board 02 — Multiple Groups Expanded + L3 Active -->
    <DesignFrame
      label="Board 02 — Multiple Groups Expanded / L3 Active"
      note="多组同时展开（个人工作台 + 项目管理 + 产品认知）；L3 激活项 = accent 文字 + surface 底 + 2px 左指示条；所在 L1/L2 父级保持高亮与展开。"
    >
      <div class="v2board">
        <NavV2Sidebar
          :domains="navV2Domains"
          :initial-active="'/v2/product/pending'"
          :initial-expanded="['personal', 'project', 'product-cognition']"
          :initial-expanded-modules="['product-management', 'project-list']"
        />
      </div>
    </DesignFrame>

    <!-- Board 03 — Project Context -->
    <DesignFrame
      label="Board 03 — Project Context Navigation"
      note="进入项目后的独立上下文：项目头（名称 / 状态 / 切换器 / 操作）+ 项目专属二级导航。团队知识与产品认知不进入项目导航（域边界保留）。"
    >
      <div class="v2board v2board--project">
        <NavV2Sidebar
          :domains="navV2ProjectDomains"
          :project="{ name: 'Atlas 平台', status: '执行中 · 12 个活跃任务' }"
          :initial-active="'/v2/p/tasks'"
          :initial-expanded="['ctx-project']"
          :initial-expanded-modules="[
            'ctx-execution',
            'ctx-knowledge',
            'ctx-config'
          ]"
        />
      </div>
    </DesignFrame>

    <!-- Board 04 — Collapsed Sidebar -->
    <DesignFrame
      label="Board 04 — Collapsed Sidebar / Icon-only (60px)"
      note="窄栏图标模式：域级图标 + 分隔线 + 未读圆点（悬停 title 提示）；激活域以 accent 图标呈现。点按域图标不展开（保持窄态）。"
    >
      <div class="v2board v2board--rail">
        <NavV2Sidebar
          :domains="navV2Domains"
          collapsed
          :initial-active="'/v2/execution/tasks'"
        />
      </div>
    </DesignFrame>

    <!-- Board 05 — Role-Based Visibility -->
    <DesignFrame
      label="Board 05 — Role-Based Visibility / Admin vs Member"
      note="左 = 管理员（6 域全量，含团队管理）；右 = 普通成员（团队管理域整体隐藏，分析历史降为受限态：锁 + 置灰 + not-allowed）。菜单可见性不替代后端授权。"
    >
      <div class="v2board-row">
        <div class="v2board">
          <NavV2Sidebar
            :domains="navV2Domains"
            role="admin"
            :initial-active="'/v2/admin/roles'"
            :initial-expanded="['team-admin', 'analysis']"
          />
        </div>
        <div class="v2board">
          <NavV2Sidebar
            :domains="navV2Domains"
            role="member"
            :initial-active="'/v2/analysis/overview'"
            :initial-expanded="['analysis', 'personal']"
          />
        </div>
      </div>
    </DesignFrame>

    <!-- Board 06 — Component Library -->
    <section class="design-page__group">
      <h2 class="design-page__group-title">
        COMPONENT LIBRARY — 三级条目全状态（可交互）
      </h2>
      <div class="design-page__row">
        <Specimen label="L1 Domain / Default（收起）">
          <NavV2Sidebar :domains="l1Demo" />
        </Specimen>
        <Specimen label="L1 Domain / Open + L2 Module / Collapsed">
          <NavV2Sidebar :domains="l1OpenDemo" :initial-expanded="['d2']" />
        </Specimen>
        <Specimen label="L2 Expanded + L3 / Default">
          <NavV2Sidebar
            :domains="l1OpenDemo"
            :initial-expanded="['d2']"
            :initial-expanded-modules="['m1']"
          />
        </Specimen>
        <Specimen label="L3 / Active">
          <NavV2Sidebar
            :domains="l1OpenDemo"
            :initial-expanded="['d2']"
            :initial-expanded-modules="['m1']"
            :initial-active="'/v2/x/tasks'"
          />
        </Specimen>
        <Specimen label="L3 / Restricted（需权限）">
          <NavV2Sidebar
            :domains="navV2Domains"
            role="member"
            :initial-expanded="['analysis']"
          />
        </Specimen>
        <Specimen label="L3 / Planned + Badge">
          <NavV2Sidebar
            :domains="navV2Domains"
            :initial-expanded="['team-knowledge', 'personal']"
          />
        </Specimen>
        <Specimen label="Module / Empty（无页面占位）">
          <NavV2Sidebar
            :domains="emptyModuleDemo"
            :initial-expanded="['d3']"
            :initial-expanded-modules="['m-empty']"
          />
        </Specimen>
        <Specimen label="Rail / Collapsed (60px)">
          <NavV2Sidebar
            :domains="navV2Domains"
            collapsed
            :initial-active="'/v2/execution/tasks'"
          />
        </Specimen>
      </div>
    </section>
  </DesignLayout>
</template>

<style scoped>
.design-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-page__group {
  margin: 0 0 var(--space-8);
}

.design-page__group-title {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--space-4);
}

/* ===== 壳画框（Board 01） ===== */
.v2shell {
  display: flex;
  flex-direction: column;
  height: 560px;
  overflow: hidden;
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.v2shell__top {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  height: 44px;
  padding: 0 var(--space-4);
  background: var(--ws-sidebar);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.v2shell__crumbs {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  min-width: 0;
  list-style: none;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.v2shell__crumbs li + li::before {
  content: "/";
  margin-right: var(--space-2);
  color: var(--ws-text-disabled);
}

.v2shell__crumbs .is-current {
  color: var(--ws-text-primary);
  font-weight: var(--weight-medium);
}

.v2shell__search {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 26px;
  margin-left: auto;
  padding: 0 var(--space-2);
  color: var(--ws-text-disabled);
  font-size: var(--font-size-caption);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.v2shell__top-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  color: var(--ws-text-secondary);
}

.v2shell__avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  color: var(--ws-text-primary);
  font-size: var(--font-size-micro);
  background: var(--accent-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-full);
}

.v2shell__body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.v2shell__body > :first-child {
  height: auto;
  flex: none;
}

.v2shell__content {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  padding: var(--space-5);
}

.v2shell__page-title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.v2shell__page-path {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.v2shell__placeholder {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  margin-top: var(--space-2);
  color: var(--ws-text-disabled);
  font-size: var(--ws-font-size-caption);
  background: var(--ws-surface);
  border: var(--border-1) dashed var(--ws-border);
  border-radius: var(--radius-sm);
}

/* ===== 侧栏独立画框（Board 02~05） ===== */
.v2board {
  width: 264px;
  height: 520px;
  overflow: hidden;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.v2board--project {
  background: var(--ws-sidebar);
}

.v2board--rail {
  width: 72px;
}

.v2board :deep(.navv2) {
  height: 100%;
  border-right: none;
}

.v2board-row {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-4);
}
</style>
