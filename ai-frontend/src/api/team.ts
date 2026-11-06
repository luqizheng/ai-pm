import { http } from "@/api/base";

/**
 * Team Onboarding & Team Management —— 前端适配层。
 *
 * 后端契约（apps/pm-api/src/team_onboarding.rs，0044_team_onboarding.sql）：
 * Team ≡ tenant（DEC-LLM-001 口径），本模块是团队域的唯一服务入口。
 *
 * 稳定错误 message（`{message}` 单字段契约，前端据其渲染结果态）：
 *   team_already_exists / slug_already_taken / pending_invitation_exists /
 *   invitation_expired / invitation_revoked / invitation_already_accepted /
 *   invitation_link_exhausted / invitation_not_found / email_mismatch /
 *   invitation_tenant_mismatch / team_suspended / invalid email 等。
 *
 * V1 边界（2026-11-06 裁决）：无邮件投递——创建邀请/链接只返回 accept_path
 * 供复制分享；同一用户多团队待上游 lmms 支持，memberships 契约已按多团队
 * shape 设计。
 */

/* ------------------------------- /v1/pm/me 团队扩展 ----------------------- */

export interface TeamProfile {
  id: string;
  name: string;
  slug: string | null;
  description: string | null;
}

/** GET /v1/pm/me 的团队成员关系（V1 单团队现实下为 0/1 行） */
export interface TeamMembership {
  team_id: string;
  team_name: string;
  role: "owner" | "admin" | "member";
  status: string;
}

export interface TeamContext {
  principal_id: string;
  display_name: string;
  is_active: boolean;
  is_admin: boolean;
  tenant_id: string;
  tenant_role: string;
  team: TeamProfile | null;
  onboarding_required: boolean;
  memberships: TeamMembership[];
}

/** 登录后的团队上下文（AuthLoadingView 分支 + TeamSwitcher 数据源） */
export async function fetchTeamContext(): Promise<TeamContext> {
  return http.get<TeamContext>("/v1/pm/me");
}

/* --------------------------------- 建团 / 档案 ---------------------------- */

export interface CreateTeamBody {
  name: string;
  slug?: string;
  description?: string;
}

/** POST /v1/pm/teams —— 认领当前 token 租户（零 ACTIVE 成员前提） */
export async function createTeam(body: CreateTeamBody): Promise<TeamProfile> {
  return http.post<TeamProfile>("/v1/pm/teams", body);
}

/** GET /v1/pm/tenants/{id} —— 团队档案（成员可读） */
export async function fetchTeamProfile(tenantId: string): Promise<TeamProfile> {
  return http.get<TeamProfile>(`/v1/pm/tenants/${tenantId}`);
}

export interface UpdateTeamBody {
  name?: string;
  slug?: string | null;
  description?: string | null;
}

/** PATCH /v1/pm/tenants/{id} —— 修改团队资料（owner/admin） */
export async function updateTeamProfile(
  tenantId: string,
  body: UpdateTeamBody
): Promise<TeamProfile> {
  return http.patch<TeamProfile>(`/v1/pm/tenants/${tenantId}`, body);
}

export interface DissolveOutcome {
  projects_deleted: number;
  members_removed: number;
}

/** POST /v1/pm/tenants/{id}/dissolve —— 解散团队（Owner 独占；projects 终态软删） */
export async function dissolveTeam(tenantId: string): Promise<DissolveOutcome> {
  return http.post<DissolveOutcome>(`/v1/pm/tenants/${tenantId}/dissolve`, {});
}

/* ------------------------------------ 成员 -------------------------------- */

/** GET /v1/pm/tenants/{id}/members 行（既有端点，Members 页数据源） */
export interface TeamMember {
  principal_id: string;
  display_name: string;
  role: "owner" | "admin" | "member";
  is_active: boolean;
  created_at: string;
}

export async function listTeamMembers(tenantId: string): Promise<TeamMember[]> {
  const res = await http.get<{ items: TeamMember[] }>(
    `/v1/pm/tenants/${tenantId}/members`
  );
  return res.items ?? [];
}

/** PATCH /v1/pm/tenants/{id}/members/{pid}/role（owner/admin；admin 不可动 owner） */
export async function setTeamMemberRole(
  tenantId: string,
  principalId: string,
  role: "admin" | "member"
): Promise<void> {
  await http.patch(`/v1/pm/tenants/${tenantId}/members/${principalId}/role`, {
    role
  });
}

/** DELETE /v1/pm/tenants/{id}/members/{pid}（末位 Owner 由服务端拒绝） */
export async function removeTeamMember(
  tenantId: string,
  principalId: string
): Promise<void> {
  await http.delete(`/v1/pm/tenants/${tenantId}/members/${principalId}`);
}

/** POST /v1/pm/tenants/{id}/owner-transfer */
export async function transferTeamOwnership(
  tenantId: string,
  newOwnerPrincipalId: string
): Promise<unknown> {
  return http.post(`/v1/pm/tenants/${tenantId}/owner-transfer`, {
    new_owner_principal_id: newOwnerPrincipalId
  });
}

