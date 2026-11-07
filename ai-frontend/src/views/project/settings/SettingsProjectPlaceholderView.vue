<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import SettingsAppFrame from "@/views/settings/SettingsAppFrame.vue";
import { useProjectSettingsFrame } from "./shared";

/**
 * Project Setting 占位节 —— General / Members / AI Configuration 三节
 * 规划中（V1.0 稿五节，2026-11-07 裁决：随后端能力逐个迁入）。
 *
 * 标题取路由 meta.title（settings.nav.*，与侧栏条目同源）；文案明示
 * 「规划中」，不冒充既有能力（Settings Framework 原则 P3）。
 */
const { t } = useI18n();
const route = useRoute();
const settingsFrame = useProjectSettingsFrame();

const title = computed(() =>
  route.meta.title
    ? t(route.meta.title as string)
    : t("settings.project.landing.title")
);
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="settingsFrame.breadcrumb(title)"
    :title="title"
    :scope-label="settingsFrame.scopeLabel.value"
    :entries="settingsFrame.navEntries"
    :icons="settingsFrame.navIcons"
  >
    <div class="ps-placeholder" role="status">
      <p class="ps-placeholder__title">
        {{ t("settings.project.proposedTitle") }}
      </p>
      <p class="ps-placeholder__hint">
        {{ t("settings.project.proposedHint") }}
      </p>
    </div>
  </SettingsAppFrame>
</template>

<style scoped>
.ps-placeholder {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-8);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ps-placeholder__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
}

.ps-placeholder__hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}
</style>
