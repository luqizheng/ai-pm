<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/Combobox" node=new date=2026-11-12 -->
<script setup lang="ts">
import { computed } from "vue";
import {
  ComboboxAnchor,
  ComboboxCancel,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxGroup,
  ComboboxInput,
  ComboboxItem,
  ComboboxItemIndicator,
  ComboboxPortal,
  ComboboxRoot,
  ComboboxTrigger,
  ComboboxViewport
} from "reka-ui";
import { cn } from "@/lib/cn";

export interface ComboboxOption {
  /** 与 modelValue 匹配的值（通常是 model id）。 */
  value: string;
  /** 下拉与已选中态展示的主文案。 */
  label: string;
  /** 右侧描述小字（可选，如 "低延迟"）。 */
  hint?: string;
  /** 分组标题（可选；同 group 的连续项会被同一个 ComboboxGroup 包住）。 */
  group?: string;
  /** 骨架态：显示灰色占位，不可交互。 */
  skeleton?: boolean;
}

interface Props {
  /** 当前选中值（受控）。 */
  modelValue?: string;
  /** 下拉选项列表；传 skeleton=true 的项会渲染骨架占位。 */
  options?: ComboboxOption[];
  /** 未选中 + 空输入框时的占位文案。 */
  placeholder?: string;
  /** ComboboxInput 搜索框的占位文案（通常和 placeholder 相同或更具体）。 */
  inputPlaceholder?: string;
  /** 禁用整个控件。 */
  disabled?: boolean;
  /** 错误态：输出 aria-invalid。 */
  invalid?: boolean;
  /** 可清空：选中后可点 × 清空。 */
  clearable?: boolean;
  /** 触发器尺寸 — 同 Select 口径。 */
  size?: "sm" | "md";
  /** 是否允许自由输入任意值（始终 true；保留 prop 供语义描述）。 */
  freeInput?: boolean;
  /** 透传到内部 <input> 的属性（data-test / id / autocomplete 等）。 */
  inputAttrs?: Record<string, unknown>;
  /** 根元素也会接收 data-test（透传给 ComboboxRoot），让外部选择器 `[data-test='x'] input` 也能命中内部原生 input。 */
  dataTest?: string;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  options: () => [],
  placeholder: "",
  inputPlaceholder: "",
  disabled: false,
  invalid: false,
  clearable: true,
  size: "md",
  freeInput: true,
  inputAttrs: () => ({}),
  dataTest: undefined
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const groupedOptions = computed<{ group?: string; items: ComboboxOption[] }[]>(
  () => {
    const groups = new Map<string | undefined, ComboboxOption[]>();
    for (const opt of props.options) {
      const key = opt.group;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(opt);
    }
    return Array.from(groups.entries()).map(([group, items]) => ({
      group,
      items
    }));
  }
);

function onInput(value: string) {
  // 自由输入场景：用户键入的任意值直接当 modelValue
  emit("update:modelValue", value);
}

function onSelect(value: string) {
  emit("update:modelValue", value);
}
</script>

<template>
  <ComboboxRoot
    :model-value="modelValue"
    :disabled="disabled"
    :data-test="dataTest"
    @update:model-value="onSelect"
  >
    <!-- ComboboxAnchor 是 popper 定位锚点；不显式提供时 reka-ui 回退到 ComboboxRoot
         （页面级宽容器），导致下拉浮层跑到整个 page 的正下方 -->
    <ComboboxAnchor as-child>
      <ComboboxTrigger
        class="ui-combobox__trigger"
        :class="cn(`ui-combobox__trigger--${size}`, { 'is-invalid': invalid })"
      >
        <ComboboxInput
          class="ui-combobox__input"
          :value="modelValue"
          :placeholder="inputPlaceholder || placeholder"
          :disabled="disabled"
          v-bind="inputAttrs" 
         
          @update:model-value="onInput"
        />
        <span v-if="clearable && modelValue" class="ui-combobox__clear">
          <ComboboxCancel aria-label="Clear">
            <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
              <path
                d="M4 4l8 8M12 4l-8 8"
                stroke="currentcolor"
                stroke-width="1.5"
                stroke-linecap="round"
              />
            </svg>
          </ComboboxCancel>
        </span>
        <span class="ui-combobox__chevron" aria-hidden="true">
          <svg viewBox="0 0 16 16" focusable="false" aria-hidden="true">
            <path
              d="M4 6.5 8 10.5l4-4"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </span>
      </ComboboxTrigger>
    </ComboboxAnchor>

    <ComboboxPortal>
      <!-- position="popper" 必须显式声明：reka-ui 只有 popper 模式才挂到
           ComboboxAnchor 锚点上，否则浮层仅 teleport 到 body 末尾、无定位 -->
      <ComboboxContent
        class="ui-combobox__content"
        position="popper"
        :side-offset="4"
        align="start"
      >
        <ComboboxViewport class="ui-combobox__viewport">
          <!-- 空态（无选项且非骨架） -->
          <ComboboxEmpty
            v-if="options.length === 0 && !options.some(o => o.skeleton)"
            class="ui-combobox__empty"
          >
            {{ placeholder }}
          </ComboboxEmpty>

