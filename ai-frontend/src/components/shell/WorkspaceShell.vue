<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import Command from "@/components/ui/command/Command.vue";
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

/** 命令面板开关 */
const commandOpen = ref(false);

/**
 * 初始化项目上下文（骨架阶段注入演示数据）。
 * 真实实现改为从项目列表接口装载 + 恢复上次选择。
 */
shell.setProjects(demoProjects);
if (!shell.currentProjectId) {
  shell.setCurrentProject(demoProject.id);
}

/** 命令面板条目 = 导航树扁平化 */
const commandItems = computed(() =>
  flatNavItems.map(item => ({
    id: item.path,
    label: t(item.labelKey),
    group: t("shell.app.side.label")
  }))
);

function onCommandSelect(id: string) {
  commandOpen.value = false;
  router.push(id);
}

/**
 * 全局快捷键：
 *   ⌘/Ctrl + K —— 命令面板
 *   ⌘/Ctrl + J —— 切换 AI 指令栏展开态
 */
function onKeydown(event: KeyboardEvent) {
  if (!(event.metaKey || event.ctrlKey)) return;
  const key = event.key.toLowerCase();
  if (key === "k") {
    event.preventDefault();
    commandOpen.value = !commandOpen.value;
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
    <TopBar class="workspace-topbar" @search="commandOpen = true" />

    <SideNav class="workspace-leftnav" />

    <main class="workspace-main">
      <div class="workspace-main-scroll">
        <RouterView />
      </div>
      <CommandBar class="workspace-commandbar" />
    </main>

    <ContextPanel v-if="shell.contextOpen" class="workspace-context" />

    <!-- 全局命令面板（⌘ K） -->
    <Command
      v-model:open="commandOpen"
      :items="commandItems"
      @select="onCommandSelect"
    />
  </div>
</template>
