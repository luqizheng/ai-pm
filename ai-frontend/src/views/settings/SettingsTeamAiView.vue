<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { translateError } from "@/api/base";
import TeamProviderCard from "./TeamProviderCard.vue";
import TeamModelCatalog from "./TeamModelCatalog.vue";
import {
  cardFormOf,
  configurationOf,
  defaultEndpointOf,
  findProvider,
  invalidTimeout,
  PROVIDER_CATALOG,
  providerTypeDropdown,
  timeoutOf,
  type LlmProvider,
  type LlmProviderType
} from "./team-llm-types";
import { useModelSource } from "./model-source";
import { Combobox, type ComboboxOption } from "@/components/ui/combobox";
import PageFrame from "@/components/shell/PageFrame.vue";
import {
  createLlmProvider,
  deleteLlmProvider,
  listLlmProviders,
  replaceLlmCredential,
  resolveTenantId,
  setLlmProviderStatus,
  testLlmProvider,
  updateLlmProvider
} from "@/api/llm";

/**
 * 团队设置 — AI 配置（Team LLM Provider Management §13）。
 * Team≡tenant：Provider 是团队资源，仅 owner/admin 可管理（后端守卫；
 * 非管理员在此会收到 403 文案）。生命周期：测试成功≠自动启用；
 * enable 需已配凭证（openai_compatible / ollama 本地端点豁免）。
 */
const { t } = useI18n();
const toast = useToast();

const tenantId = ref<string | null>(null);
const providers = ref<LlmProvider[]>([]);
const loading = ref(false);
const saving = ref<string | null>(null);
const testing = ref<string | null>(null);
const testMessages = reactive<Record<string, { ok: boolean; text: string }>>(
  {}
);

const breadcrumb = [
  { label: t("settings.team.landing.title"), to: "/settings/team" },
  { label: t("settings.nav.aiProviders") }
];

// ── 新建 ─────────────────────────────────────────────────────
// 新建表单默认类型 = DeepSeek（2026-11-12 规则）
const DEFAULT_PROVIDER_TYPE = "deepseek" as LlmProvider["provider_type"];
const showCreate = ref(false);
const creating = ref(false);
const draft = reactive({
  name: "",
  provider_type: DEFAULT_PROVIDER_TYPE,
  endpoint: "",
  model: "",
  api_key: "",
  timeout: "120"
});

const providerTypeOptions = providerTypeDropdown({ t });

// ── 默认值预填：切换类型自动填端点；名称为空时按「类型+模型」自动生成 ──
// 仅当端点为空或仍是上一次预填值时覆盖，用户手填的自定义端点不回改。
const autoEndpoint = ref("");

watch(
  () => draft.provider_type,
  type => {
    if (!draft.endpoint.trim() || draft.endpoint === autoEndpoint.value) {
      autoEndpoint.value = defaultEndpointOf(type);
      draft.endpoint = autoEndpoint.value;
    }
    // 切换 provider 时清空 model（避免旧类型的残留值）
    draft.model = "";
  },
  // immediate：默认类型（DeepSeek）的端点在表单初始化时即预填
  { immediate: true }
);

// ── Model 字段状态机：只在新建 Provider 表单启用 ──
const providerCatalog = computed(() => findProvider(draft.provider_type));
const {
  options: modelGroups,
  status: modelStatus,
  hint: modelHint,
  refresh: refreshModels,
  canLoad: canLoadModels
} = useModelSource(
  providerCatalog,
  computed(() => draft.endpoint),
  computed(() => draft.api_key)
);

const modelInputPlaceholder = computed(() => {
  switch (modelStatus.value) {
    case "unsupported":
      return t("settings.team.ai.modelInputPlaceholder.unsupported");
    case "idle":
      return canLoadModels.value
        ? t("settings.team.ai.modelInputPlaceholder.ready")
        : t("settings.team.ai.modelInputPlaceholder.fillEndpointAndKey");
    case "loading":
      return t("settings.team.ai.modelInputPlaceholder.loading");
    case "fallback":
      return t("settings.team.ai.modelInputPlaceholder.fallback");
    case "empty":
      return t("settings.team.ai.modelInputPlaceholder.empty");
    case "success":
      return t("settings.team.ai.modelInputPlaceholder.success");
    default:
      return "";
  }
});

