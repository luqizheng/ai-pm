<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/SearchInput" node=13:32905 date=2026-11-04 -->
<script setup lang="ts">
import { Search, LoaderCircle } from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * SearchInput —— Figma DS/Forms/SearchInput（13:32905）。
 * 输入行（search 图标 16 + 文本 + 快捷键 chip）+ Empty 态帮助行；
 * 行高 28/36/44；Focus 描边 2px accent；Disabled 整组 0.45。
 */

export type SearchInputSize = "sm" | "md" | "lg";

interface Props {
  modelValue?: string;
  placeholder?: string;
  /** 右端快捷键提示（mono chip）；空串隐藏 */
  shortcut?: string;
  /** Empty 态帮助文案 */
  emptyText?: string;
  loading?: boolean;
  disabled?: boolean;
  size?: SearchInputSize;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  placeholder: "Search items…",
  shortcut: "⌘ F",
  emptyText: "",
  loading: false,
  disabled: false,
  size: "md"
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}

const hasEmptyText = () => props.emptyText.length > 0;
</script>

<template>
  <div
    :class="
      cn('ui-search', `ui-search--${size}`, {
        'is-disabled': disabled,
        'is-empty': hasEmptyText()
      })
    "
  >
    <div class="ui-search__field">
      <Search class="ui-search__icon" aria-hidden="true" />
      <input
        class="ui-search__input"
        type="search"
        :value="modelValue"
        :placeholder="placeholder"
        :disabled="disabled"
        @input="onInput"
      />
      <LoaderCircle
        v-if="loading"
        class="ui-search__spinner"
        aria-hidden="true"
      />
      <kbd v-else-if="shortcut" class="ui-search__shortcut">{{ shortcut }}</kbd>
    </div>
    <p v-if="hasEmptyText()" class="ui-search__empty">{{ emptyText }}</p>
  </div>
</template>

<style scoped>
.ui-search {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
}

.ui-search.is-disabled {
  opacity: var(--opacity-disabled-field);
}

.ui-search__field {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  background-color: var(--ws-canvas);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    background-color var(--duration-fast) var(--ease-decelerate);
}

.ui-search--sm .ui-search__field {
  height: var(--size-control-sm);
}

.ui-search--md .ui-search__field {
  height: var(--size-control-md);
}

.ui-search--lg .ui-search__field {
  height: var(--size-control-lg);
}

.ui-search__field:hover:not(.is-disabled) {
  background-color: var(--surface-raised);
}

.ui-search__field:focus-within:not(.is-disabled) {
  border: var(--border-2) solid var(--accent);
  background-color: var(--ws-canvas);
}

.ui-search__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

.ui-search__input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ui-search__input::placeholder {
  color: var(--text-secondary);
}

.ui-search__input:focus {
  outline: none;
}

.ui-search__shortcut {
  flex: none;
  padding: 1px var(--space-1);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
}

.ui-search__spinner {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
  animation: ui-search-spin var(--duration-spinner) linear infinite;
}

@keyframes ui-search-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ui-search__spinner {
    animation: none;
  }
}

.ui-search__empty {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}
</style>
