import { ref } from "vue";

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

/** 壳内共享的响应式主题（初始取持久化值；applyTheme 是唯一写入点，各入口自动同步） */
const themeRef = ref<Theme>(currentTheme());

/** 应用主题：写 html data-theme（驱动 tokens.css 的 [data-theme="light"] 覆盖段）+ 持久化 */
export function applyTheme(theme: Theme) {
  themeRef.value = theme;
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // 持久化失败不影响本次切换
  }
}

/**
 * 壳内主题读写（TopBar 快捷开关与设置偏好页共用同一状态：
 * 任一入口切换，另一入口的呈现即时跟随）。
 */
export function useTheme() {
  return {
    theme: themeRef,
    setTheme: applyTheme,
    toggleTheme() {
      applyTheme(themeRef.value === "dark" ? "light" : "dark");
    }
  };
}
