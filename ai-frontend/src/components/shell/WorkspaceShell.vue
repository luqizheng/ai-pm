<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import SearchPalette from "@/components/global/search/SearchPalette.vue";
import type { SearchCommand, SearchFn } from "@/components/global/search/types";
import { listProjects } from "@/api/projects";
import { retrievalQuery } from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";
import { useShellChrome } from "./use-shell-chrome";
import AssistantColumn from "./AssistantColumn.vue";
import ProjectSwitchGuardDialog from "./ProjectSwitchGuardDialog.vue";
import ContextPanel from "./ContextPanel.vue";
import SideNav from "./SideNav.vue";
import TopBar from "./TopBar.vue";

/**
 * 应用壳根（AI R&D Workspace Application Shell）。
 *
 * 四栏主体工作区（AI Collaboration V1.0 / DR-C5 + DR-C8，1440px 基准）：
 *   顶栏 52px（横跨全部列）/
 *   左导航 232px（折叠 60px）/ 主内容区 474px / AI Chat 446px（常驻独立列）/
 *   右上下文栏 288px（可关）
 *
 * 布局由 `.workspace-shell` 的 grid 承担，状态经根元素
 * `data-sidebar` / `data-context` / `data-ai-panel` / `data-focus` 属性切换
 * （见 src/styles/index.css）。专注模式（data-focus="on"）收起
 * AI Chat 与右上下文栏，主区独占工作区。
 *
 * 列的**生效可见性**（管理页 / 会话页 / 专注 / 开关共同决定）统一来自
 * use-shell-chrome：v-if 移除列时 data-* 同步收列，grid 模板不留空列。
 */

const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const shell = useShellStore();

/** 路由相关的 chrome 可见性（管理页 / 会话页判定单源） */
const { assistantVisible, contextVisible } = useShellChrome();

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
watch(urlProjectId, projectId => {
  if (projectId && projectId !== shell.currentProjectId) {
    shell.setCurrentProject(projectId);
  }
});

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
    :data-context="contextVisible ? 'open' : 'closed'"
    :data-ai-panel="assistantVisible ? 'open' : 'closed'"
    :data-focus="shell.focusState"
  >
    <TopBar class="workspace-topbar" @search="searchOpen = true" />

    <SideNav class="workspace-leftnav" />

    <main class="workspace-main">
      <div class="workspace-main-scroll">
        <RouterView />
      </div>
    </main>

    <!-- AI Chat 常驻独立列（DR-C8）与右上下文栏组成 AI 面板：
         总开关 aiPanelOpen（顶栏「助手」按钮，全开 ⇄ 全关）；
         上下文栏另有子开关 contextOpen（AI 列头部的「关闭上下文栏」，
         仅收右栏、助手列保留）。专注模式收起两列，主区独占工作区；
         完整会话页（/project/assistant）本身即会话面，助手列收起
         （上下文栏保留，走三列特例模板）；管理页（导航「管理」组）
         为租户运营面，AI 面板不出现。可见性判定单源 = use-shell-chrome。 -->
    <AssistantColumn v-if="assistantVisible" class="workspace-chat" />

    <!-- 右上下文栏（含会话 History）：面板总开关 + 自身子开关共同决定；
         完整会话页仍保留上下文。 -->
    <ContextPanel v-if="contextVisible" class="workspace-context" />

    <!-- 切项目确认（G12 · DR-C6）：有活跃会话时切换项目先确认 -->
    <ProjectSwitchGuardDialog />

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
