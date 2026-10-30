<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { storeToRefs } from "pinia";
import { RouterView } from "vue-router";
import TopBar from "./TopBar.vue";
import LeftSidebar from "./LeftSidebar.vue";
import ContextSidebar from "./ContextSidebar.vue";
import AICommandBar from "../components/AICommandBar.vue";
import { useWorkspaceStore } from "../store";

defineProps<{ project: string; page: string }>();

const store = useWorkspaceStore();
const { sidebarCollapsed, contextOpen } = storeToRefs(store);

// 响应式契约（B-2 沿用）：主区 <1280px 收起右栏，<1024px 左栏折叠至图标栏
const NARROW_QUERY = "(max-width: 1280px)";
const COMPACT_QUERY = "(max-width: 1024px)";

function onNarrow(e: { matches: boolean }) {
  // 收窄收起、变宽恢复（终审 I1）：断点隐藏是自动行为，不吞用户手动重开
  store.contextOpen = !e.matches;
}

function onCompact(e: { matches: boolean }) {
  if (e.matches) store.sidebarCollapsed = true;
}

let narrowMql: MediaQueryList | undefined;
let compactMql: MediaQueryList | undefined;

onMounted(() => {
  narrowMql = window.matchMedia(NARROW_QUERY);
  compactMql = window.matchMedia(COMPACT_QUERY);
  if (narrowMql.matches) onNarrow(narrowMql);
  if (compactMql.matches) onCompact(compactMql);
  narrowMql.addEventListener("change", onNarrow);
  compactMql.addEventListener("change", onCompact);
});

onBeforeUnmount(() => {
  narrowMql?.removeEventListener("change", onNarrow);
  compactMql?.removeEventListener("change", onCompact);
});
</script>

<template>
  <div
    class="workspace-shell"
    :data-sidebar="sidebarCollapsed ? 'collapsed' : 'expanded'"
    :data-context="contextOpen ? 'open' : 'closed'"
  >
    <TopBar :project="project" :page="page" />
    <LeftSidebar class="workspace-leftnav" />
    <main class="workspace-main">
      <div class="workspace-main-scroll">
        <!-- 路由用法：子路由（总控台等）经默认 RouterView 渲染；显式用法可覆写 main 插槽 -->
        <slot name="main">
          <RouterView />
        </slot>
      </div>
      <slot name="commandbar">
        <AICommandBar />
      </slot>
    </main>
    <aside v-if="contextOpen" class="workspace-context">
      <slot name="context">
        <ContextSidebar />
      </slot>
    </aside>
  </div>
</template>