/** 把 Combobox 的 CombinedModel 分组转成 options（骨架态也能渲染）。 */
const modelComboboxOptions = computed<ComboboxOption[]>(() => {
  const out: ComboboxOption[] = [];
  for (const g of modelGroups.value) {
    for (const item of g.items) {
      if (item.id.startsWith("__skeleton_")) {
        out.push({ value: item.id, label: "", group: g.group, skeleton: true });
      } else {
        out.push({
          value: item.id,
          label: item.name || item.id,
          hint: item.descKey ? t(item.descKey) : undefined,
          group: g.group
        });
      }
    }
  }
  return out;
});

const defaultName = computed(() => {
  const label = t(`settings.team.ai.providerTypes.${draft.provider_type}`);
  const model = draft.model.trim();
  return model ? `${label}-${model}` : label;
});

const draftInvalid = computed(() =>
  invalidTimeout({ ...draft, model: "", endpoint: "" })
);

function statusLabel(status: LlmProvider["status"]): string {
  return t(`settings.team.ai.status.${status}`);
}

function applyRow(row: LlmProvider) {
  const index = providers.value.findIndex(item => item.id === row.id);
  if (index >= 0) {
    providers.value[index] = row;
  } else {
    providers.value.push(row);
  }
}

function fail(titleKey: string, error: unknown) {
  toast.toast({
    variant: "danger",
    title: t(titleKey),
    description: translateError(String(error))
  });
}

async function load() {
  loading.value = true;
  try {
    tenantId.value = await resolveTenantId();
    const result = await listLlmProviders(tenantId.value);
    providers.value = result.items ?? [];
  } catch (error) {
    fail("settings.team.ai.loadFailed", error);
  } finally {
    loading.value = false;
  }
}

async function create() {
  if (!tenantId.value || draftInvalid.value) return;
  creating.value = true;
  try {
    const row = await createLlmProvider(tenantId.value, {
      name: draft.name.trim() || defaultName.value,
      provider_type: draft.provider_type,
      endpoint: draft.endpoint.trim(),
      configuration: draft.model.trim() ? { model: draft.model.trim() } : {},
      api_key: draft.api_key.trim() || undefined,
      timeout_secs: timeoutOf({ ...draft, model: "", endpoint: "" })
    });
    applyRow(row);
    showCreate.value = false;
    // 重置回默认类型：端点与 autoEndpoint 同步预填（类型值未变化，watch 不会触发）
    autoEndpoint.value = defaultEndpointOf(DEFAULT_PROVIDER_TYPE);
    Object.assign(draft, {
      name: "",
      provider_type: DEFAULT_PROVIDER_TYPE,
      endpoint: autoEndpoint.value,
      model: "",
      api_key: "",
      timeout: "120"
    });
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.created"),
      description: row.name
    });
  } catch (error) {
    fail("settings.team.ai.saveFailed", error);
  } finally {
    creating.value = false;
  }
}

async function save(
  id: string,
  form: { name: string; endpoint: string; model: string; timeout: string }
) {
  const item = providers.value.find(row => row.id === id);
  if (!tenantId.value || !item || invalidTimeout(form)) return;
  saving.value = id;
  try {
    applyRow(
      await updateLlmProvider(tenantId.value, id, {
        expected_revision: item.revision,
        name: form.name.trim(),
        endpoint: form.endpoint.trim(),
        configuration: configurationOf(form),
        timeout_secs: timeoutOf(form)
      })
    );
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.saved")
    });
    // 保存后立即实测连通性——配置错误（端点/模型名写错）当场暴露。
    void test(id);
  } catch (error) {
    fail("settings.team.ai.saveFailed", error);
  } finally {
    saving.value = null;
  }
}

