<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  Bell,
  ChevronsUpDown,
  CircleQuestionMark,
  Search,
  Sparkles
} from "lucide-vue-next";

import Avatar from "@/components/ui/avatar/Avatar.vue";
import DropdownMenu from "@/components/ui/dropdown-menu/DropdownMenu.vue";
import IconTooltip from "./IconTooltip.vue";
import ProjectSwitcher from "./ProjectSwitcher.vue";
import { useShellStore } from "@/stores/shell";
import {
  demoOrganization,
  demoUnreadNotifications,
  demoUser,
  resolveProjectSwitch
} from "./demo";

/**
 * 全局顶栏（Application Shell §2A，52px）。
 *
 * 左：产品标识 + 产品名 + 当前组织指示 + 项目切换器
 * 中：全局搜索 / 命令面板入口（⌘ K）
 * 右：通知、全局 AI 助手入口、帮助、账户菜单
 *
 * 按 Shell V1.1 限制：**不引入 Workspace Switcher**——
 * 组织在此仅作上下文指示（静态展示，无切换实体）；
 * 项目则是可切换的（Figma「04 — Global Navigation」Project Context Header）。
 */

const { t } = useI18n();
const shell = useShellStore();

const emit = defineEmits<{ search: [] }>();

const accountItems = computed(() => [
  { label: t("shell.app.topbar.accountMenu.profile"), value: "profile" },
  { label: t("shell.app.topbar.accountMenu.settings"), value: "settings" },
  { label: t("shell.app.topbar.accountMenu.signOut"), value: "sign-out" }
]);

/** 切换项目（与左导航项目头共用 store 的切换状态机） */
async function onSelectProject(id: string) {
  await shell.switchProject(id, () => resolveProjectSwitch());
}
</script>

<template>
  <header class="shell-topbar">
    <!-- 左：产品标识 + 上下文 -->
    <div class="shell-topbar__left">
      <span class="shell-topbar__mark" aria-hidden="true">
        <svg width="18" height="16" viewBox="0 0 18 16" fill="none">
          <path
            d="M2 4.5L9 1l7 3.5L9 8 2 4.5Z"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linejoin="round"
          />
          <path
            d="M2 8l7 3.5L16 8M2 11.5L9 15l7-3.5"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </span>
      <span class="shell-topbar__product">{{
        t("shell.app.topbar.product")
      }}</span>

      <span class="shell-topbar__divider" aria-hidden="true" />

      <!-- 当前组织（上下文指示，非切换器） -->
      <span class="shell-topbar__context">
        <span class="shell-topbar__context-name">{{
          demoOrganization.name
        }}</span>
        <ChevronsUpDown :size="12" aria-hidden="true" />
      </span>

      <span class="shell-topbar__slash" aria-hidden="true">/</span>

      <!-- 当前项目（Project switcher：可切换，含搜索与切换中提示） -->
      <ProjectSwitcher
        variant="chip"
        searchable
        :projects="shell.projects"
        :current-id="shell.currentProjectId"
        :switching-to-id="shell.switchingProjectId"
        @select="onSelectProject"
      />
    </div>

    <!-- 中：全局搜索 / 命令面板入口 -->
    <button
      type="button"
      class="shell-topbar__search"
      :aria-label="t('shell.app.topbar.searchPlaceholder')"
      @click="emit('search')"
    >
      <Search :size="14" aria-hidden="true" />
      <span class="shell-topbar__search-text">{{
        t("shell.app.topbar.searchPlaceholder")
      }}</span>
      <kbd class="shell-topbar__kbd">{{
        t("shell.app.topbar.searchShortcut")
      }}</kbd>
    </button>

    <!-- 右：全局工具区 -->
    <div class="shell-topbar__right">
      <IconTooltip :content="t('shell.app.topbar.notifications')" side="bottom">
        <button
          type="button"
          class="shell-topbar__icon-button"
          :aria-label="t('shell.app.topbar.notifications')"
        >
          <Bell :size="16" aria-hidden="true" />
          <span
            v-if="demoUnreadNotifications > 0"
            class="shell-topbar__unread"
            aria-hidden="true"
          />
        </button>
      </IconTooltip>

      <!-- 全局 AI 助手入口 -->
      <button type="button" class="shell-topbar__assistant">
        <Sparkles :size="14" aria-hidden="true" />
        <span>{{ t("shell.app.topbar.assistant") }}</span>
      </button>

      <IconTooltip :content="t('shell.app.topbar.help')" side="bottom">
        <button
          type="button"
          class="shell-topbar__icon-button"
          :aria-label="t('shell.app.topbar.help')"
        >
          <CircleQuestionMark :size="16" aria-hidden="true" />
        </button>
      </IconTooltip>

      <span class="shell-topbar__divider" aria-hidden="true" />

      <!-- 账户菜单
           DropdownMenu 的触发器本身即 button（自带 chevron 与 aria-label），
           slot 只放内容，不再嵌套 button。 -->
      <DropdownMenu
        :items="accountItems"
        :aria-label="t('shell.app.topbar.account')"
      >
        <span class="shell-topbar__account">
          <Avatar
            class="shell-topbar__avatar"
            :name="demoUser.name"
            :label="demoUser.name"
          >
            <template #fallback>
              <span class="shell-topbar__initials">{{
                demoUser.initials
              }}</span>
            </template>
          </Avatar>
        </span>
      </DropdownMenu>
    </div>
  </header>
