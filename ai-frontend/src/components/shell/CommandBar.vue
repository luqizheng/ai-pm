<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import {
  ArrowUp,
  ChevronDown,
  ChevronUp,
  Paperclip,
  Sparkles
} from "lucide-vue-next";

import IconTooltip from "./IconTooltip.vue";
import { useShellStore } from "@/stores/shell";

/** AI 建议芯片（交互选项，不是数据）：explain / setup */
const SUGGESTION_KEYS = ["explain", "setup"] as const;

/** 指令栏提问的落点：项目 AI 会话页（复用其发送链与时间线） */
const ASSISTANT_PATH = "/project/assistant";

/**
 * 底部 AI 指令栏（Application Shell §2E，72–120px）。
 *
 * 两态：
 *   - 收起 72px：单行提示输入（占位符带当前上下文）+ ⌘ J 芯片
 *   - 展开 120px：作用域行 + 输入行 + 建议芯片 + 键盘提示
 *
 * **AI 建议与人工确认的动作必须视觉可辨**（Shell V1.1 §2E）——
 * 建议芯片走 accent 色（AI 语义），人工确认提示用次文字色。
 *
 * 作用域串随当前项目变化（切换中不显示旧项目数据，Figma「07」）。
 *
 * 提交（发送键 / Enter）= 入队到 shell store 并进入会话页，由会话页真正
 * 发送（混合工作区 B-1：指令栏是会话的收起态入口）。建议芯片只把文案
 * 填进输入框，不自动发送——发送前由人确认（「建议未执行」提示）。
 */

const { t } = useI18n();
const router = useRouter();
const shell = useShellStore();

const prompt = ref("");

const expanded = computed(() => shell.commandbarExpanded);

/**
 * 助手作用域：当前项目 / 仓库 / 分支。
 *
 * 切换中不回落旧项目名（Figma「07」No stale project data is displayed），
 * 而是用「正在切换项目…」占位，等新上下文就绪后再显示新项目。
 */
const commandScope = computed(() => {
  const project = shell.currentProject;
  const name = project ? project.name : t("shell.app.project.switching");
  const branch = shell.currentRepository?.defaultBranch;
  return branch ? `${name} / Repository / ${branch}` : name;
});

const placeholder = computed(() =>
  expanded.value
    ? t("shell.app.commandbar.placeholder")
    : t("shell.app.commandbar.collapsedPlaceholder", {
        context: commandScope.value
      })
);

const suggestionKeys = computed(() =>
  SUGGESTION_KEYS.map(key => `shell.app.commandbar.suggestion.${key}`)
);

const fieldRef = ref<HTMLInputElement | null>(null);

function submit() {
  const text = prompt.value.trim();
  if (!text) return;
  shell.queueAssistantPrompt(text);
  prompt.value = "";
  if (router.currentRoute.value.path !== ASSISTANT_PATH) {
    void router.push(ASSISTANT_PATH);
  }
}

/** 输入法组字期间的 Enter 只确认候选词，不提交 */
function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  submit();
}

function applySuggestion(key: string) {
  prompt.value = t(key);
  fieldRef.value?.focus();
}
</script>

