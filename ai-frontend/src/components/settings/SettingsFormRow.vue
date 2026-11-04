<script setup lang="ts">
import SettingsFieldLabel from "./SettingsFieldLabel.vue";
import SettingsHelperText from "./SettingsHelperText.vue";

/**
 * SettingsFormRow —— 设置表单行（Settings Framework §7）。
 *
 * 双栏布局：左侧标签 + 辅助说明，右侧控件插槽（默认）。
 * 行为约束（§6）：行本身无状态；禁用/只读由调用方传给控件，
 * 校验错误由调用方以 SettingsValidationMessage 渲染在控件下方。
 */
defineProps<{
  /** 字段标签（已翻译；空省略，纯插槽行） */
  label?: string;
  /** 关联控件 id（提供时标签为 <label for>） */
  htmlFor?: string;
  /** 必填标记 */
  required?: boolean;
  /** 标签旁只读徽标文案（空省略） */
  flag?: string;
  /** 辅助说明（已翻译；空省略） */
  helper?: string;
}>();
</script>

<template>
  <div class="settings-form-row">
    <div class="settings-form-row__label">
      <SettingsFieldLabel
        v-if="label"
        :label="label"
        :html-for="htmlFor"
        :required="required"
        :flag="flag"
      />
      <SettingsHelperText v-if="helper">{{ helper }}</SettingsHelperText>
    </div>
    <div class="settings-form-row__control">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.settings-form-row {
  display: grid;
  grid-template-columns: minmax(10rem, 2fr) minmax(0, 3fr);
  gap: var(--space-3) var(--space-6);
  align-items: start;
}

.settings-form-row__label {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding-top: var(--space-2);
}

.settings-form-row__control {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

@media (max-width: 60rem) {
  .settings-form-row {
    grid-template-columns: minmax(0, 1fr);
  }

  .settings-form-row__label {
    padding-top: 0;
  }
}
</style>
