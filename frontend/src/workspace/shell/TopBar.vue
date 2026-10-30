<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Bell, ChevronRight, Moon, Sun, User } from "lucide-vue-next";
import { useTheme } from "../useTheme";

defineProps<{ project: string; page: string }>();

const { t } = useI18n();
const theme = useTheme();
const isDark = computed(() => theme.theme.value === "dark");
</script>

<template>
  <header class="workspace-topbar ws-topbar">
    <nav class="ws-breadcrumb" :aria-label="t('workspace.shell.breadcrumb')">
      <span class="ws-breadcrumb-project">{{ project }}</span>
      <ChevronRight :size="14" class="ws-breadcrumb-sep" aria-hidden="true" />
      <span class="ws-breadcrumb-page">{{ page }}</span>
    </nav>
    <div class="ws-topbar-actions">
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
</style>