<template>
  <section
    class="workspace-commandbar shell-commandbar"
    :class="{ 'is-expanded': expanded }"
    :aria-label="t('shell.app.commandbar.title')"
  >
    <!-- 展开态：作用域与控制行 -->
    <div v-if="expanded" class="shell-commandbar__scope">
      <Sparkles :size="14" aria-hidden="true" />
      <span class="shell-commandbar__scope-title">{{
        t("shell.app.commandbar.title")
      }}</span>
      <span class="shell-commandbar__scope-context">{{ commandScope }}</span>
      <!-- 人工确认提示：与 AI 建议视觉可辨（次文字色，非 accent） -->
      <span class="shell-commandbar__confirm">{{
        t("shell.app.commandbar.reviewFirst")
      }}</span>
      <IconTooltip :content="t('shell.app.commandbar.collapse')" side="top">
        <button
          type="button"
          class="shell-commandbar__icon-button"
          :aria-label="t('shell.app.commandbar.collapse')"
          @click="shell.setCommandbarExpanded(false)"
        >
          <ChevronDown :size="16" aria-hidden="true" />
        </button>
      </IconTooltip>
    </div>

    <!-- 输入行 -->
    <div class="shell-commandbar__input">
      <Sparkles :size="16" aria-hidden="true" />
      <input
        ref="fieldRef"
        v-model="prompt"
        class="shell-commandbar__field"
        type="text"
        :placeholder="placeholder"
        :aria-label="t('shell.app.commandbar.title')"
        @keydown="onKeydown"
      />
      <kbd class="shell-commandbar__kbd">⌘ J</kbd>
      <IconTooltip :content="t('shell.app.commandbar.attach')" side="top">
        <button
          type="button"
          class="shell-commandbar__icon-button"
          :aria-label="t('shell.app.commandbar.attach')"
        >
          <Paperclip :size="14" aria-hidden="true" />
        </button>
      </IconTooltip>
      <button
        type="button"
        class="shell-commandbar__send"
        :aria-label="t('shell.app.commandbar.send')"
        :disabled="!prompt.trim()"
        @click="submit"
      >
        <ArrowUp :size="14" aria-hidden="true" />
      </button>
      <!-- 收起态：在输入行内提供展开入口（Figma 收起帧：chevron-up 位于发送键之后） -->
      <IconTooltip
        v-if="!expanded"
        :content="t('shell.app.commandbar.expand')"
        side="top"
      >
        <button
          type="button"
          class="shell-commandbar__icon-button"
          :aria-label="t('shell.app.commandbar.expand')"
          @click="shell.setCommandbarExpanded(true)"
        >
          <ChevronUp :size="16" aria-hidden="true" />
        </button>
      </IconTooltip>
    </div>

    <!-- 展开态：建议与键盘提示 -->
    <div v-if="expanded" class="shell-commandbar__hints">
      <span class="shell-commandbar__suggestions-label">{{
        t("shell.app.commandbar.suggestions")
      }}</span>
      <button
        v-for="key in suggestionKeys"
        :key="key"
        type="button"
        class="shell-commandbar__suggestion"
        @click="applySuggestion(key)"
      >
        {{ t(key) }}
      </button>
      <span class="shell-commandbar__not-run">{{
        t("shell.app.commandbar.notRun")
      }}</span>
      <span class="shell-commandbar__hint">{{
        t("shell.app.commandbar.hint")
      }}</span>
    </div>
  </section>
</template>

<style scoped>
.shell-commandbar {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4) var(--space-6);
  min-height: var(--ws-commandbar-height-min);
}

/* 展开态 120px */
.shell-commandbar.is-expanded {
  min-height: var(--ws-commandbar-height-max);
}

/* 作用域行 */
.shell-commandbar__scope {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--space-5);
  color: var(--ws-text-primary);
}

.shell-commandbar__scope-title {
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
}

.shell-commandbar__scope-context {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

/* 人工确认提示——次文字色，与 accent 色的 AI 建议区分 */
.shell-commandbar__confirm {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

/* 输入行 */
.shell-commandbar__input {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: var(--size-control-lg);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.shell-commandbar__field {
  flex: 1;
  min-width: 0;
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  background: transparent;
  border: none;
  outline: none;
}

.shell-commandbar__field::placeholder {
  color: var(--ws-text-secondary);
}

.shell-commandbar__kbd {
  padding: 1px var(--space-1);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.shell-commandbar__icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-commandbar__icon-button:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.shell-commandbar__icon-button:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.shell-commandbar__send {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  color: var(--ws-text-primary);
  cursor: pointer;
  background: var(--ws-elevated);
  border: none;
  border-radius: var(--radius-xs);
}

.shell-commandbar__send:disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}

.shell-commandbar__send:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 建议与提示 */
.shell-commandbar__hints {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--space-5);
}

/* AI 建议——accent 色（与人工确认的次文字色区分） */
.shell-commandbar__suggestions-label {
  color: var(--ws-accent);
  font-size: var(--font-size-micro);
}

.shell-commandbar__suggestion {
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  cursor: pointer;
  background: var(--ws-surface);
  border: none;
  border-radius: var(--radius-xs);
}

.shell-commandbar__suggestion:hover {
  color: var(--ws-text-primary);
}

.shell-commandbar__suggestion:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.shell-commandbar__not-run {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.shell-commandbar__hint {
  margin-left: auto;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}
</style>
