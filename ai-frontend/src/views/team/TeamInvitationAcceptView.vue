<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { CheckCircle2, XCircle } from "lucide-vue-next";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import { getAccessToken, getStoredUser } from "@/api/base/token-storage";
import {
  acceptInvitationLink,
  acceptTeamInvitation,
  invitationErrorKey,
  previewInvitation,
  previewInvitationLink,
  type InvitationPreview,
  type LinkPreview
} from "@/api/team";
import { translateError } from "@/api/base";
/**
 * Screens 06+07 — Join Team / Accept Invitation & Invitation Result（设计稿 §4）。
 * 入口：/invitation/accept?token=…&type=invite|link（邮件邀请与链接同一接受面）。
 * 未登录 → 跳登录并携带 redirect 回跳本页（接受动作必须来自认证上下文）。
 * 结果态七种 + 预览无效（expired/revoked/invalid/alreadyAccepted/exhausted/
 * emailMismatch/tenantMismatch/teamSuspended），每态配明确下一步。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const toast = useToast();

type ResultState =
  | "success"
  | "alreadyMember"
  | "expired"
  | "revoked"
  | "invalid"
  | "alreadyAccepted"
  | "exhausted"
  | "emailMismatch"
  | "tenantMismatch"
  | "teamSuspended";

const inviteType = computed(() =>
  route.query.type === "link" ? "link" : "invite"
);
const token = computed(() => String(route.query.token ?? ""));

const loading = ref(true);
const preview = ref<InvitationPreview | LinkPreview | null>(null);
const acceptEmail = ref<string | null>(null);
const accepting = ref(false);
const result = ref<{
  state: ResultState;
  teamName?: string;
  role?: string;
} | null>(null);

const successTeam = computed(() => result.value?.teamName ?? "");
const successRole = computed(() => result.value?.role ?? "");
const isLink = computed(() => inviteType.value === "link");

/** 预览元信息（两种类型字段名不同，统一读取） */
const previewMeta = computed(() => {
  if (!preview.value) return null;
  const link = preview.value as LinkPreview;
  const inv = preview.value as InvitationPreview;
  return {
    teamName: inv.team_name ?? link.team_name,
    role: isLink.value ? link.default_role : inv.role,
    inviterName: inv.inviter_name ?? null,
    email: inv.email ?? null,
    status: preview.value.status,
    expiresAt: preview.value.expires_at,
    remainingUses: link.remaining_uses ?? null
  };
});

/** 预览即失效（lazy EXPIRED/EXHAUSTED 之外的终态直接呈现结果面） */
const previewBlocked = computed<ResultState | null>(() => {
  if (!preview.value) return "invalid";
  switch (preview.value.status) {
    case "EXPIRED":
      return "expired";
    case "REVOKED":
      return "revoked";
    case "EXHAUSTED":
      return "exhausted";
    case "ACCEPTED":
      return "alreadyAccepted";
    default:
      return null;
  }
});

async function load(): Promise<void> {
  // 未登录：先登录再回来（接受必须从认证上下文取身份，设计稿 §4）。
  if (!getAccessToken()) {
    void router.replace({
      path: "/auth/login",
      query: { redirect: route.fullPath }
    });
    return;
  }
  loading.value = true;
  try {
    // email 预填：iam 登录响应的账号 email（JWT 无 email claim）。
    acceptEmail.value = getStoredUser()?.email ?? null;
    preview.value =
      inviteType.value === "link"
        ? await previewInvitationLink(token.value)
        : await previewInvitation(token.value);
  } catch {
    // 预览失败（网络/无效 token）统一呈现 invalid 结果面。
    result.value = { state: "invalid" };
    preview.value = null;
  } finally {
    loading.value = false;
  }
}

async function accept(): Promise<void> {
  if (accepting.value) return;
  accepting.value = true;
  try {
    const outcome =
      inviteType.value === "link"
        ? await acceptInvitationLink({ token: token.value })
        : await acceptTeamInvitation({
            token: token.value,
            email: acceptEmail.value ?? undefined
          });
    result.value =
      outcome.outcome === "joined"
        ? { state: "success", teamName: outcome.team_name, role: outcome.role }
        : {
            state: "alreadyMember",
            teamName: outcome.team_name,
            role: outcome.role
          };
    toast.toast({
      title: t("team.accept.joinedToast", { team: outcome.team_name })
    });
  } catch (err) {
    const message = translateError(err);
    result.value = { state: invitationErrorKey(message) as ResultState };
  } finally {
    accepting.value = false;
  }
}

function decline(): void {
  // 拒绝仅离开本页（V1 邀请单保持 PENDING，过期自然失效）。
  void router.push("/personal");
}

function goDashboard(): void {
  void router.push("/dashboard");
}

function goHome(): void {
  void router.push("/personal");
}

onMounted(load);
</script>

