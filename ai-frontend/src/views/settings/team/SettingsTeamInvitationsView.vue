<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Copy, Link2, Mail } from "lucide-vue-next";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  buildAcceptUrl,
  createInvitationLink,
  createTeamInvitation,
  listInvitationLinks,
  listTeamInvitations,
  revokeInvitationLink,
  revokeTeamInvitation,
  type InvitationCreated,
  type InvitationLink,
  type TeamInvitation
} from "@/api/team";
import { useTeamStore } from "@/stores/team";
import SettingsAppFrame from "../SettingsAppFrame.vue";
import { TEAM_APP_NAV, TEAM_ICONS } from "../nav";

/**
 * 团队设置 — 邀请管理（Invitation Management，P1）。
 * 两段式：邮箱邀请（待处理/历史 + 撤销 + 复制接受链接）与邀请链接
 * （新建 / 撤销 / 用量）。OWNER/ADMIN 可管；MEMBER 由路由层禁入
 * （后端 403 + 侧栏无权限态），本页不加重复守卫。
 */
const { t } = useI18n();
const toast = useToast();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const invitations = ref<TeamInvitation[]>([]);
const links = ref<InvitationLink[]>([]);

const tenantId = computed(() => teamStore.context?.tenant_id ?? "");

const pending = computed(() =>
  invitations.value.filter(item => item.status === "PENDING")
);
const history = computed(() =>
  invitations.value.filter(item => item.status !== "PENDING")
);

const roleOptions = [
  { value: "member", label: "MEMBER" },
  { value: "admin", label: "ADMIN" }
];

// 邮箱邀请新建（单行快捷入口；多行批量在 onboarding 流程）
const draftEmail = ref("");
const draftRole = ref<"member" | "admin">("member");
const creating = ref(false);

// 链接新建
const linkExpiryDays = ref(7);
const linkUnlimited = ref(true);
const linkMaxUses = ref(10);
const creatingLink = ref(false);

function statusVariant(
  status: string
): "success" | "warning" | "danger" | "neutral" {
  if (status === "ACCEPTED") return "success";
  if (status === "PENDING") return "warning";
  if (status === "REVOKED") return "danger";
  return "neutral";
}

function linkStatusVariant(
  status: string
): "success" | "warning" | "danger" | "neutral" {
  if (status === "ACTIVE") return "success";
  if (status === "EXPIRED" || status === "EXHAUSTED") return "warning";
  if (status === "REVOKED") return "danger";
  return "neutral";
}

async function reload(): Promise<void> {
  if (!tenantId.value) return;
  loading.value = true;
  error.value = null;
  try {
    const [invitationRows, linkRows] = await Promise.all([
      listTeamInvitations(tenantId.value),
      listInvitationLinks(tenantId.value)
    ]);
    invitations.value = invitationRows;
    links.value = linkRows;
  } catch (err) {
    error.value = translateError(err);
  } finally {
    loading.value = false;
  }
}

async function submitInvitation(): Promise<void> {
  const email = draftEmail.value.trim();
  if (!email || creating.value) return;
  creating.value = true;
  error.value = null;
  try {
    const created: InvitationCreated = await createTeamInvitation(
      tenantId.value,
      {
        email,
        role: draftRole.value
      }
    );
    draftEmail.value = "";
    try {
      await navigator.clipboard.writeText(buildAcceptUrl(created.accept_path));
      toast.toast({ title: t("team.invitations.createdCopied") });
    } catch {
      toast.toast({ title: t("team.invitations.created") });
    }
    await reload();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    creating.value = false;
  }
}

async function revoke(item: TeamInvitation): Promise<void> {
  try {
    await revokeTeamInvitation(tenantId.value, item.id);
    toast.toast({ title: t("team.invitations.revokedToast") });
    await reload();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  }
}

async function submitLink(): Promise<void> {
  if (creatingLink.value) return;
  creatingLink.value = true;
  error.value = null;
  try {
    const created = await createInvitationLink(tenantId.value, {
      expires_at: new Date(
        Date.now() + linkExpiryDays.value * 24 * 3600 * 1000
      ).toISOString(),
      max_uses: linkUnlimited.value ? null : linkMaxUses.value
    });
    try {
      await navigator.clipboard.writeText(buildAcceptUrl(created.accept_path));
      toast.toast({ title: t("team.invitations.linkCreatedCopied") });
    } catch {
      toast.toast({ title: t("team.invitations.linkCreated") });
    }
    await reload();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    creatingLink.value = false;
  }
}

async function revokeLink(link: InvitationLink): Promise<void> {
  try {
    await revokeInvitationLink(tenantId.value, link.id);
    toast.toast({ title: t("team.invitations.linkRevokedToast") });
    await reload();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  }
}

function usesLabel(link: InvitationLink): string {
  return link.max_uses === null
    ? t("team.invitations.unlimited")
    : t("team.invitations.uses", {
        used: link.used_count,
        total: link.max_uses
      });
}

onMounted(async () => {
  // 团队上下文可能未加载（深链/测试）：先拉 /v1/pm/me 再取本域数据。
  await teamStore.load();
  await reload();
});
</script>

