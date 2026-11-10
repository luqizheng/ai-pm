import { http } from "@/api/base";
import { getDocument, type KnowledgeDocument } from "./documents";

/**
 * Project Goal 读模型（S01 · I4）。
 *
 * **没有独立领域实体**——Goal 复用知识文档机制（`project_goal.rs`：「零新表」）：
 * - Goal = `ai.project_document` 里 `doc_key='project-goal'` 的那一条；
 * - 唯一性由库级 `UNIQUE (project_id, doc_key)` 保证（每项目恰好一条）；
 * - 正文 = Git 知识仓里的 Markdown，DB 不存正文；
 * - 「正式 Goal」= 最新 `published` 修订。
 *
 * 因此**读一次 Goal 要两次调用**：
 *   1. `GET /projects/{id}/goal`         → 元数据（document_id / 修订号 / hash）
 *   2. `GET /documents/{document_id}`    → 正文 markdown（从磁盘读 + hash 校验）
 *
 * 第二步可能失败且**必须如实呈现**（不是「没有目标」）：
 * - 知识仓根目录未配置（`AI_PM_KNOWLEDGE_ROOT`）→ Io 错误；
 * - 文件不在磁盘上 / hash 与账本不符 → Io / Stale 错误。
 */

/** `GET /projects/{id}/goal` 的元数据部分。 */
export interface GoalMetadata {
  document_id: string;
  revision_id: string;
  revision_no: number;
  content_hash: string;
  git_commit_sha: string;
  change_summary: string | null;
  published_at: string;
}

/** 后端 `/goal` 的两种返回。 */
type GoalResponse =
  | { project_id: string; missing: false; goal: GoalMetadata }
  | { project_id: string; missing: true; goal: null; doc_key?: string };

/**
 * Goal 视图状态。
 *
 * - `missing`：项目没有正式 Goal（后端显式信号，Agent 不得臆测目标）
 * - `ready`：元数据 + 正文都在
 * - `body_unavailable`：元数据在，正文读不出来（知识仓未配置 / 文件缺失 /
 *   hash 不符）——这是**与 missing 不同的**状态，不可混为「没有目标」
 */
export type GoalState = "missing" | "ready" | "body_unavailable";

export interface ProjectGoalView {
  state: GoalState;
  /** 元数据（missing 时为 null）。 */
  metadata: GoalMetadata | null;
  /** 正文 markdown（仅 ready 时非空）。 */
  markdown: string;
  /** body_unavailable 时的原因（技术信息，由页面决定是否展开）。 */
  bodyError: string;
}

/**
 * 读项目正式 Goal：元数据 + 正文。
 *
 * 不吞错：正文读失败时返回 `body_unavailable` 并带原因，**不降级成 missing**。
 */
export async function getProjectGoal(
  projectId: string
): Promise<ProjectGoalView> {
  const response = await http.get<GoalResponse>(
    `/v1/pm/projects/${projectId}/goal`
  );
  if (response.missing || !response.goal) {
    return { state: "missing", metadata: null, markdown: "", bodyError: "" };
  }
  const metadata = response.goal;
  try {
    const document: KnowledgeDocument = await getDocument(
      metadata.document_id,
      projectId
    );
    return {
      state: "ready",
      metadata,
      markdown: document.markdown ?? "",
      bodyError: ""
    };
  } catch (error) {
    return {
      state: "body_unavailable",
      metadata,
      markdown: "",
      bodyError: error instanceof Error ? error.message : String(error)
    };
  }
}
