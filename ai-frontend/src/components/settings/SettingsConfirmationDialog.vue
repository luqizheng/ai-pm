<script setup lang="ts">
import { TriangleAlert } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";

/**
 * SettingsConfirmationDialog —— 破坏性操作确认（Settings Framework §6.7/§7）。
 *
 * ui/confirmation-dialog 的设置域语义包装：固定 danger 变体 +
 * 警示图标 + 默认文案走 i18n；敏感/不可逆操作（移除成员、断开集成、
 * 归档项目、吊销会话）必须经此确认，不做自动执行。
 */
defineProps<{
  /** 是否打开（v-model:open） */
  open: boolean;
  /** 确认标题（已翻译） */
  title: string;
  /** 影响说明（已翻译；空省略） */
  description?: string;
  /** 确认按钮文案（已翻译；默认 t('settings.common.confirm')） */
  confirmLabel?: string;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  confirm: [];
}>();

const { t } = useI18n();
</script>

<template>
  <ConfirmationDialog
    :open="open"
    :title="title"
    :description="description"
    :confirm-label="confirmLabel ?? t('settings.common.confirm')"
    :cancel-label="t('settings.common.cancel')"
    variant="danger"
    :icon="TriangleAlert"
    size="sm"
    @update:open="emit('update:open', $event)"
    @confirm="emit('confirm')"
  />
</template>