async function replaceKey(id: string, key: string) {
  const item = providers.value.find(row => row.id === id);
  if (!tenantId.value || !item || !key) return;
  saving.value = id;
  try {
    applyRow(
      await replaceLlmCredential(tenantId.value, id, {
        expected_revision: item.revision,
        api_key: key
      })
    );
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.keyReplaced")
    });
  } catch (error) {
    fail("settings.team.ai.saveFailed", error);
  } finally {
    saving.value = null;
  }
}

async function test(id: string) {
  if (!tenantId.value) return;
  testing.value = id;
  try {
    const result = await testLlmProvider(tenantId.value, id);
    testMessages[id] = {
      ok: result.ok,
      text: result.ok
        ? t("settings.team.ai.testOk", { ms: result.latency_ms })
        : `${t("settings.team.ai.testFailed")}: ${result.message}`
    };
    // 测试可能推进状态（connected/failed）——刷新列表。
    const fresh = await listLlmProviders(tenantId.value);
    providers.value = fresh.items ?? [];
  } catch (error) {
    testMessages[id] = {
      ok: false,
      text: `${t("settings.team.ai.testFailed")}: ${translateError(String(error))}`
    };
  } finally {
    testing.value = null;
  }
}

async function changeStatus(
  id: string,
  action: "enable" | "disable" | "archive"
) {
  const item = providers.value.find(row => row.id === id);
  if (!tenantId.value || !item) return;
  saving.value = id;
  try {
    applyRow(
      await setLlmProviderStatus(tenantId.value, id, action, item.revision)
    );
    toast.toast({
      variant: "success",
      title: t(`settings.team.ai.${action}Done`)
    });
    // 启用即实测：坏端点当场暴露，不等业务调用失败才发现。
    if (action === "enable") void test(id);
  } catch (error) {
    fail("settings.team.ai.saveFailed", error);
  } finally {
    saving.value = null;
  }
}

async function remove(id: string) {
  if (!tenantId.value) return;
  saving.value = id;
  try {
    await deleteLlmProvider(tenantId.value, id);
    providers.value = providers.value.filter(row => row.id !== id);
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.deleteDone")
    });
  } catch (error) {
    fail("settings.team.ai.deleteFailed", error);
  } finally {
    saving.value = null;
  }
}

onMounted(load);
</script>

