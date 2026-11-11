<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Maximize2, Minimize2, Search, Sparkles } from "lucide-vue-next";

import { resolveAvatarSrc } from "@/api/base/self-service";
import { LocaleToggle } from "@/components/global/locale";
import NotificationDropdown from "@/components/global/notification/NotificationDropdown.vue";
import { ThemeToggle } from "@/components/global/theme";
import TeamSwitcher from "@/components/global/team/TeamSwitcher.vue";
import { HelpMenu, KeyboardShortcutsDialog } from "@/components/global/help";
import {
  UserMenuAccountMenu,
  UserMenuSignOutDialog
} from "@/components/global/usermenu";
import {
  ProjectSwitcher,
  PROJECT_SWITCHER_DEFAULT_TEXT,
  type ProjectSwitcherProject,
  type ProjectSwitcherText
} from "@/components/global/project-switcher";
import { AUTH_PATHS } from "@/router/auth";
import { useAuthStore } from "@/stores/auth";
import { useShellStore } from "@/stores/shell";
import { useAssistantStore } from "@/stores/assistant";
import { useTeamStore } from "@/stores/team";

/**
 * 全局顶栏（Application Shell §2A，52px）。
 *
 * 左：产品标识 + 产品名 + 团队上下文（TeamSwitcher，同账号团队切换；0044）
 *     + 项目切换器（Figma「06 — Project Context Switcher」33:82961，
 *     Global/ProjectSwitcher 五态 Trigger + Menu）
 * 中：全局搜索 / 命令面板入口（⌘ K）
 * 右：语言快捷开关（中/英往返）、通知（下拉）、主题快捷开关（深/浅往返）、
 *     全局 AI 助手入口、
 *     帮助（菜单 + 快捷键对话框）、账户菜单（含退出登录确认）
 *
 * 按 Shell V1.1 限制：**不引入 Workspace Switcher**——
 * 组织在此仅作上下文指示（无切换实体）；项目可切换。
 * 切换过渡（07 板）语义经 store 映射：switching 期间 label 显示
 * 切换目标名、面板呈 loading（06 板 Loading 态），不显示旧项目数据。
 */

const { t } = useI18n();
const router = useRouter();
const shell = useShellStore();
const assistant = useAssistantStore();
const auth = useAuthStore();
const teamStore = useTeamStore();

const emit = defineEmits<{ search: [] }>();

const shortcutsOpen = ref(false);
const signOutOpen = ref(false);

/**
 * 账户菜单身份块 = 当前登录用户（auth store：登录快照 + /me 刷新）。
 * 展示名优先昵称、回退用户名；组织行 = 当前团队（与 TeamSwitcher 同源），
 * 未加入团队由菜单如实呈现。项目角色暂无真实来源，不传（菜单不渲染该行）。
 */
const userName = computed(
  () => auth.user?.nickname || auth.user?.username || auth.account || ""
);
const userEmail = computed(() => auth.user?.email || undefined);
const teamName = computed(() => teamStore.currentTeam?.name);

/** 头像：/v1/file/* 需鉴权下载转 object URL，故异步解析 */
const avatarSrc = ref<string | undefined>(undefined);
watch(
  () => auth.user?.avatar ?? null,
  async avatar => {
    avatarSrc.value = await resolveAvatarSrc(avatar);
  },
  { immediate: true }
);

onMounted(() => {
  if (!auth.signedIn) return;
  // 刷新页面后菜单先用持久化快照即时渲染，再以 /me 校正（如资料已在别处改过）
  void auth.refreshProfile();
  // 团队上下文仅登录流程（AuthLoadingView）首拉、不持久化：刷新页面后须在此补拉，
  // 否则 TeamSwitcher 与组织行误显「未加入团队」。已加载则复用缓存不重复请求。
  void teamStore.load();
});

