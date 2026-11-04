<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Help/KeyboardShortcutsDialog" node=33:87688 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  DialogContent,
  DialogOverlay,
  DialogPortal,
  DialogRoot
} from "reka-ui";
import { X } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";

/**
 * 键盘快捷键对话框（Figma Global/Help/KeyboardShortcutsDialog 33:87688，736 面板）。
 *
 * 五组：全局导航 / 搜索 / 项目导航 / 编辑 / AI 助手；
 * 行 = 13 标签 + mono 11 快捷键；**未实现的键位显式标注**
 * 「未分配 / 拟议 · 未分配」——不把候选键位谎报为已确认功能（任务书 §4G 红线）。
 * 已确认实现：搜索组 Cmd/Ctrl+K（WorkspaceShell 全局监听）。
 */
interface ShortcutRow {
  labelKey: string;
  keys?: string;
  state?: "assigned" | "unassigned" | "proposed";
}

interface ShortcutGroup {
  titleKey: string;
  rows: ShortcutRow[];
}

defineProps<{ open: boolean }>();

const emit = defineEmits<{ "update:open": [value: boolean] }>();

const { t } = useI18n();

const groups = computed<ShortcutGroup[]>(() => [
  {
    titleKey: "global.help.groupGlobal",
    rows: [{ labelKey: "global.help.shortcutNavigate", state: "unassigned" }]
  },
  {
    titleKey: "global.help.groupSearch",
    rows: [
      {
        labelKey: "global.help.shortcutOpenSearch",
        keys: "Cmd/Ctrl+K",
        state: "assigned"
      },
      {
        labelKey: "global.help.shortcutSelect",
        keys: "↑ / ↓",
        state: "assigned"
      },
      {
        labelKey: "global.help.shortcutOpenResult",
        keys: "Enter",
        state: "assigned"
      },
      {
        labelKey: "global.help.shortcutCloseSearch",
        keys: "Esc",
        state: "assigned"
      }
    ]
  },
  {
    titleKey: "global.help.groupProject",
    rows: [
      { labelKey: "global.help.shortcutProjectSwitcher", state: "proposed" }
    ]
  },
  {
    titleKey: "global.help.groupEditing",
    rows: [{ labelKey: "global.help.shortcutEditing", state: "unassigned" }]
  },
  {
    titleKey: "global.help.groupAssistant",
    rows: [{ labelKey: "global.help.shortcutAssistant", state: "proposed" }]
  }
]);

function keysText(row: ShortcutRow): string {
  if (row.state === "unassigned") return t("global.help.notAssigned");
  if (row.state === "proposed") return t("global.help.proposed");
  return row.keys ?? "";
}
</script>

<template>
  <DialogRoot :open="open" @update:open="emit('update:open', $event)">
    <DialogPortal>
      <DialogOverlay class="shortcuts-dialog__overlay" />
      <DialogContent class="shortcuts-dialog" aria-describedby="shortcuts-note">
        <div class="shortcuts-dialog__header">
          <span class="shortcuts-dialog__title">{{
            t("global.help.shortcutsTitle")
          }}</span>
          <DialogClose
            class="shortcuts-dialog__close"
            :aria-label="t('global.usermenu.cancel')"
          >
            <X :size="16" aria-hidden="true" />
          </DialogClose>
        </div>

        <div class="shortcuts-dialog__body">
          <p id="shortcuts-note" class="shortcuts-dialog__note">
            {{ t("global.help.shortcutsNote") }}
          </p>
          <div
            v-for="group in groups"
            :key="group.titleKey"
            class="shortcuts-dialog__group"
          >
            <p class="shortcuts-dialog__group-title">{{ t(group.titleKey) }}</p>
            <div
              v-for="row in group.rows"
              :key="row.labelKey"
              class="shortcuts-dialog__row"
            >
              <span class="shortcuts-dialog__row-label">{{
                t(row.labelKey)
              }}</span>
              <span
                class="shortcuts-dialog__row-keys"
                :class="{ 'is-unassigned': row.state !== 'assigned' }"
                >{{ keysText(row) }}</span
              >
            </div>
          </div>
        </div>

        <div class="shortcuts-dialog__footer">
          <Button variant="secondary" @click="emit('update:open', false)">
            {{ t("global.usermenu.close") }}
          </Button>
          <Button variant="primary" @click="emit('update:open', false)">
            {{ t("global.usermenu.done") }}
          </Button>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
.shortcuts-dialog__overlay {
  position: fixed;
  inset: 0;
  z-index: 70;
  background: color-mix(in srgb, var(--ws-canvas) 28%, transparent);
}

.shortcuts-dialog {
  position: fixed;
  top: 12vh;
  left: 50%;
  z-index: 71;
  display: flex;
  flex-direction: column;
  width: var(--size-shortcuts-dialog-width);
  max-width: calc(100vw - var(--space-8));
  max-height: 76vh;
  overflow-y: auto;
  transform: translateX(-50%);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  outline: none;
}

.shortcuts-dialog__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.shortcuts-dialog__title {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.shortcuts-dialog__close {
  display: inline-flex;
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: transparent;
  border: none;
}

.shortcuts-dialog__close:hover {
  color: var(--ws-text-primary);
}

.shortcuts-dialog__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  padding: var(--space-4);
}

.shortcuts-dialog__note {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.shortcuts-dialog__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.shortcuts-dialog__group-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.shortcuts-dialog__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: var(--size-control-compact);
}

.shortcuts-dialog__row-label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.shortcuts-dialog__row-keys {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.shortcuts-dialog__row-keys.is-unassigned {
  color: var(--ws-text-secondary);
}

.shortcuts-dialog__footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
}
</style>
