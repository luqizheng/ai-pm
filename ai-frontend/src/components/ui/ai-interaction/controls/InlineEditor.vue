<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Pencil } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";

/**
 * Read / edit in place.
 *
 * `editing` is an optional controlled prop: when a parent passes it the parent
 * owns the mode switch, otherwise the editor keeps its own state so it can be
 * dropped into a card without wiring.
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    editing?: boolean;
    multiline?: boolean;
    saving?: boolean;
    disabled?: boolean;
    placeholder?: string;
  }>(),
  {
    editing: undefined,
    multiline: true,
    saving: false,
    disabled: false,
    placeholder: undefined
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  save: [];
  cancel: [];
}>();

const { t } = useI18n();

const isEditing = ref(props.editing ?? false);
const inputRef = ref<HTMLInputElement | HTMLTextAreaElement | null>(null);

const controlled = computed(() => props.editing !== undefined);

const placeholderText = computed(
  () => props.placeholder ?? t("ai.controls.inlineEditor.placeholder")
);

const showPlaceholder = computed(
  () => !isEditing.value && props.modelValue.length === 0
);

watch(
  () => props.editing,
  value => {
    if (value !== undefined) isEditing.value = value;
  }
);

watch(isEditing, value => {
  if (!value) return;
  void nextTick(() => inputRef.value?.focus());
});

if (isEditing.value) {
  void nextTick(() => inputRef.value?.focus());
}

function startEdit() {
  if (props.disabled) return;
  isEditing.value = true;
}

function onInput(event: Event) {
  const target = event.target as HTMLInputElement | HTMLTextAreaElement;
  emit("update:modelValue", target.value);
}

function commit() {
  if (props.disabled || props.saving || !isEditing.value) return;
  emit("save");
  if (!controlled.value) isEditing.value = false;
}

function discard() {
  if (props.saving) return;
  emit("cancel");
  if (!controlled.value) isEditing.value = false;
}

function onKeydown(event: KeyboardEvent) {
  if (!isEditing.value) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    discard();
    return;
  }
  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    commit();
  }
}
</script>

<template>
  <div
    class="ai-inline-editor"
    :class="{ 'is-editing': isEditing, 'is-disabled': disabled }"
    @keydown="onKeydown"
  >
    <template v-if="isEditing">
      <textarea
        v-if="multiline"
        ref="inputRef"
        class="ai-inline-editor__input"
        :value="modelValue"
        :placeholder="placeholderText"
        :disabled="disabled || saving"
        :aria-label="t('ai.controls.inlineEditor.edit')"
        rows="3"
        @input="onInput"
      />
      <input
        v-else
        ref="inputRef"
        class="ai-inline-editor__input"
        type="text"
        :value="modelValue"
        :placeholder="placeholderText"
        :disabled="disabled || saving"
        :aria-label="t('ai.controls.inlineEditor.edit')"
        @input="onInput"
      />

      <div class="ai-inline-editor__actions">
        <Button
          variant="primary"
          size="sm"
          :loading="saving"
          :disabled="disabled"
          @click="commit"
        >
          {{ t("ai.controls.inlineEditor.save") }}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          :disabled="saving || disabled"
          @click="discard"
        >
          {{ t("ai.controls.inlineEditor.cancel") }}
        </Button>
      </div>
    </template>

    <template v-else>
      <p
        class="ai-inline-editor__text"
        :class="{ 'is-placeholder': showPlaceholder }"
      >
        {{ showPlaceholder ? placeholderText : modelValue }}
      </p>
      <Button
        class="ai-inline-editor__edit"
        variant="ghost"
        size="sm"
        :disabled="disabled"
        @click="startEdit"
      >
        <Pencil :size="14" aria-hidden="true" />
        {{ t("ai.controls.inlineEditor.edit") }}
      </Button>
    </template>
  </div>
</template>

<style scoped>
.ai-inline-editor {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-inline-editor__text {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.ai-inline-editor__text.is-placeholder {
  color: var(--text-secondary);
}

.ai-inline-editor__edit {
  align-self: flex-start;
}

.ai-inline-editor__input {
  width: 100%;
  min-height: var(--size-ai-option-min-height);
  padding: var(--space-2) var(--space-3);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  background: var(--surface-sunken);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  resize: vertical;
}

.ai-inline-editor__input:hover:not(:disabled) {
  border-color: var(--border-hover-strong);
}

.ai-inline-editor__input:focus-visible {
  border-color: var(--accent);
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.ai-inline-editor__input:disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-field);
}

.ai-inline-editor__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

@media (prefers-reduced-motion: reduce) {
  .ai-inline-editor__input {
    transition: none;
  }
}
</style>
