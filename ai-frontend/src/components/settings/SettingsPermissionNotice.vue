<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import Alert from "@/components/ui/alert/Alert.vue";

/**
 * SettingsPermissionNotice —— 权限提示横幅（Settings Framework §7/§9）。
 *
 * 复用 ui/alert（§7 约束：不重复造反馈组件）；readonly = info 面，
 * denied = danger 面。文案默认走 i18n（调用方传 role 时插值角色名）。
 */
const props = defineProps<{
  /** 访问态：readonly（可见不可改）/ denied（无权访问） */
  access: "readonly" | "denied";
  /** 当前角色名（已翻译，用于插值说明） */
  role?: string;
  /** 覆盖默认文案（已翻译完整句） */
  message?: string;
}>();

const { t } = useI18n();

const variant = computed(() => (props.access === "denied" ? "danger" : "info"));

const defaultMessage = computed(() =>
  props.access === "denied"
    ? t("settings.common.permissionDeniedMsg", { role: props.role ?? "" })
    : t("settings.common.readOnlyMsg", { role: props.role ?? "" })
);

const text = computed(() => props.message ?? defaultMessage.value);
</script>

<template>
  <Alert :variant="variant" size="sm">
    {{ text }}
  </Alert>
</template>
