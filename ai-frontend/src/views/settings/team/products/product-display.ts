import type { ProductLifecycleStatus } from "@/api/products";

/**
 * Product 生命周期呈现映射（设计稿 Lifecycle 列）。
 *
 * 后端 0044 四值化 {normal, suspended, resuming, archived}；设计稿只画了
 * Active / Abandoned 两态——suspended/resuming 按同风格补位呈现，动作仍以
 * 后端规则为准。
 */
export interface LifecyclePresentation {
  labelKey: string;
  /** neutral=常规态 / accent=进行中（Active/Resuming） / warning=挂起 */
  variant: "neutral" | "accent" | "warning";
}

export function lifecyclePresentation(status: string): LifecyclePresentation {
  switch (status as ProductLifecycleStatus) {
    case "normal":
      return { labelKey: "team.products.lifecycleNormal", variant: "accent" };
    case "suspended":
      return {
        labelKey: "team.products.lifecycleSuspended",
        variant: "warning"
      };
    case "resuming":
      return { labelKey: "team.products.lifecycleResuming", variant: "accent" };
    case "archived":
      return {
        labelKey: "team.products.lifecycleArchived",
        variant: "neutral"
      };
    default:
      return { labelKey: "team.products.lifecycleNormal", variant: "neutral" };
  }
}

/**
 * Product Key 派生预览（PA-07 §4.8 创建流程 C1）——与后端
 * `derive_product_key_base` 同规则的前端镜像：非 [A-Za-z0-9] 连续序列折叠
 * 为 `-`，去首尾后大写，截断 64。仅作创建页实时预览；权威值以服务端落库为准。
 */
export function deriveProductKeyPreview(name: string): string {
  const collapsed = name
    .toUpperCase()
    .split("")
    .map(char => (/[A-Z0-9]/.test(char) ? char : "-"))
    .join("");
  return collapsed
    .split("-")
    .filter(part => part !== "")
    .join("-")
    .slice(0, 64);
}

/** 头像/徽标用展示名缩写（Avatar 组件已内置，此处供文本场景） */
export function initialsOf(displayName: string): string {
  const words = displayName.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[words.length - 1][0]).toUpperCase();
}
