import { http } from "./client";
import type { LoginResult } from "./client";
import { iam } from "./iam";
import { prue } from "./prue";

export interface LoginRequest {
  username: string;
  password: string;
}

export interface SetupStatus {
  setup_required: boolean;
}

/** 菜单树（来自 DB /v1/iam/menus） */
export interface IamMenu {
  id: string;
  parent_id?: string;
  name: string;
  code: string;
  path: string;
  component?: string;
  icon?: string;
  sort_order: number;
  visible: boolean;
  status: string;
  permission_id?: string;
  children: IamMenu[];
}

export interface IamMenuListResponse {
  items: IamMenu[];
  total: number;
}

/**
 * 动态路由节点（来自 prue_backend 的菜单配置激活版本）。
 *
 * 协议来源：GET /v1/pure/get-async-routes → { code, message, data }
 */
export interface RouteNodeMeta {
  title?: string;
  icon?: string;
  showLink?: boolean;
  rank?: number;
  auths?: string[];
  roles?: string[];
  keepAlive?: boolean;
  hiddenTag?: boolean;
}

export interface RouteNode {
  path: string;
  name?: string;
  component?: string;
  redirect?: string;
  meta?: RouteNodeMeta;
  children?: RouteNode[];
}

export interface AsyncRoutesResponse {
  code: number;
  message: string;
  data: RouteNode[];
}

/** 菜单配置版本（prue_backend 的菜单配置管理面） */
export interface MenuConfigItem {
  id: string;
  name: string;
  description?: string | null;
  version: number;
  is_active: boolean;
  created_by?: string | null;
  created_at: string;
}

export interface MenuConfigListResponse {
  items: MenuConfigItem[];
  total: number;
}

/** 登录 —— lmms iam-service（经 iam-services SDK，走 /gw → api-gateway） */
export function login(data: LoginRequest): Promise<LoginResult> {
  return iam.auth.login(data);
}

export function logout() {
  return iam.auth.logout();
}

export function getSetupStatus(): Promise<SetupStatus> {
  return iam.auth.setupStatus();
}

/** 拉取当前激活的动态路由树（prue-services SDK，信封已在 SDK 内解包） */
export async function getAsyncRoutes(): Promise<RouteNode[]> {
  return prue.routes.getAsync();
}

/** 菜单配置版本列表 */
export async function listMenuConfigs(): Promise<MenuConfigListResponse> {
  return prue.configs.list();
}

/** 发布某个菜单配置版本（激活它） */
export async function publishMenuConfig(id: string): Promise<void> {
  await prue.configs.publish(id);
}

/** 克隆某个菜单配置版本 */
export async function cloneMenuConfig(id: string): Promise<void> {
  await prue.configs.clone(id);
}

export { http, translateError } from "./client";
export type { LoginResult } from "./client";
