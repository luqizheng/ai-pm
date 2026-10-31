<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/shell/api";
import PageHeader from "./PageHeader.vue";
import ProviderCard from "./ProviderCard.vue";
import {
  invalidTimeout,
  timeoutOf,
  type ProviderAction,
  type ProviderItem
} from "./models-types";
import {
  listLlmProviders,
  testLlmProvider,
  updateLlmProvider
} from "@/api/llm";
import type { LlmProvider, LlmProviderId } from "@/api/llm";

const { t } = useI18n();
const toast = useToast();

const items = ref<ProviderItem[]>([]);
const loading = ref(false);
const saving = ref<LlmProviderId | null>(null);
const testing = ref<LlmProviderId | null>(null);

function buildItem(row: LlmProvider): ProviderItem {
  return {
    data: row,
    label: t(`workspace.pages.models.providers.${row.provider}`),
    form: {
      base_url: row.base_url,
      model: row.model,
      api_key: "",
      timeout: String(row.timeout_secs),
      enabled: row.enabled
    }
  };
}

function applyRow(row: LlmProvider) {
  const index = items.value.findIndex(
    item => item.data.provider === row.provider
  );
  const next = buildItem(row);
  if (index >= 0) {
    const previous = items.value[index];
    previous.testResult = undefined;
    items.value[index] = next;
  } else {
    items.value.push(next);
  }
}

async function load() {
  loading.value = true;
  try {
    const result = await listLlmProviders();
    items.value = (result.items ?? []).map(buildItem);
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.loadFailed"),
      description: translateError(String(error))
    });
  } finally {
    loading.value = false;
  }
}

async function save(action: ProviderAction) {
  const form = action.form;
  if (invalidTimeout(form)) return;
  saving.value = action.provider;
  try {
    const updated = await updateLlmProvider(action.provider, {
      base_url: form.base_url.trim(),
      model: form.model.trim(),
      api_key: form.api_key.trim() || undefined,
      enabled: form.enabled,
      timeout_secs: timeoutOf(form)
    });
    applyRow(updated);
    toast.toast({
      variant: "success",
      title: t("workspace.pages.models.saved"),
      description: action.label
    });
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    saving.value = null;
  }
}

async function clearKey(action: ProviderAction) {
  const form = action.form;
  if (invalidTimeout(form)) return;
  saving.value = action.provider;
  try {
    const updated = await updateLlmProvider(action.provider, {
      base_url: form.base_url.trim(),
      model: form.model.trim(),
      clear_api_key: true,
      enabled: form.enabled,
      timeout_secs: timeoutOf(form)
    });
    applyRow(updated);
    toast.toast({
      variant: "success",
      title: t("workspace.pages.models.keyCleared"),
      description: action.label
    });
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    saving.value = null;
  }
}

async function test(provider: LlmProviderId) {
  const item = items.value.find(entry => entry.data.provider === provider);
  if (!item) return;
  testing.value = provider;
  try {
    item.testResult = await testLlmProvider(provider);
  } catch (error) {
    item.testResult = {
      ok: false,
      latency_ms: 0,
      message: translateError(String(error))
    };
  } finally {
    testing.value = null;
  }
}

onMounted(load);
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.models.title')"
      :subtitle="t('workspace.pages.models.subtitle')"
    />
    <p class="ws-models-note">{{ t("workspace.pages.models.securityNote") }}</p>

    <p v-if="loading" class="ws-models-empty">
      {{ t("workspace.pages.models.loading") }}
    </p>
    <p v-else-if="items.length === 0" class="ws-models-empty">
      {{ t("workspace.pages.models.empty") }}
    </p>

    <div v-else class="ws-models-grid" data-test="provider-grid">
      <ProviderCard
        v-for="item in items"
        :key="item.data.provider"
        :item="item"
        :saving="saving === item.data.provider"
        :testing="testing === item.data.provider"
        @save="save"
        @test="test"
        @clear-key="clearKey"
      />
    </div>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 1120px;
}

.ws-models-note {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-models-empty {
  margin: 0;
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-secondary);
}

.ws-models-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

@media (max-width: 900px) {
  .ws-models-grid {
    grid-template-columns: 1fr;
  }
}
</style>
