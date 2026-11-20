// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import PersonalHomeView from "./PersonalHomeView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

const loadPersonalContext = vi.fn();
const loadPersonalInbox = vi.fn();
const loadPersonalProjects = vi.fn();
const loadPersonalActivity = vi.fn();

vi.mock("@/api/personal-home", () => ({
  loadPersonalContext: (...args: unknown[]) => loadPersonalContext(...args),
  loadPersonalInbox: (...args: unknown[]) => loadPersonalInbox(...args),
  loadPersonalProjects: (...args: unknown[]) => loadPersonalProjects(...args),
  loadPersonalActivity: (...args: unknown[]) => loadPersonalActivity(...args)
}));

const t = (key: string) => i18n.global.t(key) as string;

const context = {
  me: {
    principal_id: "prc-zhang",
    display_name: "张伟",
    is_active: true,
    is_admin: false,
    tenant_id: "cus-xinghe",
    team_role: "member",
    provider: "lmms",
    claims_tenant_key: null,
    subject: "usr-zhang",
    auth_mode: "interactive"
  },
  projects: [],
  projectById: new Map()
};

const inbox = {
  humanActions: [
    {
      id: "act-1",
      kind: "approval" as const,
      title: "确认需求修订",
      subject: "REQ-102",
      objectType: "Requirement",
      projectId: "prj-shield",
      projectName: "星河客服中台",
      objectPath: "/project/requirements?project_id=prj-shield",
      due: { dueAt: null, daysUntilDue: null, state: "none" as const },
      priority: null,
      createdAt: "2026-11-04T10:00:00Z"
    }
  ],
  assignedTasks: [
    {
      id: "task-1",
      title: "微信渠道适配器实现",
      subject: "tsk-002",
      projectId: "prj-shield",
      projectName: "星河客服中台",
      status: "in_progress",
      priority: "high",
      objectPath: "/project/execution?project_id=prj-shield",
      due: { dueAt: null, daysUntilDue: null, state: "none" as const },
      updatedAt: "2026-11-04T09:00:00Z"
    }
  ]
};

const projects = {
  projects: [
    {
      id: "prj-shield",
      name: "星河客服中台",
      code: "SHIELD",
      description: "统一工单接入",
      status: "active",
      role: "owner",
      pendingCount: 2,
      lastActivityAt: "2026-11-04T08:00:00Z",
      attention: false,
      objectPath: "/project/command-center?project_id=prj-shield"
    }
  ],
  archivedProjects: []
};

const activity = [
  {
    id: "aud-1",
    actorKind: "human" as const,
    actorName: "张伟",
    eventKind: "confirmed" as const,
    action: "requirement.approved",
    targetType: "Requirement",
    projectId: "prj-shield",
    projectName: "星河客服中台",
    objectRef: "req-101",
    occurredAt: "2026-11-04T08:00:00Z"
  }
];

async function mountView() {
  const mounted = mountShell(PersonalHomeView);
  await flushPromises();
  await new Promise(resolve => setTimeout(resolve, 0));
  await flushPromises();
  return mounted;
}

describe("PersonalHomeView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loadPersonalContext.mockResolvedValue(context);
    loadPersonalInbox.mockResolvedValue(inbox);
    loadPersonalProjects.mockResolvedValue(projects);
    loadPersonalActivity.mockResolvedValue(activity);
  });

  it("正常态呈现三个指标、两类待办与我的项目", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain(
      t("pm.personalHome.overview.needsAttention")
    );
    expect(wrapper.text()).toContain(
      t("pm.personalHome.overview.assignedToMe")
    );
    expect(wrapper.text()).toContain(
      t("pm.personalHome.overview.activeProjects")
    );
    // 概览三卡为公共 StatCard（ui-stat-card），不再是页内私有布局件
    const statCards = wrapper.findAll(".ui-stat-card");
    expect(statCards).toHaveLength(3);
    expect(statCards[0]!.text()).toContain(
      t("pm.personalHome.overview.needsAttentionHint")
    );
    expect(wrapper.text()).toContain(t("pm.personalHome.inbox.humanActions"));
    expect(wrapper.text()).toContain(t("pm.personalHome.inbox.assignedTasks"));
    expect(wrapper.text()).toContain("确认需求修订");
    expect(wrapper.text()).toContain("微信渠道适配器实现");
    expect(wrapper.text()).toContain("星河客服中台");
    expect(wrapper.text()).toContain(t("pm.personalHome.activity.title"));
  });

  it("人工事项与分配任务分区呈现，互不混入", async () => {
    const { wrapper } = await mountView();
    const text = wrapper.text();

    const humanAt = text.indexOf(t("pm.personalHome.inbox.humanActions"));
    const taskAt = text.indexOf(t("pm.personalHome.inbox.assignedTasks"));
    const approvalAt = text.indexOf("确认需求修订");
    const taskItemAt = text.indexOf("微信渠道适配器实现");

    expect(humanAt).toBeGreaterThan(-1);
    expect(taskAt).toBeGreaterThan(humanAt);
    expect(approvalAt).toBeGreaterThan(humanAt);
    expect(approvalAt).toBeLessThan(taskAt);
    expect(taskItemAt).toBeGreaterThan(taskAt);
  });

  it("Inbox 失败只塌陷 Inbox 区块，项目区块仍可用", async () => {
    loadPersonalInbox.mockRejectedValue(
      Object.assign(new Error("boom"), { status: 503 })
    );

    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain(
      t("pm.personalHome.state.error.inbox.title")
    );
    expect(wrapper.text()).toContain(t("pm.personalHome.projects.title"));
    expect(wrapper.text()).toContain("星河客服中台");
  });

  it("整页失败时给出可重试的错误态", async () => {
    loadPersonalContext.mockRejectedValue(
      Object.assign(new Error("denied"), { status: 403 })
    );

    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain(
      t("pm.personalHome.state.forbidden.title")
    );
    expect(wrapper.text()).not.toContain(t("pm.personalHome.inbox.title"));
  });

  it("没有任何项目时显示首次登录引导，不展示概览", async () => {
    loadPersonalProjects.mockResolvedValue({
      projects: [],
      archivedProjects: []
    });

    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain(
      t("pm.personalHome.state.firstLogin.title")
    );
    expect(wrapper.text()).not.toContain(
      t("pm.personalHome.overview.needsAttention")
    );
  });

  it("待办清空时给出平静空态而不是告警", async () => {
    loadPersonalInbox.mockResolvedValue({
      humanActions: [],
      assignedTasks: []
    });

    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain(t("pm.personalHome.inbox.empty.title"));
    expect(wrapper.text()).toContain(t("pm.personalHome.projects.title"));
  });

  it("「新建项目」进入 S01-02 项目创建页", async () => {
    const { wrapper, router } = await mountView();
    const push = vi.spyOn(router, "push").mockResolvedValue(undefined);

    const createButton = wrapper
      .findAll("button")
      .find(button =>
        button.text().includes(t("pm.personalHome.action.createProject"))
      );
    expect(createButton).toBeTruthy();
    await createButton!.trigger("click");

    expect(push).toHaveBeenCalledWith("/project/create");
  });
});
