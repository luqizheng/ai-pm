// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

import RequirementReviewView from "./RequirementReviewView.vue";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

function setBodyTextarea(value: string) {
  const textarea = document.body.querySelector("textarea");
  if (!textarea) throw new Error("decision textarea not found");
  textarea.value = value;
  textarea.dispatchEvent(new Event("input"));
}

/** 决策面板动作按钮：限定 .rrp__actions 容器并精确匹配，
 *  避免命中过滤器里的「已确认 / 已要求修改 / 已拒绝」 */
function decisionButton(
  wrapper: {
    find: (sel: string) => {
      findAll: (sel: string) => Array<{
        text: () => string;
        trigger: (e: string) => Promise<unknown>;
      }>;
    };
  },
  label: string
) {
  return wrapper
    .find(".rrp__actions")
    .findAll("button")
    .find(node => node.text().trim() === label);
}

describe("RequirementReviewView", () => {
  it("渲染评审页：队列、待评审计数、四节详情与人工决策面板", async () => {
    const mounted = mountShell(RequirementReviewView);
    await mounted.router.push("/project/requirements/review");
    await flushPromises();

    const text = mounted.wrapper.text();

    // 页头（S02-UI-03 §2）
    expect(text).toContain(t("pm.requirementReview.pageTitle"));
    expect(text).toContain(t("pm.requirementReview.pageSubtitle"));
    // 队列 + 待评审计数（§3）
    expect(text).toContain(t("pm.requirementReview.queueTitle"));
    expect(text).toContain(
      t("pm.requirementReview.pendingCount", { count: 3 })
    );
    // 六条提案
    for (const code of ["PRP-001", "PRP-002", "PRP-003", "PRP-006"]) {
      expect(text).toContain(code);
    }
    // 详情四节（§4 A-D）
    expect(text).toContain(t("pm.requirementReview.proposalTitle"));
    expect(text).toContain(t("pm.requirementReview.evidenceTitle"));
    expect(text).toContain(t("pm.requirementReview.analysisTitle"));
    expect(text).toContain(t("pm.requirementReview.comparisonTitle"));
    // 决策面板（§5）；冲突指示器是 title 属性，断言图标元素存在
    expect(text).toContain(t("pm.requirementReview.decisionPanelTitle"));
    expect(mounted.wrapper.find(".rrq__conflict").exists()).toBe(true);
  });

  it("AI 提案与人类提案视觉可辨，决策面板标注人类专属", async () => {
    const mounted = mountShell(RequirementReviewView);
    await mounted.router.push("/project/requirements/review");
    await flushPromises();

    const text = mounted.wrapper.text();
    // 提案来源徽标（§1/§6）
    expect(text).toContain(t("pm.requirementReview.sourceAi"));
    expect(text).toContain(t("pm.requirementReview.sourceHuman"));
    // 决策面板红线文案
    expect(text).toContain(t("pm.requirementReview.decisionPanelNote"));
    // AI 分析带 AI 产出徽标
    expect(text).toContain(t("pm.requirementReview.analysisBadge"));
  });

  it("过滤器按评审状态收敛队列", async () => {
    const mounted = mountShell(RequirementReviewView);
    await mounted.router.push("/project/requirements/review");
    await flushPromises();

    const rejectedFilter = mounted.wrapper
      .findAll("button")
      .find(node =>
        node.text().includes(t("pm.requirementReview.filterRejected"))
      );
    await rejectedFilter?.trigger("click");
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain("PRP-005");
    expect(text).not.toContain("PRP-003");
  });

  it("确认经显式弹窗与摘要，确认后提案定稿并留人类决策记录", async () => {
    const mounted = mountShell(RequirementReviewView);
    await mounted.router.push("/project/requirements/review");
    await flushPromises();

    // 选中 PRP-003（人类新需求提案，待评审）
    const rows = mounted.wrapper.findAll(".rrq__row");
    await rows[2].trigger("click");
    await flushPromises();

    const confirmButton = decisionButton(
      mounted.wrapper,
      t("pm.requirementReview.actionConfirm")
    );
    await confirmButton?.trigger("click");
    await flushPromises();

    // 确认摘要（Teleport 到 body，Deliverable 4）
    expect(document.body.textContent).toContain(
      t("pm.requirementReview.confirmNewSummary", { code: "PRP-003", count: 3 })
    );

    mounted.wrapper.findComponent(ConfirmationDialog).vm.$emit("confirm");
    await flushPromises();

    const text = mounted.wrapper.text();
    // 提案状态迁移 + 决策记录（人类署名）
    expect(text).toContain(t("pm.requirementReview.statusConfirmed"));
    expect(text).toContain(t("pm.requirementReview.decisionRecordLabel"));
  });

  it("要求修改：评论必填，提交后提案退回并留档", async () => {
    const mounted = mountShell(RequirementReviewView);
    await mounted.router.push("/project/requirements/review");
    await flushPromises();

    const changesButton = decisionButton(
      mounted.wrapper,
      t("pm.requirementReview.actionChanges")
    );
    await changesButton?.trigger("click");
    await flushPromises();

    // 抽屉打开（Deliverable 5），空内容时提交按钮禁用
    expect(document.body.textContent).toContain(
      t("pm.requirementReview.changesCommentLabel")
    );
    const submitBefore = document.body.querySelector('button[type="submit"]');
    expect(submitBefore?.hasAttribute("disabled")).toBe(true);

    setBodyTextarea("请补充管理员解锁的授权角色边界。");
    await flushPromises();
    const submitAfter = document.body.querySelector('button[type="submit"]');
    expect(submitAfter?.hasAttribute("disabled")).toBe(false);
    document.body.querySelector("form")?.dispatchEvent(new Event("submit"));
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain(t("pm.requirementReview.statusChanges"));
    expect(text).toContain("请补充管理员解锁的授权角色边界。");
  });

  it("拒绝：理由必填，提交后提案拒绝留档", async () => {
    const mounted = mountShell(RequirementReviewView);
    await mounted.router.push("/project/requirements/review");
    await flushPromises();

    const rejectButton = decisionButton(
      mounted.wrapper,
      t("pm.requirementReview.actionReject")
    );
    await rejectButton?.trigger("click");
    await flushPromises();

    expect(document.body.textContent).toContain(
      t("pm.requirementReview.rejectReasonLabel")
    );

    setBodyTextarea("归项目认知能力，不作为独立需求。");
    await flushPromises();
    document.body.querySelector("form")?.dispatchEvent(new Event("submit"));
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain(t("pm.requirementReview.statusRejected"));
    expect(text).toContain("归项目认知能力，不作为独立需求。");
  });
});
