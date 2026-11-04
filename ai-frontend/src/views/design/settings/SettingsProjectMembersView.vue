<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  PROJECT_NAV,
  PERSONA_LABEL_KEYS,
  SettingsConfirmationDialog,
  SettingsFormSection,
  SettingsPermissionNotice,
  PROJECT_MEMBERS_FIXTURE,
  useSettingsDemo,
  type DemoMember
} from "@/components/settings";
import SettingsFrame from "./SettingsFrame.vue";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Dialog from "@/components/ui/dialog/Dialog.vue";
import Input from "@/components/ui/input/Input.vue";
import Select from "@/components/ui/select/Select.vue";
import Table from "@/components/ui/table/Table.vue";
import TableBody from "@/components/ui/table/TableBody.vue";
import TableCell from "@/components/ui/table/TableCell.vue";
import TableHead from "@/components/ui/table/TableHead.vue";
import TableHeader from "@/components/ui/table/TableHeader.vue";
import TableRow from "@/components/ui/table/TableRow.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 14 — 项目成员（§4C Project Members）。
 * 负责人：改角色 / 添加 / 移除（破坏性确认）；成员与访问者只读。
 * 项目角色独立于租户角色（§9 不得混淆）。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const editable = computed(
  () => demo.projectAccess.value.members === "editable"
);
const personaLabel = computed(() =>
  t(PERSONA_LABEL_KEYS[demo.state.projectPersona])
);

const members = ref<DemoMember[]>(PROJECT_MEMBERS_FIXTURE);
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

const addOpen = ref(false);
const addUser = ref("");

const roles = [
  {
    roleKey: "settings.project.roles.lead",
    descKey: "settings.project.members.roleDescLead"
  },
  {
    roleKey: "settings.project.roles.developer",
    descKey: "settings.project.members.roleDescDeveloper"
  },
  {
    roleKey: "settings.project.roles.viewer",
    descKey: "settings.project.members.roleDescViewer"
  }
] as const;

/** 行内角色下拉选项（负责人可改开发者/访问者角色） */
const roleOptions = [
  {
    value: "settings.project.roles.developer",
    label: t("settings.project.roles.developer")
  },
  {
    value: "settings.project.roles.viewer",
    label: t("settings.project.roles.viewer")
  }
];

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.project.landing.title"),
    to: DESIGN_PATHS.settings.project
  },
  { label: t("settings.nav.projectMembers") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.projectMembers')"
    :scope-label="t('settings.project.landing.title')"
    :entries="PROJECT_NAV"
  >
    <SettingsPermissionNotice
      v-if="!editable"
      access="readonly"
      :role="personaLabel"
    />

    <SettingsFormSection
      :title="t('settings.project.members.membersTitle')"
      :description="t('settings.project.members.membersDesc')"
    >
      <template v-if="editable" #actions>
        <Button variant="primary" size="sm" @click="addOpen = true">
          {{ t("settings.project.members.add") }}
        </Button>
      </template>

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{{ t("settings.personal.profile.email") }}</TableHead>
            <TableHead>{{ t("settings.project.members.tableRole") }}</TableHead>
            <TableHead>{{
              t("settings.tenant.members.tableJoined")
            }}</TableHead>
            <TableHead>{{
              t("settings.tenant.members.tableStatus")
            }}</TableHead>
            <TableHead v-if="editable">
              {{ t("settings.personal.security.actions") }}
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="member in members" :key="member.id">
            <TableCell>
              <span class="project-members__name">{{ member.name }}</span>
              <span class="project-members__email">{{ member.email }}</span>
            </TableCell>
            <TableCell>
              <span
                v-if="
                  member.roleKey === 'settings.project.roles.lead' || !editable
                "
              >
                {{ t(member.roleKey) }}
              </span>
              <Select
                v-else
                :model-value="member.roleKey"
                :options="roleOptions"
                :aria-label="t('settings.project.members.tableRole')"
                class="project-members__role-select"
                @update:model-value="value => (member.roleKey = value)"
              />
            </TableCell>
            <TableCell>{{ member.joinedAt }}</TableCell>
            <TableCell>
              <Badge variant="success" size="sm">
                {{ t("settings.tenant.members.statusActive") }}
              </Badge>
            </TableCell>
            <TableCell v-if="editable">
              <Button
                variant="danger"
                size="sm"
                :disabled="member.roleKey === 'settings.project.roles.lead'"
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
      :title="t('settings.project.members.rolesTitle')"
      :description="t('settings.project.members.rolesDesc')"
    >
      <div class="project-members__roles">
        <div
          v-for="role in roles"
          :key="role.roleKey"
          class="project-members__role"
        >
          <p class="project-members__role-name">{{ t(role.roleKey) }}</p>
          <p class="project-members__role-desc">{{ t(role.descKey) }}</p>
        </div>
      </div>
    </SettingsFormSection>

    <Dialog
      :open="addOpen"
      :title="t('settings.project.members.addTitle')"
      :description="t('settings.project.members.addDesc')"
      size="sm"
      @update:open="addOpen = $event"
    >
      <Input
        v-model="addUser"
        placeholder="name@example.com"
        aria-label="Member"
      />
      <template #footer>
        <div class="project-members__add-actions">
          <Button variant="ghost" size="sm" @click="addOpen = false">
            {{ t("settings.common.cancel") }}
          </Button>
          <Button
            variant="primary"
            size="sm"
            :disabled="!addUser"
            @click="addOpen = false"
          >
            {{ t("settings.project.members.add") }}
          </Button>
        </div>
      </template>
    </Dialog>

    <SettingsConfirmationDialog
      :open="removeOpen"
      :title="t('settings.project.members.removeTitle')"
      :description="t('settings.project.members.removeDesc')"
      :confirm-label="t('settings.common.remove')"
      @update:open="
        value => {
          if (!value) removeTarget = null;
        }
      "
      @confirm="confirmRemove"
    />
  </SettingsFrame>
</template>

<style scoped>
.project-members__name {
  display: block;
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-primary);
}

.project-members__email {
  display: block;
  font-size: var(--ws-font-size-micro);
  color: var(--ws-text-secondary);
}

.project-members__role-select {
  min-width: 8rem;
}

.project-members__roles {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

.project-members__role {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.project-members__role-name {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.project-members__role-desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.project-members__add-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}
</style>