/** store 项目 → 06 板面板行（分组概念未建，全归 RECENT；meta 空串隐藏） */
const switcherProjects = computed<ProjectSwitcherProject[]>(() =>
  shell.projects.map(project => ({
    id: project.id,
    name: project.name,
    group: "recent",
    meta: ""
  }))
);

/** 触发器标签：切换中显示目标名（07 板过渡），否则当前项目，未选时「选择项目」 */
const switcherLabel = computed(
  () =>
    shell.switchingProject?.name ??
    shell.currentProject?.name ??
    t("shell.app.project.empty")
);

/** 面板状态：切换过渡期间呈 06 板 Loading 态（骨架行） */
const switcherStatus = computed(() =>
  shell.switching ? ("loading" as const) : ("ready" as const)
);

/** 面板文案：默认 Figma 原文为设计参考文本，应用层注入 i18n */
const switcherTexts = computed<ProjectSwitcherText>(() => ({
  ...PROJECT_SWITCHER_DEFAULT_TEXT,
  title: t("shell.app.project.menuLabel"),
  searchPlaceholder: t("shell.app.projectSwitcher.searchPlaceholder"),
  currentBadge: t("shell.app.projectSwitcher.currentBadge"),
  createLabel: t("shell.app.projectSwitcher.createLabel"),
  createUnavailableNote: t("shell.app.projectSwitcher.createUnavailableNote"),
  footerNote: t("shell.app.projectSwitcher.footerNote"),
  emptyTitle: t("shell.app.projectSwitcher.emptyTitle"),
  emptyDescription: t("shell.app.projectSwitcher.emptyDescription"),
  errorTitle: t("shell.app.projectSwitcher.errorTitle"),
  errorDescription: t("shell.app.projectSwitcher.errorDescription"),
  noResultsTitle: t("shell.app.projectSwitcher.noResultsTitle"),
  retryLabel: t("shell.app.projectSwitcher.retryLabel"),
  clearLabel: t("shell.app.projectSwitcher.clearLabel"),
  groupLabels: {
    favorites: t("shell.app.projectSwitcher.groupFavorites"),
    recent: t("shell.app.projectSwitcher.groupRecent")
  }
}));

/**
 * 切换项目（与左导航项目头共用 store 的切换状态机）。
 *
 * **切项目不得静默转移会话与上下文**（G12 · DR-C6）：当前有活跃会话时
 * 先走确认（`requestProjectSwitch` 打开守卫对话框），确认后在目标项目另起。
 */
function onSelectProject(id: string) {
  shell.requestProjectSwitch(id, assistant.activeConversationId !== null);
}

/** 通知：打开关联对象 / 前往完整通知中心 */
function onOpenNotification(target: string) {
  router.push(target);
}

function onViewAllNotifications() {
  router.push("/notifications");
}

/**
 * 顶栏 Switcher 的「Create New Project」入口 → S01-02 项目创建页
 * （Personal Home 的「新建项目」按钮与此同路）。
 */
function onCreateProject() {
  router.push("/project/create");
}

/**
 * 账户菜单普通项 → 设置域路由（Settings Framework 个人/团队域，
 * /settings/*，应用壳内渲染）。
 * 帮助文档暂无应用内落地页（HelpMenu 已覆盖文档入口）。
 */
const ACCOUNT_SELECT_ROUTES: Record<string, string> = {
  "my-profile": "/settings/personal/profile",
  "account-settings": "/settings",
  "team-settings": "/settings/team",
  security: "/settings/personal/security",
  "personal-preferences": "/settings/personal/preferences"
};

function onAccountSelect(id: string) {
  const target = ACCOUNT_SELECT_ROUTES[id];
  if (target) router.push(target);
}

