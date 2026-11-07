import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import type { BreadcrumbItem } from "@/components/ui/breadcrumb";
import { PROJECT_APP_NAV, PROJECT_ICONS } from "@/views/settings/nav";

/**
 * Project Setting 各节共用的壳装配（SettingsAppFrame 的项目域参数）。
 *
 * V1.0 稿原则：设置始终属于当前 Project——面包屑 = Projects / 项目名 /
 * 当前节；项目名从全局 scope store 派生（ensureLoaded 单飞去重，重复调用
 * 无额外请求），不在设置页内重复选择项目。
 */
export function useProjectSettingsFrame() {
  const { t } = useI18n();
  const scope = useScopeStore();
  // ensureLoaded 单飞去重；失败不缓存（下次进入可重试），浮动 promise 须兜底
  void scope.ensureLoaded().catch(() => undefined);

  /** 当前项目显示名；scope 未选中具体项目时回退为空（由调用方兜底） */
  const projectName = computed(() => {
    if (scope.currentProject) return scope.currentProject.name;
    const id = scope.currentProjectId;
    return id && id !== SCOPE_ALL ? id : "";
  });

  /** 面包屑前两层：Projects → 项目名（回总控台）；详情页在此之上续接 */
  function baseCrumbs(): BreadcrumbItem[] {
    return [
      { label: t("s01.command-center.breadcrumbProjects"), to: "/projects" },
      {
        label: projectName.value || t("environments.breadcrumb.project"),
        to: "/project/command-center"
      }
    ];
  }

  /** 面包屑：Projects → 项目名 → 当前节标题 */
  function breadcrumb(activeLabel: string): BreadcrumbItem[] {
    return [...baseCrumbs(), { label: activeLabel }];
  }

  return {
    scope,
    projectName,
    baseCrumbs,
    breadcrumb,
    /** 侧栏条目（General/Members/AI 三节 proposed，见 nav.ts 说明） */
    navEntries: PROJECT_APP_NAV,
    navIcons: PROJECT_ICONS,
    /** 侧栏域标题（PROJECT SETTINGS） */
    scopeLabel: computed(() => t("settings.project.landing.title"))
  };
}
