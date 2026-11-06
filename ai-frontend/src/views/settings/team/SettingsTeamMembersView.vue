<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Lock, ShieldAlert } from "lucide-vue-next";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Select } from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from "@/components/ui/table";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  listTeamMembers,
  removeTeamMember,
  setTeamMemberRole,
  transferTeamOwnership,
  type TeamMember
} from "@/api/team";
import { useTeamStore } from "@/stores/team";
import SettingsAppFrame from "../SettingsAppFrame.vue";
import { TEAM_APP_NAV, TEAM_ICONS } from "../nav";

/**
 * 团队设置 — 成员（Team Members，P0）。
 * 角色即权限（UI 细则 #3）：操作入口按当前用户 tenant_role 渲染，服务端同
 * 矩阵强制——OWNER 可改 admin/member 角色、移除成员、转移所有权；ADMIN 仅可
 * 管理 member 行（owner 行禁用并说明「需要 Owner 权限」）；MEMBER 整页只读
 * （操作列整体呈现无权限态说明）。末位 Owner 移除/降级由服务端拒绝并回显。
 */
const { t } = useI18n();
const toast = useToast();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const members = ref<TeamMember[]>([]);
const busyId = ref<string | null>(null);

const isAdmin = computed(() => teamStore.isTeamAdmin);
const isOwner = computed(() => teamStore.teamRole === "owner");
const tenantId = computed(() => teamStore.context?.tenant_id ?? "");
const selfId = computed(() => teamStore.context?.principal_id ?? "");

/** 可转移所有权的目标（必须已是 ACTIVE 成员；owner 行之外） */
const transferTargets = computed(() =>
  members.value.filter(
    member =>
      member.role !== "owner" &&
      member.is_active &&
      member.principal_id !== selfId.value
  )
);

const roleOptions = [
  { value: "member", label: "MEMBER" },
  { value: "admin", label: "ADMIN" }
];

/** 角色徽标语义色：owner=accent / admin=info / member=neutral */
function roleVariant(role: string): "accent" | "info" | "neutral" {
  if (role === "owner") return "accent";
  if (role === "admin") return "info";
  return "neutral";
}

/** 行内角色下拉是否可用：仅 owner 可改他人（owner 行不可自改） */
function canEditRole(member: TeamMember): boolean {
  if (!isOwner.value) return false;
  return member.role !== "owner";
}

/** 移除按钮可用：owner 全员（除自己）；admin 仅 member 行 */
function canRemove(member: TeamMember): boolean {
  if (member.principal_id === selfId.value) return false;
  if (isOwner.value) return true;
  return isAdmin.value && member.role === "member";
}

function denyReason(member: TeamMember): string | null {
  if (!isAdmin.value) return t("team.members.deny.memberReadonly");
  if (member.role === "owner") return t("team.members.deny.ownerRow");
  return null;
}

// ── 移除确认 ────────────────────────────────────────────────
const removeTarget = ref<TeamMember | null>(null);

// ── 转移所有权确认（P1：Team Settings 与本页共用端点） ─────────
const transferTarget = ref<TeamMember | null>(null);

async function reload(): Promise<void> {
  if (!tenantId.value) return;
  loading.value = true;
  error.value = null;
  try {
    members.value = await listTeamMembers(tenantId.value);
  } catch (err) {
    error.value = translateError(err);
  } finally {
    loading.value = false;
  }
}

async function changeRole(member: TeamMember, role: string): Promise<void> {
  busyId.value = member.principal_id;
  try {
    await setTeamMemberRole(
      tenantId.value,
      member.principal_id,
      role === "admin" ? "admin" : "member"
    );
    toast.toast({
      title: t("team.members.roleChangedToast", { name: member.display_name })
    });
    await reload();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busyId.value = null;
  }
}

async function confirmRemove(): Promise<void> {
  const target = removeTarget.value;
  if (!target) return;
  busyId.value = target.principal_id;
  try {
    await removeTeamMember(tenantId.value, target.principal_id);
    toast.toast({
      title: t("team.members.removedToast", { name: target.display_name })
    });
    removeTarget.value = null;
    await reload();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busyId.value = null;
  }
}

