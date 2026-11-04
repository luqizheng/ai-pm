<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Overlays/CommandPalette" node=13:34028 date=2026-11-04 -->
<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Search } from "lucide-vue-next";

export interface CommandItem {
  id: string;
  label: string;
  group?: string;
  shortcut?: string;
}

interface Props {
  /** 是否打开（v-model:open） */
  open?: boolean;
  /** 命令条目 */
  items: CommandItem[];
  /** 搜索框占位文案 */
  placeholder?: string;
  /** 无结果提示 */
  emptyText?: string;
  /** 无障碍名称 */
  label?: string;
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
  placeholder: undefined,
  emptyText: undefined,
  label: undefined
});

const { t } = useI18n();

// 默认文案走 i18n（调用方显式传入时优先）
const labelText = computed(() => props.label ?? t("shell.command.label"));
const placeholderText = computed(
  () => props.placeholder ?? t("shell.command.placeholder")
);
const emptyTextText = computed(
  () => props.emptyText ?? t("shell.command.emptyText")
);

const emit = defineEmits<{
  "update:open": [value: boolean];
  select: [id: string];
}>();

const query = ref("");
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.items;
  return props.items.filter(
    item =>
      item.label.toLowerCase().includes(q) ||
      (item.group ?? "").toLowerCase().includes(q)
  );
});

watch(filtered, () => {
  activeIndex.value = 0;
});

watch(
  () => props.open,
  async open => {
    if (!open) return;
    query.value = "";
    activeIndex.value = 0;
    await nextTick();
    inputRef.value?.focus();
  },
  { immediate: true }
);

function onInput(event: Event) {
  query.value = (event.target as HTMLInputElement).value;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    activeIndex.value = Math.min(
      activeIndex.value + 1,
      filtered.value.length - 1
    );
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    activeIndex.value = Math.max(activeIndex.value - 1, 0);
  } else if (event.key === "Enter") {
    event.preventDefault();
    const item = filtered.value[activeIndex.value];
    if (item) {
      emit("select", item.id);
      emit("update:open", false);
    }
  } else if (event.key === "Escape") {
    event.preventDefault();
    emit("update:open", false);
  }
}
</script>

<template>
  <div
    v-if="props.open"
    class="ui-command"
    role="dialog"
    :aria-label="labelText"
  >
    <div class="ui-command__panel">
      <div class="ui-command__search">
        <Search class="ui-command__search-icon" aria-hidden="true" />
        <input
          ref="inputRef"
          class="ui-command__input"
          type="text"
          role="combobox"
          aria-expanded="true"
          :aria-label="labelText"
          :placeholder="placeholderText"
          :value="query"
          @input="onInput"
          @keydown="onKeydown"
        />
        <kbd class="ui-command__esc" aria-hidden="true">Esc</kbd>
      </div>
      <ul class="ui-command__list" role="listbox">
        <li
          v-for="(item, index) in filtered"
          :key="item.id"
          class="ui-command__item"
          :class="{ 'is-active': index === activeIndex }"
          role="option"
          :aria-selected="index === activeIndex ? 'true' : 'false'"
          @click="
            emit('select', item.id);
            emit('update:open', false);
          "
        >
          <span class="ui-command__item-label">{{ item.label }}</span>
          <span v-if="item.group" class="ui-command__item-group">{{
            item.group
          }}</span>
          <kbd v-if="item.shortcut" class="ui-command__item-shortcut">{{
            item.shortcut
          }}</kbd>
        </li>
      </ul>
      <p v-if="filtered.length === 0" class="ui-command__empty">
        {{ emptyTextText }}
      </p>
      <p class="ui-command__hint" aria-hidden="true">
        ↑ ↓ Navigate · ↵ Open · Esc Close
      </p>
    </div>
  </div>
</template>

<style scoped>
/* Figma DS/Overlays/CommandPalette（13:34028）：
 * elevated 面板 + radius 8 + 内边 8 + gap 8 / Medium 宽 448；
 * 搜索框 canvas 底 + border 描边（高 36，search 图标 16 + Esc chip）；
 * 菜单项 36px / radius 8 / 主文字；选中项 accent 着色面；
 * 底部键盘指引行（11px 次文字 + 顶部分隔线）。 */
.ui-command {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 15vh;
  /* 遮罩 28% 透明必须做进背景色：写在元素 opacity 上会把整个面板（含文字）一起压透明，
     页面文字会透出来污染面板（Phase 2 §3.2 遮罩意图） */
  background: color-mix(in srgb, var(--surface-ink) 28%, transparent);
}

.ui-command__panel {
  width: calc(100% - var(--space-8));
  max-width: var(--size-overlay-panel);
  max-height: 60vh;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  overflow: hidden;
  padding: var(--space-2);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-lg);
}

/* 搜索框：canvas 底 + border 描边（区别于旧版仅底部分隔线） */
.ui-command__search {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
}

.ui-command__search:focus-within {
  border: var(--border-2) solid var(--accent);
}

.ui-command__search-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

.ui-command__input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: 0;
  background: transparent;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ui-command__input::placeholder {
  color: var(--text-secondary);
}

.ui-command__input:focus {
  outline: none;
}

.ui-command__esc {
  flex: none;
  padding: 1px var(--space-1);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
}

.ui-command__list {
  margin: 0;
  padding: 0;
  list-style: none;
  overflow-y: auto;
}

.ui-command__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-nav-item);
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
  cursor: pointer;
}

.ui-command__item.is-active {
  background: var(--accent-surface);
  color: var(--text-primary);
}

.ui-command__item-label {
  flex: 1;
}

.ui-command__item-group {
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
}

.ui-command__item-shortcut {
  padding: 1px var(--space-1);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
}

.ui-command__empty {
  margin: 0;
  padding: var(--space-6);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

/* 键盘指引行：11px 次文字 + 顶部 1px 分隔（Figma Keyboard guidance） */
.ui-command__hint {
  margin: 0;
  flex: none;
  padding: var(--space-2);
  border-top: var(--border-1) solid var(--border-strong);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}
</style>
