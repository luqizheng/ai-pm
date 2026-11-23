<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw, TriangleAlert } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import SettingsAppFrame from "@/views/settings/SettingsAppFrame.vue";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  closeProject,
  getProjectInfo,
  reopenProject,
  updateProjectInfo,
  type ProjectInfo
} from "@/api/projects";
import { useProjectSettingsFrame } from "./shared";

/**
 * Project Setting · General 节（P1 落地，替换 proposed 占位页）。
 *
 * 基本信息：名称/描述编辑（PATCH /v1/pm/projects/{id}，守卫=生命周期
 * 管理者；后端 403 时表单禁用并显式提示）。项目标识 code 只读展示——
 * 它承担租户内唯一键与知识库 canonical path 前缀，改名属另立正式变更。
 * 危险操作：关闭项目（POST /close，ConfirmationDialog + 理由）与重开
 * （POST /reopen），复用既有生命周期端点，不在本页复制语义。
 */
const { t } = useI18n();
const toast = useToast();
const settingsFrame = useProjectSettingsFrame();

const loading = ref(true);
const error = ref<string | null>(null);
const info = ref<ProjectInfo | null>(null);

const editName = ref("");
const editDescription = ref("");
const saving = ref(false);
const closing = ref(false);
const reopening = ref(false);
const closeDialogOpen = ref(false);
const closeReason = ref("");
const reopenDialogOpen = ref(false);

const isClosed = computed(() => info.value?.status === "closed");

const dirty = computed(() => {
  if (!info.value) return false;
  return (
    editName.value.trim() !== info.value.name ||
    editDescription.value.trim() !== (info.value.description ?? "")
  );
});

