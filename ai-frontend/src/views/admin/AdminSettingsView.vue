<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import PageFrame from "@/components/shell/PageFrame.vue";
import Button from "@/components/ui/button/Button.vue";
import Checkbox from "@/components/ui/checkbox/Checkbox.vue";
import Input from "@/components/ui/input/Input.vue";
import { translateError } from "@/api/base";
import {
  getEmbeddingConfig,
  saveEmbeddingConfig,
  testEmbeddingConfig,
  type EmbeddingTestResult
} from "@/api/adminConfig";

/**
 * 系统设置（/admin/settings）——当前面：embedding 供给（Ollama 地址 + 模型）。
 *
 * 门：后端 is_admin（AI/Key/普通成员 403）。测试连接 = 实调 /embeddings 探针
 * （不写库），返回维度与延迟。错误文案透出服务端真实原因（网关路由缺失、
 * 403 等），不再笼统归因为「需要系统管理员」。
 */
const { t } = useI18n();

const endpoint = ref("");
const model = ref("");
const enabled = ref(false);
const loading = ref(false);
const saving = ref(false);
const savedNotice = ref("");
const testing = ref(false);
const testResult = ref<EmbeddingTestResult | null>(null);
const errorText = ref("");

async function load() {
  loading.value = true;
  errorText.value = "";
  try {
    const cfg = await getEmbeddingConfig();
    endpoint.value = cfg.endpoint;
    model.value = cfg.model;
    enabled.value = cfg.enabled;
  } catch (error) {
    errorText.value = t("shell.adminSettings.error.load", {
      reason: translateError(String(error))
    });
  } finally {
    loading.value = false;
  }
}

async function save() {
  if (saving.value) return;
  saving.value = true;
  savedNotice.value = "";
  errorText.value = "";
  try {
    const cfg = await saveEmbeddingConfig({
      endpoint: endpoint.value.trim(),
      model: model.value.trim(),
      enabled: enabled.value
    });
    endpoint.value = cfg.endpoint;
    model.value = cfg.model;
    enabled.value = cfg.enabled;
    savedNotice.value = t("shell.adminSettings.notice.saved");
  } catch (error) {
    errorText.value = t("shell.adminSettings.error.save", {
      reason: translateError(String(error))
    });
  } finally {
    saving.value = false;
  }
}

async function runTest() {
  if (testing.value) return;
  testing.value = true;
  errorText.value = "";
  try {
    testResult.value = await testEmbeddingConfig({
      endpoint: endpoint.value.trim(),
      model: model.value.trim()
    });
  } catch (error) {
    testResult.value = null;
    errorText.value = t("shell.adminSettings.error.test", {
      reason: translateError(String(error))
    });
  } finally {
    testing.value = false;
  }
}

onMounted(load);
</script>

<template>
  <PageFrame
    :title="t('shell.adminSettings.title')"
    :description="t('shell.adminSettings.description')"
  >
    <div class="admin-settings">
      <p v-if="errorText" class="settings-error" role="alert">
        {{ errorText }}
      </p>
      <p v-if="loading" class="settings-hint">
        {{ t("shell.adminSettings.hint.loading") }}
      </p>

      <section class="settings-card">
        <h3 class="settings-heading">
          {{ t("shell.adminSettings.embedding.heading") }}
        </h3>
        <p class="settings-desc">
          {{ t("shell.adminSettings.embedding.description") }}
        </p>

        <div class="settings-field">
          <label for="embed-endpoint">{{
            t("shell.adminSettings.embedding.endpoint")
          }}</label>
          <Input
            id="embed-endpoint"
            v-model="endpoint"
            class="settings-input"
            type="text"
            placeholder="http://192.168.2.10:11434/v1"
          />
        </div>
        <div class="settings-field">
          <label for="embed-model">{{
            t("shell.adminSettings.embedding.model")
          }}</label>
          <Input
            id="embed-model"
            v-model="model"
            class="settings-input"
            type="text"
            placeholder="bge-m3"
          />
        </div>
        <label class="settings-check">
          <Checkbox v-model="enabled" />
          <span>{{ t("shell.adminSettings.embedding.enabled") }}</span>
        </label>

        <div class="admin-actions">
          <Button
            variant="primary"
            class="admin-save"
            :disabled="saving"
            @click="save"
          >
            {{
              saving
                ? t("shell.adminSettings.action.saving")
                : t("shell.adminSettings.action.save")
            }}
          </Button>
          <Button
            variant="secondary"
            class="admin-test"
            :disabled="testing"
            @click="runTest"
          >
            {{
              testing
                ? t("shell.adminSettings.action.testing")
                : t("shell.adminSettings.action.test")
            }}
          </Button>
        </div>
        <p v-if="savedNotice" class="settings-ok" role="status">
          {{ savedNotice }}
        </p>

        <div
          v-if="testResult"
          class="test-result"
          :class="{ 'is-bad': !testResult.ok }"
        >
          <template v-if="testResult.ok">
            {{
              t("shell.adminSettings.test.ok", {
                dims: testResult.dims,
                ms: testResult.latency_ms
              })
            }}
          </template>
          <template v-else>
            {{
              t("shell.adminSettings.test.fail", {
                error: testResult.error ?? ""
              })
            }}
          </template>
        </div>

        <p class="settings-hint">
          {{ t("shell.adminSettings.embedding.hint") }}
        </p>
      </section>
    </div>
  </PageFrame>
</template>

<style scoped>
.admin-settings {
  display: flex;
  flex-direction: column;
  gap: var(--space-4, 16px);
}

.settings-card {
  border: 1px solid var(--border-subtle, var(--ws-border, #d0d0d0));
  border-radius: var(--radius-sm);
  background: var(--surface-card, var(--ws-surface, #fff));
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3, 12px);
}

.settings-heading {
  margin: 0;
  font-size: var(--font-size-body-strong);
  font-weight: 600;
  color: var(--text-primary, var(--ws-text, #111));
}

.settings-desc,
.settings-hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary, var(--ws-text, #555));
}

.settings-error {
  margin: 0;
  font-size: var(--font-size-body);
  color: var(--text-danger, #b42318);
}

.settings-ok {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-success, #1b7a43);
}

.settings-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.settings-field label {
  font-size: var(--font-size-caption);
  color: var(--text-secondary, var(--ws-text, #555));
}

.settings-check {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-body);
  color: var(--text-secondary, var(--ws-text, #555));
}

.admin-actions {
  display: flex;
  gap: var(--space-2, 8px);
}

.test-result {
  font-size: var(--font-size-caption);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  background: var(--surface-success-subtle, #e7f5ec);
  color: var(--text-success, #1b7a43);
}

.test-result.is-bad {
  background: var(--surface-danger-subtle, #fdeceb);
  color: var(--text-danger, #b42318);
}
</style>
