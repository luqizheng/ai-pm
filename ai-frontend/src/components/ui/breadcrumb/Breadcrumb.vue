<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Navigation/Breadcrumb" node=13:33452 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import { ChevronRight } from "lucide-vue-next";

export interface BreadcrumbItem {
  label: string;
  to?: string;
}

interface Props {
  /** 层级条目 */
  items: BreadcrumbItem[];
  /** 导航的无障碍名称 */
  ariaLabel?: string;
}

const props = withDefaults(defineProps<Props>(), { ariaLabel: undefined });

const { t } = useI18n();

// 默认文案走 i18n（调用方显式传入时优先）
const ariaLabelText = computed(
  () => props.ariaLabel ?? t("shell.nav.breadcrumb")
);
</script>

<template>
  <nav class="ui-breadcrumb" :aria-label="ariaLabelText">
    <ol class="ui-breadcrumb__list">
      <li
        v-for="(item, index) in items"
        :key="index"
        class="ui-breadcrumb__item"
        :aria-current="index === items.length - 1 ? 'page' : undefined"
      >
        <!-- RouterLink 保证 SPA 内导航（原生 href 会整页刷新） -->
        <RouterLink
          v-if="item.to && index !== items.length - 1"
          class="ui-breadcrumb__link"
          :to="item.to"
        >
          {{ item.label }}
        </RouterLink>
        <span v-else class="ui-breadcrumb__current">{{ item.label }}</span>
        <ChevronRight
          v-if="index < items.length - 1"
          class="ui-breadcrumb__separator"
          aria-hidden="true"
        />
      </li>
    </ol>
  </nav>
</template>

<style scoped>
/* Figma DS/Navigation/Breadcrumb（13:33452）：
 * row padding 0 4px / gap 8 / 分隔符 chevron-right 12px；
 * 前层级次文字（Hover=accent，非主文字）；当前页主文字。 */
.ui-breadcrumb__list {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  padding: 0 var(--space-1);
  list-style: none;
}

.ui-breadcrumb__item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
}

.ui-breadcrumb__link {
  color: var(--text-secondary);
  text-decoration: none;
}

/* Figma Hover 态：前层级文字变 accent */
.ui-breadcrumb__link:hover {
  color: var(--accent);
}

.ui-breadcrumb__link:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  border-radius: var(--radius-xs);
}

.ui-breadcrumb__current {
  color: var(--text-primary);
}

.ui-breadcrumb__separator {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  color: var(--text-secondary);
}
</style>
