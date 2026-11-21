<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { HttpError } from "@digua/http";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from "@/components/ui/table";
import { listTeamMembers, type TeamMember } from "@/api/team";
import { useAuthStore } from "@/stores/auth";
import { useTeamStore } from "@/stores/team";
import TeamDomainTabs from "@/views/team/TeamDomainTabs.vue";
import TeamRestrictedState from "@/views/team/TeamRestrictedState.vue";
import { formatMembershipDate } from "@/views/team/format-date";

/**
 * 团队成员页（P4-ORG UX 修订 V1.0 / Figma T2「Team members」+ T5 保存态）。
 *
 * 成员表**只读**：Human Principal / Team identity / Joined / Status——不设
 * 行内角色与移除操作（成员变更经邀请流程进入，Owner 调整走 Owner 继任
 * 流程页）。唯一操作 = 「变更 Team Owner」（仅 Team Owner 可见），进入
 * /settings/team/members/owner-succession。资格不足（403）时整页呈现
 * T4 受限态——这是呈现态而非权限控制，服务端同矩阵强制。
 */
const { t, locale } = useI18n();
const route = useRoute();
const auth = useAuthStore();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const restricted = ref(false);
const members = ref<TeamMember[]>([]);

const isOwner = computed(() => teamStore.teamRole === "owner");
const teamId = computed(() => teamStore.context?.team?.id ?? "");
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const organization = computed(() => auth.organization ?? "");

const identityLabel = computed(() =>
  isOwner.value
    ? t("team.members.identityOwner")
    : t("team.members.identityMember")
);

/** T5 保存态：Owner 继任完成后带 query 回跳，显示一次性保存说明 */
const savedFrom = computed(() => String(route.query.owner_changed_from ?? ""));
const savedTo = computed(() => String(route.query.owner_changed_to ?? ""));
const hasSavedNote = computed(
  () => route.query.owner_changed === "1" && savedFrom.value && savedTo.value
);

const activeOwners = computed(() =>
  members.value.filter(member => member.role === "owner" && member.is_active)
);

async function reload(): Promise<void> {
  if (!teamId.value) return;
  loading.value = true;
  error.value = null;
  restricted.value = false;
  try {
    members.value = await listTeamMembers(teamId.value);
  } catch (err) {
    if (err instanceof HttpError && err.status === 403) {
      restricted.value = true;
    } else {
      error.value =
        err instanceof Error ? err.message : t("team.members.loadFailed");
    }
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  // 团队上下文可能未加载（深链/测试）：先拉 /v1/pm/me 再取本域数据。
  await teamStore.load();
  await reload();
});
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: $t('team.tabs.domain') },
      { label: teamName, to: '/dashboard' },
      { label: $t('team.members.title') }
    ]"
    :title="$t('team.members.title')"
    :description="
      organization
        ? $t('team.members.subtitle', { org: organization })
        : undefined
    "
  >
    <TeamDomainTabs />

    <TeamRestrictedState v-if="restricted" />

    <template v-else>
      <p v-if="error" class="tm-error">{{ error }}</p>

      <!-- Owner 治理说明（T2 顶部 Info 横幅） -->
      <Alert variant="info" :title="$t('team.members.ownerBannerTitle')">
        {{ $t("team.members.ownerBannerDetail") }}
      </Alert>

      <!-- 成员表（只读） -->
      <section class="tm-card" data-testid="team-members-card">
        <h2 class="tm-card__title">
          {{ $t("team.members.cardTitle", { count: members.length }) }}
        </h2>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ $t("team.members.colHumanPrincipal") }}</TableHead>
              <TableHead>{{ $t("team.members.colTeamIdentity") }}</TableHead>
              <TableHead>{{ $t("team.members.colJoined") }}</TableHead>
              <TableHead>{{ $t("team.members.colStatus") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="member in members"
              :key="member.principal_id"
              :data-testid="`team-member-row-${member.principal_id}`"
            >
              <TableCell class="tm-name">{{ member.display_name }}</TableCell>
              <TableCell>
                <Badge v-if="member.role === 'owner'" variant="neutral">
                  {{ $t("team.members.identityOwner") }}
                </Badge>
                <span v-else>{{ $t("team.members.identityMember") }}</span>
              </TableCell>
              <TableCell>{{
                formatMembershipDate(member.created_at, locale)
              }}</TableCell>
              <TableCell>
                {{
                  member.is_active
                    ? $t("team.members.statusActive")
                    : $t("team.members.statusInactive")
                }}
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </section>

      <!-- Owner 继任入口（角色即权限：仅 Team Owner）+ 身份说明 -->
      <div class="tm-footer">
        <Button
          v-if="isOwner && activeOwners.length > 0"
          variant="secondary"
          size="sm"
          data-test="team-members-change-owner"
          @click="$router.push('/settings/team/members/owner-succession')"
        >
          {{ $t("team.members.changeOwner") }}
        </Button>
        <span class="tm-footer__note">{{
          $t("team.members.currentIdentity", { identity: identityLabel })
        }}</span>
      </div>

      <!-- T5 保存态：继任完成回跳后的一次性说明 -->
      <p
        v-if="hasSavedNote"
        class="tm-saved"
        data-testid="team-members-saved-note"
      >
        {{
          $t("team.members.savedNote", {
            from: savedFrom,
            to: savedTo
          })
        }}
      </p>
    </template>
  </PageFrame>
</template>

<style scoped>
.tm-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.tm-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.tm-card__title {
  margin: 0 0 var(--space-4);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.tm-name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.tm-footer {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.tm-footer__note {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.tm-saved {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}
</style>
