<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import MarkdownEditor from "@/components/ui/markdown-editor/MarkdownEditor.vue";
import { resolveAttachmentUrl, uploadAttachment } from "@/api/base/attachments";

/**
 * Markdown rich-text field for long AI answers / artifact bodies.
 * `disabled` is folded into the editor's `readonly` (Vditor has no disabled mode).
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    minHeight?: number;
    readonly?: boolean;
    disabled?: boolean;
    label?: string;
  }>(),
  {
    minHeight: undefined,
    readonly: false,
    disabled: false,
    label: undefined
  }
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const { t } = useI18n();

const isReadonly = computed(() => props.readonly || props.disabled);
</script>

<template>
  <div class="ai-rich-text" :class="{ 'is-disabled': disabled }">
    <p v-if="label" class="ai-rich-text__label">{{ label }}</p>
    <div class="ai-rich-text__editor" :aria-disabled="disabled || undefined">
      <MarkdownEditor
        :model-value="modelValue"
        :min-height="minHeight"
        :readonly="isReadonly"
        :placeholder="t('ai.controls.richText.placeholder')"
        :uploader="uploadAttachment"
        :resolve-url="resolveAttachmentUrl"
        @update:model-value="emit('update:modelValue', $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.ai-rich-text {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-rich-text__label {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
}

.ai-rich-text__editor {
  min-width: 0;
}

.ai-rich-text.is-disabled .ai-rich-text__editor {
  pointer-events: none;
  opacity: var(--opacity-disabled-field);
}
</style>