<template>
  <div class="settings-shell-pad">
    <SettingsAppFrame
      :breadcrumb="[
        {
          label: $t('settings.team.landing.title'),
          to: '/settings/team'
        },
        { label: $t('team.invitations.title') }
      ]"
      :title="$t('team.invitations.title')"
      :description="$t('team.invitations.description')"
      :scope-label="$t('settings.team.landing.title')"
      :entries="TEAM_APP_NAV"
      :icons="TEAM_ICONS"
    >
      <p v-if="error" class="ti-error">{{ error }}</p>

      <!-- 邮箱邀请 -->
      <section class="ti-section">
        <h2 class="ti-section__title">
          <Mail :size="15" aria-hidden="true" />
          {{ $t("team.invitations.emailTitle") }}
        </h2>

        <div class="ti-form" data-test="invitations-create">
          <input
            v-model="draftEmail"
            type="email"
            class="ti-input"
            :placeholder="$t('team.invitations.emailPlaceholder')"
            :aria-label="$t('team.invitations.emailLabel')"
            data-test="invitations-email"
            @keyup.enter="submitInvitation"
          />
          <select
            v-model="draftRole"
            class="ti-input ti-input--select"
            :aria-label="$t('team.invitations.roleLabel')"
            data-test="invitations-role"
          >
            <option
              v-for="option in roleOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </option>
          </select>
          <Button
            variant="primary"
            size="sm"
            :loading="creating"
            :disabled="!draftEmail.trim()"
            data-test="invitations-submit"
            @click="submitInvitation"
          >
            {{ $t("team.invitations.invite") }}
          </Button>
        </div>

        <EmptyState
          v-if="pending.length === 0"
          :title="$t('team.invitations.pendingEmpty')"
          size="sm"
        />
        <ul v-else class="ti-list">
          <li
            v-for="item in pending"
            :key="item.id"
            class="ti-row"
            :data-test="`invitation-${item.id}`"
          >
            <span class="ti-row__email">{{ item.email }}</span>
            <Badge variant="neutral">{{ item.role.toUpperCase() }}</Badge>
            <Badge variant="warning">{{
              $t(`team.invitations.status.${item.status}`)
            }}</Badge>
            <span class="ti-row__meta">{{
              $t("team.invitations.invitedBy", { name: item.invited_by_name })
            }}</span>
            <span class="ti-row__meta">{{
              $t("team.invitations.expires", {
                time: item.expires_at.slice(0, 16).replace("T", " ")
              })
            }}</span>
            <Button variant="secondary" size="sm" @click="revoke(item)">
              {{ $t("team.invitations.revoke") }}
            </Button>
          </li>
        </ul>

        <details v-if="history.length" class="ti-history">
          <summary>
            {{ $t("team.invitations.history", { count: history.length }) }}
          </summary>
          <ul class="ti-list">
            <li
              v-for="item in history"
              :key="item.id"
              class="ti-row ti-row--muted"
            >
              <span class="ti-row__email">{{ item.email }}</span>
              <Badge variant="neutral">{{ item.role.toUpperCase() }}</Badge>
              <Badge :variant="statusVariant(item.status)">
                {{ $t(`team.invitations.status.${item.status}`) }}
              </Badge>
              <span v-if="item.accepted_at" class="ti-row__meta">
                {{
                  $t("team.invitations.acceptedAt", {
                    time: item.accepted_at.slice(0, 16).replace("T", " ")
                  })
                }}
              </span>
            </li>
          </ul>
        </details>
      </section>

      <!-- 邀请链接 -->
      <section class="ti-section">
        <h2 class="ti-section__title">
          <Link2 :size="15" aria-hidden="true" />
          {{ $t("team.invitations.linkTitle") }}
        </h2>
        <p class="ti-section__desc">{{ $t("team.invitations.linkDesc") }}</p>

        <div class="ti-form" data-test="links-create">
          <label class="ti-field">
            {{ $t("team.invitations.linkExpiry") }}
            <input
              v-model.number="linkExpiryDays"
              type="number"
              min="1"
              class="ti-input ti-input--num"
              data-test="links-expiry"
            />
          </label>
          <label class="ti-field ti-field--check">
            <input
              v-model="linkUnlimited"
              type="checkbox"
              data-test="links-unlimited"
            />
            {{ $t("team.invitations.unlimited") }}
          </label>
          <input
            v-if="!linkUnlimited"
            v-model.number="linkMaxUses"
            type="number"
            min="1"
            class="ti-input ti-input--num"
            :aria-label="$t('team.invitations.maxUses')"
          />
          <Button
            variant="primary"
            size="sm"
            :loading="creatingLink"
            data-test="links-submit"
            @click="submitLink"
          >
            {{ $t("team.invitations.createLink") }}
          </Button>
        </div>

        <EmptyState
          v-if="links.length === 0"
          :title="$t('team.invitations.linksEmpty')"
          size="sm"
        />
        <ul v-else class="ti-list">
          <li
            v-for="link in links"
            :key="link.id"
            class="ti-row"
            :data-test="`link-${link.id}`"
          >
            <Badge :variant="linkStatusVariant(link.status)">
              {{ $t(`team.invitations.linkStatus.${link.status}`) }}
            </Badge>
            <span class="ti-row__meta">{{ usesLabel(link) }}</span>
            <span class="ti-row__meta">{{
              $t("team.invitations.expires", {
                time: link.expires_at.slice(0, 16).replace("T", " ")
              })
            }}</span>
            <Button
              v-if="link.status === 'ACTIVE'"
              variant="secondary"
              size="sm"
              @click="revokeLink(link)"
            >
              {{ $t("team.invitations.revoke") }}
            </Button>
          </li>
        </ul>
      </section>
    </SettingsAppFrame>
  </div>
</template>

<style scoped>
.ti-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.ti-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ti-section__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ti-section__desc {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.ti-form {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.ti-input {
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
}

.ti-input--select {
  flex-basis: 120px;
}

.ti-input--num {
  flex-basis: 90px;
}

.ti-field {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.ti-field--check {
  gap: var(--space-1);
}

.ti-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ti-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.ti-row--muted {
  opacity: 0.75;
}

.ti-row__email {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
  min-width: 180px;
}

.ti-row__meta {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.ti-history summary {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  cursor: pointer;
}
</style>