/* ---------------------------------- 邮箱邀请 ------------------------------ */

export type InvitationStatus = "PENDING" | "ACCEPTED" | "EXPIRED" | "REVOKED";

export interface TeamInvitation {
  id: string;
  email: string;
  role: "admin" | "member";
  status: InvitationStatus;
  invited_by: string;
  invited_by_name: string;
  expires_at: string;
  created_at: string;
  accepted_at: string | null;
}

export async function listTeamInvitations(
  tenantId: string
): Promise<TeamInvitation[]> {
  return http.get<TeamInvitation[]>(`/v1/pm/tenants/${tenantId}/invitations`);
}

/** 创建邀请的一次性返回：明文 token/接受路径只在此刻出现 */
export interface InvitationCreated {
  id: string;
  token: string;
  accept_path: string;
  expires_at: string;
}

export async function createTeamInvitation(
  tenantId: string,
  body: { email: string; role: "admin" | "member" }
): Promise<InvitationCreated> {
  return http.post<InvitationCreated>(
    `/v1/pm/tenants/${tenantId}/invitations`,
    body
  );
}

export async function revokeTeamInvitation(
  tenantId: string,
  invitationId: string
): Promise<void> {
  await http.post(
    `/v1/pm/tenants/${tenantId}/invitations/${invitationId}/revoke`,
    {}
  );
}

/* ---------------------------------- 邀请链接 ------------------------------ */

export interface InvitationLink {
  id: string;
  default_role: "member";
  status: "ACTIVE" | "EXHAUSTED" | "EXPIRED" | "REVOKED";
  max_uses: number | null;
  used_count: number;
  expires_at: string;
  created_at: string;
}

export async function listInvitationLinks(
  tenantId: string
): Promise<InvitationLink[]> {
  return http.get<InvitationLink[]>(
    `/v1/pm/tenants/${tenantId}/invitation-links`
  );
}

export interface LinkCreated {
  id: string;
  token: string;
  accept_path: string;
  expires_at: string;
}

export async function createInvitationLink(
  tenantId: string,
  body: { expires_at?: string; max_uses?: number | null }
): Promise<LinkCreated> {
  return http.post<LinkCreated>(
    `/v1/pm/tenants/${tenantId}/invitation-links`,
    body
  );
}

export async function revokeInvitationLink(
  tenantId: string,
  linkId: string
): Promise<void> {
  await http.post(
    `/v1/pm/tenants/${tenantId}/invitation-links/${linkId}/revoke`,
    {}
  );
}

/* ------------------------------- 预览与接受（自助） ------------------------ */

export interface InvitationPreview {
  team_name: string;
  team_slug: string | null;
  inviter_name: string;
  role: string;
  expires_at: string;
  status: string;
  email: string;
}

export async function previewInvitation(
  token: string
): Promise<InvitationPreview> {
  return http.get<InvitationPreview>(
    `/v1/pm/invitations/preview?token=${encodeURIComponent(token)}`
  );
}

export interface LinkPreview {
  team_name: string;
  default_role: string;
  expires_at: string;
  status: string;
  remaining_uses: number | null;
}

export async function previewInvitationLink(
  token: string
): Promise<LinkPreview> {
  return http.get<LinkPreview>(
    `/v1/pm/invitation-links/preview?token=${encodeURIComponent(token)}`
  );
}

export interface TeamAcceptOutcome {
  outcome: "joined" | "already_member";
  team_id: string;
  team_name: string;
  role: string;
}

export async function acceptTeamInvitation(body: {
  token: string;
  /** iam UserInfo 的 email（JWT 无 email claim，V1 尽力比对） */
  email?: string;
}): Promise<TeamAcceptOutcome> {
  return http.post<TeamAcceptOutcome>("/v1/pm/invitations/accept", body);
}

export async function acceptInvitationLink(body: {
  token: string;
}): Promise<TeamAcceptOutcome> {
  return http.post<TeamAcceptOutcome>("/v1/pm/invitation-links/accept", body);
}

/* ---------------------------------- 工具 ---------------------------------- */

/** 接受路径 → 完整 URL（复制分享用；V1 无邮件投递） */
export function buildAcceptUrl(acceptPath: string): string {
  if (typeof window === "undefined") return acceptPath;
  return new URL(acceptPath, window.location.origin).toString();
}

/** 邀请域稳定错误 → 结果态 key（Screen 07 七态 + 预览无效） */
export function invitationErrorKey(message: string): string {
  switch (message) {
    case "invitation_expired":
      return "expired";
    case "invitation_revoked":
      return "revoked";
    case "invitation_already_accepted":
      return "alreadyAccepted";
    case "invitation_link_exhausted":
      return "exhausted";
    case "email_mismatch":
      return "emailMismatch";
    case "invitation_tenant_mismatch":
      return "tenantMismatch";
    case "team_suspended":
      return "teamSuspended";
    default:
      return "invalid";
  }
}
