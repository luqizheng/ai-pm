<script setup lang="ts">
import { useI18n } from "vue-i18n";

import Button from "@/components/ui/button/Button.vue";
import Dialog from "@/components/ui/dialog/Dialog.vue";

/**
 * SettingsUnsavedChangesDialog —— 未保存更改警告（Settings Framework §6.6/§7）。
 *
 * 组合 ui/dialog（§7：不重复造弹窗）：域内跳转且有未保存更改时弹出，
 * 三选一 —— 保存后离开 / 放弃更改 / 继续编辑。不做自动静默丢弃。
 */
defineProps<{
  /** 是否打开（v-model:open） */
  open: boolean;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  /** 保存并离开 */
  save: [];
  /** 放弃更改并离开 */
  discard: [];
}>();

const { t } = useI18n();

function close() {
  emit("update:open", false);
}

function onSave() {
  emit("save");
  close();
}

function onDiscard() {
  emit("discard");
  close();
}
</script>

<template>
  <Dialog
    :open="open"
    :title="t('settings.common.unsavedTitle')"
    :description="t('settings.common.unsavedDesc')"
    size="sm"
    @update:open="emit('update:open', $event)"
  >
    <template #footer>
      <div class="settings-unsaved__actions">
        <Button variant="ghost" size="sm" @click="close">
          {{ t("settings.common.keepEditing") }}
        </Button>
        <Button variant="tertiary" size="sm" @click="onDiscard">
          {{ t("settings.common.discard") }}
        </Button>
        <Button variant="primary" size="sm" @click="onSave">
          {{ t("settings.common.saveAndLeave") }}
        </Button>
      </div>
    </template>
  </Dialog>
</template>

<style scoped>
.settings-unsaved__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}
</style>
