<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { KeyRound, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import SettingsAppFrame from "@/views/settings/SettingsAppFrame.vue";
import {
  importSshKey,
  listSshKeys,
  renameSshKey,
  revokeSshKey as revokeSshKeyApi
} from "@/api/repositories";
import type { SshKey } from "@/api/types";
import { useProjectSettingsFrame } from "./shared";

/**
 * SSH 私钥独立设置页（2026-11-14 自仓库设置页移出，与【仓库】同级）。
 *
 * 列表 / 粘贴导入 / 改名 / 删除（两段式）；仓库用哪把在仓库设置页的
 * 绑定下拉里选（详情面板 bind-key-select），本页不再管仓库。
 * `label` 即名称：导入同名 = 替换该把（UNIQUE (project_id, label)）。
 */
const { t } = useI18n();
const settingsFrame = useProjectSettingsFrame();
const { projectId } = settingsFrame;

const keys = ref<SshKey[]>([]);
const loading = ref(false);
const busy = ref("");
const errorMessage = ref("");

/* ── 导入表单（粘贴通道；同名替换，第一把自动成为默认） ── */
const keyName = ref("");
const keyInput = ref("");

/* ── 改名（行内编辑；409 label_taken 就地提示） ── */
const renamingId = ref("");
const renameInput = ref("");

/* ── 两段式删除（第一击武装、第二击执行；409 key_in_use 提示先解绑） ── */
const confirmingDeleteId = ref("");
const copiedKeyId = ref("");

const activeCount = computed(
  () => keys.value.filter(key => key.status === "active").length
);

/** 后端错误 → 文案：已知稳定码本地化，未知透出 message（@digua/http 只透传 message）。 */
function describeError(error: unknown): string {
  const code = ((error as { data?: { error?: unknown } } | null)?.data?.error ??
    "") as string;
  if (code === "label_taken") return t("s01.ssh-keys.error.labelTaken");
  if (code === "key_in_use") return t("s01.ssh-keys.error.referenced");
  const message =
    error instanceof Error
      ? error.message
      : ((error as { message?: string } | null)?.message ?? "");
  return t("s01.ssh-keys.error.failed", { message });
}

async function load() {
  if (!projectId.value) return;
  loading.value = true;
  errorMessage.value = "";
  try {
    const page = await listSshKeys(projectId.value);
    keys.value = page.items;
  } catch (error) {
    keys.value = [];
    errorMessage.value = describeError(error);
  } finally {
    loading.value = false;
  }
}

function run(action: string, fn: () => Promise<unknown>) {
  busy.value = action;
  errorMessage.value = "";
  return fn()
    .then(() => load())
    .catch((error: unknown) => {
      errorMessage.value = describeError(error);
    })
    .finally(() => {
      busy.value = "";
    });
}

function submitImport() {
  const value = keyInput.value.trim();
  const name = keyName.value.trim();
  if (!projectId.value || !value || !name) return;
  void run("ssh-import", async () => {
    await importSshKey(projectId.value, value, name);
    keyInput.value = "";
    keyName.value = "";
    confirmingDeleteId.value = "";
  });
}

function startRename(key: SshKey) {
  renamingId.value = key.id;
  renameInput.value = key.label;
}

function submitRename(key: SshKey) {
  const name = renameInput.value.trim();
  if (!name || name === key.label) {
    renamingId.value = "";
    return;
  }
  void run(`ssh-rename-${key.id}`, async () => {
    await renameSshKey(key.id, name);
    renamingId.value = "";
  });
}

/** 两段式删除（吊销；被 verified 仓库绑定 → 409 key_in_use）。 */
function onDelete(key: SshKey) {
  if (confirmingDeleteId.value !== key.id) {
    confirmingDeleteId.value = key.id;
    return;
  }
  confirmingDeleteId.value = "";
  void run(`ssh-delete-${key.id}`, () => revokeSshKeyApi(key.id));
}

function copyPublicKey(key: SshKey) {
  void navigator.clipboard.writeText(key.public_key).then(() => {
    copiedKeyId.value = key.id;
    window.setTimeout(() => {
      if (copiedKeyId.value === key.id) copiedKeyId.value = "";
    }, 2000);
  });
}

onMounted(load);
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="settingsFrame.breadcrumb(t('s01.ssh-keys.page.title'))"
    :title="t('s01.ssh-keys.page.title')"
    :description="t('s01.ssh-keys.page.description')"
    :scope-label="settingsFrame.scopeLabel.value"
    :entries="settingsFrame.navEntries.value"
    :icons="settingsFrame.navIcons"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("s01.ssh-keys.action.refresh") }}
      </Button>
    </template>

    <div class="sk-root">
      <Alert v-if="errorMessage" variant="danger">
        {{ errorMessage }}
      </Alert>

      <section class="rm-panel" data-testid="ssh-key-panel">
        <header class="sk-head">
          <h3 class="rm-panel__title">
            {{
              t("s01.ssh-keys.list.title", {
                active: activeCount,
                total: keys.length
              })
            }}
          </h3>
          <Badge
            :variant="activeCount > 0 ? 'success' : 'warning'"
            size="sm"
            data-testid="ssh-key-status"
          >
            {{
              activeCount > 0
                ? t("s01.ssh-keys.list.statusConfigured")
                : t("s01.ssh-keys.list.statusNone")
            }}
          </Badge>
        </header>

        <EmptyState
          v-if="keys.length === 0"
          data-testid="ssh-key-empty"
          :icon="KeyRound"
          :title="t('s01.ssh-keys.list.empty')"
        />

        <ul v-else class="sk-list" data-testid="ssh-key-list">
          <li
            v-for="key in keys"
            :key="key.id"
            class="sk-item"
            data-testid="ssh-key-item"
          >
            <div class="sk-item__head">
              <template v-if="renamingId === key.id">
                <input
                  v-model="renameInput"
                  :data-testid="`rename-input-${key.id}`"
                  class="rm-input sk-rename__input"
                  type="text"
                  :disabled="busy === `ssh-rename-${key.id}`"
                  @keydown.enter.prevent="submitRename(key)"
                  @keydown.esc.prevent="renamingId = ''"
                />
                <Button
                  variant="primary"
                  size="sm"
                  :disabled="
                    busy === `ssh-rename-${key.id}` || !renameInput.trim()
                  "
                  :data-testid="`rename-save-${key.id}`"
                  @click="submitRename(key)"
                >
                  {{ t("s01.ssh-keys.action.renameSave") }}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  :data-testid="`rename-cancel-${key.id}`"
                  @click="renamingId = ''"
                >
                  {{ t("s01.ssh-keys.action.renameCancel") }}
                </Button>
              </template>
              <template v-else>
                <span
                  class="sk-item__name"
                  :class="{ 'is-revoked': key.status === 'revoked' }"
                >
                  {{ key.label }}
                </span>
                <Badge v-if="key.is_default" variant="accent" size="sm">
                  {{ t("s01.ssh-keys.list.default") }}
                </Badge>
                <Badge
                  v-if="key.status === 'revoked'"
                  variant="warning"
                  size="sm"
                >
                  {{ t("s01.ssh-keys.list.revoked") }}
                </Badge>
                <span class="sk-item__spacer" />
                <Button
                  variant="ghost"
                  size="sm"
                  :disabled="busy === `ssh-rename-${key.id}`"
                  :data-testid="`rename-btn-${key.id}`"
                  @click="startRename(key)"
                >
                  {{ t("s01.ssh-keys.action.rename") }}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  :disabled="busy === `ssh-delete-${key.id}`"
                  :data-testid="`delete-btn-${key.id}`"
                  @click="onDelete(key)"
                >
                  {{
                    confirmingDeleteId === key.id
                      ? t("s01.ssh-keys.action.deleteArmed")
                      : t("s01.ssh-keys.action.delete")
                  }}
                </Button>
              </template>
            </div>
            <p class="sk-item__meta">
              <span class="rm-mono">{{ key.fingerprint }}</span>
            </p>
            <div class="sk-item__pub">
              <code class="rm-mono sk-item__pub-text" :title="key.public_key">
                {{ key.public_key }}
              </code>
              <Button variant="ghost" size="sm" @click="copyPublicKey(key)">
                {{
                  copiedKeyId === key.id
                    ? t("s01.ssh-keys.action.copied")
                    : t("s01.ssh-keys.action.copyPublicKey")
                }}
              </Button>
            </div>
          </li>
        </ul>
      </section>

      <section class="rm-panel" data-testid="ssh-import-form">
        <h3 class="rm-panel__title">
          {{ t("s01.ssh-keys.form.title") }}
        </h3>
        <label class="rm-field">
          {{ t("s01.ssh-keys.form.nameLabel") }}
          <input
            v-model="keyName"
            data-testid="ssh-key-name"
            class="rm-input"
            type="text"
            :placeholder="t('s01.ssh-keys.form.namePlaceholder')"
          />
        </label>
        <label class="rm-field">
          {{ t("s01.ssh-keys.form.inputLabel") }}
          <textarea
            v-model="keyInput"
            data-testid="ssh-key-input"
            class="rm-input rm-textarea"
            rows="6"
            placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
            spellcheck="false"
            autocomplete="off"
          />
        </label>
        <p class="rm-hint">
          {{ t("s01.ssh-keys.form.hint") }}
        </p>
        <div class="sk-actions">
          <Button
            variant="primary"
            size="sm"
            :disabled="
              busy === 'ssh-import' || !keyInput.trim() || !keyName.trim()
            "
            data-testid="ssh-key-submit"
            @click="submitImport"
          >
            {{ t("s01.ssh-keys.form.submit") }}
          </Button>
        </div>
      </section>
    </div>
  </SettingsAppFrame>
