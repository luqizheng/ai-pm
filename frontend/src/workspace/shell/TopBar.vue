<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import {
  Bell,
  ChevronRight,
  Moon,
  PanelRightOpen,
  Sun,
  User
} from "lucide-vue-next";
import { useTheme } from "../useTheme";
import { useWorkspaceStore } from "../store";
import { navDefaultSlug, navSlugs, type NavMode } from "../nav-config";

defineProps<{ project: string; page: string }>();

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const theme = useTheme();
const store = useWorkspaceStore();
const isDark = computed(() => theme.theme.value === "dark");

// 两套导航的全部 slug 并集：切模式时只在这两套之间做回退，
// focus / classic/* 等共享路由不劫持。
const ALL_NAV_SLUGS = new Set([...navSlugs("classic"), ...navSlugs("ai")]);

function currentSlug(): string {
  return route.path.split("/").filter(Boolean)[2] ?? "";
}

function switchMode(mode: NavMode) {
  if (store.navMode === mode) return;
  store.setNavMode(mode);
  const slug = currentSlug();
  // 目标模式也有当前页 → 原地不动（记住当前页）
  if (navSlugs(mode).includes(slug)) return;
  if (!ALL_NAV_SLUGS.has(slug)) return;
  const projectId = String(route.params.projectId ?? "");
  router.push(`/workspace/${projectId}/${navDefaultSlug(mode)}`);
}
</script>

<template>
  <header class="workspace-topbar ws-topbar">
    <nav class="ws-breadcrumb" :aria-label="t('workspace.shell.breadcrumb')">
      <span class="ws-brand" data-test="brand">
        <span class="ws-brand-mark" aria-hidden="true">✦</span>
        <span class="ws-brand-name">Atlas R&amp;D</span>
      </span>
      <ChevronRight :size="14" class="ws-breadcrumb-sep" aria-hidden="true" />
      <span class="ws-breadcrumb-project">{{ project }}</span>
      <ChevronRight :size="14" class="ws-breadcrumb-sep" aria-hidden="true" />
      <span class="ws-breadcrumb-page">{{ page }}</span>
    </nav>
    <div class="ws-topbar-actions">
      <button
        v-if="!store.contextOpen"
        type="button"
        class="ws-reopen-btn ws-focusable"
        data-test="context-reopen"
        @click="store.toggleContext()"
      >
        <PanelRightOpen :size="16" aria-hidden="true" />
        <span>{{ t("workspace.context.show") }}</span>
      </button>
      <button
        type="button"
        class="ws-icon-btn ws-focusable"
        data-test="theme-toggle"
        :aria-label="t('workspace.shell.theme')"
        @click="theme.toggle()"
      >
        <Sun v-if="isDark" :size="16" aria-hidden="true" />
        <Moon v-else :size="16" aria-hidden="true" />
      </button>
      <div
        class="ws-mode-switch"
        role="group"
        data-test="mode-switch"
        :aria-label="t('workspace.shell.modeSwitch')"
      >
        <button
          type="button"
          class="ws-mode-seg ws-focusable"
          :class="{ 'is-active': store.navMode === 'ai' }"
          data-test="mode-ai"
          :aria-pressed="store.navMode === 'ai'"
          @click="switchMode('ai')"
        >
          {{ t("workspace.shell.modeAi") }}
        </button>
        <button
          type="button"
          class="ws-mode-seg ws-focusable"
          :class="{ 'is-active': store.navMode === 'classic' }"
          data-test="mode-toggle"
          :aria-pressed="store.navMode === 'classic'"
          @click="switchMode('classic')"
        >
          {{ t("workspace.shell.modeClassic") }}
        </button>
      </div>
      <button
        type="button"
        class="ws-icon-btn ws-focusable"
        disabled
        :aria-label="t('workspace.shell.notifications')"
      >
        <Bell :size="16" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="ws-icon-btn ws-focusable"
        disabled
        :aria-label="t('workspace.shell.account')"
      >
        <User :size="16" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.ws-topbar {
  padding: 0 calc(var(--ws-space-unit) * 4);
}

.ws-breadcrumb {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--ws-font-size-body);
}

/* 品牌位（原型对齐：Atlas R&D 标识 + 面包屑） */
.ws-brand {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin-right: 4px;
}

.ws-brand-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--ws-radius-card);
  background: var(--ws-accent);
  color: var(--ws-canvas);
  font-size: 12px;
  font-weight: 700;
}

.ws-brand-name {
  font-weight: 700;
  letter-spacing: 0.02em;
}

.ws-breadcrumb-project {
  color: var(--ws-text-secondary);
}

.ws-breadcrumb-sep {
  color: var(--ws-text-secondary);
}

.ws-breadcrumb-page {
  font-weight: 600;
}

.ws-topbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-secondary);
  cursor: pointer;
}

.ws-icon-btn:not(:disabled):hover {
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
}

.ws-icon-btn:disabled {
  cursor: default;
  opacity: 0.5;
}
.ws-reopen-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.ws-reopen-btn:hover {
  background: var(--ws-surface);
}

.ws-mode-switch {
  display: inline-flex;
  align-items: center;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow: hidden;
}

.ws-mode-seg {
  height: 28px;
  padding: 0 12px;
  border: none;
  background: transparent;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.ws-mode-seg.is-active {
  background: var(--ws-elevated);
  color: var(--ws-accent);
  font-weight: 600;
}

.ws-mode-seg:not(.is-active):hover {
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
}
</style>
