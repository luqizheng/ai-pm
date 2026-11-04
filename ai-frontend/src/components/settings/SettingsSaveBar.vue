<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import Button from "@/components/ui/button/Button.vue";

/**
 * SettingsSaveBar —— 设置页保存条（Settings Framework §6.3/§6.12/§7）。
 *
 * 高影响配置不做自动保存（§6）：有未保存更改时吸底浮现，
 * 提供 保存/取消；saving 态锁定按钮；saved 短暂展示成功反馈。
 */
const props = defineProps<{
  /** 是否有未保存更改（控制浮现与可保存性） */
  dirty: boolean;
  /** 保存请求进行中 */
  saving?: boolean;
  /** 刚保存成功（短暂态，由调用方计时复位） */
  saved?: boolean;
  /** 保存失败信息（已翻译；非空展示错误） */
  error?: string;
}>();

const emit = defineEmits<{
  save: [];
  cancel: [];
}>();

const { t } = useI18n();

const visible = computed(
  () => props.dirty || props.saving || props.saved || !!props.error
);

const statusText = computed(() => {
  if (props.error) return t("settings.common.saveFailed");
  if (props.saving) return t("settings.common.saving");
  if (props.saved) return t("settings.common.saved");
  return t("settings.common.unsavedHint");
});
</script>

<template>
  <Transition name="settings-savebar">
    <div
      v-if="visible"
      class="settings-savebar"
      :class="{ 'is-error': !!error }"
      role="status"
    >
      <span class="settings-savebar__status">{{ statusText }}</span>
      <div class="settings-savebar__actions">
        <Button
          variant="ghost"
          size="sm"
          :disabled="saving"
          @click="emit('cancel')"
        >
          {{ t("settings.common.cancel") }}
        </Button>
        <Button
          variant="primary"
          size="sm"
          :loading="saving"
          :disabled="!dirty"
          @click="emit('save')"
        >
          {{ t("settings.common.save") }}
        </Button>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.settings-savebar {
  position: sticky;
  bottom: 0;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  margin-top: var(--space-6);
  padding: var(--space-3) var(--space-4);
  background-color: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
}

.settings-savebar.is-error {
  border-color: var(--ws-color-danger-border);
}

.settings-savebar__status {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.settings-savebar__actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
}

.settings-savebar-enter-active,
.settings-savebar-leave-active {
  transition:
    opacity var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.settings-savebar-enter-from,
.settings-savebar-leave-to {
  opacity: 0;
  transform: translateY(var(--space-2));
}

@media (prefers-reduced-motion: reduce) {
  .settings-savebar-enter-active,
  .settings-savebar-leave-active {
    transition: none;
  }
}
</style>
