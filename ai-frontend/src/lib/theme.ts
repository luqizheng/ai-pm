export type Theme = "dark" | "light";

/** 与 index.html 防闪烁脚本同一持久化键与取值（"dark" | "light"） */
export const THEME_STORAGE_KEY = "aipm.theme";

/** 读取当前主题（无存储/非法值回退深色默认） */
export function currentTheme(): Theme {
  try {
    const value = localStorage.getItem(THEME_STORAGE_KEY);
    return value === "light" || value === "dark" ? value : "dark";
  } catch {
    return "dark";
  }
}

/** 应用主题：写 html data-theme（驱动 tokens.css 的 [data-theme="light"] 覆盖段）+ 持久化 */
export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // 持久化失败不影响本次切换
  }
}
