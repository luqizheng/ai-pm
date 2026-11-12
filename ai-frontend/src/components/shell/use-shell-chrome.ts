import { computed } from "vue";
import { useRoute } from "vue-router";
import { isManagementPath } from "@/router/navigation";
import { useShellStore } from "@/stores/shell";

/** 完整会话页路由——该页本身即会话面。 */
export const ASSISTANT_PATH = "/project/assistant";

/**
 * 壳层 chrome 可见性（路由 × 壳开关的**单源**派生）。
 *
 * 「哪些页面不出现 AI 助手与上下文栏」此前散落在 WorkspaceShell / TopBar
 * 两处各自的 computed；本 composable 把它收拢为一处，壳的各消费方
 * （列渲染、壳根 data-* 属性、顶栏按钮）全部从这里取值。
 *
 * 关键点是**生效态**（effective）：列被 v-if 移除时，壳根 data-* 必须同步
 * 收列——否则 grid 模板仍按四列计算，主区右侧留出 AI 列宽的空带
 * （管理页内容挤在左侧、右侧大块空白即该缺陷）。
 */
export function useShellChrome() {
  const route = useRoute();
  const shell = useShellStore();

  /** 管理页（导航「管理」组）：租户运营面，AI 助手不出现。 */
  const isManagementRoute = computed(() => isManagementPath(route.path));

  /** 完整会话页：该页本身即会话面，助手列收起避免同屏重复。 */
  const isAssistantRoute = computed(() => route.path === ASSISTANT_PATH);

  /** AI Chat 列生效可见性（面板总开关 × 专注 × 会话页 × 管理页）。 */
  const assistantVisible = computed(
    () =>
      shell.aiPanelOpen &&
      !shell.focusMode &&
      !isAssistantRoute.value &&
      !isManagementRoute.value
  );

  /**
   * 右上下文栏生效可见性。**不含** isAssistantRoute：完整会话页仍保留
   * 上下文（仅助手列收起，由 data-ai-panel=closed + data-context=open
   * 的三列特例模板承载）。
   */
  const contextVisible = computed(
    () =>
      shell.aiPanelOpen &&
      shell.contextOpen &&
      !shell.focusMode &&
      !isManagementRoute.value
  );

  /** 专注模式可用性：管理页本无 AI 面板可收，专注对其无意义（按钮隐藏）。 */
  const focusAvailable = computed(() => !isManagementRoute.value);

  return {
    isManagementRoute,
    isAssistantRoute,
    assistantVisible,
    contextVisible,
    focusAvailable
  };
}
