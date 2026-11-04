<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";

import Command from "@/components/ui/command/Command.vue";
import { flatNavItems } from "@/router/navigation";
import CommandBar from "@/components/shell/CommandBar.vue";
import ContextPanel from "@/components/shell/ContextPanel.vue";
import SideNav from "@/components/shell/SideNav.vue";
import TopBar from "@/components/shell/TopBar.vue";

/**
 * DesignShellFrame —— 固定 1440×900 画框内的真实应用壳
 *（Figma 板 01/02：Global Navigation Expanded / Collapsed）。
 *
 * 复用全局定位 class（.workspace-topbar/.workspace-leftnav/.workspace-main/
 * .workspace-commandbar/.workspace-context，见 src/styles/index.css），
 * 但**不使用** `.workspace-shell`（其 height:100dvh 会撑满视口）；
 * 画框自带等价 grid：列 232|1fr|320、行 52|1fr。
 * 折叠态经 shell store（全局）切换 —— 演示页可交互，但会写
 * localStorage（aipm.sidebar-collapsed），演示后残留属预期。
 */
import { computed } from "vue";
import { useRouter } from "vue-router";

const props = defineProps<{
  /** 画框标注（如 Board 01 — Expanded） */
  label?: string;
}>();

const router = useRouter();

/** 命令面板（⌘K），镜像 WorkspaceShell 装配 */
const commandOpen = ref(false);

const commandItems = computed(() =>
  flatNavItems.map(item => ({
    id: item.path,
    label: item.labelKey
  }))
);

function onCommandSelect(id: string) {
  commandOpen.value = false;
  router.push(id);
}

function onKeydown(event: KeyboardEvent) {
  if (!(event.metaKey || event.ctrlKey)) return;
  if (event.key.toLowerCase() === "k") {
    event.preventDefault();
    commandOpen.value = !commandOpen.value;
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown));

onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <figure class="design-shell-frame">
    <figcaption v-if="label" class="design-shell-frame__label">
      {{ label }}
    </figcaption>
    <div class="design-shell-frame__stage">
      <div class="design-shell-frame__grid">
        <TopBar class="workspace-topbar" @search="commandOpen = true" />
        <SideNav class="workspace-leftnav" />
        <main class="workspace-main">
          <div class="workspace-main-scroll">
            <div class="design-shell-frame__placeholder">
              <p>Atlas Platform</p>
              <p>Overview</p>
            </div>
          </div>
          <CommandBar class="workspace-commandbar" />
        </main>
        <ContextPanel class="workspace-context" />
      </div>
      <Command
        v-model:open="commandOpen"
        :items="commandItems"
        @select="onCommandSelect"
      />
    </div>
  </figure>
</template>

<style scoped>
.design-shell-frame {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-8);
}

.design-shell-frame__label {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-shell-frame__stage {
  width: 1440px;
  max-width: 100%;
}

.design-shell-frame__grid {
  display: grid;
  grid-template-columns:
    var(--ws-sidebar-width)
    minmax(0, 1fr)
    var(--ws-context-width);
  grid-template-rows: var(--ws-topbar-height) minmax(0, 1fr);
  height: 900px;
  overflow: hidden;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  background-color: var(--ws-canvas);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
}

/* 折叠态（store 驱动）：列宽收 60px、右栏关闭（Figma 板 02 无右栏） */
.design-shell-frame__grid[data-sidebar="collapsed"] {
  grid-template-columns:
    var(--ws-sidebar-collapsed)
    minmax(0, 1fr)
    var(--ws-context-width);
}

.design-shell-frame__grid[data-context="closed"] {
  grid-template-columns:
    var(--ws-sidebar-width)
    minmax(0, 1fr);
}

.design-shell-frame__grid[data-sidebar="collapsed"][data-context="closed"] {
  grid-template-columns:
    var(--ws-sidebar-collapsed)
    minmax(0, 1fr);
}

.design-shell-frame__placeholder {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-5) var(--space-8);
}

.design-shell-frame__placeholder p {
  margin: 0;
  color: var(--ws-text-secondary);
}

.design-shell-frame__placeholder p:first-child {
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}
</style>