<template>
  <div class="accept">
    <div class="accept__card">
      <template v-if="loading">
        <p class="accept__hint">{{ t("team.accept.loading") }}</p>
      </template>

      <!-- 结果面（Screen 07）：成功与七种异常，各配下一步 -->
      <template v-else-if="result">
        <div
          class="accept__result"
          :data-state="result.state"
          data-testid="accept-result"
        >
          <CheckCircle2
            v-if="
              result.state === 'success' || result.state === 'alreadyMember'
            "
            :size="28"
            aria-hidden="true"
          />
          <XCircle v-else :size="28" aria-hidden="true" />
          <h1 class="accept__title">
            {{ t(`team.accept.result.${result.state}.title`) }}
          </h1>
          <p class="accept__desc">
            {{
              t(`team.accept.result.${result.state}.desc`, {
                team: result.teamName ?? "",
                role: result.role ?? ""
              })
            }}
          </p>
        </div>
        <div class="accept__actions">
          <Button
            v-if="
              result.state === 'success' || result.state === 'alreadyMember'
            "
            variant="primary"
            data-testid="accept-continue"
            @click="goDashboard"
          >
            {{ t("team.accept.continueDashboard") }}
          </Button>
          <Button
            v-else
            variant="secondary"
            data-testid="accept-back"
            @click="goHome"
          >
            {{ t("team.accept.backHome") }}
          </Button>
        </div>
      </template>

      <!-- 预览面（Screen 06） -->
      <template v-else-if="previewBlocked">
        <div class="accept__result" data-testid="accept-preview-blocked">
          <XCircle :size="28" aria-hidden="true" />
          <h1 class="accept__title">
            {{ t(`team.accept.result.${previewBlocked}.title`) }}
          </h1>
          <p class="accept__desc">
            {{ t(`team.accept.result.${previewBlocked}.desc`, { team: "" }) }}
          </p>
        </div>
        <div class="accept__actions">
          <Button variant="secondary" @click="goHome">
            {{ t("team.accept.backHome") }}
          </Button>
        </div>
      </template>

      <template v-else-if="previewMeta">
        <span class="accept__avatar" aria-hidden="true">{{
          previewMeta.teamName.slice(0, 2).toUpperCase()
        }}</span>
        <h1 class="accept__title">{{ previewMeta.teamName }}</h1>
        <p class="accept__desc">
          {{
            t("team.accept.invitedYou", {
              inviter: previewMeta.inviterName ?? ""
            })
          }}
        </p>

        <dl class="facts">
          <div class="facts__row">
            <dt>{{ t("team.accept.factRole") }}</dt>
            <dd>
              <Badge variant="accent">{{
                previewMeta.role.toUpperCase()
              }}</Badge>
            </dd>
          </div>
          <div class="facts__row">
            <dt>{{ t("team.accept.factType") }}</dt>
            <dd>
              {{
                isLink ? t("team.accept.typeLink") : t("team.accept.typeInvite")
              }}
            </dd>
          </div>
          <div v-if="!isLink && previewMeta.email" class="facts__row">
            <dt>{{ t("team.accept.factEmail") }}</dt>
            <dd>{{ previewMeta.email }}</dd>
          </div>
          <div class="facts__row">
            <dt>{{ t("team.accept.factExpires") }}</dt>
            <dd>{{ previewMeta.expiresAt }}</dd>
          </div>
          <div v-if="previewMeta.remainingUses !== null" class="facts__row">
            <dt>{{ t("team.accept.factUses") }}</dt>
            <dd>{{ previewMeta.remainingUses }}</dd>
          </div>
        </dl>

        <p v-if="!isLink" class="accept__emailHint">
          {{ t("team.accept.emailHint") }}
        </p>

        <div class="accept__actions">
          <Button variant="secondary" :disabled="accepting" @click="decline">
            {{ t("team.accept.decline") }}
          </Button>
          <Button
            variant="primary"
            :loading="accepting"
            data-testid="accept-confirm"
            @click="accept"
          >
            {{ t("team.accept.confirm") }}
          </Button>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.accept {
  min-height: 100vh;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: var(--space-8) var(--space-5);
  background: var(--surface-sunken);
  color: var(--text-primary);
  overflow-y: auto;
}

.accept__card {
  width: 100%;
  max-width: 460px;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-6);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  background: var(--surface-raised);
}

.accept__avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--space-12);
  height: var(--space-12);
  border-radius: var(--radius-md);
  background: var(--accent-surface);
  color: var(--accent);
  font-family: var(--font-ui);
  font-size: var(--font-size-h4);
  font-weight: var(--weight-semibold);
}

.accept__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h4);
  font-weight: var(--weight-semibold);
}

.accept__desc {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
}

.accept__result {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  text-align: center;
  padding: var(--space-4) 0;
}

.accept__result svg {
  color: var(--ws-color-success);
}

.accept__result[data-state="invalid"] svg,
.accept__result[data-state="expired"] svg,
.accept__result[data-state="revoked"] svg,
.accept__result[data-state="exhausted"] svg,
.accept__result[data-state="emailMismatch"] svg,
.accept__result[data-state="tenantMismatch"] svg,
.accept__result[data-state="teamSuspended"] svg,
.accept__result[data-state="alreadyAccepted"] svg {
  color: var(--ws-color-danger);
}

.facts {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.facts__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.facts__row dt {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.facts__row dd {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.accept__emailHint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-tertiary);
}

.accept__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
}

.accept__hint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-tertiary);
}
</style>
