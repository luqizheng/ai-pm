<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  TENANT_NAV,
  SettingsConfirmationDialog,
  SettingsFormSection,
  SettingsSectionHeader,
  TENANT_MEMBERS_FIXTURE,
  useSettingsDemo,
  type DemoMember
} from "@/components/settings";
import SettingsFrame from "./SettingsFrame.vue";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Dialog from "@/components/ui/dialog/Dialog.vue";
import Input from "@/components/ui/input/Input.vue";
import Table from "@/components/ui/table/Table.vue";
import TableBody from "@/components/ui/table/TableBody.vue";
import TableCell from "@/components/ui/table/TableCell.vue";
import TableHead from "@/components/ui/table/TableHead.vue";
import TableHeader from "@/components/ui/table/TableHeader.vue";
import TableRow from "@/components/ui/table/TableRow.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 10 — 租户成员与访问（§4B Members & Access）。
 * 成员表（角色/状态/移除=破坏性确认）+ 邀请（弹窗表单）
 * + 租户角色说明；无授权走 denied 态。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const denied = computed(() => demo.tenantAccess.value === "hidden");

const members = ref<DemoMember[]>(TENANT_MEMBERS_FIXTURE);
const removeTarget = ref<DemoMember | null>(null);
const removeOpen = computed(() => removeTarget.value !== null);

function confirmRemove() {
  if (removeTarget.value) {
    members.value = members.value.filter(
      member => member.id !== removeTarget.value?.id
    );
  }
  removeTarget.value = null;
}

const inviteOpen = ref(false);
const inviteEmail = ref("");

const roles = [
  {
    roleKey: "settings.tenant.roles.owner",
    descKey: "settings.tenant.members.roleDescOwner"
  },
  {
    roleKey: "settings.tenant.roles.admin",
    descKey: "settings.tenant.members.roleDescAdmin"
  },
  {
    roleKey: "settings.tenant.roles.member",
    descKey: "settings.tenant.members.roleDescMember"
  }
] as const;

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.tenant.landing.title"),
    to: DESIGN_PATHS.settings.tenant
  },
  { label: t("settings.nav.members") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.members')"
    :scope-label="t('settings.tenant.landing.title')"
    :entries="TENANT_NAV"
    :denied="denied"
    persona="tenant-member"
  >
    <template v-if="!denied">
      <SettingsFormSection
        :title="t('settings.tenant.members.membersTitle')"
        :description="t('settings.tenant.members.membersDesc')"
      >
        <template #actions>
          <Button variant="primary" size="sm" @click="inviteOpen = true">
            {{ t("settings.tenant.members.invite") }}
          </Button>
        </template>

        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("settings.personal.profile.email") }}</TableHead>
              <TableHead>{{
                t("settings.tenant.members.tableRole")
              }}</TableHead>
              <TableHead>{{
                t("settings.tenant.members.tableJoined")
              }}</TableHead>
              <TableHead>{{
                t("settings.tenant.members.tableStatus")
              }}</TableHead>
              <TableHead>{{
                t("settings.personal.security.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="member in members" :key="member.id">
              <TableCell>
                <span class="tenant-members__name">{{ member.name }}</span>
                <span class="tenant-members__email">{{ member.email }}</span>
              </TableCell>
              <TableCell>{{ t(member.roleKey) }}</TableCell>
              <TableCell>{{ member.joinedAt }}</TableCell>
              <TableCell>
                <Badge
                  :variant="member.status === 'active' ? 'success' : 'info'"
                  size="sm"
                >
                  {{
                    member.status === "active"
                      ? t("settings.tenant.members.statusActive")
                      : t("settings.tenant.members.statusInvited")
                  }}
                </Badge>
              </TableCell>
              <TableCell>
                <Button
                  variant="danger"
                  size="sm"
                  @click="removeTarget = member"
                >
                  {{ t("settings.common.remove") }}
                </Button>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </SettingsFormSection>

      <SettingsFormSection
        :title="t('settings.tenant.members.rolesTitle')"
        :description="t('settings.tenant.members.rolesDesc')"
      >
        <div class="tenant-members__roles">
          <div
            v-for="role in roles"
            :key="role.roleKey"
            class="tenant-members__role"
          >
            <p class="tenant-members__role-name">{{ t(role.roleKey) }}</p>
            <p class="tenant-members__role-desc">{{ t(role.descKey) }}</p>
          </div>
        </div>
      </SettingsFormSection>

      <Dialog
        :open="inviteOpen"
        :title="t('settings.tenant.members.inviteTitle')"
        :description="t('settings.tenant.members.inviteDesc')"
        size="sm"
        @update:open="inviteOpen = $event"
      >
        <Input
          v-model="inviteEmail"
          type="email"
          placeholder="name@example.com"
          aria-label="Email"
        />
        <template #footer>
          <div class="tenant-members__invite-actions">
            <Button variant="ghost" size="sm" @click="inviteOpen = false">
              {{ t("settings.common.cancel") }}
            </Button>
            <Button
              variant="primary"
              size="sm"
              :disabled="!inviteEmail"
              @click="inviteOpen = false"
            >
              {{ t("settings.tenant.members.invite") }}
            </Button>
          </div>
        </template>
      </Dialog>

      <SettingsConfirmationDialog
        :open="removeOpen"
        :title="t('settings.tenant.members.removeTitle')"
        :description="t('settings.tenant.members.removeDesc')"
        :confirm-label="t('settings.common.remove')"
        @update:open="
          value => {
            if (!value) removeTarget = null;
          }
        "
        @confirm="confirmRemove"
      />
    </template>
  </SettingsFrame>
</template>

<style scoped>
.tenant-members__name {
  display: block;
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-primary);
}

.tenant-members__email {
  display: block;
  font-size: var(--ws-font-size-micro);
  color: var(--ws-text-secondary);
}

.tenant-members__roles {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

.tenant-members__role {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.tenant-members__role-name {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.tenant-members__role-desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.tenant-members__invite-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}
</style>
