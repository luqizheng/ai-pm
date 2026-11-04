<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/SearchInput + Global/Search/Input" node=13:32905;33:87811 date=2026-11-04 -->
<script setup lang="ts">
import { LoaderCircle, Search } from "lucide-vue-next";

/**
 * 全局搜索输入框（DS/Forms/SearchInput 13:32905 + Global/Search/Input 四态板 33:87811）。
 *
 * 四态：Default（canvas 底）/ Hover（elevated 底）/ Focus（2px accent 描边）/
 * Disabled（整体 0.45 不透明度）；Loading 时内嵌 spinner 替代快捷键芯片。
 * 搜索中恒有进度反馈（Figma：Searching always adds progress feedback）。
 */
interface Props {
  placeholder?: string;
  loading?: boolean;
  disabled?: boolean;
  /** 输入框下方 11px 说明（NoResults 态提示语） */
  helperText?: string;
  /** 右侧 Esc 快捷键芯片（面板顶部的输入框展示） */
  escapeChip?: boolean;
}

withDefaults(defineProps<Props>(), {
  placeholder: undefined,
  loading: false,
  disabled: false,
  helperText: undefined,
  escapeChip: false
});

const model = defineModel<string>({ default: "" });
</script>

<template>
  <div
    class="global-search-field"
    :class="{ 'is-disabled': disabled }"
    :aria-disabled="disabled"
  >
    <div class="global-search-field__control">
      <Search :size="16" class="global-search-field__icon" aria-hidden="true" />
      <input
        v-model="model"
        type="text"
        class="global-search-field__input"
        role="combobox"
        aria-expanded="true"
        aria-autocomplete="list"
        :placeholder="placeholder"
        :disabled="disabled"
      />
      <LoaderCircle
        v-if="loading"
        :size="16"
        class="global-search-field__spinner"
        aria-hidden="true"
      />
      <kbd v-else-if="escapeChip" class="global-search-field__kbd">Esc</kbd>
    </div>
    <p v-if="helperText" class="global-search-field__helper">
      {{ helperText }}
    </p>
  </div>
</template>

<style scoped>
.global-search-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

/* Disabled：输入类字段整体 0.45（Figma 组件库口径） */
.global-search-field.is-disabled {
  opacity: var(--opacity-disabled-field);
}

.global-search-field__control {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-row-height);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  /* Focus：2px accent 描边（外层 1px 变色 + 内嵌 1px 阴影补足，不抖布局） */
  transition: border-color var(--duration-instant) var(--ease-standard);
}

.global-search-field__control:hover {
  background: var(--ws-elevated);
}

.global-search-field__control:focus-within {
  border-color: var(--ws-accent);
  box-shadow: inset 0 0 0 1px var(--ws-accent);
}

.global-search-field__input {
  flex: 1;
  min-width: 0;
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  background: transparent;
  border: none;
  outline: none;
}

.global-search-field__input::placeholder {
  color: var(--ws-text-secondary);
}

.global-search-field__icon {
  flex: none;
}

.global-search-field__spinner {
  flex: none;
  color: var(--ws-accent);
  animation: global-search-field-spin var(--duration-spinner) linear infinite;
}

@keyframes global-search-field-spin {
  to {
    transform: rotate(360deg);
  }
}

.global-search-field__kbd {
  flex: none;
  padding: 1px var(--space-1);
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.global-search-field__helper {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

@media (prefers-reduced-motion: reduce) {
  .global-search-field__spinner {
    animation: none;
  }
}
</style>
