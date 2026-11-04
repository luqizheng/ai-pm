/**
 * 认证域共享组件（Figma「02 — Authentication」九帧的公共骨架）。
 *
 * 九帧共用同一页面骨架与信息层级：页头/页脚（AuthLayout）、面板（AuthPanel）、
 * 标题组（AuthHeading）、反馈（复用 @/components/ui/alert/Alert）、
 * 文字动作（AuthTextAction）、提供方分隔线（AuthProviderDivider）、
 * 信息卡与信息行（AuthDetailCard / AuthDetailRow）、
 * 口令要求清单（AuthRequirementList）、认证进度（AuthProgressSteps）。
 */
export { default as AuthDetailCard } from "./AuthDetailCard.vue";
export { default as AuthDetailRow } from "./AuthDetailRow.vue";
export { default as AuthHeading } from "./AuthHeading.vue";
export { default as AuthLayout } from "./AuthLayout.vue";
export { default as AuthPanel } from "./AuthPanel.vue";
export { default as AuthProgressSteps } from "./AuthProgressSteps.vue";
export type { AuthStep, AuthStepState } from "./AuthProgressSteps.vue";
export { default as AuthProviderDivider } from "./AuthProviderDivider.vue";
export { default as AuthRequirementList } from "./AuthRequirementList.vue";
export type { AuthRequirement } from "./AuthRequirementList.vue";
export { default as AuthTextAction } from "./AuthTextAction.vue";
export { default as ProductMark } from "./ProductMark.vue";
