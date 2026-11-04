import { demoOrganization, demoProject, demoUser } from "@/components/shell/demo";

/**
 * 账户菜单演示身份（骨架阶段）。
 * 专有名词沿用壳演示数据（Alex Morgan / Acme Research / Atlas Platform）；
 * 接真实数据源时改为会话身份（stores/auth 的 account/organization）。
 */
export const demoUserProfile = {
  name: demoUser.name,
  initials: demoUser.initials,
  email: "alex.morgan@acme.dev",
  tenant: demoOrganization.name,
  /** 项目内角色串（Figma「Atlas Platform · Contributor」口径） */
  projectRole: `${demoProject.name} · Contributor`
} as const;
