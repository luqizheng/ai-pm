<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Help/Menu + MenuItem" node=33:87801;33:87802 date=2026-11-04 -->
<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { CircleQuestionMark } from "lucide-vue-next";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger
} from "reka-ui";

import UserMenuItem from "@/components/global/usermenu/UserMenuItem.vue";

/**
 * 帮助菜单（Figma Global/Help/Menu 33:87801 + MenuItem 33:87802，360 面板）。
 *
 * 顶栏帮助图标触发；三项：帮助与文档 / 键盘快捷键 / 反馈。
 * 帧注记：反馈入口仅为 specimen（Entry specimen only. No delivery
 * guarantee is implied）——事件交调用方，本组件不实现送达。
 */
defineProps<{
  /** 图标按钮的无障碍名称覆盖（默认取 i18n） */
  disabled?: boolean;
}>();

const emit = defineEmits<{
  documentation: [];
  shortcuts: [];
  feedback: [];
}>();

const { t } = useI18n();

const open = ref(false);
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child>
      <button
        type="button"
        class="help-menu__trigger"
        :aria-label="t('global.help.label')"
        :disabled="disabled"
      >
        <CircleQuestionMark :size="16" aria-hidden="true" />
      </button>
    </PopoverTrigger>

    <PopoverPortal>
      <PopoverContent class="help-menu" side="bottom" align="end" :side-offset="4">
        <p class="help-menu__title">{{ t("global.help.label") }}</p>
        <nav class="help-menu__items" role="menu" :aria-label="t('global.help.label')">
          <UserMenuItem
            :label="t('global.help.documentation')"
            @click="
              open = false;
              emit('documentation');
            "
          />
          <UserMenuItem
            :label="t('global.help.shortcuts')"
            @click="
              open = false;
              emit('shortcuts');
            "
          />
          <UserMenuItem
            :label="t('global.help.feedback')"
            @click="
              open = false;
              emit('feedback');
            "
          />
        </nav>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
.help-menu__trigger {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.help-menu__trigger:hover:not(:disabled) {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.help-menu__trigger:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.help-menu__trigger:disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}

:deep(.help-menu),
.help-menu {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  width: var(--size-help-menu-width);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  z-index: 50;
}

.help-menu__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.help-menu__items {
  display: flex;
  flex-direction: column;
}
</style>
