<script setup lang="ts">
import { useI18n } from "vue-i18n";

import DesignLayout from "./DesignLayout.vue";

/**
 * /design/tokens —— 令牌全表（只读展示 tokens.css 核心数值）。
 * 数值与 tokens.css 对齐；改 token 请改 tokens.css 并跑 tokens.test.ts。
 */
const { t } = useI18n();

/** 颜色 swatch：名 + var 引用（浅色行在预览区由 data-theme 呈现） */
const colorVars = [
  { name: "--ws-canvas", cssVar: "var(--ws-canvas)" },
  { name: "--ws-sidebar", cssVar: "var(--ws-sidebar)" },
  { name: "--ws-surface", cssVar: "var(--ws-surface)" },
  { name: "--ws-elevated", cssVar: "var(--ws-elevated)" },
  { name: "--ws-border", cssVar: "var(--ws-border)" },
  { name: "--ws-text-primary", cssVar: "var(--ws-text-primary)" },
  { name: "--ws-text-secondary", cssVar: "var(--ws-text-secondary)" },
  { name: "--ws-text-disabled", cssVar: "var(--ws-text-disabled)" },
  { name: "--ws-accent", cssVar: "var(--ws-accent)" },
  { name: "--ws-accent-hover", cssVar: "var(--ws-accent-hover)" },
  { name: "--ws-accent-pressed", cssVar: "var(--ws-accent-pressed)" },
  {
    name: "--ws-color-accent-surface",
    cssVar: "var(--ws-color-accent-surface)"
  },
  { name: "--ws-color-info", cssVar: "var(--ws-color-info)" },
  { name: "--ws-color-success", cssVar: "var(--ws-color-success)" },
  { name: "--ws-color-warning", cssVar: "var(--ws-color-warning)" },
  { name: "--ws-color-danger", cssVar: "var(--ws-color-danger)" },
  {
    name: "--ws-color-danger-surface",
    cssVar: "var(--ws-color-danger-surface)"
  },
  { name: "--ws-color-danger-border", cssVar: "var(--ws-color-danger-border)" },
  { name: "--ws-color-danger-title", cssVar: "var(--ws-color-danger-title)" }
];

const controlSizes = [
  { name: "--size-control-sm", value: "28px" },
  { name: "--size-control-md", value: "36px" },
  { name: "--size-control-lg", value: "44px" },
  { name: "--size-nav-item", value: "36px" },
  { name: "--size-control-compact", value: "32px" },
  { name: "--size-overlay-panel", value: "448px" }
];

const spacingScale = [
  { name: "--space-1", value: "4px" },
  { name: "--space-2", value: "8px" },
  { name: "--space-3", value: "12px" },
  { name: "--space-4", value: "16px" },
  { name: "--space-6", value: "24px" },
  { name: "--radius-xs", value: "4px" },
  { name: "--radius-sm", value: "8px" },
  { name: "--radius-lg", value: "12px" }
];

const typeScale = [
  { name: "--font-size-page", value: "21px" },
  { name: "--font-size-body-strong", value: "15px" },
  { name: "--font-size-body", value: "13px" },
  { name: "--font-size-caption", value: "12px" },
  { name: "--font-size-micro", value: "11px" },
  { name: "--font-size-nano", value: "10px" }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-tokens__title">{{ t("design.tokens.title") }}</h1>
      <p class="design-tokens__subtitle">{{ t("design.tokens.subtitle") }}</p>
    </template>

    <section class="design-tokens__group">
      <h2>{{ t("design.tokens.colors") }}</h2>
      <div class="design-tokens__swatches">
        <figure
          v-for="c in colorVars"
          :key="c.name"
          class="design-tokens__swatch"
        >
          <span
            class="design-tokens__chip"
            :style="{ backgroundColor: c.cssVar }"
          />
          <figcaption>{{ c.name }}</figcaption>
        </figure>
      </div>
    </section>

    <section class="design-tokens__group">
      <h2>{{ t("design.tokens.controls") }}</h2>
      <table class="design-tokens__table">
        <tbody>
          <tr v-for="row in controlSizes" :key="row.name">
            <th>{{ row.name }}</th>
            <td>{{ row.value }}</td>
            <td>
              <span
                class="design-tokens__bar"
                :style="{ width: row.value, height: row.value }"
              />
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="design-tokens__group">
      <h2>{{ t("design.tokens.typography") }}</h2>
      <table class="design-tokens__table">
        <tbody>
          <tr v-for="row in typeScale" :key="row.name">
            <th>{{ row.name }}</th>
            <td>{{ row.value }}</td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="design-tokens__group">
      <h2>{{ t("design.tokens.spacing") }}</h2>
      <table class="design-tokens__table">
        <tbody>
          <tr v-for="row in spacingScale" :key="row.name">
            <th>{{ row.name }}</th>
            <td>{{ row.value }}</td>
          </tr>
        </tbody>
      </table>
    </section>
  </DesignLayout>
</template>

<style scoped>
.design-tokens__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-tokens__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-tokens__group {
  margin: 0 0 var(--space-6);
}

.design-tokens__group h2 {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-tokens__swatches {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: var(--space-3);
}

.design-tokens__swatch {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
}

.design-tokens__chip {
  width: var(--size-control-md);
  height: var(--size-control-md);
  flex: none;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.design-tokens__swatch figcaption {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
  word-break: break-all;
}

.design-tokens__table {
  border-collapse: collapse;
}

.design-tokens__table th,
.design-tokens__table td {
  padding: var(--space-1) var(--space-3) var(--space-1) 0;
  text-align: left;
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.design-tokens__bar {
  display: inline-block;
  background-color: var(--ws-accent);
  border-radius: var(--radius-xs);
}
</style>
