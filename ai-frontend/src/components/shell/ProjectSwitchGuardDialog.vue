<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowRightLeft } from "lucide-vue-next";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { useShellStore } from "@/stores/shell";

/**
 * 切项目确认（基线 G12 · DR-C6 · AI-07）。
 *
 * **切项目不得静默转移会话与上下文**：当前有活跃会话时，切换前明确确认——
 * 确认 = 保留当前会话并在目标项目另起（历史会话保留原项目归属）；
 * 取消 = 维持现状。
 *
 * 壳级组件：顶栏与左导航两个项目切换入口共用（两处都经 store 的
 * `requestProjectSwitch` 走同一道门）。
 */
const { t } = useI18n();
const shell = useShellStore();

/** 待切换的目标项目（null = 未打开） */
const targetId = computed(() => shell.pendingProjectSwitch);
const open = computed(() => targetId.value !== null);

const targetName = computed(
  () => shell.projects.find(p => p.id === targetId.value)?.name ?? ""
);

function onCancel() {
  shell.cancelProjectSwitch();
}

async function onConfirm() {
  await shell.confirmProjectSwitch();
}
</script>

<template>
  <ConfirmationDialog
    :open="open"
    :title="t('shell.app.projectSwitchGuard.title')"
    :description="
      targetName
        ? t('shell.app.projectSwitchGuard.description', { project: targetName })
        : t('shell.app.projectSwitchGuard.descriptionFallback')
    "
    :confirm-label="t('shell.app.projectSwitchGuard.confirm')"
    :cancel-label="t('shell.app.projectSwitchGuard.cancel')"
    variant="default"
    :icon="ArrowRightLeft"
    size="sm"
    @update:open="onCancel"
    @confirm="onConfirm"
  />
</template>