async function load(): Promise<void> {
  if (!settingsFrame.projectId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    info.value = await getProjectInfo(settingsFrame.projectId.value);
    editName.value = info.value.name;
    editDescription.value = info.value.description ?? "";
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

async function save(): Promise<void> {
  const current = info.value;
  if (!current || saving.value || !dirty.value) return;
  saving.value = true;
  try {
    info.value = await updateProjectInfo(current.id, {
      name: editName.value.trim(),
      description: editDescription.value.trim() || null
    });
    toast.toast({ title: t("settings.project.general.savedToast") });
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    saving.value = false;
  }
}

async function confirmClose(): Promise<void> {
  const current = info.value;
  if (!current || closing.value) return;
  closing.value = true;
  try {
    const outcome = await closeProject(
      current.id,
      closeReason.value.trim() || undefined
    );
    toast.toast({
      title: t("settings.project.general.closedToast", {
        status: outcome.status
      })
    });
    closeDialogOpen.value = false;
    closeReason.value = "";
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    closing.value = false;
  }
}

async function confirmReopen(): Promise<void> {
  const current = info.value;
  if (!current || reopening.value) return;
  reopening.value = true;
  try {
    const outcome = await reopenProject(current.id);
    toast.toast({
      title: t("settings.project.general.reopenedToast", {
        status: outcome.status
      })
    });
    reopenDialogOpen.value = false;
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    reopening.value = false;
  }
}

onMounted(load);
watch(() => settingsFrame.projectId.value, load);
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="settingsFrame.breadcrumb(t('settings.nav.projectGeneral'))"
    :title="t('settings.nav.projectGeneral')"
    :description="t('settings.project.general.description')"
    :scope-label="settingsFrame.scopeLabel.value"
    :entries="settingsFrame.navEntries.value"
    :icons="settingsFrame.navIcons"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("settings.project.general.refresh") }}
      </Button>
    </template>

    <EmptyState
      v-if="!settingsFrame.projectId.value"
      :title="t('settings.project.general.noProject')"
    />
    <EmptyState v-else-if="error" :title="error" />
    <EmptyState
      v-else-if="loading && !info"
      :title="t('settings.project.general.loading')"
    />
    <div v-else-if="info" class="pg-body" data-testid="pg-body">
      <!-- 基本信息 -->
      <section class="pg-card" data-testid="pg-info">
        <header class="pg-card__head">
          <h3 class="pg-card__title">
            {{ t("settings.project.general.infoTitle") }}
          </h3>
          <Badge :variant="isClosed ? 'danger' : 'neutral'">
            {{ info.status }}
          </Badge>
        </header>
        <dl class="pg-identity">
          <div class="pg-identity__row">
            <dt>{{ t("settings.project.general.codeLabel") }}</dt>
            <dd>
              <code class="pg-code">{{ info.code }}</code>
              <span class="pg-hint">{{
                t("settings.project.general.codeHint")
              }}</span>
            </dd>
          </div>
          <div class="pg-identity__row">
            <dt>{{ t("settings.project.general.customerLabel") }}</dt>
            <dd>{{ info.customer_name ?? info.customer_id }}</dd>
          </div>
        </dl>
        <label class="pg-field">
          <span class="pg-field__label">{{
            t("settings.project.general.nameLabel")
          }}</span>
          <input
            v-model="editName"
            type="text"
            class="pg-input"
            maxlength="200"
            data-testid="pg-name"
          />
        </label>
        <label class="pg-field">
          <span class="pg-field__label">{{
            t("settings.project.general.descriptionLabel")
          }}</span>
          <textarea
            v-model="editDescription"
            class="pg-input pg-input--area"
            rows="4"
            maxlength="2000"
            data-testid="pg-description"
          />
        </label>
        <div class="pg-actions">
          <Button
            variant="primary"
            size="sm"
            :disabled="saving || !dirty || editName.trim().length === 0"
            data-testid="pg-save"
            @click="save"
          >
            {{ t("settings.project.general.save") }}
          </Button>
          <span v-if="!dirty" class="pg-hint">{{
            t("settings.project.general.noChanges")
          }}</span>
        </div>
      </section>

      <!-- 危险操作 -->
      <section class="pg-card pg-card--danger" data-testid="pg-danger">
        <header class="pg-card__head">
          <h3 class="pg-card__title">
            <TriangleAlert :size="14" aria-hidden="true" />
            {{ t("settings.project.general.dangerTitle") }}
          </h3>
        </header>
        <div class="pg-danger-row">
          <div class="pg-danger-row__text">
            <p class="pg-danger-row__title">
              {{ t("settings.project.general.closeTitle") }}
            </p>
            <p class="pg-hint">{{ t("settings.project.general.closeHint") }}</p>
          </div>
          <Button
            v-if="!isClosed"
            variant="secondary"
            size="sm"
            :disabled="closing"
            data-testid="pg-close"
            @click="closeDialogOpen = true"
          >
            {{ t("settings.project.general.closeAction") }}
          </Button>
          <Button
            v-else
            variant="secondary"
            size="sm"
            :disabled="reopening"
            data-testid="pg-reopen"
            @click="reopenDialogOpen = true"
          >
            {{ t("settings.project.general.reopenAction") }}
          </Button>
        </div>
      </section>
    </div>

    <ConfirmationDialog
      :open="closeDialogOpen"
      :title="t('settings.project.general.closeConfirmTitle')"
      :description="t('settings.project.general.closeConfirmDescription')"
      :confirm-label="t('settings.project.general.closeAction')"
      :cancel-label="t('settings.project.general.cancel')"
      variant="danger"
      data-testid="pg-close-dialog"
      @confirm="confirmClose"
      @update:open="closeDialogOpen = $event"
      @cancel="closeDialogOpen = false"
    />
    <ConfirmationDialog
      :open="reopenDialogOpen"
      :title="t('settings.project.general.reopenConfirmTitle')"
      :description="t('settings.project.general.reopenConfirmDescription')"
      :confirm-label="t('settings.project.general.reopenAction')"
      :cancel-label="t('settings.project.general.cancel')"
      data-testid="pg-reopen-dialog"
      @confirm="confirmReopen"
      @update:open="reopenDialogOpen = $event"
      @cancel="reopenDialogOpen = false"
    />
  </SettingsAppFrame>
</template>

<style scoped>
.pg-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.pg-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
}

.pg-card--danger {
  border-color: var(--status-danger-border, var(--ws-border));
}

.pg-card__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pg-card__title {
  margin: 0;
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-h3);
}

.pg-identity {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.pg-identity__row {
  display: grid;
  grid-template-columns: 140px 1fr;
  gap: var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
}

.pg-identity__row dt {
  color: var(--text-secondary);
}

.pg-identity__row dd {
  margin: 0;
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pg-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.pg-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.pg-field__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.pg-input {
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  padding: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pg-input--area {
  resize: vertical;
  min-height: 6rem;
}

.pg-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.pg-danger-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.pg-danger-row__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
}

.pg-hint {
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}
</style>
