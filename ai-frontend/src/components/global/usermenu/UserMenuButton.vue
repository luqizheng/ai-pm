<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/UserMenu/AvatarButton" node=33:87785 date=2026-11-04 -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";

import Avatar from "@/components/ui/avatar/Avatar.vue";

/**
 * 账户触发按钮（Figma Global/UserMenu/AvatarButton 33:87785）。
 *
 * 40×40 外框（radius 8）内嵌 Avatar Small（28，首字母）；
 * Default 透明 / Hover elevated / Focus 2px accent 描边 /
 * Active accent 着色面（菜单展开中）/ Disabled 整体 0.4。
 */
interface Props {
  name: string;
  initials?: string;
  /** 菜单展开中（Active 态） */
  active?: boolean;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  initials: undefined,
  active: false,
  disabled: false
});

const emit = defineEmits<{ click: [] }>();

const { t } = useI18n();
</script>

<template>
  <button
    type="button"
    class="user-menu-button"
    :class="{ 'is-active': props.active }"
    :aria-label="t('global.usermenu.label')"
    :aria-haspopup="'menu'"
    :disabled="props.disabled"
    @click="emit('click')"
  >
    <Avatar :name="props.name" :label="props.name" size="sm">
      <template v-if="props.initials" #fallback>
        <span class="user-menu-button__initials">{{ props.initials }}</span>
      </template>
    </Avatar>
  </button>
</template>

<style scoped>
.user-menu-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--space-10);
  height: var(--space-10);
  padding: 0;
  cursor: pointer;
  background: transparent;
  border: var(--border-2) solid transparent;
  border-radius: var(--radius-sm);
}

.user-menu-button:hover:not(:disabled) {
  background: var(--ws-elevated);
}

.user-menu-button:focus-visible {
  outline: none;
  border-color: var(--ws-accent);
}

.user-menu-button.is-active {
  background: var(--ws-color-accent-surface);
}

.user-menu-button:disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}

.user-menu-button__initials {
  color: var(--ws-text-primary);
  font-size: var(--font-size-nano);
  font-weight: var(--weight-medium);
}
</style>
