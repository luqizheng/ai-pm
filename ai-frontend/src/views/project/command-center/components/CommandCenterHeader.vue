<script setup lang="ts">
import { ChevronRight, Lock, Settings } from "lucide-vue-next";
import { RouterLink } from "vue-router";
import Button from "@/components/ui/button/Button.vue";
import IconButton from "@/components/ui/icon-button/IconButton.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { TaskStatTone } from "../task-overview";

/**
 * 总控台页头（Pixso「Project Command Center」3:7248）。
 *
 * 结构：面包屑（Projects › 项目名）→ 标题行（标题 + 运行状态 chip +
 * 可见性 chip）→ 项目描述 → 元信息行（负责人 ｜ 创建）+ 右侧动作
 * （项目设置 + 设置图标钮）。文案由调用方翻译后传入。
 */
interface Props {
  /** 面包屑上级文案（如「项目」）与上级路由 */
  breadcrumbLabel: string;
  breadcrumbTo?: string;
  /** 项目名 */
  projectName: string;
  /** 运行状态 chip 文案（如「正常运行」） */
  statusLabel?: string;
  /** 运行状态语义档 */
  statusTone?: TaskStatTone;
  /** 可见性 chip 文案（如「仅组织内可见」）；不传不渲染 */
  visibilityLabel?: string;
  /** 项目描述 */
  description?: string;
  /** 元信息对（label + value），如 负责人/创建 */
  metaEntries?: Array<{ label: string; value: string }>;
  /** 「项目设置」按钮文案 */
  settingsLabel?: string;
  /** 设置图标钮的无障碍名 */
  settingsButtonLabel?: string;
}

withDefaults(defineProps<Props>(), {
  breadcrumbTo: "",
  statusLabel: "",
  statusTone: "success",
  visibilityLabel: "",
  description: "",
  metaEntries: () => [],
  settingsLabel: "",
  settingsButtonLabel: ""
});

const emit = defineEmits<{ (e: "settings"): void }>();
</script>

<template>
  <header class="pm-cc-header">
    <nav class="pm-cc-header__breadcrumb" aria-label="breadcrumb">
      <RouterLink
        v-if="breadcrumbTo"
        :to="breadcrumbTo"
        class="pm-cc-header__crumb-link"
      >
        {{ breadcrumbLabel }}
      </RouterLink>
      <span v-else class="pm-cc-header__crumb">{{ breadcrumbLabel }}</span>
      <ChevronRight class="pm-cc-header__crumb-icon" aria-hidden="true" />
      <span class="pm-cc-header__crumb pm-cc-header__crumb--current">
        {{ projectName }}
      </span>
    </nav>

    <div class="pm-cc-header__title-row">
      <div class="pm-cc-header__title-group">
        <h1 class="pm-cc-header__title">{{ projectName }}</h1>
        <StatusIndicator
          v-if="statusLabel"
          :status="statusTone"
          size="sm"
          :label="statusLabel"
        />
        <span v-if="visibilityLabel" class="pm-cc-header__visibility">
          <Lock class="pm-cc-header__visibility-icon" aria-hidden="true" />
          {{ visibilityLabel }}
        </span>
      </div>
      <div class="pm-cc-header__actions">
        <Button
          v-if="settingsLabel"
          variant="secondary"
          size="sm"
          @click="emit('settings')"
        >
          {{ settingsLabel }}
        </Button>
        <IconButton
          v-if="settingsButtonLabel"
          :label="settingsButtonLabel"
          size="sm"
          @click="emit('settings')"
        >
          <Settings class="pm-cc-header__settings-icon" aria-hidden="true" />
        </IconButton>
      </div>
    </div>

    <p v-if="description" class="pm-cc-header__description">
      {{ description }}
    </p>

    <div v-if="metaEntries.length" class="pm-cc-header__meta">
      <template v-for="(entry, index) in metaEntries" :key="entry.label">
        <span
          v-if="index > 0"
          class="pm-cc-header__meta-divider"
          aria-hidden="true"
        />
        <span class="pm-cc-header__meta-entry">
          <span class="pm-cc-header__meta-label">{{ entry.label }}</span>
          <span class="pm-cc-header__meta-value">{{ entry.value }}</span>
        </span>
      </template>
    </div>
  </header>
</template>

<style scoped>
.pm-cc-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-bottom: var(--space-3);
  border-bottom: var(--border-1) solid var(--border-default);
}

.pm-cc-header__breadcrumb {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.pm-cc-header__crumb {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.pm-cc-header__crumb--current {
  color: var(--text-secondary);
}

.pm-cc-header__crumb-link {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  text-decoration: none;
  transition: color var(--duration-fast) var(--ease-standard);
}

.pm-cc-header__crumb-link:hover {
  color: var(--accent);
}

.pm-cc-header__crumb-icon {
  width: var(--size-icon-2xs);
  height: var(--size-icon-2xs);
  color: var(--text-disabled);
}

.pm-cc-header__title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.pm-cc-header__title-group {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.pm-cc-header__title {
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h3);
  color: var(--text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 可见性 chip：透明底 + 描边（区别于状态 chip 的语义点） */
.pm-cc-header__visibility {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 2px var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  white-space: nowrap;
}

.pm-cc-header__visibility-icon {
  width: var(--size-icon-2xs);
  height: var(--size-icon-2xs);
  flex: none;
}

.pm-cc-header__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}

.pm-cc-header__settings-icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
}

.pm-cc-header__description {
  margin: 0;
  max-width: 72ch;
  font-size: var(--font-size-micro);
  line-height: 1.45;
  color: var(--text-secondary);
}

.pm-cc-header__meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.pm-cc-header__meta-entry {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-1);
}

.pm-cc-header__meta-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.pm-cc-header__meta-value {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--text-secondary);
}

.pm-cc-header__meta-divider {
  width: var(--border-1);
  height: var(--size-meta-divider);
  background: var(--border-default);
}
</style>
