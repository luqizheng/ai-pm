<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Ellipsis } from "lucide-vue-next";
import DropdownMenu from "@/components/ui/dropdown-menu/DropdownMenu.vue";
import type { DropdownMenuItemOption } from "@/components/ui/dropdown-menu/types";
import type { ProjectActionKind, ProjectEntry } from "../project-entry";

/**
 * 项目行/卡片的操作菜单（Projects List §6 More menu）。
 *
 * 操作按域支持取舍：Open（项目总控台）/ Archive|Restore（按状态二选一，可逆）
 * / Delete（不可逆，danger）。Edit 无落地编辑页，不提供。
 * 删除的显式确认由页根 ConfirmationDialog 承载（红线：不可逆操作须人工确认）。
 */
interface Props {
  entry: ProjectEntry;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "action", kind: ProjectActionKind, id: string): void;
}>();

const { t } = useI18n();

const items = computed<DropdownMenuItemOption[]>(() => [
  { label: t("pm.projects.menuOpen"), value: "open" },
  props.entry.status === "archived"
    ? { label: t("pm.projects.menuRestore"), value: "restore" }
    : { label: t("pm.projects.menuArchive"), value: "archive" },
  {
    label: t("pm.projects.menuDelete"),
    value: "delete",
    danger: true,
    dangerLabel: t("pm.projects.menuDeleteDanger")
  }
]);

function onSelect(item: DropdownMenuItemOption) {
  emit("action", (item.value ?? "open") as ProjectActionKind, props.entry.id);
}
</script>

<template>
  <DropdownMenu
    :items="items"
    align="end"
    :aria-label="t('pm.projects.menuAria', { name: props.entry.name })"
    @select="onSelect"
  >
    <span
      class="proj-menu__trigger-face"
      role="presentation"
      aria-hidden="true"
    >
      <Ellipsis aria-hidden="true" />
    </span>
  </DropdownMenu>
</template>

<style scoped>
.proj-menu__trigger-face {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--space-6);
  height: var(--space-6);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
}

.proj-menu__trigger-face svg {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
}
</style>
