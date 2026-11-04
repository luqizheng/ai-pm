<script setup lang="ts">
import { computed } from "vue";

/**
 * SettingsFieldLabel —— 设置表单字段标签（Settings Framework §7）。
 *
 * 关联控件 id 时渲染真实 <label>（点击聚焦控件），否则渲染 <span>。
 * 必填标记为装饰性星号，读屏语义由控件自身的 required/aria 传达。
 */
const props = defineProps<{
  /** 标签文案（已翻译） */
  label: string;
  /** 关联控件 id（提供时渲染 <label for>） */
  htmlFor?: string;
  /** 必填标记 */
  required?: boolean;
  /** 只读徽标文案（如「只读」；配合 SettingsReadOnlyField；空则不展示） */
  flag?: string;
}>();

const Tag = computed(() => (props.htmlFor ? "label" : "span"));
</script>

<template>
  <component :is="Tag" :for="htmlFor" class="settings-field-label">
    <span class="settings-field-label__text">{{ label }}</span>
    <span
      v-if="required"
      class="settings-field-label__required"
      aria-hidden="true"
      >*</span
    >
    <span v-if="flag" class="settings-field-label__flag">{{ flag }}</span>
  </component>
</template>

<style scoped>
.settings-field-label {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-1);
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-medium);
  color: var(--ws-text-primary);
}

.settings-field-label__required {
  color: var(--ws-color-danger-text);
}

.settings-field-label__flag {
  font-size: var(--ws-font-size-micro);
  font-weight: var(--weight-regular);
  color: var(--ws-text-secondary);
}
</style>