</template>

<style scoped>
/* rm-* 工具类与仓库设置页同源（scoped 不共享，按需内联）；
   token 全用 ws 语义变量（check:prism-tokens 门禁）。 */
.sk-root {
  container: sk-root / inline-size;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.sk-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rm-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.rm-panel__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.rm-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-input {
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  background: var(--ws-canvas);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-input::placeholder {
  color: var(--ws-text-secondary);
}

.rm-input:focus-visible {
  outline: var(--size-focus-ring, 2px) solid var(--ws-accent);
  outline-offset: 0;
}

.rm-textarea {
  height: auto;
  padding: var(--space-2);
  font-family: var(--font-mono);
  resize: vertical;
}

.rm-mono {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

.sk-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.sk-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  background: var(--ws-canvas);
}

.sk-item__head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.sk-item__name {
  color: var(--ws-text-primary);
  font-weight: var(--weight-semibold);
}

.sk-item__name.is-revoked {
  text-decoration: line-through;
  color: var(--ws-text-secondary);
}

.sk-item__spacer {
  flex: 1;
}

.sk-rename__input {
  flex: 1;
  min-width: 8rem;
}

.sk-item__meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.sk-item__pub {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.sk-item__pub-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ws-text-secondary);
}

.sk-actions {
  display: flex;
  gap: var(--space-2);
}

@container sk-root (max-width: 480px) {
  .sk-item__pub {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