async function confirmTransfer(): Promise<void> {
  const target = transferTarget.value;
  if (!target) return;
  busyId.value = target.principal_id;
  try {
    await transferTeamOwnership(tenantId.value, target.principal_id);
    toast.toast({
      title: t("team.members.transferredToast", { name: target.display_name })
    });
    transferTarget.value = null;
    await Promise.all([reload(), teamStore.refresh()]);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busyId.value = null;
  }
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
          label: $t('settings.personal.landing.title'),
          to: '/settings/personal'
        },
        { label: $t('team.members.title') }
      ]"
      :title="$t('team.members.title')"
      :description="$t('team.members.description')"
      :scope-label="$t('settings.team.landing.title')"
      :entries="TEAM_APP_NAV"
      :icons="TEAM_ICONS"
    >
      <p v-if="error" class="tm-error">{{ error }}</p>

      <!-- MEMBER 视角：整页只读的显式说明（不是隐形禁用） -->
      <p
        v-if="!isAdmin"
        class="tm-readonly"
        data-testid="team-members-readonly-note"
      >
        <Lock :size="13" aria-hidden="true" />
        {{ $t("team.members.readonlyNote") }}
      </p>

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{{ $t("team.members.colMember") }}</TableHead>
            <TableHead>{{ $t("team.members.colRole") }}</TableHead>
            <TableHead>{{ $t("team.members.colStatus") }}</TableHead>
            <TableHead>{{ $t("team.members.colJoined") }}</TableHead>
            <TableHead class="tm-col-actions">
              {{
                isAdmin
                  ? $t("team.members.colActions")
                  : $t("team.members.colActionsLocked")
              }}
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="member in members" :key="member.principal_id">
            <TableCell>
              <span class="tm-name">{{ member.display_name }}</span>
              <span v-if="member.principal_id === selfId" class="tm-self">
                {{ $t("team.members.you") }}
              </span>
            </TableCell>
            <TableCell>
              <Badge :variant="roleVariant(member.role)">
                {{ member.role.toUpperCase() }}
              </Badge>
            </TableCell>
            <TableCell>
              <Badge :variant="member.is_active ? 'success' : 'warning'">
                {{
                  member.is_active
                    ? $t("team.members.statusActive")
                    : $t("team.members.statusInactive")
                }}
              </Badge>
            </TableCell>
            <TableCell>{{ member.created_at?.slice(0, 10) }}</TableCell>
            <TableCell class="tm-col-actions">
              <div v-if="isAdmin" class="tm-actions">
                <template v-if="canEditRole(member)">
                  <Select
                    class="tm-role-select"
                    :model-value="member.role"
                    :options="roleOptions"
                    :disabled="busyId === member.principal_id"
                    :aria-label="
                      $t('team.members.changeRole', {
                        name: member.display_name
                      })
                    "
                    @update:model-value="
                      value => changeRole(member, String(value))
                    "
                  />
                </template>
                <span
                  v-else-if="denyReason(member)"
                  class="tm-deny"
                  :title="denyReason(member) ?? undefined"
                >
                  <ShieldAlert :size="13" aria-hidden="true" />
                  {{ denyReason(member) }}
                </span>

                <Button
                  v-if="canRemove(member)"
                  variant="danger"
                  size="sm"
                  :disabled="busyId === member.principal_id"
                  :data-test="`team-member-remove-${member.principal_id}`"
                  @click="removeTarget = member"
                >
                  {{ $t("team.members.remove") }}
                </Button>
                <Button
                  v-if="
                    isOwner &&
                    member.role !== 'owner' &&
                    member.is_active &&
                    member.principal_id !== selfId
                  "
                  variant="secondary"
                  size="sm"
                  :disabled="busyId === member.principal_id"
                  :data-test="`team-member-transfer-${member.principal_id}`"
                  @click="transferTarget = member"
                >
                  {{ $t("team.members.makeOwner") }}
                </Button>
              </div>
              <span v-else class="tm-deny">
                <Lock :size="13" aria-hidden="true" />
                {{ $t("team.members.deny.memberReadonly") }}
              </span>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>

      <!-- 移除确认 -->
      <Dialog
        :open="removeTarget !== null"
        :title="
          $t('team.members.removeConfirmTitle', {
            name: removeTarget?.display_name
          })
        "
        :description="$t('team.members.removeConfirmDesc')"
        size="sm"
        @update:open="
          value => {
            if (!value) removeTarget = null;
          }
        "
      >
        <template #footer>
          <Button variant="secondary" size="sm" @click="removeTarget = null">
            {{ $t("team.members.cancel") }}
          </Button>
          <Button
            variant="danger"
            size="sm"
            :loading="
              removeTarget !== null && busyId === removeTarget.principal_id
            "
            data-test="team-member-remove-confirm"
            @click="confirmRemove"
          >
            {{ $t("team.members.remove") }}
          </Button>
        </template>
      </Dialog>

      <!-- 转移所有权确认 -->
      <Dialog
        :open="transferTarget !== null"
        :title="
          $t('team.members.transferTitle', {
            name: transferTarget?.display_name
          })
        "
        :description="$t('team.members.transferDesc')"
        size="sm"
        @update:open="
          value => {
            if (!value) transferTarget = null;
          }
        "
      >
        <template #footer>
          <Button variant="secondary" size="sm" @click="transferTarget = null">
            {{ $t("team.members.cancel") }}
          </Button>
          <Button
            variant="primary"
            size="sm"
            :loading="
              transferTarget !== null && busyId === transferTarget.principal_id
            "
            data-test="team-member-transfer-confirm"
            @click="confirmTransfer"
          >
            {{ $t("team.members.transferConfirm") }}
          </Button>
        </template>
      </Dialog>
    </SettingsAppFrame>
  </div>
</template>

<style scoped>
.tm-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.tm-readonly {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.tm-name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.tm-self {
  margin-left: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.tm-col-actions {
  width: 40%;
}

.tm-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.tm-role-select {
  flex-basis: 120px;
}

.tm-deny {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
