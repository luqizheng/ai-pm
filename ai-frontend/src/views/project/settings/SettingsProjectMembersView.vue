<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw, UserPlus } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import SettingsAppFrame from "@/views/settings/SettingsAppFrame.vue";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  grantProjectMember,
  listPrincipals,
  listProjectMembers,
  revokeProjectMember,
  transferProjectOwnership,
  type PrincipalOption,
  type ProjectMember
} from "@/api/projects";
import { useProjectSettingsFrame } from "./shared";

/**
 * Project Setting · Members 节（P1 落地，替换 proposed 占位页）。
 *
 * 全部走既有授权面（core.project_member 唯一写入路径 project_members.rs）：
 * 列表（GET members）、添加（POST，角色固定 member——owner 只能走所有权
 * 转移）、移除（DELETE，确认对话框）、所有权转移（POST owner-transfer，
 * 确认对话框）。成员资格 = Product Member 授权面（P4-ORG），本页不做
 * 邀请/注册流（租户主体管理在 Team 管理面，不复制语义）。
 */
const { t } = useI18n();
const toast = useToast();
const settingsFrame = useProjectSettingsFrame();

const loading = ref(true);
const error = ref<string | null>(null);
const members = ref<ProjectMember[]>([]);
const acting = ref(false);

const addOpen = ref(false);
const candidates = ref<PrincipalOption[]>([]);
const candidateId = ref("");
const candidatesLoading = ref(false);

const removing = ref<ProjectMember | null>(null);
const transferring = ref<ProjectMember | null>(null);

const memberOptions = computed(() =>
  candidates.value.filter(
    candidate =>
      candidate.is_active &&
      !members.value.some(m => m.principal_id === candidate.id)
  )
);