          <template v-for="g in groupedOptions" :key="g.group ?? '__root__'">
            <ComboboxGroup v-if="g.group" class="ui-combobox__group">
              <div class="ui-combobox__group-label">{{ g.group }}</div>
              <template v-for="opt in g.items" :key="opt.value">
                <div v-if="opt.skeleton" class="ui-combobox__skeleton" />
                <ComboboxItem
                  v-else
                  :value="opt.value"
                  class="ui-combobox__item"
                >
                  <ComboboxItemIndicator class="ui-combobox__check">
                    <svg
                      viewBox="0 0 16 16"
                      width="12"
                      height="12"
                      aria-hidden="true"
                    >
                      <path
                        d="M3.5 8.5l3 3 6-6.5"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.6"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                      />
                    </svg>
                  </ComboboxItemIndicator>
                  <div class="ui-combobox__item-text">
                    <span class="ui-combobox__item-label">{{ opt.label }}</span>
                    <span v-if="opt.hint" class="ui-combobox__item-hint">{{
                      opt.hint
                    }}</span>
                  </div>
                </ComboboxItem>
              </template>
            </ComboboxGroup>
            <template v-else>
              <template v-for="opt in g.items" :key="opt.value">
                <div v-if="opt.skeleton" class="ui-combobox__skeleton" />
                <ComboboxItem
                  v-else
                  :value="opt.value"
                  class="ui-combobox__item"
                >
                  <ComboboxItemIndicator class="ui-combobox__check">
                    <svg
                      viewBox="0 0 16 16"
                      width="12"
                      height="12"
                      aria-hidden="true"
                    >
                      <path
                        d="M3.5 8.5l3 3 6-6.5"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.6"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                      />
                    </svg>
                  </ComboboxItemIndicator>
                  <div class="ui-combobox__item-text">
                    <span class="ui-combobox__item-label">{{ opt.label }}</span>
                    <span v-if="opt.hint" class="ui-combobox__item-hint">{{
                      opt.hint
                    }}</span>
                  </div>
                </ComboboxItem>
              </template>
            </template>
          </template>
        </ComboboxViewport>
      </ComboboxContent>
    </ComboboxPortal>
  </ComboboxRoot>
</template>

<style>
/* 触发器同 Select 口径（白底 / border-default / radius-sm / 36px） */
.ui-combobox__trigger {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  color: var(--text-primary);
  background: var(--surface-sunken);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}
.ui-combobox__trigger--sm {
  height: var(--size-control-sm);
}
.ui-combobox__trigger--md {
  height: var(--size-control-md);
}

.ui-combobox__trigger:hover:not([data-disabled]) {
  border-color: var(--border-hover-strong);
}
.ui-combobox__trigger:focus-within {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 var(--border-1) var(--accent);
  outline: none;
}
.ui-combobox__trigger.is-invalid {
  border-color: var(--danger-title);
}
.ui-combobox__trigger.is-invalid:focus-within {
  box-shadow: inset 0 0 0 var(--border-1) var(--danger-title);
}
.ui-combobox__trigger[data-disabled] {
  background: var(--surface-raised);
  opacity: var(--opacity-disabled-field);
  cursor: not-allowed;
}

/* 内嵌 input（reka-ui ComboboxInput 实际上是原生 <input>） */
.ui-combobox__input {
  flex: 1 1 auto;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: inherit;
  font: inherit;
  caret-color: var(--accent);
}
.ui-combobox__input::placeholder {
  color: var(--text-secondary);
}
.ui-combobox__input:disabled {
  cursor: not-allowed;
}

.ui-combobox__clear {
  display: inline-flex;
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-secondary);
  border-radius: var(--radius-xs);
}
.ui-combobox__clear:hover {
  color: var(--text-primary);
  background: var(--surface-hover-veil);
}
.ui-combobox__chevron {
  display: inline-flex;
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
}

/* 浮层容器 — 同 Select 口径 */
.ui-combobox__content {
  width: var(--reka-combobox-trigger-width);
  min-width: var(--size-menu-min-width, 220px);
  padding: var(--space-1);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  animation: ui-combobox-enter var(--duration-fast) var(--ease-decelerate);
  box-shadow: var(--shadow-md);
  max-height: 280px;
  overflow-y: auto;
}

/* 空态 */
.ui-combobox__empty {
  padding: var(--space-3);
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  text-align: center;
}

/* 分组 */
.ui-combobox__group-label {
  padding: var(--space-1) var(--space-2);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
  letter-spacing: var(--tracking-body);
}

/* 选项行 — 同 Select 口径 */
.ui-combobox__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-md);
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  cursor: pointer;
  user-select: none;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}
.ui-combobox__item[data-highlighted] {
  background: var(--accent-surface);
}
.ui-combobox__item[data-disabled] {
  opacity: 0.35;
  cursor: not-allowed;
}
.ui-combobox__item.is-selected {
  background: var(--accent-surface);
}
.ui-combobox__check {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--accent);
  opacity: 0;
}
.ui-combobox__item.is-selected .ui-combobox__check {
  opacity: 1;
}

.ui-combobox__item-text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.ui-combobox__item-label {
  white-space: nowrap;
  text-overflow: ellipsis;
  overflow: hidden;
}
.ui-combobox__item-hint {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
  line-height: 1.2;
}

/* 骨架占位 */
.ui-combobox__skeleton {
  height: var(--size-control-md);
  margin: 2px var(--space-2);
  border-radius: var(--radius-sm);
  background: linear-gradient(
    90deg,
    var(--surface-hover-veil, rgba(255, 255, 255, 0.04)) 0%,
    var(--surface-raised) 50%,
    var(--surface-hover-veil) 100%
  );
  background-size: 200% 100%;
  animation: ui-combobox-shimmer 1.4s ease-in-out infinite;
}

@keyframes ui-combobox-enter {
  from {
    opacity: 0;
    transform: scale(0.96);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
@keyframes ui-combobox-shimmer {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ui-combobox__content {
    animation: none;
  }
  .ui-combobox__skeleton {
    animation: none;
  }
}
</style>
