import { computed, ref } from "vue";
import { defineStore } from "pinia";

import {
  fetchTeamContext,
  type TeamContext,
  type TeamMembership,
  type TeamProfile
} from "@/api/team";

/**
 * 团队上下文 store（app-team）。
 *
 * 数据源 = GET /v1/pm/me 的团队扩展面（team / onboarding_required /
 * memberships）。登录后由 AuthLoadingView 首拉：
 *   - onboarding_required=true → /onboarding（首登必须建团或入团，Q1）；
 *   - 否则按原流程进入工作区。
 *
 * V1 边界（2026-11-06 裁决）：同一用户多团队待上游 lmms 支持——TeamSwitcher
 * 按 memberships 渲染，单团队时切换项置灰说明；本 store 不做跨团队切换写侧。
 */
export const useTeamStore = defineStore("app-team", () => {
  const context = ref<TeamContext | null>(null);
  const loading = ref(false);
  const loaded = ref(false);
  const error = ref<string | null>(null);

  const currentTeam = computed<TeamProfile | null>(
    () => context.value?.team ?? null
  );
  const memberships = computed<TeamMembership[]>(
    () => context.value?.memberships ?? []
  );
  /** 团队内角色（角色即权限：页面据此渲染操作入口，服务端同矩阵强制）。
   * P4-ORG：Team 角色仅 owner|member（admin 已取消），事实源 = core.team_member。 */
  const teamRole = computed<"owner" | "member">(() =>
    context.value?.team_role === "owner" ? "owner" : "member"
  );
  /** Team Owner（Team 面管理操作的唯一角色；旧 isTeamAdmin 语义收敛于此） */
  const isTeamOwner = computed(() => teamRole.value === "owner");
  const onboardingRequired = computed(
    () => context.value?.onboarding_required ?? false
  );

  /** 首拉团队上下文；已加载且不强制时直接复用 */
  async function load(force = false): Promise<TeamContext | null> {
    if (loaded.value && !force) return context.value;
    loading.value = true;
    error.value = null;
    try {
      context.value = await fetchTeamContext();
      loaded.value = true;
      return context.value;
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
      return null;
    } finally {
      loading.value = false;
    }
  }

  /** 建团/接受邀请等写操作后刷新本地缓存 */
  async function refresh(): Promise<TeamContext | null> {
    return load(true);
  }

  function reset(): void {
    context.value = null;
    loaded.value = false;
    error.value = null;
  }

  return {
    context,
    loading,
    loaded,
    error,
    currentTeam,
    memberships,
    teamRole,
    isTeamOwner,
    onboardingRequired,
    load,
    refresh,
    reset
  };
});
