<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Copy, Link2 } from "lucide-vue-next";

import OnboardingShell from "./OnboardingShell.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import {
  buildAcceptUrl,
  createInvitationLink,
  type LinkCreated
} from "@/api/team";
import { translateError } from "@/api/base";
import { useTeamStore } from "@/stores/team";

/**
 * Screen 05 — Invitation Link（设计稿 §4）。
 * 链接语义（V1 裁决）：默认角色固定 MEMBER（不可经链接产生 ADMIN/OWNER）；
 * 有效期默认 7 天；不限次数或上限次数。链接只是「进入接受流程」的入口，
 * 不会自动把任何人加进团队；结果卡展示链接+复制+用量，可撤销后续管理在
 * /settings/team/invitations。
 */
const { t } = useI18n();
const router = useRouter();
const teamStore = useTeamStore();
const toast = useToast();

const EXPIRY_PRESETS = [
  { value: "7", label: "7d" },
  { value: "14", label: "14d" },
  { value: "30", label: "30d" }
] as const;

const expiryDays = ref("7");
const unlimited = ref(true);
const maxUses = ref<number>(10);
const creating = ref(false);
const error = ref<string | null>(null);
const issued = ref<LinkCreated | null>(null);
/** 请求时声明的次数上限（LinkCreated 不回传，结果卡本地记住） */
const issuedMaxUses = ref<number | null>(null);
const acceptUrl = computed(() =>
  issued.value ? buildAcceptUrl(issued.value.accept_path) : ""
);

const tenantId = computed(() => teamStore.context?.tenant_id ?? "");
const expiryOptions = computed(() =>
  EXPIRY_PRESETS.map(p => ({
    value: p.value,
    label: t("team.onboarding.link.expiryOption", { days: p.value })
  }))
);

async function createLink(): Promise<void> {
  if (!tenantId.value || creating.value) return;
  creating.value = true;
  error.value = null;
  try {
    const expiresAt = new Date(
      Date.now() + Number(expiryDays.value) * 24 * 3600 * 1000
    ).toISOString();
    issuedMaxUses.value = unlimited.value ? null : maxUses.value;
    issued.value = await createInvitationLink(tenantId.value, {
      expires_at: expiresAt,
      max_uses: issuedMaxUses.value
    });
  } catch (err) {
    error.value = translateError(err);
  } finally {
    creating.value = false;
  }
}

async function copyLink(): Promise<void> {
  if (!acceptUrl.value) return;
  try {
    await navigator.clipboard.writeText(acceptUrl.value);
    toast.toast({ title: t("team.onboarding.copied") });
  } catch {
    error.value = acceptUrl.value;
  }
}

function finish(): void {
  void router.push("/dashboard");
}
</script>

<template>
  <OnboardingShell>
    <div class="link">
      <section class="panel">
        <h2 class="panel__title">
          <Link2 :size="16" aria-hidden="true" />
          {{ t("team.onboarding.link.title") }}
        </h2>
        <p class="panel__desc">{{ t("team.onboarding.link.desc") }}</p>

        <div class="form">
          <FormField :label="t('team.onboarding.link.roleLabel')">
            <Input value="MEMBER" disabled data-testid="onboarding-link-role" />
          </FormField>

          <FormField
            :label="t('team.onboarding.link.expiryLabel')"
            html-for="link-expiry"
          >
            <Select
              id="link-expiry"
              v-model="expiryDays"
              :options="expiryOptions"
              data-testid="onboarding-link-expiry"
            />
          </FormField>

          <FormField :label="t('team.onboarding.link.usesLabel')">
            <div class="uses">
              <label class="uses__option">
                <input
                  v-model="unlimited"
                  type="radio"
                  :value="true"
                  name="uses-mode"
                />
                {{ t("team.onboarding.link.unlimited") }}
              </label>
              <label class="uses__option">
                <input
                  v-model="unlimited"
                  type="radio"
                  :value="false"
                  name="uses-mode"
                />
                <Input
                  v-model.number="maxUses"
                  type="number"
                  :min="1"
                  :disabled="unlimited"
                  class="uses__count"
                  :aria-label="t('team.onboarding.link.maxUses')"
                />
                {{ t("team.onboarding.link.usesUnit") }}
              </label>
            </div>
          </FormField>
        </div>

        <p v-if="error" class="panel__error">{{ error }}</p>

        <div class="panel__actions">
          <Button
            variant="primary"
            :loading="creating"
            data-testid="onboarding-link-create"
            @click="createLink"
          >
            {{ t("team.onboarding.link.create") }}
          </Button>
          <Button
            variant="secondary"
            data-testid="onboarding-link-finish"
            @click="finish"
          >
            {{ t("team.onboarding.invite.finish") }}
          </Button>
        </div>
      </section>

      <!-- 结果卡：链接 + 复制 + 用量说明 -->
      <section
        v-if="issued"
        class="panel result"
        data-testid="onboarding-link-result"
      >
        <h2 class="panel__title">
          {{ t("team.onboarding.link.resultTitle") }}
        </h2>
        <div class="result__row">
          <code class="result__url">{{ acceptUrl }}</code>
          <Button variant="secondary" size="sm" @click="copyLink">
            <Copy :size="14" aria-hidden="true" />
            {{ t("team.onboarding.copy") }}
          </Button>
        </div>
        <div class="result__meta">
          <Badge variant="neutral">
            {{ t("team.onboarding.link.roleBadge") }}
          </Badge>
          <span>{{
            t("team.onboarding.link.expiresAt", { time: issued.expires_at })
          }}</span>
          <span>{{
            issuedMaxUses === null
              ? t("team.onboarding.link.unlimitedUses")
              : t("team.onboarding.link.limitedUses", { count: issuedMaxUses })
          }}</span>
        </div>
        <p class="panel__desc">{{ t("team.onboarding.link.resultHint") }}</p>
      </section>
    </div>
  </OnboardingShell>
</template>

<style scoped>
.link {
  width: 100%;
  max-width: 560px;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding-top: var(--space-6);
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.panel__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.panel__desc {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.uses {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.uses__option {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.uses__count {
  flex-basis: 90px;
}

.panel__error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.panel__actions {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
  margin-top: var(--space-2);
}

.result__row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.result__url {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono, monospace);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.result__meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
