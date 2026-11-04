<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/Textarea" node=13:32944 date=2026-11-04 -->
<script setup lang="ts">
import { computed, inject } from "vue";
import { cn } from "@/lib/cn";
import { CircleAlert } from "lucide-vue-next";
import { FORM_FIELD_ERROR_ID } from "../form-field/context";

interface Props {
  modelValue?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 错误态：danger 描边 + 内嵌警示图标，并输出 aria-invalid */
  invalid?: boolean;
  /** 尺寸档 —— Figma：sm 72 / md 88 / lg 112（padding 8 12） */
  size?: "sm" | "md" | "lg";
  rows?: number;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  placeholder: undefined,
  disabled: false,
  invalid: false,
  size: "md",
  rows: 4
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

// 位于 FormField 内时自动关联错误文案
const fieldErrorId = inject(FORM_FIELD_ERROR_ID, undefined);

const isInvalid = computed(() => props.invalid || Boolean(fieldErrorId?.value));
const describedBy = computed(() => fieldErrorId?.value);

const classes = computed(() =>
  cn("ui-textarea", `ui-textarea--${props.size}`, {
    "is-invalid": isInvalid.value
  })
);

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLTextAreaElement).value);
}
</script>

<!-- Figma 的 Textarea 框同 Input：canvas 底容器帧，error 态右下角内嵌警示图标 -->
<template>
  <div :class="classes">
    <textarea
      class="ui-textarea__native"
      :value="modelValue"
      :placeholder="placeholder"
      :rows="rows"
      :disabled="disabled"
      :aria-invalid="isInvalid ? 'true' : undefined"
      :aria-describedby="describedBy"
      @input="onInput"
    />
    <CircleAlert
      v-if="isInvalid"
      class="ui-textarea__icon"
      aria-hidden="true"
    />
  </div>
</template>

<style scoped>
/* Figma DS/Forms/Textarea（13:32944）：canvas 底 / border 描边 / radius 8 /
 * 三档高 72/88/112 / padding 8 12；交互态同 TextInput：
 * hover 描边=次文字色，focus=accent 2px，error=#E69A9F + 内嵌图标，禁用 0.45。 */
.ui-textarea {
  position: relative;
  display: flex;
  width: 100%;
  min-height: var(--size-textarea);
  padding: var(--space-2) var(--space-3);
  background: var(--surface-sunken);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-textarea--sm {
  min-height: var(--size-textarea-sm);
}

.ui-textarea--lg {
  min-height: var(--size-textarea-lg);
}

.ui-textarea__native {
  flex: 1;
  min-width: 0;
  padding: 0;
  border: 0;
  outline: none;
  background: transparent;
  resize: vertical;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}

.ui-textarea__native::placeholder {
  color: var(--text-secondary);
}

.ui-textarea:hover:has(.ui-textarea__native:not(:disabled)) {
  border-color: var(--border-hover-strong);
}

/* 聚焦 —— Figma：accent 2px 描边（inset 阴影补 1px 免布局抖动） */
.ui-textarea:focus-within {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 var(--border-1) var(--accent);
}

/* 错误态 —— Figma：#E69A9F 描边 */
.ui-textarea.is-invalid {
  border-color: var(--danger-title);
}

.ui-textarea.is-invalid:focus-within {
  box-shadow: inset 0 0 0 var(--border-1) var(--danger-title);
}

/* 禁用 —— Figma：elevated 底 + 整体 0.45 不透明度 */
.ui-textarea:has(.ui-textarea__native:disabled) {
  background: var(--surface-raised);
  opacity: var(--opacity-disabled-field);
}

.ui-textarea:has(.ui-textarea__native:disabled) .ui-textarea__native {
  cursor: not-allowed;
}

/* 错误图标 —— 右下角内嵌（Figma Error 态） */
.ui-textarea__icon {
  position: absolute;
  right: var(--space-3);
  bottom: var(--space-2);
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--danger-title);
  pointer-events: none;
}
</style>
