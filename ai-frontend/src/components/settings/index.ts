/**
 * Settings Framework 组件（Settings Framework V1.0 §7）。
 *
 * 复用组件清单（与规格一一对应）：
 *   Settings/PageLayout            → SettingsPageLayout
 *   Settings/Sidebar               → SettingsSidebar
 *   Settings/NavigationItem        → SettingsNavigationItem
 *   Settings/SectionHeader         → SettingsSectionHeader
 *   Settings/FormSection           → SettingsFormSection
 *   Settings/FormRow               → SettingsFormRow
 *   Settings/FieldLabel            → SettingsFieldLabel
 *   Settings/HelperText            → SettingsHelperText
 *   Settings/SaveBar               → SettingsSaveBar
 *   Settings/UnsavedChangesDialog  → SettingsUnsavedChangesDialog
 *   Settings/PermissionNotice      → SettingsPermissionNotice
 *   Settings/ReadOnlyField         → SettingsReadOnlyField
 *   Settings/ValidationMessage     → SettingsValidationMessage
 *   Settings/ConfirmationDialog    → SettingsConfirmationDialog
 *
 * 领域模型与演示 fixture 见 ./settings.model；演示表单闭环见
 * ./useDemoForm（保存/取消/错误态演示）。
 * 约束：按钮/输入/弹窗/通知一律复用 ui/ 基座，不重复造（§7）。
 *
 * 演示节内容（./sections/）：个人域五节的完整表单体，设计目录
 * /design/settings/personal/* 与应用设置页 /settings/personal/*
 * 共用同一份内容（单一事实源，不重复造）。
 */
export { default as SettingsPageLayout } from "./SettingsPageLayout.vue";
export { default as SettingsSidebar } from "./SettingsSidebar.vue";
export { default as SettingsNavigationItem } from "./SettingsNavigationItem.vue";
export { default as SettingsSectionHeader } from "./SettingsSectionHeader.vue";
export { default as SettingsFormSection } from "./SettingsFormSection.vue";
export { default as SettingsFormRow } from "./SettingsFormRow.vue";
export { default as SettingsFieldLabel } from "./SettingsFieldLabel.vue";
export { default as SettingsHelperText } from "./SettingsHelperText.vue";
export { default as SettingsSaveBar } from "./SettingsSaveBar.vue";
export { default as SettingsUnsavedChangesDialog } from "./SettingsUnsavedChangesDialog.vue";
export { default as SettingsPermissionNotice } from "./SettingsPermissionNotice.vue";
export { default as SettingsReadOnlyField } from "./SettingsReadOnlyField.vue";
export { default as SettingsValidationMessage } from "./SettingsValidationMessage.vue";
export { default as SettingsConfirmationDialog } from "./SettingsConfirmationDialog.vue";

export { useDemoForm, type DemoForm } from "./useDemoForm";
export { default as PersonalProfileSection } from "./sections/PersonalProfileSection.vue";
export { default as PersonalSecuritySection } from "./sections/PersonalSecuritySection.vue";
export { default as PersonalPreferencesSection } from "./sections/PersonalPreferencesSection.vue";
export { default as PersonalNotificationsSection } from "./sections/PersonalNotificationsSection.vue";
export { default as PersonalAiSection } from "./sections/PersonalAiSection.vue";

export * from "./settings.model";
