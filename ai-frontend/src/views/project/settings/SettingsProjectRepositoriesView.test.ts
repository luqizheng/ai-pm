// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectRepositoriesView from "./SettingsProjectRepositoriesView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { RepositoryV2 } from "@/api/initialization";
import type { SshKey } from "@/api/types";

const listRepositoriesV2 = vi.fn();
const createRepositoryV2 = vi.fn();
const confirmRepositoryVersion = vi.fn();
const disconnectRepository = vi.fn();
const updateRepositoryV2 = vi.fn();
const verifyRepositoryV2 = vi.fn();
const analyzeRepository = vi.fn();
const listSshKeys = vi.fn();
const bindRepositoryKey = vi.fn();
const listImports = vi.fn();
const startImport = vi.fn();
const cancelImport = vi.fn();

vi.mock("@/api/initialization", () => ({
  listRepositoriesV2: (...args: unknown[]) => listRepositoriesV2(...args),
  createRepositoryV2: (...args: unknown[]) => createRepositoryV2(...args),
  confirmRepositoryVersion: (...args: unknown[]) =>
    confirmRepositoryVersion(...args),
  disconnectRepository: (...args: unknown[]) => disconnectRepository(...args),
  updateRepositoryV2: (...args: unknown[]) => updateRepositoryV2(...args),
  verifyRepositoryV2: (...args: unknown[]) => verifyRepositoryV2(...args),
  analyzeRepository: (...args: unknown[]) => analyzeRepository(...args)
}));

vi.mock("@/api/repositories", () => ({
  listSshKeys: (...args: unknown[]) => listSshKeys(...args),
  bindRepositoryKey: (...args: unknown[]) => bindRepositoryKey(...args)
}));

vi.mock("@/api/imports", () => ({
  listImports: (...args: unknown[]) => listImports(...args),
  startImport: (...args: unknown[]) => startImport(...args),
  cancelImport: (...args: unknown[]) => cancelImport(...args)
}));

/** 后端真实行形状（RepositoryRow 契约）的最小夹具。 */
function repo(overrides: Partial<RepositoryV2>): RepositoryV2 {
  return {
    id: "repo-1",
    project_id: "prj-1",
    name: "payment-service",
    url: "git@git.example.com:acme/payment-service.git",
    role: "main",
    default_branch: "main",
    description: null,
    ssh_key_id: null,
    status: "verified",
    head_commit_sha: "abc1234",
    selected_branch: "main",
    selected_commit: "abc1234",
    selected_confirmed_at: "2026-11-05T00:00:00Z",
    selected_confirmed_by: null,
    last_verified_at: "2026-11-05T00:00:00Z",
    last_error_code: null,
    last_error_detail: null,
    revision: 3,
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-05T00:00:00Z",
    ...overrides
  };
}

/** SshKey 契约（迁移 0022 多把：公钥 + 指纹 + is_default，无私钥字段）的最小夹具。 */
function sshKey(overrides: Partial<SshKey>): SshKey {
  return {
    id: "key-1",
    project_id: "prj-1",
    label: "github",
    public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... ai-pm:demo",
    fingerprint: "SHA256:AbCdEf123456",
    status: "active",
    is_default: true,
    created_at: "2026-11-13T00:00:00Z",
    ...overrides
  };
}

async function mountView() {
  // 项目作用域经 route.query.project_id 注入（与 BaselinePage 同款取值链）
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push("/project/settings/repositories?project_id=prj-1");
  await router.isReady();
  const mounted = mountShell(SettingsProjectRepositoriesView, {}, router);
  await flushPromises();
  return mounted;
}

function findButtonByText(
  wrapper: { findAll: (sel: string) => { text: () => string }[] },
  text: string
) {
  const target = wrapper.findAll("button").find(b => b.text().includes(text));
  expect(target, `button "${text}" should exist`).toBeTruthy();
  return target!;
}

