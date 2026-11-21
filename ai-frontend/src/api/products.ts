import { http } from "@/api/base";

/**
 * Product Membership & Lifecycle —— 前端适配层（P4-ORG Phase 3）。
 *
 * 后端契约（apps/pm-api/src/products.rs；DR-PA-01/02/03）：
 * Product Membership 由 Product 内部管理（Team Member 不自动成为 Product
 * 成员）；双身份 Owner/Member；生命周期 {active, abandoned}；Link 建立/
 * 解除 = Team Owner ∪ Product Owner，abandoned 拦截。
 *
 * 稳定错误 code/message：team_owner_required / product_owner_required /
 * not_a_product_member / last_owner / already_product_member / already_owner /
 * product_member_not_found / product_abandoned / link_already_exists /
 * product_lifecycle_noop。
 */

/* --------------------------------- 类型 ----------------------------------- */

/** 生命周期（0044 四值化）：normal/suspended/resuming/archived */
export type ProductLifecycleStatus =
  | "normal"
  | "suspended"
  | "resuming"
  | "archived";

export interface ProductProfile {
  id: string;
  team_id: string;
  /** 稳定业务标识（PA-07 §4.8）：全局唯一、创建时自动生成、不可变；引用用 key，关联走 id */
  key: string;
  name: string;
  description: string | null;
  status: ProductLifecycleStatus;
  updated_at: string;
}

export interface ProductMember {
  principal_id: string;
  display_name: string;
  role: "owner" | "member";
  is_active: boolean;
  created_at: string;
}

export interface ProductLink {
  id: string;
  product_id: string;
  project_id: string;
  created_at: string;
}

/* ------------------------------- Team 面（创建/列表） ---------------------- */

export interface CreateProductBody {
  name: string;
  description?: string;
  first_owner_principal_id: string;
}

/** POST /v1/pm/teams/{team_id}/products（Team Owner；指定首位 Product Owner） */
export async function createProduct(
  teamId: string,
  body: CreateProductBody
): Promise<ProductProfile> {
  return http.post<ProductProfile>(`/v1/pm/teams/${teamId}/products`, body);
}

/**
 * GET /v1/pm/teams/{team_id}/products —— Team Owner 全量；普通 Team Member
 * 仅返回自己参与的（PA §6 信息路由）。
 */
export async function listProducts(teamId: string): Promise<ProductProfile[]> {
  const res = await http.get<{ items: ProductProfile[] }>(
    `/v1/pm/teams/${teamId}/products`
  );
  return res.items ?? [];
}

/* ------------------------------- Product 面 -------------------------------- */

/** GET /v1/pm/products/{id}（Product Member 可读） */
export async function fetchProduct(productId: string): Promise<ProductProfile> {
  return http.get<ProductProfile>(`/v1/pm/products/${productId}`);
}

export async function listProductMembers(
  productId: string
): Promise<ProductMember[]> {
  const res = await http.get<{ items: ProductMember[] }>(
    `/v1/pm/products/${productId}/members`
  );
  return res.items ?? [];
}

/** POST /v1/pm/products/{id}/members（所有 Product Member 平权；role 恒 member） */
export async function addProductMember(
  productId: string,
  principalId: string
): Promise<void> {
  await http.post(`/v1/pm/products/${productId}/members`, {
    principal_id: principalId
  });
}

/** DELETE /v1/pm/products/{id}/members/{pid}（移除普通成员；末位 Owner 服务端拒绝） */
export async function removeProductMember(
  productId: string,
  principalId: string
): Promise<void> {
  await http.delete(`/v1/pm/products/${productId}/members/${principalId}`);
}

/** POST .../members/{pid}/appoint-owner（任一 Product Owner：继任指定，多 Owner 并存） */
export async function appointProductOwner(
  productId: string,
  principalId: string
): Promise<void> {
  await http.post(
    `/v1/pm/products/${productId}/members/${principalId}/appoint-owner`,
    {}
  );
}

/** POST .../members/{pid}/demote-owner（第一轮仅 Team Owner） */
export async function demoteProductOwner(
  productId: string,
  principalId: string
): Promise<void> {
  await http.post(
    `/v1/pm/products/${productId}/members/${principalId}/demote-owner`,
    {}
  );
}

/** POST /v1/pm/products/{id}/lifecycle {action: abandon|restart}（任意 Product Owner） */
export async function setProductLifecycle(
  productId: string,
  action: "abandon" | "restart"
): Promise<ProductProfile> {
  return http.post<ProductProfile>(`/v1/pm/products/${productId}/lifecycle`, {
    action
  });
}

/* ---------------------------------- Link ---------------------------------- */

export async function listProductLinks(
  productId: string
): Promise<ProductLink[]> {
  const res = await http.get<{ items: ProductLink[] }>(
    `/v1/pm/products/${productId}/links`
  );
  return res.items ?? [];
}

export async function createProductLink(
  productId: string,
  projectId: string
): Promise<{ id: string }> {
  return http.post<{ id: string }>(`/v1/pm/products/${productId}/links`, {
    project_id: projectId
  });
}

export async function endProductLink(linkId: string): Promise<void> {
  await http.post(`/v1/pm/product-links/${linkId}/end`, {});
}
