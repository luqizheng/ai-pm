export const BUTTON_VARIANTS = [
  "primary",
  "secondary",
  "tertiary",
  "ghost",
  "danger"
] as const;
export type ButtonVariant = (typeof BUTTON_VARIANTS)[number];

export const BUTTON_SIZES = ["sm", "md", "lg", "icon"] as const;
export type ButtonSize = (typeof BUTTON_SIZES)[number];

/**
 * secondary 变体的表面档。
 *
 * base —— 组件库默认（surface #1D212A），用于画布底上的次级动作；
 * elevated —— 抬起面（elevated #252A35），用于表面色面板内的次级动作
 * （Figma「02 — Authentication」的 SSO / 返回按钮）。
 */
export const BUTTON_SURFACES = ["base", "elevated"] as const;
export type ButtonSurface = (typeof BUTTON_SURFACES)[number];