describe("SettingsProjectRepositoriesView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listRepositoriesV2.mockResolvedValue({ items: [], total: 0 });
    // 默认未配置密钥（空列表）
    listSshKeys.mockResolvedValue({ items: [], total: 0 });
    listImports.mockResolvedValue({ items: [], total: 0 });
    startImport.mockResolvedValue({
      import_id: "imp-1",
      status: "queued",
      replayed: false
    });
    analyzeRepository.mockResolvedValue({
      repository_id: "repo-1",
      project_id: "prj-1",
      commit: "abc1234",
      intake_id: "intake-1",
      status: "ROUTED",
      replayed: false
    });
  });

  it("零仓库时空态明确传达项目仍可工作（V1.2 §6.3）", async () => {
    const { wrapper } = await mountView();
    const empty = wrapper.find('[data-testid="repo-empty"]');
    expect(empty.exists()).toBe(true);
    // 空态文案必须传达「未接入 ≠ 不可用」——字面量断言防 i18n MISS 恒真
    expect(empty.text()).toContain("尚未关联仓库");
    expect(empty.text()).toContain("项目已可以开展工作");
  });

  it("待验证/已验证行状态旁给分析报告跳转，unreachable 不给", async () => {
    listRepositoriesV2.mockResolvedValue({
      items: [
        repo({ id: "r1", name: "main-repo", status: "verified" }),
        repo({ id: "r2", name: "pending-repo", status: "pending" }),
        repo({ id: "r3", name: "dead-repo", status: "unreachable" })
      ],
      total: 3
    });
    const { wrapper, router } = await mountView();
    expect(wrapper.find('[data-testid="report-link-r1"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="report-link-r2"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="report-link-r3"]').exists()).toBe(false);
    // spy 而非真跳：Analysis 列表视图会挂载并自行拉数据，与本文件 mock 面不符
    const pushSpy = vi.spyOn(router, "push").mockResolvedValue(undefined);
    await wrapper.find('[data-testid="report-link-r1"]').trigger("click");
    expect(pushSpy).toHaveBeenCalledWith({
      path: "/project/analysis",
      query: { project_id: "prj-1" }
    });
  });

  it("统计卡渲染标签（compact + icon；迁入前 stacked 无 icon 只剩裸数字）", async () => {
    const { wrapper } = await mountView();
    const text = wrapper.text();
    for (const label of [
      "已关联仓库",
      "可达性已验证",
      "需要关注",
      "已确认基准"
    ]) {
      expect(text).toContain(label);
    }
  });

  it("无项目上下文时不发请求（scope 哨兵 __all__ 不得当作项目 id）", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    router.push("/project/settings/repositories");
    await router.isReady();
    mountShell(SettingsProjectRepositoriesView, {}, router);
    await flushPromises();
    expect(listRepositoriesV2).not.toHaveBeenCalled();
  });

  it("侧栏条目携带项目上下文，且当前节保持激活", async () => {
    const { wrapper } = await mountView();
    const runners = wrapper
      .findAll("a")
      .find(a => a.attributes("href")?.startsWith("/project/settings/runners"));
    expect(runners?.attributes("href")).toBe(
      "/project/settings/runners?project_id=prj-1"
    );
    const active = wrapper.find(".settings-nav-item.is-active");
    expect(active.attributes("href")).toBe(
      "/project/settings/repositories?project_id=prj-1"
    );
  });

  it("Primary 由 role=main 唯一性派生：恰好一个显示建议标记，两个都不显示（G-03 前端派生）", async () => {
    listRepositoriesV2.mockResolvedValue({
      items: [
        repo({ id: "r-main", role: "main" }),
        repo({ id: "r-docs", role: "docs", name: "docs" })
      ],
      total: 2
    });
    const { wrapper } = await mountView();
    expect(wrapper.findAll('[data-testid="primary-badge"]')).toHaveLength(1);

    listRepositoriesV2.mockResolvedValue({
      items: [
        repo({ id: "r-main-1", role: "main", name: "a" }),
        repo({ id: "r-main-2", role: "main", name: "b" })
      ],
      total: 2
    });
    const again = await mountView();
    expect(again.wrapper.findAll('[data-testid="primary-badge"]')).toHaveLength(
      0
    );
  });

  it("HEAD 与确认基准不一致时只提示 + 人工确认更新采用 Observed HEAD（R-03-04，不自动更新）", async () => {
    listRepositoriesV2.mockResolvedValue({
      items: [
        repo({
          id: "r-ahead",
          head_commit_sha: "def5678",
          selected_commit: "abc1234"
        })
      ],
      total: 1
    });
    confirmRepositoryVersion.mockResolvedValue(repo({}));
    const { wrapper } = await mountView();

    // 详情面板随行选中渲染：先点列表行
    await wrapper.find('[data-testid="repo-list"] > div').trigger("click");
    const change = wrapper.find('[data-testid="head-change"]');
    expect(change.exists()).toBe(true);
    expect(change.text()).toContain("确认基准未被改动");

    await findButtonByText(wrapper, "确认更新").trigger("click");
    await flushPromises();
    // 人类确认：采用 Observed HEAD，而不是静默改动
    expect(confirmRepositoryVersion).toHaveBeenCalledWith("r-ahead", {
      selected_branch: "main",
      selected_commit: "def5678",
      reason: "confirm observed head"
    });
  });

  it("接入表单提交走真实创建契约（含可选初始基准）", async () => {
    createRepositoryV2.mockResolvedValue(repo({}));
    const { wrapper } = await mountView();
    await findButtonByText(wrapper, "接入仓库").trigger("click");

    await wrapper.find('[data-testid="connect-name"]').setValue("docs");
    await wrapper
      .find('[data-testid="connect-url"]')
      .setValue("git@git.example.com:acme/docs.git");
    // 用 testid 精确命中提交按钮（头部「接入仓库」按钮同样包含「接入」字样）
    await wrapper.find('[data-testid="connect-submit"]').trigger("click");
    await flushPromises();

    expect(createRepositoryV2).toHaveBeenCalledWith("prj-1", {
      name: "docs",
      url: "git@git.example.com:acme/docs.git",
      role: "main",
      default_branch: null,
      selected_branch: null,
      selected_commit: null,
      description: null
    });
  });

  it("仓库详情可绑定/解绑 SSH 密钥（迁移 0022；空串 = 跟随项目默认）", async () => {
    listSshKeys.mockResolvedValue({
      items: [
        sshKey({}),
        sshKey({ id: "key-2", label: "gitlab", is_default: false })
      ],
      total: 2
    });
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r-bind" })],
      total: 1
    });
    bindRepositoryKey.mockResolvedValue(
      repo({ id: "r-bind", ssh_key_id: "key-2" })
    );
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="repo-list"] > div').trigger("click");
    const select = wrapper.find('[data-testid="bind-key-select"]');
    expect(select.exists()).toBe(true);

    await select.setValue("key-2");
    await flushPromises();
    expect(bindRepositoryKey).toHaveBeenCalledWith("r-bind", "key-2");

    await wrapper.find('[data-testid="bind-key-select"]').setValue("");
    await flushPromises();
    expect(bindRepositoryKey).toHaveBeenCalledWith("r-bind", null);
  });

  /** ConfirmationDialog Teleport 到 body：按文案找确认按钮并触发。 */
  async function confirmDialogAction(label: string) {
    const button = [...document.body.querySelectorAll("button")].find(item =>
      item.textContent?.includes(label)
    );
    expect(button, `dialog button "${label}" should exist`).toBeTruthy();
    button!.click();
    await flushPromises();
  }

  function importRow(
    status: string,
    progress = 0,
    overrides: Record<string, unknown> = {}
  ) {
    return {
      id: "imp-1",
      status,
      progress,
      stage_detail: null,
      model_name: "",
      draft_revision: 0,
      analysis_run_id: null,
      created_at: "2026-11-13T00:00:00Z",
      updated_at: "2026-11-13T00:00:00Z",
      proposed_at: null,
      applied_at: null,
      error_code: null,
      error_detail: null,
      ...overrides
    };
  }

  it("S2 多选不预选：仅 verified 可勾，确认后 startImport 带幂等键", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [
        repo({ id: "r1" }),
        repo({ id: "r2", name: "docs", status: "pending" })
      ],
      total: 2
    });
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="analysis-bar"]').exists()).toBe(true);
    const start = wrapper.find('[data-testid="analysis-start"]');
    // 不预选：初始禁用
    expect(start.attributes("disabled")).toBeDefined();

    const checks = wrapper.findAll('[data-testid^="analysis-check-"]');
    expect(checks).toHaveLength(2);
    // pending 行不可勾：点击后按钮仍禁用（行为断言，不依赖禁用属性实现）
    await checks[1]!.find("button").trigger("click");
    expect(
      wrapper.find('[data-testid="analysis-start"]').attributes("disabled")
    ).toBeDefined();

    await checks[0]!.find("button").trigger("click");
    expect(
      wrapper.find('[data-testid="analysis-start"]').attributes("disabled")
    ).toBeUndefined();
    expect(wrapper.find('[data-testid="analysis-start"]').text()).toContain(
      "(1)"
    );

    await wrapper.find('[data-testid="analysis-start"]').trigger("click");
    await flushPromises();
    await confirmDialogAction("开始分析");

    expect(startImport).toHaveBeenCalledTimes(1);
    const call = startImport.mock.calls[0]!;
    expect(call[0]).toBe("prj-1");
    expect(call[1]).toEqual(["r1"]);
    expect(typeof call[2]).toBe("string");
    expect(call[2]).toBeTruthy();
  });

  it("S2 有进行中导入时按钮禁用并展示行内进度", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    listImports.mockResolvedValue({
      items: [importRow("analyzing", 42)],
      total: 1
    });
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="analysis-progress"]').text()).toContain(
      "42"
    );
    expect(wrapper.find('[data-testid="analysis-progress-bar"]').exists()).toBe(
      true
    );
    expect(
      wrapper.find('[data-testid="analysis-start"]').attributes("disabled")
    ).toBeDefined();
  });

  it("S2 进行中的分析可两段式取消（卡死批次解锁），取消后状态刷新", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    listImports.mockResolvedValue({
      items: [importRow("analyzing", 70)],
      total: 1
    });
    cancelImport.mockResolvedValue({ id: "imp-1" });
    const { wrapper } = await mountView();

    const cancelBtn = wrapper.find('[data-testid="analysis-cancel"]');
    expect(cancelBtn.text()).toContain("取消分析");
    await cancelBtn.trigger("click");
    expect(cancelBtn.text()).toContain("确认取消");
    await cancelBtn.trigger("click");
    await flushPromises();

    expect(cancelImport).toHaveBeenCalledWith("prj-1", "imp-1");
  });

  it("S2 分析失败：就地展示本地化失败原因（error_code 透传自 list）", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    listImports.mockResolvedValue({
      items: [
        importRow("failed", 70, {
          error_code: "model_configuration",
          error_detail: "provider 192.168.11.5 unreachable"
        })
      ],
      total: 1
    });
    const { wrapper } = await mountView();

    const alert = wrapper.find('[data-testid="analysis-failure"]');
    expect(alert.exists()).toBe(true);
    expect(alert.text()).toContain("模型配置不可用");
    expect(alert.text()).toContain("192.168.11.5");
  });

  it("S2 分析完成：draft_ready 展示就绪提示（不再停留「分析中」）", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    listImports.mockResolvedValue({
      items: [importRow("draft_ready", 100)],
      total: 1
    });
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="analysis-done"]').text()).toContain(
      "草案已就绪"
    );
  });

  it("S2 进行中每 3s 轮询导入状态，到达终态后停止（2026-11-14）", async () => {
    document.body.innerHTML = "";
    vi.useFakeTimers();
    try {
      listRepositoriesV2.mockResolvedValue({
        items: [repo({ id: "r1" })],
        total: 1
      });
      listImports.mockResolvedValue({
        items: [importRow("analyzing", 70)],
        total: 1
      });
      const { wrapper } = await mountView();
      const afterMount = listImports.mock.calls.length;
      expect(afterMount).toBeGreaterThan(0);

      await vi.advanceTimersByTimeAsync(3100);
      expect(listImports.mock.calls.length).toBeGreaterThan(afterMount);

      // 终态 → 轮询停止
      listImports.mockResolvedValue({
        items: [importRow("draft_ready", 100)],
        total: 1
      });
      await vi.advanceTimersByTimeAsync(3100);
      await flushPromises();
      expect(wrapper.find('[data-testid="analysis-done"]').exists()).toBe(true);
      const stable = listImports.mock.calls.length;
      await vi.advanceTimersByTimeAsync(7000);
      expect(listImports.mock.calls.length).toBe(stable);
    } finally {
      vi.useRealTimers();
    }
  });

  it("S2 已 apply 项目 409 → 展示后端错误码兜底文案", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    startImport.mockRejectedValue({
      data: { error: "already_applied" },
      message: "Request failed with status code 409"
    });
    const { wrapper } = await mountView();
    const checks = wrapper.findAll('[data-testid^="analysis-check-"]');
    await checks[0]!.find("button").trigger("click");
    await wrapper.find('[data-testid="analysis-start"]').trigger("click");
    await flushPromises();
    await confirmDialogAction("开始分析");

    expect(wrapper.text()).toContain("该项目的分析结果已应用");
  });

  it("S4 仓库详情「重新分析」走单仓库手动增量端点（带幂等键）", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    const { wrapper } = await mountView();
    await wrapper.find('[data-testid="repo-list"] > div').trigger("click");

    const reanalyze = wrapper.find('[data-testid="reanalyze-btn"]');
    expect(reanalyze.exists()).toBe(true);
    await reanalyze.trigger("click");
    await flushPromises();

    expect(analyzeRepository).toHaveBeenCalledWith("r1", {
      idempotency_key: expect.any(String),
      mode: "incremental"
    });
  });

  it("S4 重新分析遇 409（analysis_in_progress）→ 本地化兜底文案", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    analyzeRepository.mockRejectedValue({
      data: { error: "analysis_in_progress" },
      message: "repository analysis is already in progress"
    });
    const { wrapper } = await mountView();
    await wrapper.find('[data-testid="repo-list"] > div').trigger("click");

    await wrapper.find('[data-testid="reanalyze-btn"]').trigger("click");
    await flushPromises();

    expect(analyzeRepository).toHaveBeenCalledTimes(1);
    // 提示必须落在详情面板按钮旁（页顶 Alert 在右栏视口外不可见）。
    const notice = wrapper.find(
      '[data-testid="repo-detail"] [data-testid="panel-notice"]'
    );
    expect(notice.exists()).toBe(true);
    expect(notice.text()).toContain("已有进行中的分析");
  });

  it("S4 核验可达性：成功/不可达都在详情面板按钮旁就地反馈", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r-verify" })],
      total: 1
    });
    verifyRepositoryV2.mockResolvedValue({
      repository_id: "r-verify",
      status: "verified",
      default_branch: "main",
      head_commit_sha: "abcdef1234567890",
      error_code: null,
      error_detail: null
    });
    let { wrapper } = await mountView();
    await wrapper.find('[data-testid="repo-list"] > div').trigger("click");
    await wrapper.find('[data-testid="verify-btn"]').trigger("click");
    await flushPromises();

    let notice = wrapper.find(
      '[data-testid="repo-detail"] [data-testid="panel-notice"]'
    );
    expect(notice.exists()).toBe(true);
    expect(notice.text()).toContain("核验成功");
    expect(notice.text()).toContain("abcdef12");

    // unreachable 也是 200：就地 warning，不伪装成功
    document.body.innerHTML = "";
    verifyRepositoryV2.mockResolvedValue({
      repository_id: "r-verify",
      status: "unreachable",
      default_branch: null,
      head_commit_sha: null,
      error_code: "auth",
      error_detail: "permission denied (publickey)"
    });
    ({ wrapper } = await mountView());
    await wrapper.find('[data-testid="repo-list"] > div').trigger("click");
    await wrapper.find('[data-testid="verify-btn"]').trigger("click");
    await flushPromises();
    notice = wrapper.find(
      '[data-testid="repo-detail"] [data-testid="panel-notice"]'
    );
    expect(notice.text()).toContain("仓库不可达");
    expect(notice.text()).toContain("permission denied");
  });

  it("scope=__all__（无项目）整段隐藏分析按钮且不拉取导入状态", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    router.push("/project/settings/repositories");
    await router.isReady();
    const { wrapper } = mountShell(SettingsProjectRepositoriesView, {}, router);
    await flushPromises();

    expect(wrapper.find('[data-testid="analysis-bar"]').exists()).toBe(false);
    expect(listImports).not.toHaveBeenCalled();
  });

  /** 直接点弹层主按钮（按 role 定位，避免与工具条同名文案混淆）。 */
  async function clickDialogConfirm() {
    const button = document.body.querySelector<HTMLButtonElement>(
      '[role="alertdialog"] .ui-confirm__btn--primary'
    );
    expect(button, "confirm dialog primary button should exist").toBeTruthy();
    button!.click();
    await flushPromises();
  }

  async function selectFirstRow(
    wrapper: Awaited<ReturnType<typeof mountView>>["wrapper"]
  ) {
    const checks = wrapper.findAll('[data-testid^="analysis-check-"]');
    await checks[0]!.find("button").trigger("click");
  }

  it("S2 failed 批次复用主按钮文案「再次分析」，点击即重发（无额外 retry 按钮）", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    listImports.mockResolvedValue({
      items: [importRow("failed", 100)],
      total: 1
    });
    const { wrapper } = await mountView();

    const bar = wrapper.find('[data-testid="analysis-bar"]');
    // 主按钮即「再次分析」入口：工具条内只有这一个重发按钮。
    const retryButtons = bar
      .findAll("button")
      .filter(button => button.text().includes("再次分析"));
    expect(retryButtons).toHaveLength(1);

    await selectFirstRow(wrapper);
    const start = wrapper.find('[data-testid="analysis-start"]');
    expect(start.text()).toContain("再次分析");
    expect(start.attributes("disabled")).toBeUndefined();

    await start.trigger("click");
    await flushPromises();
    await clickDialogConfirm();

    expect(startImport).toHaveBeenCalledTimes(1);
    expect(startImport.mock.calls[0]![1]).toEqual(["r1"]);
  });

  it("S2 连点确认不双发（幂等键守卫），成功后清除键与选择以便下次重发", async () => {
    document.body.innerHTML = "";
    let release: (value: unknown) => void = () => undefined;
    startImport.mockImplementationOnce(
      () =>
        new Promise(resolve => {
          release = resolve;
        })
    );
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    const { wrapper } = await mountView();

    await selectFirstRow(wrapper);
    await wrapper.find('[data-testid="analysis-start"]').trigger("click");
    await flushPromises();

    const dialogConfirm = document.body.querySelector<HTMLButtonElement>(
      '[role="alertdialog"] .ui-confirm__btn--primary'
    );
    expect(dialogConfirm).toBeTruthy();
    // 同一轮连点两次：ref 守卫只放行一次请求。
    dialogConfirm!.click();
    dialogConfirm!.click();
    await flushPromises();
    expect(startImport).toHaveBeenCalledTimes(1);
    const firstKey = startImport.mock.calls[0]![2];
    expect(typeof firstKey).toBe("string");
    expect(firstKey).toBeTruthy();

    release({ import_id: "imp-1", status: "queued", replayed: false });
    await flushPromises();
    // 成功：选择清空（按钮回到禁用态）。
    expect(
      wrapper.find('[data-testid="analysis-start"]').attributes("disabled")
    ).toBeDefined();

    // 键已清除 → 再选再发能发出第二次，且换用新键（不复用上一轮的键）。
    await selectFirstRow(wrapper);
    await wrapper.find('[data-testid="analysis-start"]').trigger("click");
    await flushPromises();
    await clickDialogConfirm();
    expect(startImport).toHaveBeenCalledTimes(2);
    expect(startImport.mock.calls[1]![2]).not.toBe(firstKey);
  });

  it("S2 无稳定错误码时不展示「（-）」占位码，透出后端 message", async () => {
    document.body.innerHTML = "";
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r1" })],
      total: 1
    });
    startImport.mockRejectedValue(
      new Error("Request failed with status code 500")
    );
    const { wrapper } = await mountView();

    await selectFirstRow(wrapper);
    await wrapper.find('[data-testid="analysis-start"]').trigger("click");
    await flushPromises();
    await clickDialogConfirm();

    const text = wrapper.text();
    expect(text).toContain("分析请求失败");
    expect(text).toContain("Request failed with status code 500");
    expect(text).not.toContain("（-）");
  });
});
