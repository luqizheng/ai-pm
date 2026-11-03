<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ChevronLeft, ChevronRight } from "lucide-vue-next";
import { useWorkspaceStore } from "../store";
import { demoProject } from "../fixtures";
import { NAV_GROUPS, type NavItemDef } from "../nav-config";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const store = useWorkspaceStore();

const projectId = computed(
  () => (route.params.projectId as string | undefined) ?? ""
);

function workspaceTo(slug: string): string {
  return `/workspace/${projectId.value}/${slug}`;
}

// 导航集由当前模式决定：传统 9 项 / AI(V1.2) 5 项（见 nav-config）。
const groups = computed(() => NAV_GROUPS[store.navMode]);

const activeTo = computed(() => route.path);

function isActive(item: NavItemDef): boolean {
  return activeTo.value === workspaceTo(item.slug);
}

function onItem(item: NavItemDef) {
  router.push(workspaceTo(item.slug));
}
</script>

<template>
  <!-- 折叠态（60px 图标栏）只留图标：窄栏里完整文字会逐字竖排换行（用户反馈 bugfix） -->
  <nav
    class="ws-leftnav"
    :class="{ 'is-collapsed': store.sidebarCollapsed }"
    aria-label="workspace"
  >
    <div v-for="group in groups" :key="group.key" class="ws-nav-group">
      <div v-if="!store.sidebarCollapsed" class="ws-nav-group-label">
        {{ t(group.labelKey) }}
      </div>
      <button
        v-for="item in group.items"
        :key="item.key"
        type="button"
        class="ws-nav-item ws-focusable"
        :class="{ 'is-active': isActive(item) }"
        :title="store.sidebarCollapsed ? t(item.labelKey) : undefined"
        @click="onItem(item)"
      >
        <component :is="item.icon" :size="16" aria-hidden="true" />
        <span v-if="!store.sidebarCollapsed" class="ws-nav-label">
          {{ t(item.labelKey) }}
        </span>
      </button>
    </div>

    <div class="ws-leftnav-footer">
      <template v-if="!store.sidebarCollapsed">
        <div class="ws-iteration">
          <span class="ws-iteration-label">
            {{ t("workspace.service.iteration") }}
          </span>
          <span class="ws-iteration-value">{{ demoProject.iteration }}</span>
        </div>
        <div class="ws-service" data-test="service-ok">
          <span
            class="ws-service-dot"
            data-test="service-dot"
            aria-hidden="true"
          />
          <span class="ws-nav-label">{{ t("workspace.service.ok") }}</span>
        </div>
      </template>
      <button
        type="button"
        class="ws-icon-btn ws-focusable"
        data-test="sidebar-toggle"
        :aria-label="
          store.sidebarCollapsed
            ? t('workspace.shell.expand')
            : t('workspace.shell.collapse')
        "
        @click="store.toggleSidebar()"
      >
        <ChevronRight
          v-if="store.sidebarCollapsed"
          :size="16"
          aria-hidden="true"
        />
        <ChevronLeft v-else :size="16" aria-hidden="true" />
      </button>
    </div>
  </nav>
</template>

<style scoped>
.ws-leftnav {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 12px 8px;
  gap: 4px;
}

.ws-nav-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-bottom: 12px;
}

.ws-nav-group-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  padding: 8px 10px 4px;
}

.ws-nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  height: var(--ws-row-height);
  padding: 0 10px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
  cursor: pointer;
  text-align: left;
}

.ws-nav-item:hover {
  background: var(--ws-elevated);
}

/* 紫色克制：AI 紫仅用于选中态 */
.ws-nav-item.is-active {
  background: var(--ws-elevated);
  color: var(--ws-accent);
  font-weight: 600;
}

.ws-leftnav-footer {
  margin-top: auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 6px 0;
  border-top: 1px solid var(--ws-border);
}

/* 折叠态：文字已由模板 v-if 隐藏，这里只负责图标在 60px 栏内居中 */
.ws-leftnav.is-collapsed .ws-nav-item {
  justify-content: center;
  padding: 0;
}

.ws-leftnav.is-collapsed .ws-leftnav-footer {
  justify-content: center;
}

.ws-service {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-service-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ws-color-success);
}

.ws-iteration {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 8px;
}

.ws-iteration-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-iteration-value {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
}
</style>
