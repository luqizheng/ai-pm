<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Search, Sparkles } from "lucide-vue-next";

import NotificationDropdown from "@/components/global/notification/NotificationDropdown.vue";
import { TenantIndicator } from "@/components/global/tenant";
import { HelpMenu, KeyboardShortcutsDialog } from "@/components/global/help";
import {
  UserMenuAccountMenu,
  UserMenuSignOutDialog,
  demoUserProfile
} from "@/components/global/usermenu";
import ProjectSwitcher from "./ProjectSwitcher.vue";
import { AUTH_PATHS } from "@/router/auth";
import { useAuthStore } from "@/stores/auth";
import { useShellStore } from "@/stores/shell";
import { demoOrganization, resolveProjectSwitch } from "./demo";

/**
 * 全局顶栏（Application Shell §2A，52px）。
 *
 * 左：产品标识 + 产品名 + 租户上下文指示（Global/TenantIndicator，只读）
 *     + 项目切换器
 * 中：全局搜索 / 命令面板入口（⌘ K）
 * 右：通知（下拉）、全局 AI 助手入口、帮助（菜单 + 快捷键对话框）、
 *     账户菜单（含退出登录确认）
 *
 * 按 Shell V1.1 限制：**不引入 Workspace Switcher**——
 * 组织在此仅作上下文指示（无切换实体）；
 * 项目则是可切换的（Figma「04 — Global Navigation」Project Context Header）。
 */

const { t } = useI18n();
const router = useRouter();
const shell = useShellStore();
const auth = useAuthStore();

const emit = defineEmits<{ search: [] }>();

const shortcutsOpen = ref(false);
const signOutOpen = ref(false);

/** 切换项目（与左导航项目头共用 store 的切换状态机） */
async function onSelectProject(id: string) {
  await shell.switchProject(id, () => resolveProjectSwitch());
}

/** 通知：打开关联对象 / 前往完整通知中心 */
function onOpenNotification(target: string) {
  router.push(target);
}

function onViewAllNotifications() {
  router.push("/notifications");
}

/**
 * 账户菜单普通项（个人资料/账户设置/安全/个人偏好/帮助文档）。
 * 对应页面路由骨架期未建，落点待业务域接入。
 */
function onAccountSelect(_id: string) {}

/** 退出登录：清会话回登录页（确认对话框先决） */
function onSignOutConfirm() {
  auth.signOut();
  router.push(AUTH_PATHS.login);
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

      <!-- 租户上下文指示（只读：hover Tooltip + 点击身份卡；非切换器） -->
      <TenantIndicator
        :tenant="demoOrganization.name"
        :identity="demoUserProfile.name"
        :email="demoUserProfile.email"
      />

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
      <!-- 通知下拉（铃铛触发，含文本未读徽标；Global/Notification 族） -->
      <NotificationDropdown
        @open="onOpenNotification"
        @view-all="onViewAllNotifications"
      />

      <!-- 全局 AI 助手入口 -->
      <button type="button" class="shell-topbar__assistant">
        <Sparkles :size="14" aria-hidden="true" />
        <span>{{ t("shell.app.topbar.assistant") }}</span>
      </button>

      <!-- 帮助菜单（文档入口 / 键盘快捷键 / 反馈） -->
      <HelpMenu @shortcuts="shortcutsOpen = true" />

      <span class="shell-topbar__divider" aria-hidden="true" />

      <!-- 账户菜单（Figma Global/UserMenu 族） -->
      <UserMenuAccountMenu
        :name="demoUserProfile.name"
        :email="demoUserProfile.email"
        :initials="demoUserProfile.initials"
        :tenant="demoUserProfile.tenant"
        :project-role="demoUserProfile.projectRole"
        @select="onAccountSelect"
        @shortcuts="shortcutsOpen = true"
        @sign-out="signOutOpen = true"
      />
    </div>

    <!-- 键盘快捷键对话框（账户菜单 / 帮助菜单共用） -->
    <KeyboardShortcutsDialog v-model:open="shortcutsOpen" />

    <!-- 退出登录确认（人工勾选后才可用；确认后清会话回登录页） -->
    <UserMenuSignOutDialog
      v-model:open="signOutOpen"
      :email="demoUserProfile.email"
      @confirm="onSignOutConfirm"
    />
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

@media (prefers-reduced-motion: reduce) {
  .shell-topbar__search {
    transition: none;
  }
}
</style>
