<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowUp } from "lucide-vue-next";
import { useWorkspaceStore } from "../store";

const { t } = useI18n();
const store = useWorkspaceStore();

const draft = ref("");
const inputEl = ref<HTMLInputElement | null>(null);
/** 生命周期硬规则：无真回执不得进入完成态（spec §六） */
const phase = ref<"idle" | "submitting">("idle");

const MOCK_REPLY_DELAY_MS = 800;
let replyTimer: number | undefined;

const hasProject = computed(() => store.projectId !== "");

function submit() {
  if (!hasProject.value || phase.value === "submitting") return;
  const text = draft.value.trim();
  if (!text) return;
  store.pushCommandMessage("user", text);
  draft.value = "";
  phase.value = "submitting";
  replyTimer = window.setTimeout(() => {
    store.pushCommandMessage(
      "assistant",
      t("workspace.commandbar.mockReply", { prompt: text })
    );
    phase.value = "idle";
  }, MOCK_REPLY_DELAY_MS);
}

function cancel() {
  window.clearTimeout(replyTimer);
  phase.value = "idle";
}

function focusInput() {
  inputEl.value?.focus();
}

function onWindowKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === "/") {
    e.preventDefault();
    focusInput();
    return;
  }
  if (e.key === "Escape") {
    store.clearCommandMessages();
  }
}

onMounted(() => window.addEventListener("keydown", onWindowKeydown));
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onWindowKeydown);
  window.clearTimeout(replyTimer);
});

defineExpose({ focusInput });
</script>

<template>
  <div class="ws-commandbar" data-test="command-bar">
    <div
      v-if="store.commandMessages.length"
      class="ws-commandbar-messages"
      role="log"
    >
      <div
        v-for="message in store.commandMessages"
        :key="message.id"
        class="ws-command-msg"
        :class="`is-${message.role}`"
      >
        {{ message.text }}
      </div>
    </div>

    <div
      v-if="phase === 'submitting'"
      class="ws-commandbar-phase"
      data-test="command-phase"
    >
      <span class="ws-commandbar-phase-text">
        {{ t("workspace.commandbar.phase") }}
      </span>
      <button
        type="button"
        class="ws-commandbar-cancel ws-focusable"
        data-test="command-cancel"
        @click="cancel"
      >
        {{ t("workspace.commandbar.cancel") }}
      </button>
    </div>

    <div class="ws-commandbar-row">
      <div class="ws-commandbar-chip">
        <span class="ws-chip-role">
          {{ t("workspace.commandbar.chipRole") }}
        </span>
        <span class="ws-chip-sep" aria-hidden="true">·</span>
        <span class="ws-chip-context">
          {{ t("workspace.commandbar.chipContext") }}
        </span>
      </div>
      <span v-if="!hasProject" class="ws-commandbar-hint">
        {{ t("workspace.commandbar.needProject") }}
      </span>
      <input
        ref="inputEl"
        v-model="draft"
        type="text"
        class="ws-commandbar-input ws-focusable"
        data-test="command-input"
        :disabled="!hasProject"
        :placeholder="t('workspace.commandbar.placeholder')"
        :aria-label="t('workspace.commandbar.send')"
        @keydown.enter="submit"
        @keydown.esc="store.clearCommandMessages()"
      />
      <button
        type="button"
        class="ws-commandbar-tool ws-focusable"
        data-test="command-reference"
        disabled
      >
        {{ t("workspace.commandbar.reference") }}
      </button>
      <button
        type="button"
        class="ws-commandbar-tool ws-focusable"
        data-test="command-at-agent"
        disabled
      >
        {{ t("workspace.commandbar.atAgent") }}
      </button>
      <button
        type="button"
        class="ws-commandbar-send ws-focusable"
        data-test="command-send"
        :disabled="!hasProject || phase === 'submitting'"
        @click="submit"
      >
        {{ t("workspace.commandbar.send") }}
        <ArrowUp :size="14" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.ws-commandbar {
  padding: 10px 16px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ws-commandbar-messages {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 160px;
  overflow-y: auto;
}

.ws-command-msg {
  font-size: var(--ws-font-size-body);
  line-height: 1.5;
  padding: 6px 10px;
  border-radius: var(--ws-radius-card);
  background: var(--ws-surface);
  border: 1px solid var(--ws-border);
}

.ws-command-msg.is-user {
  align-self: flex-end;
  background: var(--ws-elevated);
}

.ws-command-msg.is-assistant {
  align-self: flex-start;
}

.ws-commandbar-phase {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-commandbar-cancel {
  height: 24px;
  padding: 0 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-primary);
  cursor: pointer;
  font-size: var(--ws-font-size-caption);
}

.ws-commandbar-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-commandbar-chip {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--ws-font-size-caption);
  padding: 4px 10px;
  border: 1px solid var(--ws-border);
  border-radius: 999px;
  background: var(--ws-surface);
}

.ws-chip-role {
  color: var(--ws-accent);
  font-weight: 600;
}

/* 紫色克制：chip 角色用 AI 紫，上下文文案保持中性 */
.ws-chip-context {
  color: var(--ws-text-secondary);
}

.ws-chip-sep {
  color: var(--ws-text-secondary);
}

.ws-commandbar-hint {
  flex: none;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-color-warning-text);
}

.ws-commandbar-input {
  flex: 1;
  min-width: 0;
  height: 36px;
  padding: 0 12px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-surface);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
}

.ws-commandbar-input:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.ws-commandbar-tool {
  flex: none;
  height: 32px;
  padding: 0 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.ws-commandbar-tool:disabled {
  cursor: default;
  opacity: 0.5;
}

.ws-commandbar-send {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 32px;
  padding: 0 14px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: var(--ws-accent);
  color: var(--ws-canvas);
  font-size: var(--ws-font-size-body);
  font-weight: 600;
  cursor: pointer;
}

/* 紫色克制：发送是少数允许 AI 紫的主操作（A4） */
.ws-commandbar-send:disabled {
  cursor: default;
  opacity: 0.5;
}
</style>
