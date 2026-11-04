<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Boxes } from "lucide-vue-next";

import {
  SettingsConfirmationDialog,
  SettingsFormRow,
  SettingsNavigationItem,
  SettingsPermissionNotice,
  SettingsReadOnlyField,
  SettingsSaveBar,
  SettingsUnsavedChangesDialog,
  SettingsValidationMessage
} from "@/components/settings";

import DesignLayout from "../DesignLayout.vue";
import { useDemoForm } from "./useDemoForm";
import Specimen from "../design-kit/Specimen.vue";

import { Toast } from "@/components/ui/toast";
import { Skeleton } from "@/components/ui/skeleton";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Input from "@/components/ui/input/Input.vue";
import Button from "@/components/ui/button/Button.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 18 — 设置交互状态（§6 十二项逐项演示）。
 */
const { t } = useI18n();

// 2/3/4/12 · 编辑→保存→取消→成功反馈共用一条演示链
const form = useDemoForm(() => ({ displayName: "Han Lin" }));
const showSuccess = computed(() => form.saved.value);

// 5 · 校验错误
const invalidValue = ref("a");
const invalid = computed(() => invalidValue.value.length > 0);

// 6 · 未保存警告
const unsavedOpen = ref(false);

// 7 · 破坏性确认
const destructiveOpen = ref(false);

// 9 · 权限不足
const deniedRole = ref("");

// 12 · 成功反馈
const toastVisible = ref(false);
let toastTimer = 0;
function flashToast() {
  toastVisible.value = true;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toastVisible.value = false;
  }, 2400);
}
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="states__title">{{ t("settings.states.title") }}</h1>
      <p class="states__desc">{{ t("settings.states.desc") }}</p>
    </template>

    <div class="states__grid">
      <Specimen :label="t('settings.states.navigation.title')">
        <div class="states__nav">
          <SettingsNavigationItem label="Profile" :icon="Boxes" :frame="'03'" />
          <SettingsNavigationItem
            label="Security"
            :icon="Boxes"
            active
            :frame="'04'"
          />
          <SettingsNavigationItem label="Governance" :icon="Boxes" proposed />
          <SettingsNavigationItem label="Integrations" :icon="Boxes" disabled />
        </div>
        <p class="states__hint">{{ t("settings.states.navigation.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.editing.title')">
        <SettingsFormRow label="Display name" html-for="states-edit-input">
          <Input
            id="states-edit-input"
            v-model="form.draft.value.displayName"
          />
        </SettingsFormRow>
        <SettingsSaveBar
          :dirty="form.dirty.value"
          :saving="form.saving.value"
          :saved="form.saved.value"
          :error="form.error.value ? t('settings.common.saveFailed') : ''"
          @save="form.save()"
          @cancel="form.cancel()"
        />
        <p class="states__hint">{{ t("settings.states.editing.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.saving.title')">
        <SettingsSaveBar
          :dirty="true"
          :saving="true"
          @save="() => {}"
          @cancel="() => {}"
        />
        <p class="states__hint">{{ t("settings.states.saving.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.cancel.title')">
        <SettingsFormRow label="Display name">
          <Input
            :model-value="form.dirty.value ? 'changed…' : 'Han Lin'"
            disabled
          />
        </SettingsFormRow>
        <Button variant="secondary" size="sm" @click="form.cancel()">
          {{ t("settings.states.demoReset") }}
        </Button>
        <p class="states__hint">{{ t("settings.states.cancel.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.validation.title')">
        <SettingsFormRow label="Display name" html-for="states-invalid">
          <Input
            id="states-invalid"
            v-model="invalidValue"
            :invalid="invalid"
          />
          <SettingsValidationMessage
            :message="t('settings.personal.security.passwordTooShort')"
          />
        </SettingsFormRow>
        <p class="states__hint">{{ t("settings.states.validation.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.unsaved.title')">
        <Button variant="secondary" size="sm" @click="unsavedOpen = true">
          {{ t("settings.states.demoOpenDialog") }}
        </Button>
        <p class="states__hint">{{ t("settings.states.unsaved.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.destructive.title')">
        <Button variant="danger" size="sm" @click="destructiveOpen = true">
          {{ t("settings.states.demoOpenDialog") }}
        </Button>
        <p class="states__hint">{{ t("settings.states.destructive.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.readonly.title')">
        <SettingsReadOnlyField value="tenant_8f3a2c" />
        <p class="states__hint">{{ t("settings.states.readonly.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.denied.title')">
        <SettingsPermissionNotice
          access="denied"
          :role="t('settings.demo.persona.tenantMember')"
        />
        <p class="states__hint">{{ t("settings.states.denied.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.loading.title')">
        <div class="states__skeleton">
          <Skeleton tone="strong" />
          <Skeleton />
          <Skeleton />
        </div>
        <p class="states__hint">{{ t("settings.states.loading.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.empty.title')">
        <EmptyState
          :title="t('settings.states.empty.title')"
          :action-label="t('settings.common.viewAll')"
        />
        <p class="states__hint">{{ t("settings.states.empty.desc") }}</p>
      </Specimen>

      <Specimen :label="t('settings.states.success.title')">
        <div class="states__toast-row">
          <Button variant="secondary" size="sm" @click="flashToast">
            {{ t("settings.states.demoSuccess") }}
          </Button>
          <Toast
            v-if="toastVisible"
            variant="success"
            :title="t('settings.common.saved')"
            :closable="false"
          />
        </div>
        <p class="states__hint">{{ t("settings.states.success.desc") }}</p>
      </Specimen>
    </div>

    <SettingsUnsavedChangesDialog
      :open="unsavedOpen"
      @update:open="unsavedOpen = $event"
      @save="() => {}"
      @discard="() => {}"
    />
    <SettingsConfirmationDialog
      :open="destructiveOpen"
      :title="t('settings.project.general.archiveTitle')"
      :description="t('settings.project.general.archiveDesc')"
      @update:open="destructiveOpen = $event"
      @confirm="destructiveOpen = false"
    />
  </DesignLayout>
</template>

<style scoped>
.states__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.states__desc {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.states__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-5);
}

.states__nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.states__hint {
  margin: var(--space-2) 0 0;
  font-size: var(--ws-font-size-micro);
  color: var(--ws-text-secondary);
}

.states__skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.states__toast-row {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-3);
}
</style>
