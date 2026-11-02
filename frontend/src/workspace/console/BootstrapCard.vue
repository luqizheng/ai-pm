<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Copy, Check } from "lucide-vue-next";

/**
 * 环境接入卡片（T-091 / T-080 遗留前端 / Codex Bootstrap V1.3 图1）。
 *
 * - 文案 = 标准安装指令（V1.3 §1.1 七步版）；安装地址按部署实际替换。
 * - 纯展示 + 复制：**不**承载任何凭据（Key 由用户当面交给 agent——指令里
 *   只有 URL，这是评审反复确认的边界）。
 * - 平台地址从运行时取：同源部署下 `window.location.origin` 即部署地址。
 */

const { t } = useI18n();
const copied = ref(false);

const baseUrl = computed(() => {
  // 同源部署：安装说明与 MCP 都在 pm-api 同一 authority 下。
  // 开发态（vite 5173 代理）退回显式端口提示——文案里仍给可替换的完整地址。
  const origin = window.location.origin;
  return origin.startsWith("http") ? origin : "https://pm-ai.example.com";
});

const installUrl = computed(() => `${baseUrl.value}/install/codex`);

const instruction = computed(() =>
  [
    t("workspace.bootstrap.instruction.title"),
    "",
    t("workspace.bootstrap.instruction.body"),
    "",
    t("workspace.bootstrap.instruction.urlLabel"),
    installUrl.value
  ].join("\n")
);

async function copy() {
  try {
    await navigator.clipboard.writeText(instruction.value);
    copied.value = true;
    window.setTimeout(() => (copied.value = false), 2000);
  } catch {
    // 剪贴板权限被拒时不静默成功：提示用户手动复制。
    window.alert(t("workspace.bootstrap.copyFailed"));
  }
}
</script>

<template>
  <section class="bootstrap-card" data-test="bootstrap-card">
    <div class="card-head">
      <h3>{{ t("workspace.bootstrap.title") }}</h3>
      <span class="pill">{{ t("workspace.bootstrap.pill") }}</span>
    </div>
    <p class="card-desc">{{ t("workspace.bootstrap.desc") }}</p>
    <pre class="instruction">{{ instruction }}</pre>
    <div class="card-foot">
      <button type="button" class="copy-btn" data-test="bootstrap-copy" @click="copy">
        <Check v-if="copied" class="icon" />
        <Copy v-else class="icon" />
        {{ copied ? t("workspace.bootstrap.copied") : t("workspace.bootstrap.copy") }}
      </button>
      <span class="hint">{{ t("workspace.bootstrap.replaceHint") }}</span>
    </div>
  </section>
</template>

<style scoped>
.bootstrap-card {
  border: 1px solid var(--ws-border);
  border-radius: 10px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.card-head { display: flex; align-items: center; gap: 10px; }
.card-head h3 { margin: 0; font-size: 14px; font-weight: 600; }
.pill {
  font-size: 11px; padding: 1px 8px; border-radius: 999px;
  border: 1px solid var(--ws-color-success-text); color: var(--ws-color-success-text);
}
.card-desc { margin: 0; font-size: 12px; opacity: 0.75; }
.instruction {
  margin: 0; padding: 10px 12px; border-radius: 6px;
  background: var(--ws-hover-veil);
  font-size: 12px; line-height: 1.5; white-space: pre-wrap;
  word-break: break-all;
}
.card-foot { display: flex; align-items: center; gap: 10px; }
.copy-btn {
  display: inline-flex; align-items: center; gap: 6px;
  border: 1px solid var(--ws-accent); color: var(--ws-accent);
  border-radius: 6px; padding: 5px 12px; font-size: 12px; cursor: pointer;
  background: transparent;
}
.copy-btn .icon { width: 13px; height: 13px; }
.hint { font-size: 11px; opacity: 0.55; }
</style>
