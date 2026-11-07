import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import type { BreadcrumbItem } from "@/components/ui/breadcrumb";
import { PROJECT_APP_NAV, PROJECT_ICONS } from "@/views/settings/nav";

/**
 * Project Setting 各节共用的壳装配与项目上下文（单一来源）。
 *
 * V1.0 稿原则：设置始终属于当前 Project，进入后各节自动继承项目上下文、
 * 不再重复选择项目。上下文 = 路由 `?project_id=`（总控台带入）优先，回退
 * 全局 scope store；站内跳转（侧栏切节 / 列表进详情 / 面包屑回总控台）
 * 一律经 withProject 携带，避免切节丢上下文。
 */
export function useProjectSettingsFrame() {
  const { t } = useI18n();
  const route = useRoute();
  const scope = useScopeStore();
  // ensureLoaded 单飞去重；失败不缓存（下次进入可重试），浮动 promise 须兜底
  void scope.ensureLoaded().catch(() => undefined);

  /** 当前项目 id；无上下文时为空串（scope 哨兵 SCOPE_ALL 视为无） */
  const projectId = computed(() => {
    const query = route.query.project_id;
    if (typeof query === "string" && query) return query;
    const id = scope.currentProjectId;
    return id && id !== SCOPE_ALL ? id : "";
  });

  /** 当前项目显示名；项目目录未命中时为空（面包屑省略该层，不显示裸 id） */
  const projectName = computed(
    () =>
      scope.projects.find(project => project.id === projectId.value)?.name ?? ""
  );

  /** 站内路径附带项目上下文（无上下文时原样返回） */
  function withProject(path: string): string {
    return projectId.value
      ? `${path}?project_id=${encodeURIComponent(projectId.value)}`
      : path;
  }

  /** 面包屑前缀：Projects →（项目名，回总控台）；详情页在此之上续接 */
  function baseCrumbs(): BreadcrumbItem[] {
    const crumbs: BreadcrumbItem[] = [
      { label: t("s01.command-center.breadcrumbProjects"), to: "/projects" }
    ];
    if (projectName.value) {
      crumbs.push({
        label: projectName.value,
        to: withProject("/project/command-center")
      });
    }
    return crumbs;
  }

  /** 面包屑：Projects → 项目名 → 当前节标题 */
  function breadcrumb(activeLabel: string): BreadcrumbItem[] {
    return [...baseCrumbs(), { label: activeLabel }];
  }

  /** 侧栏条目（携带项目上下文；General/Members/AI 三节 proposed，见 nav.ts） */
  const navEntries = computed(() =>
    PROJECT_APP_NAV.map(entry => ({ ...entry, to: withProject(entry.to) }))
  );

  return {
    scope,
    projectId,
    projectName,
    withProject,
    baseCrumbs,
    breadcrumb,
    navEntries,
    navIcons: PROJECT_ICONS,
    /** 侧栏域标题（PROJECT SETTINGS） */
    scopeLabel: computed(() => t("settings.project.landing.title"))
  };
}
