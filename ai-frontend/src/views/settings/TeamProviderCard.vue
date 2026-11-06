<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import {
  invalidTimeout,
  type LlmProviderStatus
} from "./team-llm-types";

/**
 * Team LLM Provider 卡片（0041 Team-owned 实例）。
 * 表单是本地副本；保存/换 Key 把整份表单回传给页面动作层。
 */
const props = defineProps<{
  providerId: string;
  label: string;
  providerType: string;
  status: LlmProviderStatus;
  statusLabel: string;
  hasCredential: boolean;
  credentialHint: string | null;
  lastTestedAt: string | null;
  /** 卡片表单初值（name/endpoint/model/timeout） */
  form: { name: string; endpoint: string; model: string; timeout: string };
  saving: boolean;
  testing: boolean;
  testMessage: string | null;
  testOk: boolean;
}>();

const emit = defineEmits<{
  save: [form: { name: string; endpoint: string; model: string; timeout: string }];
  replaceKey: [key: string];
  test: [];
  enable: [];
  disable: [];
  archive: [];
  remove: [];
}>();

const { t } = useI18n();

const draft = computed(() => ({ ...props.form }));
const apiKey = ref("");


function statusVariant(
  status: LlmProviderStatus
): "success" | "danger" | "neutral" {
  if (status === "enabled" || status === "connected") return "success";
  if (status === "failed") return "danger";
  return "neutral";
}

function lastTestedText(): string {
  if (!props.lastTestedAt) return t("settings.team.ai.neverTested");
  return t("settings.team.ai.lastTested", {
    time: new Date(props.lastTestedAt).toLocaleString()
  });
}
</script>

<template>
  <section class="tp-card" data-test="provider-card" :data-provider="providerId">
    <header class="tp-card__head">
      <div class="tp-card__title">
        <h3 class="tp-card__name">{{ label }}</h3>
        <span class="tp-card__slug">{{ providerType }}</span>
      </div>
      <Badge :variant="statusVariant(status)">{{ statusLabel }}</Badge>
    </header>

    <div class="tp-card__grid">
      <FormField :label="t('settings.team.ai.fields.name')">
        <Input v-model="draft.name" />
      </FormField>
      <FormField :label="t('settings.team.ai.fields.endpoint')">
        <Input v-model="draft.endpoint" placeholder="https://" />
      </FormField>
      <FormField :label="t('settings.team.ai.fields.model')">
        <Input v-model="draft.model" />
      </FormField>
      <FormField
        :label="t('settings.team.ai.fields.timeout')"
        :description="t('settings.team.ai.fields.timeoutHint')"
      >
        <Input
          v-model="draft.timeout"
          type="number"
          :invalid="invalidTimeout(draft)"
        />
      </FormField>
    </div>

    <FormField
      :label="t('settings.team.ai.fields.apiKey')"
      :description="
        hasCredential
          ? t('settings.team.ai.keyConfigured', { hint: credentialHint ?? '' })
          : t('settings.team.ai.keyMissing')
      "
    >
      <Input
        v-model="apiKey"
        type="password"
        :placeholder="t('settings.team.ai.keyPlaceholder')"
      />
    </FormField>

    <p class="tp-card__meta">{{ lastTestedText() }}</p>

    <footer class="tp-card__actions">
      <Button
        data-test="save"
        size="sm"
        :loading="saving"
        :disabled="invalidTimeout(draft)"
        @click="emit('save', { ...draft })"
      >
        {{ t("settings.team.ai.save") }}
      </Button>
      <Button
        v-if="apiKey.trim()"
        data-test="replace-key"
        size="sm"
        variant="secondary"
        :loading="saving"
        @click="emit('replaceKey', apiKey.trim())"
      >
        {{ t("settings.team.ai.replaceKey") }}
      </Button>
      <Button
        data-test="test"
        size="sm"
        variant="secondary"
        :loading="testing"
        @click="emit('test')"
      >
        {{ t("settings.team.ai.test") }}
      </Button>
      <Button
        v-if="status !== 'enabled' && status !== 'archived'"
        data-test="enable"
        size="sm"
        variant="secondary"
        :loading="saving"
        @click="emit('enable')"
      >
        {{ t("settings.team.ai.enable") }}
      </Button>
      <Button
        v-if="status === 'enabled'"
        data-test="disable"
        size="sm"
        variant="secondary"
        :loading="saving"
        @click="emit('disable')"
      >
        {{ t("settings.team.ai.disable") }}
      </Button>
      <Button
        v-if="status !== 'archived'"
        data-test="archive"
        size="sm"
        variant="ghost"
        :loading="saving"
        @click="emit('archive')"
      >
        {{ t("settings.team.ai.archive") }}
      </Button>
      <Button
        v-if="status === 'archived'"
        data-test="delete"
        size="sm"
        variant="ghost"
        :loading="saving"
        @click="emit('remove')"
      >
        {{ t("settings.team.ai.delete") }}
      </Button>
    </footer>

    <p
      v-if="testMessage"
      class="tp-card__test"
      :class="{ 'is-ok': testOk, 'is-bad': !testOk }"
      data-test="test-result"
    >
      {{ testMessage }}
    </p>
  </section>
</template>

<style scoped>
.tp-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
}

.tp-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.tp-card__title {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.tp-card__name {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tp-card__slug {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.tp-card__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.tp-card__meta {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.tp-card__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.tp-card__test {
  margin: 0;
  font-size: var(--ws-font-size-caption);
}

.tp-card__test.is-ok {
  color: var(--ws-accent);
}

.tp-card__test.is-bad {
  color: var(--ws-text-secondary);
}

@media (max-width: 720px) {
  .tp-card__grid {
    grid-template-columns: 1fr;
  }
}
</style>
