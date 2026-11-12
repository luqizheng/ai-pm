<script setup lang="ts">
/**
 * VizFeedbackArea —— 用户反馈与重新生成（V0-8.13）。
 *
 * 依据 18 号 UX 规格 §4.1 E：用户用自然语言告诉 AI-PM 如何调整图形
 * （「只显示已确认的需求」「突出显示阻塞任务」「把间接关系单独显示」…）。
 *
 * 纪律：
 * - 反馈**不直接修改业务对象**——它触发新一轮 AI 生成与 VizSpec（走 V0-5 链路）；
 * - 反馈**不覆盖已发布产物**——新版本另起（ADR-VIZ-006）；
 * - 反馈文本是**用户输入**，不是模型输出；不参与任何 HTML 渲染（纯文本插值）。
 */
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import Button from "@/components/ui/button/Button.vue";

interface Props {
  /** 提交中（等待新一轮生成）。 */
  submitting?: boolean;
  /** 常用示例（引导用户）。 */
  suggestions?: string[];
}

const props = withDefaults(defineProps<Props>(), {
  submitting: false,
  suggestions: () => []
});

const { t } = useI18n();

const emit = defineEmits<{ feedback: [text: string] }>();

const text = ref("");

function submit(): void {
  const value = text.value.trim();
  if (!value || props.submitting) return;
  emit("feedback", value);
  text.value = "";
}

function useSuggestion(suggestion: string): void {
  text.value = suggestion;
}
</script>

<template>
  <section class="viz-feedback" :data-testid="'viz-feedback-area'">
    <label class="viz-feedback__label" for="viz-feedback-input">
      {{ t("viz.feedback.label") }}
    </label>
    <textarea
      id="viz-feedback-input"
      v-model="text"
      class="viz-feedback__input"
      rows="2"
      :placeholder="t('viz.feedback.placeholder')"
      :data-testid="'viz-feedback-input'"
    />
    <div class="viz-feedback__actions">
      <Button
        size="sm"
        variant="primary"
        :disabled="!text.trim() || submitting"
        :data-testid="'viz-feedback-submit'"
        @click="submit"
      >
        {{
          submitting ? t("viz.feedback.submitting") : t("viz.feedback.submit")
        }}
      </Button>
    </div>
    <ul v-if="props.suggestions.length" class="viz-feedback__suggestions">
      <li v-for="(item, i) in props.suggestions" :key="i">
        <button
          type="button"
          class="viz-feedback__suggestion"
          @click="useSuggestion(item)"
        >
          {{ item }}
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.viz-feedback {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.viz-feedback__label {
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.viz-feedback__input {
  width: 100%;
  padding: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  resize: vertical;
}

.viz-feedback__actions {
  display: flex;
  justify-content: flex-end;
}

.viz-feedback__suggestions {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.viz-feedback__suggestion {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  background: transparent;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  padding: 0 var(--space-2);
  height: var(--size-control-md);
  cursor: pointer;
}
</style>
