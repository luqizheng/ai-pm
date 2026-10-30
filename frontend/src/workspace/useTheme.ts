import { ref, type Ref } from "vue";

/** 工作区主题。深色为默认工作主题（用户裁决 2026-10-31）。 */
export type Theme = "dark" | "light";

export const THEME_STORAGE_KEY = "aipm.theme";

export interface ThemeController {
  theme: Ref<Theme>;
  setTheme(theme: Theme): void;
  toggle(): void;
}

/**
 * 创建独立的主题控制器（读当前 DOM/localStorage 快照）。
 * 测试用它拿干净实例；应用入口用下方 useTheme 单例。
 */
export function createThemeController(): ThemeController {
  const initial = readInitial();
  const theme = ref<Theme>(initial);
  apply(initial);

  function readInitial(): Theme {
    const fromDom = document.documentElement.dataset.theme;
    if (fromDom === "light" || fromDom === "dark") return fromDom;
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === "light" || stored === "dark") return stored;
    return "dark";
  }

  function apply(value: Theme) {
    document.documentElement.dataset.theme = value;
  }

  function setTheme(value: Theme) {
    theme.value = value;
    apply(value);
    localStorage.setItem(THEME_STORAGE_KEY, value);
  }

  function toggle() {
    setTheme(theme.value === "dark" ? "light" : "dark");
  }

  return { theme, setTheme, toggle };
}

let singleton: ThemeController | undefined;

/** 应用内共享的主题控制器（TopBar 等处使用）。 */
export function useTheme(): ThemeController {
  singleton ??= createThemeController();
  return singleton;
}
