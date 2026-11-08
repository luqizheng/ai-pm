<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@/components/ui/toast";
import { HttpError } from "@digua/http";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { FormField } from "@/components/ui/form-field";
import { Select } from "@/components/ui/select";
import {
  getProjectAiPolicy,
  listLlmProviders,
  resolveTenantId,
  setProjectAiPolicy,
  type LlmProvider,
  type ProjectAiPolicy
} from "@/api/llm";
import AiTeamFrame from "./AiTeamFrame.vue";
import { useProjectSettingsFrame } from "../shared";

/**
 * Project Setting · AI Team · Model Policies 子页（0043 项目级策略）。
 *
 * 语义（§10.2/§16）：
 * - 未指定（provider_id = null，含策略行不存在）= 继承 Team 默认——调用时用
 *   团队内首个 enabled 实例（创建序）；本页明确把该语义讲给用户。
 * - 指定后严格不回退：绑定的 Provider 即使被停用也照用（响应里的
 *   provider_status ≠ enabled 时给黄色提示）。
 * - 保存走 CAS：expected_revision 不一致 → 409 revision_conflict，提示重试
 *   并自动回读最新策略。
 * Provider 下拉数据 = Team Provider 列表（只列 enabled）；tenant_id 经
 * resolveTenantId()（Team ≡ tenant，与租户级 Provider 页同源）。
 */

/** reka-ui Select 不接受空串 value：继承语义用哨兵表示 */
const INHERIT = "__inherit__";

const { t } = useI18n();
const toast = useToast();
const settingsFrame = useProjectSettingsFrame();
const projectId = settingsFrame.projectId;

const policy = ref<ProjectAiPolicy | null>(null);
const providers = ref<LlmProvider[]>([]);
const loading = ref(true);
const errorMessage = ref("");
const providersError = ref("");
const selected = ref(INHERIT);
const saving = ref(false);
const conflict = ref(false);

const providerOptions = computed(() => [
  { value: INHERIT, label: t("aiTeam.policies.option.inherit") },
  ...providers.value.map(provider => ({
    value: provider.id,
    label: provider.name
  }))
]);

/** 当前选中是否等于服务端已绑定值（未变则禁用保存，避免无意义 CAS 写） */
const dirty = computed(() => {
  if (!policy.value) return false;
  const bound = policy.value.provider_id ?? INHERIT;
  return selected.value !== bound;
});

const boundProviderStatus = computed(() =>
  policy.value?.provider_status && policy.value.provider_status !== "enabled"
    ? policy.value.provider_status
    : null
);