</template>

<style scoped>
.shell-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  height: var(--ws-topbar-height);
  padding: 0 var(--space-4);
  background: var(--ws-sidebar);
  border-bottom: var(--border-1) solid var(--ws-border);
}

/* 左：产品 + 上下文 */
.shell-topbar__left {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.shell-topbar__mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  color: var(--ws-accent);
  background: var(--ws-color-accent-surface);
  border-radius: var(--radius-sm);
}

.shell-topbar__product {
  flex: none;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  white-space: nowrap;
}

.shell-topbar__divider {
  flex: none;
  width: var(--border-1);
  height: var(--space-4);
  background: var(--ws-border);
}

.shell-topbar__context {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-compact);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  white-space: nowrap;
}

.shell-topbar__context-name {
  overflow: hidden;
  text-overflow: ellipsis;
}

.shell-topbar__slash {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

/* 中：搜索入口 */
.shell-topbar__search {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  width: var(--size-search-width);
  max-width: 30%;
  height: var(--size-control-compact);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  text-align: left;
  cursor: pointer;
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.shell-topbar__search:hover {
  border-color: var(--ws-text-secondary);
}

.shell-topbar__search:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.shell-topbar__search-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 键盘芯片 */
.shell-topbar__kbd,
.shell-sidenav__kbd {
  padding: 1px var(--space-1);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

/* 右：工具区 */
.shell-topbar__right {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}

.shell-topbar__icon-button {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-topbar__icon-button:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.shell-topbar__icon-button:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 未读圆点 */
.shell-topbar__unread {
  position: absolute;
  top: var(--space-1);
  right: var(--space-1);
  width: var(--space-1);
  height: var(--space-1);
  background: var(--ws-accent);
  border-radius: var(--radius-full);
}

.shell-topbar__assistant {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  color: var(--ws-accent);
  font-size: var(--font-size-body);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-topbar__assistant:hover {
  background: var(--ws-hover-veil);
}

.shell-topbar__assistant:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 账户 */
/* 账户（DropdownMenu 触发器已是 button，这里只是内容容器） */
.shell-topbar__account {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
}

.shell-topbar__avatar {
  width: var(--size-control-sm);
  height: var(--size-control-sm);
}

.shell-topbar__initials {
  color: var(--ws-text-primary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-semibold);
}

@media (prefers-reduced-motion: reduce) {
  .shell-topbar__search,
  .shell-topbar__icon-button {
    transition: none;
  }
}
</style>
