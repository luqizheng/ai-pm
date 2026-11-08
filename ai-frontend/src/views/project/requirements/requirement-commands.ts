/**
 * 需求命令编排助手（S02-I1 统一服务的前端命令面）。
 *
 * 三个页面（工作区 / 详情页 / 历史页）共用的「起草-提交」路径：
 *   - 起草：最新修订为 draft 则 PATCH 更新之，否则 POST 新 draft
 *     （后端允许多 draft 并存，update 仅限 draft 态——I1 §5.4 pending 冻结）；
 *   - 提交：有 draft 直接 submit；无 draft 以给定正文起草后 submit
 *     （create + submit 两步，服务端各自动作审计）。
 * 状态迁移的权威在后端；助手只做「找哪份修订、走哪个端点」的编排。
 */

import {
  createDraftRevision,
  submitRevision,
  updateDraftRevision,
  type DraftRevisionInput,
  type RequirementCoreDetail,
  type RequirementCoreRevision
} from "@/api/requirements";

/** 最新修订（任意状态）。 */
export function latestRevision(
  detail: RequirementCoreDetail
): RequirementCoreRevision | undefined {
  return detail.revisions.length > 0
    ? detail.revisions.reduce((a, b) => (a.version > b.version ? a : b))
    : undefined;
}

/** 最新 draft 修订（编辑/提交的落点；无则 undefined）。 */
export function latestDraftRevision(
  detail: RequirementCoreDetail
): RequirementCoreRevision | undefined {
  const latest = latestRevision(detail);
  return latest && latest.state === "draft" ? latest : undefined;
}

/** 起草或更新：最新修订为 draft → 更新该份；否则新建一份 draft。 */
export function upsertDraft(
  requirementId: string,
  detail: RequirementCoreDetail,
  body: DraftRevisionInput
) {
  const draft = latestDraftRevision(detail);
  return draft
    ? updateDraftRevision(requirementId, draft.id, body)
    : createDraftRevision(requirementId, body);
}

/** 提交确认（S02D-04：draft → submit → pending_confirmation）。 */
export function submitDraft(
  requirementId: string,
  detail: RequirementCoreDetail
) {
  const draft = latestDraftRevision(detail);
  return draft
    ? submitRevision(requirementId, draft.id)
    : Promise.reject(new Error("no draft revision to submit"));
}

/** 以正文起草并提交（无在途 draft 时的 propose 路径）。 */
export async function draftAndSubmit(
  requirementId: string,
  body: DraftRevisionInput
) {
  const { revision } = await createDraftRevision(requirementId, body);
  return submitRevision(requirementId, revision.id);
}
