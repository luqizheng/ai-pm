<script setup lang="ts">
import { computed } from "vue";
import {
  DropdownMenu,
  type DropdownMenuItemOption
} from "@/components/ui/dropdown-menu";
import { Avatar } from "@/components/ui/avatar";
import { useI18n } from "vue-i18n";
import { i18n, setLocale, type SupportedLocale } from "../i18n";

const { t } = useI18n();

interface Props {
  user?: { name: string; email?: string; avatar?: string; roles?: string[] };
  profileLabel?: string;
  logoutLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  user: undefined,
  profileLabel: undefined,
  logoutLabel: undefined
});

const emit = defineEmits<{ profile: []; logout: [] }>();

/** 当前语言与目标语言（单条目切换：显示「切到另一种语言」） */
const currentLocale = computed(
  () => i18n.global.locale.value as SupportedLocale
);
const targetLocale = computed<SupportedLocale>(() =>
  currentLocale.value === "zh-CN" ? "en" : "zh-CN"
);

const items = computed<DropdownMenuItemOption[]>(() => [
  { label: props.profileLabel ?? t("shell.user.profile"), value: "profile" },
  {
    label: t("shell.locale.switchTo", {
      language: t(`shell.locale.${targetLocale.value}`)
    }),
    value: "locale"
  },
  {
    label: props.logoutLabel ?? t("shell.user.logout"),
    value: "logout",
    danger: true
  }
]);

function onSelect(item: DropdownMenuItemOption) {
  if (item.value === "logout") emit("logout");
  else if (item.value === "locale") setLocale(targetLocale.value);
  else emit("profile");
}
</script>

<template>
  <DropdownMenu
    :items="items"
    :aria-label="props.user?.name ?? t('shell.user.label')"
    align="end"
    @select="onSelect"
  >
    <button
      type="button"
      class="ui-user-menu__trigger"
      :aria-label="props.user?.name"
    >
      <Avatar
        :src="props.user?.avatar"
        :name="props.user?.name"
        :label="props.user?.name"
      />
    </button>
  </DropdownMenu>
</template>

<style scoped>
.ui-user-menu__trigger {
  display: inline-flex;
  align-items: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius-full);
  background: transparent;
  cursor: pointer;
}

.ui-user-menu__trigger:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}
</style>
