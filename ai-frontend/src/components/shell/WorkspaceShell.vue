<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import SearchPalette from "@/components/global/search/SearchPalette.vue";
import {
  demoRecentSearches,
  demoSearchFn
} from "@/components/global/search/demo";
import type { SearchCommand } from "@/components/global/search/types";
import { flatNavItems } from "@/router/navigation";
import { useShellStore } from "@/stores/shell";
import CommandBar from "./CommandBar.vue";
import ContextPanel from "./ContextPanel.vue";
import SideNav from "./SideNav.vue";
import TopBar from "./TopBar.vue";
import { demoProject, demoProjects } from "./demo";

/**
 * 应用壳根（AI R&D Workspace Application Shell）。
 *
 * 五区域（Shell V1.1 §2）：
 *   顶栏 52px / 左导航 232px（折叠 60px）/ 主内容区 / 右上下文栏 320px（可关）/
 *   底部 AI 指令栏 72–120px
 *
 * 布局由 `.workspace-shell` 的 grid 承担，折叠态经根元素
 * `data-sidebar` / `data-context` 属性切换（见 src/styles/index.css）。
 */

const { t } = useI18n();
const router = useRouter();
const shell = useShellStore();

/** 全局搜索面板开关（⌘K / 顶栏搜索入口） */
const searchOpen = ref(false);

/**
 * 初始化项目上下文（骨架阶段注入演示数据）。
 * 真实实现改为从项目列表接口装载 + 恢复上次选择。
 */
shell.setProjects(demoProjects);
if (!shell.currentProjectId) {
  shell.setCurrentProject(demoProject.id);
}

/**
 * 全局命令（Figma「01 — Global Search」Global commands：导航 / 创建 /
 * 附加命令）。文案走 i18n，专有名词（项目名 / 模块名）取自壳状态与导航表。
 */
const searchCommands = computed<SearchCommand[]>(() => {
  const repositories = flatNavItems.find(
    item => item.path === "/project/repository"
  );
  return [
    {
      id: "cmd-open-project",
      label: `${t("global.search.commands.openProject")} · ${shell.currentProject?.name ?? ""}`,
      group: "navigate",
      icon: "folder",
      target: "/project/repository"
    },
    {
      id: "cmd-navigate-repositories",
      label: `${t("global.search.commands.navigateModule")} · ${repositories ? t(repositories.labelKey) : ""}`,
      group: "navigate",
      icon: "gitBranch",
      target: "/project/repository"
    },
    {
      id: "cmd-create-project",
      label: t("global.search.commands.createProject"),
      group: "create",
      icon: "plus",
      target: "/projects"
    },
    {
      id: "cmd-open-personal-workspace",
      label: t("global.search.commands.openPersonalHome"),
      group: "action",
      icon: "user",
      target: "/personal"
    },
    {
      id: "cmd-open-inbox",
      label: t("global.search.commands.openInbox"),
      group: "action",
      icon: "inbox",
      target: "/inbox"
    }
  ];
});

function onSearchSelect(target: string) {
  router.push(target);
}

/**
 * 全局快捷键：
 *   ⌘/Ctrl + K —— 全局搜索面板
 *   ⌘/Ctrl + J —— 切换 AI 指令栏展开态
 */
function onKeydown(event: KeyboardEvent) {
  if (!(event.metaKey || event.ctrlKey)) return;
  const key = event.key.toLowerCase();
  if (key === "k") {
    event.preventDefault();
    searchOpen.value = !searchOpen.value;
  } else if (key === "j") {
    event.preventDefault();
    shell.toggleCommandbar();
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div
    class="workspace-shell"
    :data-sidebar="shell.sidebarState"
    :data-context="shell.contextState"
  >
    <TopBar class="workspace-topbar" @search="searchOpen = true" />

    <SideNav class="workspace-leftnav" />

    <main class="workspace-main">
      <div class="workspace-main-scroll">
        <RouterView />
      </div>
      <CommandBar class="workspace-commandbar" />
    </main>

    <ContextPanel v-if="shell.contextOpen" class="workspace-context" />

    <!-- 全局搜索 / 命令面板（⌘ K，Figma「01 — Global Search」） -->
    <SearchPalette
      v-model:open="searchOpen"
      :commands="searchCommands"
      :recent-searches="demoRecentSearches"
      :search-fn="demoSearchFn"
      @select="onSearchSelect"
    />
  </div>
</template>
