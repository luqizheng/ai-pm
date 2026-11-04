<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Notification/Bell" node=33:87783 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { Bell } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import Badge from "@/components/ui/badge/Badge.vue";

/**
 * 通知铃铛（Figma Global/Notification/Bell 33:87783，36×36 / radius 8）。
 *
 * 态：Default（sidebar 底）/ Hover（elevated）/ Focus（2px accent 描边）/
 * Active（accent 着色面 = 下拉展开中）/ Disabled（整体 0.4）。
 * 未读计数是**文本徽标**（Badge Count，铃铛右侧同行，不是色点）——
 * Figma 帧注记："Unread count is textual, not just a colored dot"。
 */
interface Props {
  /** 未读数（>0 显示文本徽标） */
  unreadCount?: number;
  /** 下拉展开中（Active 态：accent 着色面） */
  active?: boolean;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  unreadCount: 0,
  active: false,
  disabled: false
});

const emit = defineEmits<{ click: [] }>();

const { t } = useI18n();

const ariaLabel = computed(() => t("global.notification.label"));
</script>

<template>
  <span class="notification-bell">
    <button
      type="button"
      class="notification-bell__button"
      :class="{ 'is-active': props.active }"
      :aria-label="ariaLabel"
      :disabled="props.disabled"
      @click="emit('click')"
    >
      <Bell :size="16" aria-hidden="true" />
    </button>
    <Badge v-if="props.unreadCount > 0" variant="count" size="sm">
      {{ props.unreadCount }}
    </Badge>
  </span>
</template>

<style scoped>
.notification-bell {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}

/* Default：sidebar 底（顶栏同色域）；Hover elevated；Active accent 着色面 */
.notification-bell__button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-button);
  height: var(--size-icon-button);
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: var(--ws-sidebar);
  border: none;
  border-radius: var(--radius-sm);
}

.notification-bell__button:hover:not(:disabled) {
  color: var(--ws-text-primary);
  background: var(--ws-elevated);
}

.notification-bell__button:active:not(:disabled) {
  background: var(--ws-hover-veil);
}

.notification-bell__button.is-active {
  color: var(--ws-text-primary);
  background: var(--ws-color-accent-surface);
}

/* Focus：2px accent 描边（Figma Focus 态） */
.notification-bell__button:focus-visible {
  outline: none;
  border: var(--border-1) solid var(--ws-accent);
  box-shadow: inset 0 0 0 1px var(--ws-accent);
}

/* Disabled：整体 0.4（Figma：Disabled 表示不可用，不是空事件流） */
.notification-bell__button:disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}
</style>
