<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { CheckCircle2, Copy, Trash2 } from "lucide-vue-next";

import OnboardingShell from "./OnboardingShell.vue";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import {
  createTeamInvitation,
  buildAcceptUrl,
  type InvitationCreated
} from "@/api/team";
import { translateError } from "@/api/base";
import { useTeamStore } from "@/stores/team";

/**
 * Screen 03+04 — Team Created & Invite Members（设计稿 §4）。
 * 建团成功横幅（团队名 + 角色 OWNER + 下一步邀请）之上承载多邮箱邀请：
 * OWNER 不可经邀请指派（仅 admin|member）；每行提交后返回一次性接受链接
 * （V1 无邮件投递 → 复制分享）；可全部跳过继续。
 */
const { t } = useI18n();
const router = useRouter();
const teamStore = useTeamStore();
const toast = useToast();

const teamName = computed(
  () => teamStore.currentTeam?.name ?? t("team.onboarding.invite.fallbackTeam")
);

interface DraftRow {
  email: string;
  role: "member" | "admin";
}

const drafts = reactive<DraftRow[]>([{ email: "", role: "member" }]);
const submitting = ref(false);
const error = ref<string | null>(null);
const issued = ref<{ email: string; role: string; url: string }[]>([]);

const roleOptions = computed(() => [
  { value: "member", label: t("team.onboarding.role.member") },
  { value: "admin", label: t("team.onboarding.role.admin") }
]);

const tenantId = computed(() => teamStore.context?.tenant_id ?? "");

function addRow(): void {
  drafts.push({ email: "", role: "member" });
}

function removeRow(index: number): void {
  if (drafts.length === 1) return;
  drafts.splice(index, 1);
}

const validRows = computed(() =>
  drafts
    .map(d => ({ ...d, email: d.email.trim() }))
    .filter(d => d.email.length > 0 && d.email.includes("@"))
);

async function submitInvitations(): Promise<void> {
  if (!tenantId.value || validRows.value.length === 0) return;
  submitting.value = true;
  error.value = null;
  try {
    const created: InvitationCreated[] = [];
    for (const row of validRows.value) {
      created.push(
        await createTeamInvitation(tenantId.value, {
          email: row.email,
          role: row.role
        })
      );
    }
    issued.value = created.map((item, i) => ({
      email: validRows.value[i].email,
      role: validRows.value[i].role,
      url: buildAcceptUrl(item.accept_path)
    }));
    drafts.splice(0, drafts.length, { email: "", role: "member" });
    toast.toast({ title: t("team.onboarding.invite.sentToast") });
  } catch (err) {
    error.value = translateError(err);
  } finally {
    submitting.value = false;
  }
}

async function copyUrl(url: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(url);
    toast.toast({ title: t("team.onboarding.copied") });
  } catch {
    error.value = url;
  }
}

function goLink(): void {
  void router.push("/onboarding/link");
}

function finish(): void {
  void router.push("/dashboard");
}
</script>

<template>
  <OnboardingShell>
    <div class="invite">
      <!-- Screen 03：建团成功 -->
      <div class="success" data-testid="onboarding-created-banner">
        <CheckCircle2 :size="20" aria-hidden="true" />
        <div>
          <p class="success__title">
            {{ t("team.onboarding.invite.createdTitle") }}
          </p>
          <p class="success__desc">
            {{ t("team.onboarding.invite.createdDesc", { team: teamName }) }}
          </p>
        </div>
      </div>

      <!-- Screen 04：邀请成员 -->
      <section class="panel">
        <h2 class="panel__title">{{ t("team.onboarding.invite.title") }}</h2>
        <p class="panel__desc">{{ t("team.onboarding.invite.desc") }}</p>

        <div
          v-for="(row, index) in drafts"
          :key="index"
          class="row"
          :data-testid="`onboarding-invite-row-${index}`"
        >
          <FormField
            class="row__email"
            :label="
              index === 0 ? t('team.onboarding.invite.emailLabel') : undefined
            "
            html-for="invite-email"
          >
            <Input
              v-model="row.email"
              type="email"
              :placeholder="t('team.onboarding.invite.emailPlaceholder')"
              :aria-label="t('team.onboarding.invite.emailLabel')"
            />
          </FormField>
          <FormField
            class="row__role"
            :label="
              index === 0 ? t('team.onboarding.invite.roleLabel') : undefined
            "
          >
            <Select v-model="row.role" :options="roleOptions" />
          </FormField>
          <Button
            variant="ghost"
            size="icon"
            :disabled="drafts.length === 1"
            :aria-label="t('team.onboarding.invite.removeRow')"
            @click="removeRow(index)"
          >
            <Trash2 :size="15" />
          </Button>
        </div>

        <Button
          variant="tertiary"
          size="sm"
          data-testid="onboarding-invite-add"
          @click="addRow"
        >
          {{ t("team.onboarding.invite.addRow") }}
        </Button>

        <p v-if="error" class="panel__error">{{ error }}</p>

        <div class="panel__actions">
          <Button
            variant="secondary"
            data-testid="onboarding-invite-skip"
            @click="goLink"
          >
            {{ t("team.onboarding.invite.skip") }}
          </Button>
          <Button
            variant="primary"
            :loading="submitting"
            :disabled="validRows.length === 0"
            data-testid="onboarding-invite-submit"
            @click="submitInvitations"
          >
            {{
              t("team.onboarding.invite.submit", { count: validRows.length })
            }}
          </Button>
        </div>
      </section>

      <!-- 已签发邀请的接受链接（复制分享，V1 无邮件投递） -->
      <section
        v-if="issued.length"
        class="panel"
        data-testid="onboarding-issued"
      >
        <h2 class="panel__title">
          {{ t("team.onboarding.invite.issuedTitle") }}
        </h2>
        <p class="panel__desc">{{ t("team.onboarding.invite.issuedDesc") }}</p>
        <ul class="issued">
          <li v-for="item in issued" :key="item.url" class="issued__row">
            <span class="issued__meta">
              {{ item.email }} · {{ item.role }}
            </span>
            <code class="issued__url">{{ item.url }}</code>
            <Button
              variant="secondary"
              size="sm"
              :aria-label="t('team.onboarding.copy')"
              @click="copyUrl(item.url)"
            >
              <Copy :size="14" aria-hidden="true" />
              {{ t("team.onboarding.copy") }}
            </Button>
          </li>
        </ul>
        <div class="panel__actions">
          <Button
            variant="primary"
            data-testid="onboarding-invite-finish"
            @click="finish"
          >
            {{ t("team.onboarding.invite.finish") }}
          </Button>
        </div>
      </section>
    </div>
  </OnboardingShell>
</template>

<style scoped>
.invite {
  width: 100%;
  max-width: 640px;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding-top: var(--space-6);
}

.success {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
  border: 1px solid var(--ws-color-success);
  border-radius: var(--radius-md);
  background: var(--success-subtle);
  color: var(--ws-color-success-text);
}

.success__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
}

.success__desc {
  margin: var(--space-1) 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
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

.row {
  display: flex;
  align-items: flex-end;
  gap: var(--space-3);
}

.row__email {
  flex: 1;
}

.row__role {
  flex-basis: 140px;
  flex-shrink: 0;
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

.issued {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.issued__row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.issued__meta {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  white-space: nowrap;
}

.issued__url {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono, monospace);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
