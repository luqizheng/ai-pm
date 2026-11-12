<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Info,
  Link2,
  Lock,
  Mail,
  MailPlus,
  ShieldAlert
} from "lucide-vue-next";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { EmptyState } from "@/components/ui/empty-state";
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
  buildAcceptUrl,
  createInvitationLink,
  createTeamInvitation,
  listInvitationLinks,
  listTeamInvitations,
  listTeamMembers,
  removeTeamMember,
  revokeInvitationLink,
  revokeTeamInvitation,
  setTeamMemberRole,
  transferTeamOwnership,
  type InvitationCreated,
  type InvitationLink,
  type TeamInvitation,
  type TeamMember
} from "@/api/team";
import { useTeamStore } from "@/stores/team";

/**
 * 团队成员管理（/admin/members，P0）。
 *
 * 角色即权限（UI 细则 #3）：操作入口按当前用户 tenant_role 渲染，服务端同
 * 矩阵强制——OWNER 可改 admin/member 角色、移除成员、转移所有权；ADMIN 仅可
 * 管理 member 行（owner 行禁用并说明「需要 Owner 权限」）；MEMBER 整页只读
 * （操作列整体呈现无权限态说明）。末位 Owner 移除/降级由服务端拒绝并回显。
 *
 * 邀请功能（原独立 /settings/team/invitations 页）合并为右上 Invite 按钮 →
 * 弹 Dialog：邮箱邀请 + 邀请链接（P1）。
 */
const { t } = useI18n();
const toast = useToast();
const teamStore = useTeamStore();

// ── 成员列表 ────────────────────────────────────────────────
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

// ── 转移所有权确认 ───────────────────────────────────────────
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

// ── Invite Dialog（原 SettingsTeamInvitationsView 业务逻辑合并） ──
const inviteOpen = ref(false);
const inviteLoading = ref(false);
const inviteError = ref<string | null>(null);
const invitations = ref<TeamInvitation[]>([]);
const links = ref<InvitationLink[]>([]);

const pending = computed(() =>
  invitations.value.filter(item => item.status === "PENDING")
);
const history = computed(() =>
  invitations.value.filter(item => item.status !== "PENDING")
);

const roleOptionsForInvite = [
  { value: "member", label: "MEMBER" },
  { value: "admin", label: "ADMIN" }
];

const draftEmail = ref("");
const draftRole = ref<"member" | "admin">("member");
const creatingInvite = ref(false);

const linkExpiryDays = ref(7);
const linkUnlimited = ref(true);
const linkMaxUses = ref(10);
const creatingLink = ref(false);

async function openInvite(): Promise<void> {
  inviteOpen.value = true;
  await reloadInviteData();
}

async function reloadInviteData(): Promise<void> {
  if (!tenantId.value) return;
  inviteLoading.value = true;
  inviteError.value = null;
  try {
    const [invitationRows, linkRows] = await Promise.all([
      listTeamInvitations(tenantId.value),
      listInvitationLinks(tenantId.value)
    ]);
    invitations.value = invitationRows;
    links.value = linkRows;
  } catch (err) {
    inviteError.value = translateError(err);
  } finally {
    inviteLoading.value = false;
  }
}

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

async function submitInvitation(): Promise<void> {
  const email = draftEmail.value.trim();
  if (!email || creatingInvite.value) return;
  creatingInvite.value = true;
  inviteError.value = null;
  try {
    const created: InvitationCreated = await createTeamInvitation(
      tenantId.value,
      { email, role: draftRole.value }
    );
    draftEmail.value = "";
    try {
      await navigator.clipboard.writeText(buildAcceptUrl(created.accept_path));
      toast.toast({ title: t("team.invitations.createdCopied") });
    } catch {
      toast.toast({ title: t("team.invitations.created") });
    }
    await reloadInviteData();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    creatingInvite.value = false;
  }
}

async function revokeInvite(item: TeamInvitation): Promise<void> {
  try {
    await revokeTeamInvitation(tenantId.value, item.id);
    toast.toast({ title: t("team.invitations.revokedToast") });
    await reloadInviteData();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  }
}

async function submitLink(): Promise<void> {
  if (creatingLink.value) return;
  creatingLink.value = true;
  inviteError.value = null;
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
    await reloadInviteData();
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
    await reloadInviteData();
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
  <PageFrame
    :title="$t('team.members.title')"
    :description="$t('team.members.description')"
  >
    <template #actions>
      <Button
        v-if="isAdmin"
        variant="primary"
        size="sm"
        :data-test="'admin-members-invite'"
        @click="openInvite"
      >
        <MailPlus :size="15" aria-hidden="true" />
        {{ $t("team.invitations.title") }}
      </Button>
    </template>

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

    <!-- Invite Dialog（邮箱邀请 + 邀请链接） -->
    <Dialog
      :open="inviteOpen"
      :title="$t('team.invitations.title')"
      :description="$t('team.invitations.description')"
      size="lg"
      @update:open="value => { if (!value) inviteOpen = false; }"
    >
      <p v-if="inviteError" class="tm-error">{{ inviteError }}</p>

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
              v-for="option in roleOptionsForInvite"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </option>
          </select>
          <Button
            variant="primary"
            size="sm"
            :loading="creatingInvite"
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
            <Button variant="secondary" size="sm" @click="revokeInvite(item)">
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
    </Dialog>
  </PageFrame>
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

/* ── 邀请区（原 InvitationsView 样式） ── */
.ti-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ti-section + .ti-section {
  margin-top: var(--space-5);
  padding-top: var(--space-5);
  border-top: 1px solid var(--border-subtle);
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
