import { http } from "@/api/base";
import type { Customer, Paged } from "./types";

/**
 * 客户主数据端点（0017）。
 *
 * 契约：列表裸 `{items,total,page,size}`；错误体只带 `{message}`。
 * 客户状态 `placeholder` 表示 0017 之前按项目 code 隐式生成的未归类客户，
 * 需人工改为真实客户后再归档。
 */
export function listCustomers(
  params: { q?: string; status?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<Customer>>("/v1/pm/customers", params);
}

export function getCustomer(id: string) {
  return http.get<Customer>(`/v1/pm/customers/${id}`);
}

export function createCustomer(body: {
  code: string;
  name: string;
  short_name?: string;
  industry?: string;
  notes?: string;
  parent_id?: string;
}) {
  return http.post<Customer>("/v1/pm/customers", body);
}

/** 字段缺省即不改；可空文本传空串即清空。 */
export function updateCustomer(
  id: string,
  body: {
    code?: string;
    name?: string;
    short_name?: string;
    industry?: string;
    notes?: string;
    status?: "active" | "archived";
    parent_id?: string;
  }
) {
  return http.patch<Customer>(`/v1/pm/customers/${id}`, body);
}
