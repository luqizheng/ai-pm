import { computed } from "vue";
import { useRoute } from "vue-router";
import { isAssistantHiddenPath } from "@/router/navigation";
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

  /** 不出现 AI 助手的页面（管理组 + 项目设置）：顶栏入口与 AI 面板齐收。 */
  const isAssistantHiddenRoute = computed(() =>
    isAssistantHiddenPath(route.path)
  );

  /** 完整会话页：该页本身即会话面，助手列收起避免同屏重复。 */
  const isAssistantRoute = computed(() => route.path === ASSISTANT_PATH);

  /** AI Chat 列生效可见性（面板总开关 × 专注 × 会话页 × 助手隐藏页）。 */
  const assistantVisible = computed(
    () =>
      shell.aiPanelOpen &&
      !shell.focusMode &&
      !isAssistantRoute.value &&
      !isAssistantHiddenRoute.value
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
      !isAssistantHiddenRoute.value
  );

  /** 专注模式可用性：助手不可用的页面本无 AI 面板可收，专注对其无意义。 */
  const focusAvailable = computed(() => !isAssistantHiddenRoute.value);

  return {
    isAssistantHiddenRoute,
    isAssistantRoute,
    assistantVisible,
    contextVisible,
    focusAvailable
  };
}
