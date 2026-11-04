<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { DESIGN_PATHS } from "@/router/design";
import DesignLayout from "./DesignLayout.vue";

/**
 * /design —— 设计系统总览：分区索引 + 深浅主题说明。
 */
const { t } = useI18n();

const cards = [
  {
    section: t("design.index.sectionComponents"),
    items: [
      {
        label: t("design.components.buttons.title"),
        to: DESIGN_PATHS.components.buttons
      },
      {
        label: t("design.components.forms.title"),
        to: DESIGN_PATHS.components.forms
      },
      {
        label: t("design.components.navigation.title"),
        to: DESIGN_PATHS.components.navigation
      },
      {
        label: t("design.components.overlays.title"),
        to: DESIGN_PATHS.components.overlays
      },
      {
        label: t("design.components.feedback.title"),
        to: DESIGN_PATHS.components.feedback
      },
      {
        label: t("design.components.data.title"),
        to: DESIGN_PATHS.components.data
      },
      { label: t("design.components.ai.title"), to: DESIGN_PATHS.components.ai }
    ]
  },
  {
    section: t("design.index.sectionNavigation"),
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
    section: t("design.index.sectionShell"),
    items: [
      { label: t("design.shell.states.title"), to: DESIGN_PATHS.shell.states }
    ]
  }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-index__title">{{ t("design.index.title") }}</h1>
      <p class="design-index__subtitle">{{ t("design.index.subtitle") }}</p>
    </template>

    <p class="design-index__hint">{{ t("design.index.hint") }}</p>

    <section
      v-for="group in cards"
      :key="group.section"
      class="design-index__group"
    >
      <h2 class="design-index__group-title">{{ group.section }}</h2>
      <div class="design-index__cards">
        <RouterLink
          v-for="item in group.items"
          :key="item.to"
          class="design-index__card"
          :to="item.to"
        >
          {{ item.label }}
        </RouterLink>
      </div>
    </section>
  </DesignLayout>
</template>

<style scoped>
.design-index__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-index__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-index__hint {
  margin: 0 0 var(--space-6);
  font-size: var(--ws-font-size-micro);
  color: var(--ws-text-secondary);
}

.design-index__group {
  margin: 0 0 var(--space-6);
}

.design-index__group-title {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-index__cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--space-3);
}

.design-index__card {
  padding: var(--space-3) var(--space-4);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-primary);
  text-decoration: none;
}

.design-index__card:hover {
  background-color: var(--surface-raised);
}

.design-index__card:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}
</style>
