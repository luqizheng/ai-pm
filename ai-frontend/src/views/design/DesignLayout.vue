<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 设计系统目录外壳 —— 全屏页布局（无应用壳导航）。
 * 左侧目录导航 + 顶部标题条（含深浅主题预览切换）+ 右侧内容插槽。
 * 主题切换作用于本布局根元素 data-theme（token 变量级联，只影响本页）。
 */
import { ref } from "vue";

const { t } = useI18n();

const isLight = ref(false);

const navSections = computed(() => [
  {
    label: t("design.index.sectionTokens"),
    items: [{ label: t("design.tokens.title"), to: DESIGN_PATHS.tokens }]
  },
  {
    label: t("design.index.sectionComponents"),
    items: [
      {
        label: t("design.components.buttons.title").split(" — ")[0],
        to: DESIGN_PATHS.components.buttons
      },
      {
        label: t("design.components.forms.title").split(" — ")[0],
        to: DESIGN_PATHS.components.forms
      },
      {
        label: t("design.components.navigation.title").split(" — ")[0],
        to: DESIGN_PATHS.components.navigation
      },
      {
        label: t("design.components.overlays.title").split(" — ")[0],
        to: DESIGN_PATHS.components.overlays
      },
      {
        label: t("design.components.feedback.title").split(" — ")[0],
        to: DESIGN_PATHS.components.feedback
      },
      {
        label: t("design.components.data.title").split(" — ")[0],
        to: DESIGN_PATHS.components.data
      },
      {
        label: t("design.components.ai.title").split(" — ")[0],
        to: DESIGN_PATHS.components.ai
      }
    ]
  },
  {
    label: t("design.index.sectionNavigation"),
    items: [
      {
        label: t("design.navigation.global.title"),
        to: DESIGN_PATHS.navigation.global
      },
      {
        label: t("design.navigation.contexts.title"),
        to: DESIGN_PATHS.navigation.contexts
      },
      {
        label: t("design.navigation.switcher.title"),
        to: DESIGN_PATHS.navigation.switcher
      },
      {
        label: t("design.navigation.states.title"),
        to: DESIGN_PATHS.navigation.states
      },
      {
        label: t("design.navigation.switching.title"),
        to: DESIGN_PATHS.navigation.switching
      },
      {
        label: t("design.navigation.library.title"),
        to: DESIGN_PATHS.navigation.library
      }
    ]
  },
  {
    label: t("design.index.sectionShell"),
    items: [
      { label: t("design.shell.states.title"), to: DESIGN_PATHS.shell.states }
    ]
  }
]);
</script>

<template>
  <div class="design-layout" :data-theme="isLight ? 'light' : undefined">
    <aside class="design-layout__rail">
      <RouterLink class="design-layout__home" :to="DESIGN_PATHS.index">
        <span class="design-layout__brand">AI R&D</span>
        <span class="design-layout__brand-sub">DESIGN CATALOG</span>
      </RouterLink>

      <nav class="design-layout__nav">
        <div
          v-for="section in navSections"
          :key="section.label"
          class="design-layout__section"
        >
          <p class="design-layout__section-title">{{ section.label }}</p>
          <RouterLink
            v-for="item in section.items"
            :key="item.to"
            class="design-layout__link"
            :to="item.to"
          >
            {{ item.label }}
          </RouterLink>
        </div>
      </nav>
    </aside>

    <div class="design-layout__main">
      <header class="design-layout__header">
        <div class="design-layout__heading">
          <slot name="title" />
        </div>
        <button
          type="button"
          class="design-layout__theme"
          :aria-pressed="isLight ? 'true' : 'false'"
          @click="isLight = !isLight"
        >
          {{ isLight ? "Dark" : "Light" }} · {{ t("design.theme.toggle") }}
        </button>
      </header>

      <main class="design-layout__content">
        <slot />
      </main>
    </div>
  </div>
</template>

<style scoped>
.design-layout {
  display: grid;
  grid-template-columns: var(--ws-sidebar-width) minmax(0, 1fr);
  min-height: 100dvh;
  background-color: var(--ws-canvas);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-body);
}

/* 左侧目录栏：侧栏底 + 右描边（壳同构） */
.design-layout__rail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4) var(--space-3);
  background-color: var(--ws-sidebar);
  border-right: var(--border-1) solid var(--ws-border);
}

.design-layout__home {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-2) var(--space-2);
  border-radius: var(--radius-sm);
  text-decoration: none;
}

.design-layout__home:hover {
  background-color: var(--surface-raised);
}

.design-layout__brand {
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-layout__brand-sub {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-layout__nav {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  overflow-y: auto;
}

.design-layout__section {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.design-layout__section-title {
  margin: 0 0 var(--space-1);
  padding: 0 var(--space-2);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-layout__link {
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-xs);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  text-decoration: none;
}

.design-layout__link:hover {
  background-color: var(--surface-raised);
  color: var(--ws-text-primary);
}

.design-layout__link.router-link-active {
  background-color: var(--accent-surface);
  color: var(--ws-text-primary);
}

.design-layout__link:focus-visible,
.design-layout__home:focus-visible,
.design-layout__theme:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.design-layout__main {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.design-layout__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-6);
  border-bottom: var(--border-1) solid var(--ws-border);
  background-color: var(--ws-sidebar);
}

.design-layout__heading {
  min-width: 0;
}

.design-layout__theme {
  flex: none;
  height: var(--size-control-compact);
  padding: 0 var(--space-3);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  background-color: var(--surface-raised);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  cursor: pointer;
}

.design-layout__theme:hover {
  color: var(--ws-text-primary);
}

.design-layout__content {
  flex: 1;
  min-width: 0;
  padding: var(--space-6);
  overflow-x: auto;
}

@media (prefers-reduced-motion: reduce) {
  .design-layout__link,
  .design-layout__home,
  .design-layout__theme {
    transition: none;
  }
}
</style>
