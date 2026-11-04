<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import {
  ProjectSwitcher,
  ProjectSwitcherMenu,
  type ProjectSwitcherProject
} from "@/components/global/project-switcher";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/**
 * /design/navigation/switcher —— Figma「06 — Project Context Switcher」
 * （33:82961，Global/ProjectSwitcher 组件板，2026-11-04）。
 *
 * 四段（Dark 为主，Menu 另出 Light 对照）：
 *   1. TopBarSlice / Open —— 顶栏 52px 切片：mono 工作区标签 + 项目锚点（Selected）
 *   2. Menu / Open Dark + Light —— 面板全量（搜索 / FAVORITES / RECENT / Create）
 *   3. Menu 反馈态 —— Loading / Empty / Error / Search NoResults
 *   4. Trigger 五态 + CreateProject Disabled
 * Hover / Focus 为真实交互（目录页惯例，不伪造静态伪类）。
 * 板注：Selected=当前项目；Loading/Error 绝不静默切换项目上下文。
 */
const { t } = useI18n();

const projects: ProjectSwitcherProject[] = [
  { id: "atlas", name: "Atlas Platform", group: "favorites" },
  { id: "sdk-toolkit", name: "SDK Toolkit", group: "recent", meta: "2h ago" },
  {
    id: "developer-portal",
    name: "Developer Portal",
    group: "recent",
    meta: "Yesterday"
  }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.navigation.switcher.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.navigation.switcher.subtitle") }}
      </p>
    </template>

    <!-- ① TopBarSlice / Open / Dark -->
    <Specimen label="TopBarSlice  /  Open  /  Dark" width="100%">
      <div class="switcher__topbar">
        <span class="switcher__meta">AI R&amp;D Workspace</span>
        <span class="switcher__meta">Acme Research</span>
        <span class="switcher__meta">/</span>
        <ProjectSwitcher
          :projects="projects"
          label="Atlas Platform"
          current-id="atlas"
          selected
        />
      </div>
    </Specimen>

    <!-- ② Menu / Open Dark + Light -->
    <div class="design-page__row">
      <Specimen label="Menu  /  Open  /  Dark" width="480px">
        <ProjectSwitcherMenu :projects="projects" current-id="atlas" />
      </Specimen>

      <Specimen label="Menu  /  Open  /  Light" width="480px">
        <div data-theme="light">
          <ProjectSwitcherMenu :projects="projects" current-id="atlas" />
        </div>
      </Specimen>
    </div>

    <!-- ③ Menu 反馈态（Dark） -->
    <div class="design-page__row">
      <Specimen label="Menu  /  Loading  /  Dark" width="480px">
        <ProjectSwitcherMenu :projects="projects" status="loading" />
      </Specimen>

      <Specimen label="Menu  /  Empty  /  Dark" width="480px">
        <ProjectSwitcherMenu :projects="projects" status="empty" />
      </Specimen>

      <Specimen label="Menu  /  Error  /  Dark" width="480px">
        <ProjectSwitcherMenu
          :projects="projects"
          status="error"
          :texts="{
            errorDescription:
              'Current project remains Atlas Platform. Retry to load available projects.'
          }"
        />
      </Specimen>

      <Specimen label="Search  /  No results  /  Dark" width="480px">
        <ProjectSwitcherMenu :projects="projects" query="helios" />
      </Specimen>
    </div>

    <!-- ④ Trigger 五态（Hover / Focus 为真实交互） -->
    <h2 class="switcher__h2">Current project anchor</h2>
    <div class="switcher__triggers">
      <Specimen label="Trigger  /  Default  /  Dark" width="496px">
        <div class="switcher__trigger-bar">
          <ProjectSwitcher :projects="projects" label="Atlas Platform" />
        </div>
      </Specimen>

      <Specimen
        label="Trigger  /  Hover + Focus  /  Dark（live）"
        width="496px"
      >
        <div class="switcher__trigger-bar">
          <ProjectSwitcher :projects="projects" label="Atlas Platform" />
        </div>
      </Specimen>

      <Specimen label="Trigger  /  Selected  /  Dark" width="496px">
        <div class="switcher__trigger-bar">
          <ProjectSwitcher
            :projects="projects"
            label="Atlas Platform"
            selected
          />
        </div>
      </Specimen>

      <Specimen label="Trigger  /  Disabled  /  Dark" width="496px">
        <div class="switcher__trigger-bar">
          <ProjectSwitcher
            :projects="projects"
            label="Atlas Platform"
            disabled
          />
        </div>
      </Specimen>

      <Specimen label="CreateProject  /  Disabled  /  Dark" width="496px">
        <div class="switcher__create-disabled">
          <Button variant="primary" size="sm" disabled>
            Create New Project
          </Button>
          <p class="switcher__caption">
            Creation unavailable for this identity. No hidden projects are
            disclosed.
          </p>
        </div>
      </Specimen>
    </div>

    <p class="design-page__hint">
      Selected is the current project. Hover and focus apply to the trigger or a
      project row. Loading and error never silently switch project context.
    </p>
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

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--space-4);
}

.design-page__hint {
  margin-top: var(--space-4);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

/* TopBarSlice：52px 顶栏切片（padding 0 16 / gap 16 / sidebar 底） */
.switcher__topbar {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  height: var(--ws-topbar-height);
  padding: 0 var(--space-4);
  background-color: var(--ws-sidebar);
  border-radius: var(--radius-sm);
}

.switcher__meta {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  white-space: nowrap;
}

/* Trigger 态条：52px 切片（Figma padding 0 12） */
.switcher__trigger-bar {
  display: flex;
  align-items: center;
  height: var(--ws-topbar-height);
  padding: 0 var(--space-3);
  background-color: var(--ws-sidebar);
  border-radius: var(--radius-sm);
}

.switcher__h2 {
  margin: var(--space-4) 0 0;
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.switcher__triggers {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  margin-top: var(--space-4);
}

/* CreateProject Disabled：按钮 0.38（Button disabled token）+ 说明 */
.switcher__create-disabled {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  align-items: flex-start;
}

.switcher__caption {
  margin: 0;
  font-size: var(--font-size-body);
  color: var(--ws-text-secondary);
}
</style>
