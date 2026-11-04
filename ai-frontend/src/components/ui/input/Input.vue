<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/TextInput + PasswordInput" node=13:32788/13:32847 date=2026-11-04 -->
<script setup lang="ts">
import { computed, inject } from "vue";
import { cn } from "@/lib/cn";
import { CircleAlert, LoaderCircle } from "lucide-vue-next";
import { FORM_FIELD_ERROR_ID } from "../form-field/context";

interface Props {
  /** 原生 id（便于外部 label 的 for 关联） */
  id?: string;
  modelValue?: string | number;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 异步校验/提交中：内嵌 loader 图标（Figma Loading 态） */
  loading?: boolean;
  /** 错误态：danger 描边 + 内嵌警示图标，并输出 aria-invalid */
  invalid?: boolean;
  size?: "sm" | "md" | "lg";
}

const props = withDefaults(defineProps<Props>(), {
  id: undefined,
  modelValue: "",
  type: "text",
  placeholder: undefined,
  disabled: false,
  loading: false,
  invalid: false,
  size: "md"
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

// 位于 FormField 内时自动关联错误文案
const fieldErrorId = inject(FORM_FIELD_ERROR_ID, undefined);

const isInvalid = computed(() => props.invalid || Boolean(fieldErrorId?.value));
const describedBy = computed(() => fieldErrorId?.value);

const classes = computed(() =>
  cn("ui-input", `ui-input--${props.size}`, {
    "is-invalid": isInvalid.value,
    "is-mono": props.type === "password"
  })
);

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}
</script>

<!-- Figma 的 Input 是"容器帧"：文字 + 右侧内嵌图标（loading/error）。
     原生 input 无法包含子元素，故用外壳 div 承载状态类与图标。 -->
<template>
  <div :class="classes">
    <input
      :id="props.id"
      class="ui-input__native"
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :aria-invalid="isInvalid ? 'true' : undefined"
      :aria-describedby="describedBy"
      @input="onInput"
    />
    <LoaderCircle v-if="loading" class="ui-input__icon" aria-hidden="true" />
    <CircleAlert
      v-else-if="isInvalid"
      class="ui-input__icon ui-input__icon--danger"
      aria-hidden="true"
    />
    <!-- 尾部插槽：口令可见性切换等控件（Figma DS/Forms/PasswordInput 的 eye 图标位） -->
    <slot name="trailing" />
  </div>
</template>

<style scoped>
/* Figma 组件库 DS/Forms/TextInput（13:32788）：
 * canvas 底 #111318 / border #353B48 / radius 8 / sm28 md36 lg44 / padding 0 12。
 * Hover 描边 = 次文字色；Focus = accent 2px（inset 阴影补 1px 免布局抖动）；
 * Error 描边 = #E69A9F + 内嵌 circle-alert；Disabled 整体 0.45 不透明度。 */
.ui-input {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  background: var(--surface-sunken);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-input__native {
  flex: 1;
  min-width: 0;
  border: 0;
  outline: none;
  padding: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}

/* 密码输入用 mono 字族（Figma PasswordInput = IBM Plex Mono） */
.ui-input.is-mono .ui-input__native {
  font-family: var(--font-mono);
}

.ui-input__native::placeholder {
  color: var(--text-secondary);
}

.ui-input:hover:has(.ui-input__native:not(:disabled)) {
  border-color: var(--border-hover-strong);
}

/* 聚焦 —— Figma：accent 2px 描边（1px border + 1px inset 阴影模拟，不抖布局） */
.ui-input:focus-within {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 var(--border-1) var(--accent);
}

/* 错误态 —— Figma：#E69A9F 描边 + 内嵌警示图标 */
.ui-input.is-invalid {
  border-color: var(--danger-title);
}

.ui-input.is-invalid:focus-within {
  box-shadow: inset 0 0 0 var(--border-1) var(--danger-title);
}

/* 禁用 —— Figma：elevated 底 + 整体 0.45 不透明度 */
.ui-input:has(.ui-input__native:disabled) {
  background: var(--surface-raised);
  opacity: var(--opacity-disabled-field);
}

.ui-input:has(.ui-input__native:disabled) .ui-input__native {
  cursor: not-allowed;
}

.ui-input--sm {
  height: var(--size-control-sm);
}

.ui-input--lg {
  height: var(--size-control-lg);
}

/* 内嵌状态图标 —— Figma 16px；loading 用次文字色，error 用 danger */
.ui-input__icon {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-secondary);
  animation: ui-input-spin var(--duration-spinner) linear infinite;
}

.ui-input__icon--danger {
  color: var(--danger-title);
  animation: none;
}

@keyframes ui-input-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ui-input__icon {
    animation: none;
  }
}
</style>