async function load(): Promise<void> {
  if (!settingsFrame.projectId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const res = await listProjectMembers(settingsFrame.projectId.value);
    members.value = res.items ?? [];
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

/** 添加成员走内联表单（授予非危险操作，无需对话框）；候选=租户活跃主体。 */
async function toggleAdd(): Promise<void> {
  addOpen.value = !addOpen.value;
  candidateId.value = "";
  if (!addOpen.value) return;
  candidatesLoading.value = true;
  try {
    const res = await listPrincipals({ is_active: true });
    candidates.value = res.items ?? [];
  } catch {
    candidates.value = [];
  } finally {
    candidatesLoading.value = false;
  }
}

async function grant(): Promise<void> {
  const projectId = settingsFrame.projectId.value;
  if (!projectId || !candidateId.value || acting.value) return;
  acting.value = true;
  try {
    await grantProjectMember(projectId, candidateId.value);
    toast.toast({ title: t("settings.project.members.grantedToast") });
    addOpen.value = false;
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

async function removeMember(): Promise<void> {
  const projectId = settingsFrame.projectId.value;
  const target = removing.value;
  if (!projectId || !target || acting.value) return;
  acting.value = true;
  try {
    await revokeProjectMember(projectId, target.principal_id);
    toast.toast({ title: t("settings.project.members.removedToast") });
    removing.value = null;
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

async function transfer(): Promise<void> {
  const projectId = settingsFrame.projectId.value;
  const target = transferring.value;
  if (!projectId || !target || acting.value) return;
  acting.value = true;
  try {
    await transferProjectOwnership(projectId, target.principal_id);
    toast.toast({ title: t("settings.project.members.transferredToast") });
    transferring.value = null;
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

function roleBadgeVariant(role: string): "accent" | "neutral" {
  return role === "owner" ? "accent" : "neutral";
}

onMounted(load);
watch(() => settingsFrame.projectId.value, load);
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="settingsFrame.breadcrumb(t('settings.nav.projectMembers'))"
    :title="t('settings.nav.projectMembers')"
    :description="t('settings.project.members.description')"
    :scope-label="settingsFrame.scopeLabel.value"
    :entries="settingsFrame.navEntries.value"
    :icons="settingsFrame.navIcons"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("settings.project.members.refresh") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        :disabled="!settingsFrame.projectId.value"
        data-testid="pm-add-toggle"
        @click="toggleAdd"
      >
        <UserPlus :size="14" aria-hidden="true" />
        {{ t("settings.project.members.addAction") }}
      </Button>
    </template>

    <EmptyState
      v-if="!settingsFrame.projectId.value"
      :title="t('settings.project.general.noProject')"
    />
    <EmptyState v-else-if="error" :title="error" />
    <EmptyState
      v-else-if="loading && members.length === 0"
      :title="t('settings.project.general.loading')"
    />
    <EmptyState
      v-else-if="members.length === 0"
      :title="t('settings.project.members.emptyTitle')"
    />
    <div v-else class="pm-body" data-testid="pm-body">
      <table class="pm-table">
        <thead>
          <tr>
            <th>{{ t("settings.project.members.colMember") }}</th>
            <th>{{ t("settings.project.members.colRole") }}</th>
            <th>{{ t("settings.project.members.colKind") }}</th>
            <th>{{ t("settings.project.members.colStatus") }}</th>
            <th aria-label="actions" />
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="row in members"
            :key="row.principal_id"
            data-testid="pm-row"
          >
            <td class="pm-table__name">{{ row.display_name }}</td>
            <td>
              <Badge :variant="roleBadgeVariant(row.role)">{{
                row.role
              }}</Badge>
            </td>
            <td class="pm-table__kind">{{ row.kind }}</td>
            <td>
              <Badge :variant="row.is_active ? 'neutral' : 'danger'">
                {{
                  row.is_active
                    ? t("settings.project.members.active")
                    : t("settings.project.members.inactive")
                }}
              </Badge>
            </td>
            <td class="pm-table__actions">
              <Button
                v-if="row.role !== 'owner'"
                variant="ghost"
                size="sm"
                :disabled="acting"
                :data-testid="`pm-remove-${row.principal_id.slice(0, 8)}`"
                @click="removing = row"
              >
                {{ t("settings.project.members.removeAction") }}
              </Button>
              <Button
                v-if="row.role !== 'owner' && row.is_active"
                variant="ghost"
                size="sm"
                :disabled="acting"
                :data-testid="`pm-transfer-${row.principal_id.slice(0, 8)}`"
                @click="transferring = row"
              >
                {{ t("settings.project.members.transferAction") }}
              </Button>
            </td>
          </tr>
        </tbody>
      </table>
      <p class="pm-note">{{ t("settings.project.members.authNote") }}</p>

      <!-- 内联添加（授予非危险操作；角色固定 member，owner 只能走所有权转移） -->
      <div v-if="addOpen" class="pm-add-row" data-testid="pm-add-form">
        <select
          v-model="candidateId"
          class="pm-select"
          :aria-label="t('settings.project.members.pickPrincipal')"
          data-testid="pm-candidate"
        >
          <option value="" disabled>
            {{
              candidatesLoading
                ? t("settings.project.general.loading")
                : t("settings.project.members.pickPrincipal")
            }}
          </option>
          <option v-for="row in memberOptions" :key="row.id" :value="row.id">
            {{ row.display_name }}（{{ row.kind }}）
          </option>
        </select>
        <Button
          variant="primary"
          size="sm"
          :disabled="!candidateId || acting"
          data-testid="pm-grant"
          @click="grant"
        >
          {{ t("settings.project.members.grantAction") }}
        </Button>
      </div>
    </div>

    <!-- 移除成员 -->
    <ConfirmationDialog
      :open="removing !== null"
      :title="t('settings.project.members.removeConfirmTitle')"
      :description="
        removing
          ? t('settings.project.members.removeConfirmDescription', {
              name: removing.display_name
            })
          : ''
      "
      :confirm-label="t('settings.project.members.removeAction')"
      :cancel-label="t('settings.project.general.cancel')"
      variant="danger"
      data-testid="pm-remove-dialog"
      @confirm="removeMember"
      @update:open="removing = null"
      @cancel="removing = null"
    />

    <!-- 所有权转移 -->
    <ConfirmationDialog
      :open="transferring !== null"
      :title="t('settings.project.members.transferConfirmTitle')"
      :description="
        transferring
          ? t('settings.project.members.transferConfirmDescription', {
              name: transferring.display_name
            })
          : ''
      "
      :confirm-label="t('settings.project.members.transferAction')"
      :cancel-label="t('settings.project.general.cancel')"
      variant="danger"
      data-testid="pm-transfer-dialog"
      @confirm="transfer"
      @update:open="transferring = null"
      @cancel="transferring = null"
    />
  </SettingsAppFrame>
</template>

<style scoped>
.pm-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.pm-table {
  width: 100%;
  border-collapse: collapse;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
}

.pm-table th {
  text-align: left;
  padding: var(--space-2);
  border-bottom: 1px solid var(--ws-border);
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.pm-table td {
  padding: var(--space-2);
  border-bottom: 1px solid var(--ws-border);
}

.pm-table__name {
  font-weight: var(--weight-medium);
}

.pm-table__kind {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
}

.pm-table__actions {
  display: flex;
  gap: var(--space-1);
  justify-content: flex-end;
}

.pm-add-row {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}

.pm-select {
  width: 100%;
  height: var(--size-control-md, 2.5rem);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  color: var(--text-primary);
  font-family: var(--font-ui);
}

.pm-note {
  margin: 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}
</style>
