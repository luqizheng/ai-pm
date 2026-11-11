<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import SearchPalette from "@/components/global/search/SearchPalette.vue";
import type { SearchCommand, SearchFn } from "@/components/global/search/types";
import { listProjects } from "@/api/projects";
import { retrievalQuery } from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";
import CommandBar from "./CommandBar.vue";
import ContextPanel from "./ContextPanel.vue";
import SideNav from "./SideNav.vue";
import TopBar from "./TopBar.vue";

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
const route = useRoute();
const shell = useShellStore();

/** URL query 上的项目上下文（?project_id=；各入口跳转统一走该通道） */
function urlProjectId(): string | null {
  const query = route.query.project_id;
  return typeof query === "string" && query ? query : null;
}

/** 全局搜索面板开关（⌘K / 顶栏搜索入口） */
const searchOpen = ref(false);

/** 上次选中的项目（仅本地偏好，不承载业务数据） */
const CURRENT_PROJECT_KEY = "aipm.current-project";
/** mock 预览（本地 dev / 单测）：用设计稿样本；发布构建改为真实项目列表 */
const mockPreview = import.meta.env.VITE_USE_MOCK === "1";

async function bootstrapProjectContext() {
  if (mockPreview) {
    const { demoProject, demoProjects } = await import("./demo");
    shell.setProjects(demoProjects);
    if (!shell.currentProjectId) shell.setCurrentProject(demoProject.id);
    return;
  }

  try {
    const page = await listProjects({ size: 100 });
    shell.setProjects(
      page.items.map(project => ({
        id: project.id,
        name: project.name,
        description: project.description ?? undefined
      }))
    );
    let restored: string | null = null;
    try {
      restored = localStorage.getItem(CURRENT_PROJECT_KEY);
    } catch {
      restored = null;
    }
    // 恢复优先级：URL 上下文 > 上次本地偏好 > 首个项目
    // （URL query 是页面数据真源，顶栏必须与当前路由一致）
    const preferred =
      page.items.find(project => project.id === urlProjectId()) ??
      page.items.find(project => project.id === restored) ??
      page.items[0];
    if (preferred) shell.setCurrentProject(preferred.id);
  } catch {
    // 项目列表不可用时保持空壳（各页自行展示空态），不注入演示数据
    shell.setProjects([]);
  }
}

/**
 * 全局搜索：接统一检索 API（/v1/retrieval/query）。
 *
 * - KB 文档 / PC 对象证据 → 「document」结果类，target 落知识库页；
 * - Planner 未开启时后端走确定性双域直查（degraded=planner_disabled）——
 *   对搜索面板透明，端点始终可用；
 * - 无项目上下文或请求失败 → 空结果（面板 Error 态仅留给真正异常）。
 */
const searchFn: SearchFn = async query => {
  const projectId = shell.currentProjectId;
  if (!projectId || !query.trim()) return [];
  const resp = await retrievalQuery(query, projectId);
  return resp.evidence.evidence.map(item => {
    const isPc = item.source_domain === "PC";
    return {
      id: item.evidence_id,
      objectType: "document" as const,
      label: item.title ?? item.content_excerpt.slice(0, 48),
      description: isPc
        ? `${t("global.search.domainPc")} · ${item.authority_status}`
        : `${t("global.search.domainKb")} · ${item.authority_status}`,
      meta: item.truncated ? t("global.search.metaTruncated") : undefined,
      target: "/project/knowledge"
    };
  });
};

bootstrapProjectContext();
watch(
  () => shell.currentProjectId,
  projectId => {
    if (projectId) {
      try {
        localStorage.setItem(CURRENT_PROJECT_KEY, projectId);
      } catch {
        // 持久化失败不影响切换
      }
    }
    void shell.loadRepositoryContext(projectId);
  },
  { immediate: true }
);

// 路由携带 ?project_id= 时，顶栏项目选择器跟随同一项目
// （打开项目 = 切换当前项目上下文；store 的持久化与仓库上下文由上方 watch 联动）
watch(
  urlProjectId,
  projectId => {
    if (projectId && projectId !== shell.currentProjectId) {
      shell.setCurrentProject(projectId);
    }
  }
);

/**
 * 全局命令（Figma「01 — Global Search」Global commands：导航 / 创建 /
 * 附加命令）。文案走 i18n，专有名词（项目名 / 模块名）取自壳状态与导航表。
 */
const searchCommands = computed<SearchCommand[]>(() => {
  return [
    {
      id: "cmd-open-project",
      label: `${t("global.search.commands.openProject")} · ${shell.currentProject?.name ?? ""}`,
      group: "navigate",
      icon: "folder",
      // 项目上下文入口 = 项目总控台（Git 仓库等模块已迁入 Project Setting）；
      // 总控台只认 ?project_id=，命令必须携带当前项目
      target: shell.currentProjectId
        ? `/project/command-center?project_id=${encodeURIComponent(shell.currentProjectId)}`
        : "/project/command-center"
    },
    {
      id: "cmd-navigate-repositories",
      label: `${t("global.search.commands.navigateModule")} · ${t("shell.app.side.item.repository")}`,
      group: "navigate",
      icon: "gitBranch",
      // 仓库管理已迁入 Project Setting（/project/settings/repositories）
      target: "/project/settings/repositories"
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
      :recent-searches="[]"
      :search-fn="searchFn"
      @select="onSearchSelect"
    />
  </div>
</template>