/** 退出登录：清会话回登录页（确认对话框先决） */
async function onSignOutConfirm() {
  await auth.signOut();
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

      <!-- 团队上下文（TeamSwitcher）：同一账号的团队列表；切换团队而非账号
           （UI 细则 #2）；V1 单团队时切换项置灰说明（上游 lmms 单租户用户）。 -->
      <TeamSwitcher />

      <span class="shell-topbar__slash" aria-hidden="true">/</span>

      <!-- 当前项目（Figma 06 板 ProjectSwitcher：五态 Trigger + 搜索/分组/反馈态面板） -->
      <ProjectSwitcher
        :projects="switcherProjects"
        :label="switcherLabel"
        :current-id="shell.currentProjectId"
        :status="switcherStatus"
        :selected="!!shell.currentProject"
        :can-create="true"
        :texts="switcherTexts"
        @select="onSelectProject"
        @create="onCreateProject"
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
      <!-- 语言快捷开关（铃铛旁：中/英单击往返；与设置偏好页同源 lib/i18n） -->
      <LocaleToggle />

      <!-- 通知下拉（铃铛触发，含文本未读徽标；Global/Notification 族） -->
      <NotificationDropdown
        @open="onOpenNotification"
        @view-all="onViewAllNotifications"
      />

      <!-- 主题快捷开关（铃铛旁：深/浅单击往返；与设置偏好页同源 lib/theme） -->
      <ThemeToggle />

      <!-- 专注模式开关（DR-C8）：临时收起 AI Chat 与右栏，主区独占工作区。
           置于顶栏——专注模式下 AI Chat 列已收起，此处是唯一的退出入口。 -->
      <button
        type="button"
        class="shell-topbar__focus"
        :aria-label="
          shell.focusMode
            ? t('shell.app.chat.focusExit')
            : t('shell.app.chat.focusEnter')
        "
        :title="
          shell.focusMode
            ? t('shell.app.chat.focusExit')
            : t('shell.app.chat.focusEnter')
        "
        @click="shell.toggleFocusMode()"
      >
        <Minimize2 v-if="shell.focusMode" :size="14" aria-hidden="true" />
        <Maximize2 v-else :size="14" aria-hidden="true" />
        <span>{{ t("shell.app.chat.focus") }}</span>
      </button>

      <!-- 全局 AI 助手入口（2026-11-08 接线：push 项目 AI 会话页） -->
      <button
        type="button"
        class="shell-topbar__assistant"
        @click="router.push('/project/assistant')"
      >
        <Sparkles :size="14" aria-hidden="true" />
        <span>{{ t("shell.app.topbar.assistant") }}</span>
      </button>

      <!-- 帮助菜单（文档入口 / 键盘快捷键 / 反馈） -->
      <HelpMenu @shortcuts="shortcutsOpen = true" />

      <span class="shell-topbar__divider" aria-hidden="true" />

      <!-- 账户菜单（Figma Global/UserMenu 族）：身份块 = 当前登录用户 -->
      <UserMenuAccountMenu
        :name="userName"
        :email="userEmail"
        :avatar="avatarSrc"
        :tenant="teamName"
        @select="onAccountSelect"
        @shortcuts="shortcutsOpen = true"
        @sign-out="signOutOpen = true"
      />
    </div>

    <!-- 键盘快捷键对话框（账户菜单 / 帮助菜单共用） -->
    <KeyboardShortcutsDialog v-model:open="shortcutsOpen" />

    <!-- 退出登录确认（人工勾选后才可用；确认后清会话回登录页）；
         无邮箱的账号以用户名标识当前身份 -->
    <UserMenuSignOutDialog
      v-model:open="signOutOpen"
      :email="userEmail ?? userName"
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

/* 专注模式开关（DR-C8）——顶栏常驻，是专注模式下唯一的退出入口 */
.shell-topbar__focus {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-topbar__focus:hover {
  background: var(--ws-hover-veil);
  color: var(--ws-text-primary);
}

.shell-topbar__focus:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

@media (prefers-reduced-motion: reduce) {
  .shell-topbar__search {
    transition: none;
  }
}
</style>
