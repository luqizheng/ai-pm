<script setup lang="ts">
import Badge from "@/components/ui/badge/Badge.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

/**
 * 统一决策理由输入（R05）：Finding 必填 answer / Proposal·Delivery 选填
 * note/reason——差异全部由调用方以 props 配置，组件内不做领域判断。
 *
 * 纯展示件：文案由调用方翻译后经 props 传入（D8）。
 * 注意（V1.1 §4.1）：Finding 的 answer 是确认与拒绝都要填的决策说明，
 * 不是「拒绝原因」，label 由调用方决定。
 */
interface Props {
  label: string;
  placeholder: string;
  required: boolean;
  requiredBadgeLabel: string;
  optionalBadgeLabel: string;
  /** required 且为空时的提示文案；空串 = 不提示。 */
  requiredHint: string;
  maxLength?: number;
  modelValue: string;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  maxLength: undefined,
  disabled: false
});
defineEmits<{ "update:modelValue": [value: string] }>();

const isEmpty = (): boolean => props.modelValue.trim().length === 0;
</script>

<template>
  <div class="review-reason" :data-required="required">
    <div class="review-reason__head">
      <span class="review-reason__label">{{ label }}</span>
      <Badge :variant="required ? 'warning' : 'neutral'" size="sm" outline>
        {{ required ? requiredBadgeLabel : optionalBadgeLabel }}
      </Badge>
      <span v-if="maxLength" class="review-reason__count">
        {{ modelValue.length }} / {{ maxLength }}
      </span>
    </div>
    <Textarea
      :model-value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :invalid="required && isEmpty()"
      :rows="3"
      @update:model-value="$emit('update:modelValue', String($event))"
    />
    <p v-if="required && isEmpty()" class="review-reason__hint" role="note">
      {{ requiredHint }}
    </p>
  </div>
</template>

<style scoped>
.review-reason__head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.review-reason__label {
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 600;
}

.review-reason__count {
  margin-left: auto;
  color: var(--text-secondary);
  font-size: 12px;
}

.review-reason__hint {
  margin: 6px 0 0;
  color: var(--text-secondary);
  font-size: 12px;
}
</style>
