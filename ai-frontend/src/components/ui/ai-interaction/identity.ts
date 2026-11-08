import type { AiIdentity } from "./types";

/**
 * 组件库内置缺省 AI 身份（回落值，非演示数据）。
 *
 * 该回落历史上放在 fixtures.ts；发布构建 mock 门禁（build/release-guard.ts）
 * 禁止任何产物 chunk 引用 fixtures 模块，业务页接真实会话后组件不得再静态
 * 依赖演示数据，故抽出为本模块。演示目录页仍可显式传 fixtures.aiIdentity
 * （含角色副标）；缺省回落仅保留产品名，不再携带演示角色文案。
 */
export const defaultAiIdentity: AiIdentity = {
  name: "AI-PM",
  avatarLabel: "AI"
};
