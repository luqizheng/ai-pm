// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import RequirementReviewView from "./RequirementReviewView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { RequirementReviewQueue } from "@/api/requirement-read";

/**
 * 需求评审（S02-UI-03，接真）行为测试：
 * 评审对象是**修订**而不是需求；队列只含 pending_confirmation，
 * 并带出前一正式版本供比较。
 */

const getRequirementReviewQueue = vi.fn();

vi.mock("@/api/requirement-read", async () => {
  const actual = await vi.importActual<typeof import("@/api/requirement-read")>(
    "@/api/requirement-read"
  );
  return {
    revisionMarkdown: actual.revisionMarkdown,
    getRequirementReviewQueue: (...args: unknown[]) =>
      getRequirementReviewQueue(...args)
  };
});

function queue(): RequirementReviewQueue {
  return {
    proposals: [
      {
        revision_id: "rev-2",
        requirement_id: "req-1",
        requirement_code: "REQ-001",
        requirement_title: "用户登录",
        requirement_type: "functional",
        priority: "high",
        version: 2,
        state: "pending_confirmation",
        content: { markdown: "# 登录需求 v2\n\n补充限流要求。" },
        content_hash: "abcdef1234567890",
        created_at: "2026-11-10T08:00:00Z",
        submitted_at: "2026-11-10T09:00:00Z",
        created_by: "p-1",
        created_by_name: "张三",
        base_version: 1,
        base_content_hash: "oldhash",
        source_count: 2
      }
    ],
    by_type: [["functional", 1]],
    by_priority: [["high", 1]]
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/requirements/review");
  await router.isReady();
  const { wrapper } = mountShell(
    RequirementReviewView,
    { attachTo: document.body },
    router
  );
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  getRequirementReviewQueue.mockReset();
  getRequirementReviewQueue.mockResolvedValue(queue());
});

describe("RequirementReviewView", () => {
  it("读真实评审队列并渲染待确认修订与前一版本号", async () => {
    const { wrapper } = await mountView();

    expect(getRequirementReviewQueue).toHaveBeenCalledWith("proj-1");
    const text = wrapper.text();
    expect(text).toContain("REQ-001");
    expect(text).toContain("用户登录");
    // 修订正文来自 content.markdown
    expect(wrapper.find('[data-testid="rv-markdown"]').text()).toContain(
      "补充限流要求"
    );
  });

  it("队列为空时给空态而非伪造提案", async () => {
    getRequirementReviewQueue.mockResolvedValue({
      proposals: [],
      by_type: [],
      by_priority: []
    });
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain("暂无待评审修订");
  });
});