async function load() {
  if (!projectId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  errorMessage.value = "";
  providersError.value = "";
  try {
    // 策略与 Provider 列表并行拉取；Provider 失败不阻塞策略展示
    const [policyRow, providersResult] = await Promise.allSettled([
      getProjectAiPolicy(projectId.value),
      resolveTenantId().then(tenantId => listLlmProviders(tenantId))
    ]);
    if (policyRow.status === "fulfilled") {
      applyPolicy(policyRow.value);
    } else {
      errorMessage.value =
        policyRow.reason instanceof Error
          ? policyRow.reason.message
          : String(policyRow.reason ?? "");
    }
    if (providersResult.status === "fulfilled") {
      providers.value = (providersResult.value.items ?? []).filter(
        provider => provider.status === "enabled"
      );
    } else {
      providersError.value =
        providersResult.reason instanceof Error
          ? providersResult.reason.message
          : String(providersResult.reason ?? "");
    }
  } finally {
    loading.value = false;
  }
}

function applyPolicy(row: ProjectAiPolicy) {
  policy.value = row;
  selected.value = row.provider_id ?? INHERIT;
}

async function save() {
  if (!projectId.value || !policy.value || saving.value || !dirty.value) return;
  saving.value = true;
  conflict.value = false;
  errorMessage.value = "";
  try {
    const row = await setProjectAiPolicy(projectId.value, {
      provider_id: selected.value === INHERIT ? null : selected.value,
      expected_revision: policy.value.revision
    });
    applyPolicy(row);
    toast.toast({
      variant: "success",
      title: t("aiTeam.policies.saved")
    });
  } catch (error) {
    if (
      error instanceof HttpError &&
      (error.status === 409 || error.code === "revision_conflict")
    ) {
      // CAS 冲突：回读最新策略（revision 已前进），提示用户确认后重试
      conflict.value = true;
      try {
        applyPolicy(await getProjectAiPolicy(projectId.value));
      } catch {
        /* 回读失败保留当前态，保存按钮按旧 revision 仍会 409 */
      }
    } else {
      errorMessage.value = `${t("aiTeam.policies.saveFailed")}: ${
        error instanceof Error ? error.message : String(error ?? "")
      }`;
    }
  } finally {
    saving.value = false;
  }
}

function formatTime(value: string | null): string {
  return value
    ? new Date(value).toLocaleString()
    : t("aiTeam.policies.meta.never");
}

/** 绑定 Provider 状态文案（复用租户级 Provider 页的状态词表） */
function statusLabel(status: string): string {
  return t(`settings.team.ai.status.${status}`);
}

onMounted(load);
</script>

<template>
  <AiTeamFrame :description="t('aiTeam.policies.description')">
    <div class="ai-policies">
      <Alert
        v-if="errorMessage"
        variant="danger"
        data-testid="ai-policies-error"
      >
        {{ errorMessage }}
      </Alert>
      <Alert
        v-if="providersError"
        variant="warning"
        data-testid="ai-policies-providers-error"
      >
        {{ t("aiTeam.policies.providersLoadFailed") }}: {{ providersError }}
      </Alert>

      <!-- 无项目上下文：不发请求（scope 哨兵不做项目 id） -->
      <EmptyState
        v-if="!projectId"
        :title="t('aiTeam.noProject.title')"
        :description="t('aiTeam.noProject.hint')"
        data-testid="ai-policies-no-project"
      />

      <template v-else>
        <Alert
          v-if="conflict"
          variant="warning"
          data-testid="ai-policies-conflict"
        >
          {{ t("aiTeam.policies.conflict") }}
        </Alert>

        <!-- 当前生效 Provider（0043 解析语义的可读呈现） -->
        <section
          v-if="policy"
          class="ai-policies__current"
          data-testid="ai-policies-current"
        >
          <p class="ai-policies__current-label">
            {{ t("aiTeam.policies.currentHeading") }}
          </p>
          <p v-if="policy.provider_id" class="ai-policies__current-value">
            {{ policy.provider_name ?? policy.provider_id }}
            <Badge v-if="boundProviderStatus" variant="warning" size="sm">
              {{ statusLabel(boundProviderStatus) }}
            </Badge>
          </p>
          <p
            v-else
            class="ai-policies__current-value"
            data-testid="ai-policies-inherit"
          >
            {{ t("aiTeam.policies.inheritLabel") }}
          </p>
          <p class="ai-policies__current-hint">
            {{
              policy.provider_id
                ? t("aiTeam.policies.boundDetail", {
                    name: policy.provider_name ?? policy.provider_id
                  })
                : t("aiTeam.policies.inheritDetail")
            }}
          </p>
          <p v-if="boundProviderStatus" class="ai-policies__strict-warning">
            {{
              t("aiTeam.policies.boundDisabledWarning", {
                status: statusLabel(boundProviderStatus)
              })
            }}
          </p>
          <p class="ai-policies__meta">
            {{ t("aiTeam.policies.meta.updatedAt") }}:
            {{ formatTime(policy.updated_at) }}
          </p>
        </section>

        <!-- Provider 选择 + 保存（CAS） -->
        <section v-if="policy" class="ai-policies__form">
          <FormField
            :label="t('aiTeam.policies.field.provider')"
            :description="t('aiTeam.policies.field.providerHint')"
          >
            <Select
              v-model="selected"
              :options="providerOptions"
              data-testid="ai-policies-select"
            />
          </FormField>
          <p
            v-if="providers.length === 0 && !providersError && !loading"
            class="ai-policies__no-providers"
            data-testid="ai-policies-no-providers"
          >
            {{ t("aiTeam.policies.noEnabledProviders") }}
            <RouterLink
              class="ai-policies__team-link"
              to="/settings/team/ai-providers"
            >
              {{ t("aiTeam.policies.gotoTeamAi") }}
            </RouterLink>
          </p>
          <div class="ai-policies__actions">
            <Button
              variant="primary"
              size="sm"
              :disabled="!dirty || saving"
              :loading="saving"
              data-testid="ai-policies-save"
              @click="save"
            >
              {{
                saving ? t("aiTeam.policies.saving") : t("aiTeam.policies.save")
              }}
            </Button>
          </div>
        </section>
      </template>
    </div>
  </AiTeamFrame>
</template>

<style scoped>
.ai-policies {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.ai-policies__current {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ai-policies__current-label {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-policies__current-value {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
  font-weight: var(--weight-semibold);
}

.ai-policies__current-hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-policies__strict-warning {
  margin: 0;
  padding: var(--space-3);
  background: var(--surface-sunken);
  border-radius: var(--radius-sm);
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-policies__meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-policies__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ai-policies__no-providers {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-policies__team-link {
  color: var(--ws-accent);
}

.ai-policies__team-link:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 1px;
  border-radius: var(--radius-xs);
}

.ai-policies__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
</style>