<template>
  <PageFrame
    :breadcrumbs="breadcrumb"
    :title="t('settings.nav.aiProviders')"
    :description="t('settings.team.ai.desc')"
  >
    <p class="tp-note">{{ t("settings.team.ai.securityNote") }}</p>

    <div class="tp-toolbar">
      <Button
        data-test="add-provider"
        size="sm"
        @click="showCreate = !showCreate"
      >
        {{ t("settings.team.ai.addProvider") }}
      </Button>
    </div>

    <section v-if="showCreate" class="tp-create" data-test="create-form">
      <div class="tp-create__grid">
        <FormField
          :label="t('settings.team.ai.fields.name')"
          :description="t('settings.team.ai.nameAutoHint')"
        >
          <Input
            v-model="draft.name"
            :placeholder="defaultName"
            data-test="create-name"
          />
        </FormField>
        <FormField :label="t('settings.team.ai.fields.type')">
          <Select
            v-model="draft.provider_type"
            :options="providerTypeOptions"
            data-test="create-type"
          />
        </FormField>
        <FormField :label="t('settings.team.ai.fields.endpoint')">
          <Input
            v-model="draft.endpoint"
            placeholder="https://"
            data-test="create-endpoint"
          />
        </FormField>
        <FormField :label="t('settings.team.ai.fields.model')">
          <div class="tp-model-field">
            <Combobox
              v-model="draft.model"
              autocomplete="off"
              :options="modelComboboxOptions"
              :input-placeholder="modelInputPlaceholder"
              :placeholder="modelInputPlaceholder"
              data-test="create-model"
            />
            <Button
              v-if="providerCatalog?.modelSource === 'api'"
              size="sm"
              variant="ghost"
              :disabled="!canLoadModels || modelStatus === 'loading'"
              :title="t('settings.team.ai.refreshModels')"
              data-test="refresh-models"
              @click="refreshModels"
            >
              ↻
            </Button>
          </div>
          <p v-if="modelHint" class="tp-model-hint" :data-test="'model-hint'">
            {{ modelHint }}
          </p>
        </FormField>
        <FormField
          :label="t('settings.team.ai.fields.apiKey')"
          :description="t('settings.team.ai.keyMissing')"
        >
          <Input
            v-model="draft.api_key"
            type="password"
            data-test="create-key"
            autocomplete="off"
          />
        </FormField>
        <FormField
          :label="t('settings.team.ai.fields.timeout')"
          :description="t('settings.team.ai.fields.timeoutHint')"
        >
          <Input
            v-model="draft.timeout"
            type="number"
            data-test="create-timeout"
          />
        </FormField>
      </div>
      <footer class="tp-create__actions">
        <Button
          data-test="create-submit"
          size="sm"
          :loading="creating"
          :disabled="draftInvalid"
          @click="create"
        >
          {{ t("settings.team.ai.create") }}
        </Button>
        <Button size="sm" variant="ghost" @click="showCreate = false">
          {{ t("settings.team.ai.cancel") }}
        </Button>
      </footer>
    </section>

    <p v-if="loading" class="tp-empty">{{ t("settings.team.ai.loading") }}</p>
    <p v-else-if="providers.length === 0" class="tp-empty">
      {{ t("settings.team.ai.empty") }}
    </p>

    <div v-else class="tp-grid-frame">
      <div class="tp-grid" data-test="provider-grid">
        <TeamProviderCard
          v-for="row in providers"
          :key="row.id"
          :provider-id="row.id"
          :label="row.name"
          :provider-type="row.provider_type"
          :status="row.status"
          :status-label="statusLabel(row.status)"
          :has-credential="row.has_credential"
          :credential-hint="row.credential_hint"
          :last-tested-at="row.last_tested_at"
          :form="cardFormOf(row)"
          :saving="saving === row.id"
          :testing="testing === row.id"
          :test-message="testMessages[row.id]?.text ?? null"
          :test-ok="testMessages[row.id]?.ok ?? false"
          @save="form => save(row.id, form)"
          @replace-key="key => replaceKey(row.id, key)"
          @test="test(row.id)"
          @enable="changeStatus(row.id, 'enable')"
          @disable="changeStatus(row.id, 'disable')"
          @archive="changeStatus(row.id, 'archive')"
          @remove="remove(row.id)"
        />
      </div>
    </div>

    <!-- 模型目录/授权/策略（R3/B01-04）：Provider 启用后 AI 激活的第二步。 -->
    <TeamModelCatalog
      v-if="tenantId"
      :tenant-id="tenantId"
      :providers="providers"
    />
  </PageFrame>
</template>

<style scoped>
.tp-note {
  margin: 0 0 var(--space-4);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.tp-toolbar {
  display: flex;
  justify-content: flex-end;
  margin-bottom: var(--space-4);
}

.tp-create {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  margin-bottom: var(--space-5);
  background-color: var(--ws-surface);
  border: var(--border-1) dashed var(--ws-border);
  border-radius: var(--radius-md);
}

.tp-create__grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

.tp-create__actions {
  display: flex;
  gap: var(--space-2);
}

.tp-empty {
  margin: 0;
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-secondary);
}

.tp-model-field {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}
.tp-model-field :deep(.ui-combobox__trigger) {
  flex: 1 1 auto;
}

.tp-model-hint {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  line-height: 1.4;
}

.tp-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

/* 断点按容器宽而非 viewport：workspace 主区列（1280 视口下仅 ~314px）
   永远到不了 880px 的 viewport 断点，三列表单被压破（2026-11-13 冒烟：
   模型选择器被 API Key 列遮挡）。容器查询查不到自身，故声明在父级。 */
.tp-create,
.tp-grid-frame {
  container-type: inline-size;
  min-width: 0;
}

@container (max-width: 880px) {
  .tp-create__grid {
    grid-template-columns: 1fr;
  }
}

@container (max-width: 640px) {
  .tp-grid {
    grid-template-columns: 1fr;
  }
}
</style>
