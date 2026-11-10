import { http } from "@/api/base";

/**
 * 系统级配置 API（管理面；`/admin/settings` 消费）。
 *
 * 门：后端 `is_admin`（系统管理员；AI/Key/普通成员 403）。
 * 当前唯一配置面 = embedding 供给（D-1：bge-m3 @ Ollama 本地）。
 */

export interface EmbeddingConfig {
  endpoint: string;
  model: string;
  enabled: boolean;
  updated_at: string;
}

export interface EmbeddingTestResult {
  ok: boolean;
  dims: number;
  latency_ms: number;
  error: string | null;
}

/** 读系统级 embedding 配置。 */
export function getEmbeddingConfig() {
  return http.get<EmbeddingConfig>("/v1/admin/embedding-config");
}

/** 保存（last-write；管理员）。 */
export function saveEmbeddingConfig(body: {
  endpoint: string;
  model: string;
  enabled: boolean;
}) {
  return http.put<EmbeddingConfig>("/v1/admin/embedding-config", body);
}

/**
 * 连通性测试：用已存（或覆盖的）endpoint/model 实调 /embeddings 探针，
 * 返回 ok/dims/latency_ms/error——不写库。
 */
export function testEmbeddingConfig(overrides?: {
  endpoint: string;
  model: string;
}) {
  return http.post<EmbeddingTestResult>(
    "/v1/admin/embedding-config/test",
    overrides ?? {}
  );
}
