<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/PasswordInput" node=13:32847 date=2026-11-04 -->
<script setup lang="ts">
import { computed, ref } from "vue";
import { Eye, EyeOff } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import Input from "./Input.vue";

/**
 * 口令输入 —— Figma 组件库 DS/Forms/PasswordInput（13:32847）。
 *
 * 外壳复用 TextInput（canvas 底 / radius 8 / 36px / mono 字族），
 * 尾部挂 eye 图标切换明文：
 *   - 隐藏态：IBM Plex Mono 13px + 圆点（与 Figma 一致）
 *   - 可见态：Inter 13px 明文 + eye-off 图标
 *   - 图标带「显示/隐藏口令」可访问名称（读屏可辨，不靠图标形状猜）
 */
interface Props {
  /** 原生 id */
  id?: string;
  modelValue?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 异步校验 / 提交中 */
  loading?: boolean;
  /** 错误态 */
  invalid?: boolean;
  size?: "sm" | "md" | "lg";
  /** 初始明文可见（状态参照板用） */
  defaultVisible?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  id: undefined,
  modelValue: "",
  placeholder: undefined,
  disabled: false,
  loading: false,
  invalid: false,
  size: "md",
  defaultVisible: false
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const { t } = useI18n();

/** 当前是否明文可见 */
const visible = ref(props.defaultVisible);

const inputType = computed(() => (visible.value ? "text" : "password"));
const toggleLabel = computed(() =>
  visible.value ? t("common.password.hide") : t("common.password.show")
);

function onInput(value: string) {
  emit("update:modelValue", value);
}
</script>

<template>
  <Input
    :id="props.id"
    :model-value="props.modelValue"
    :type="inputType"
    :placeholder="props.placeholder"
    :disabled="props.disabled"
    :loading="props.loading"
    :invalid="props.invalid"
    :size="props.size"
    @update:model-value="onInput"
  >
    <template #trailing>
      <button
        type="button"
        class="ui-password-input__toggle"
        :aria-label="toggleLabel"
        :aria-pressed="visible"
        :disabled="props.disabled"
        @click="visible = !visible"
      >
        <EyeOff
          v-if="visible"
          class="ui-password-input__icon"
          aria-hidden="true"
        />
        <Eye v-else class="ui-password-input__icon" aria-hidden="true" />
      </button>
    </template>
  </Input>
</template>

<style scoped>
/* 可见性切换 —— Figma eye 图标 16px（--size-icon-xs） */
.ui-password-input__toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: var(--radius-xs);
}

.ui-password-input__toggle:hover:not(:disabled) {
  color: var(--text-primary);
}

.ui-password-input__toggle:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--space-1);
}

.ui-password-input__toggle:disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-field);
}

.ui-password-input__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}
</style>
