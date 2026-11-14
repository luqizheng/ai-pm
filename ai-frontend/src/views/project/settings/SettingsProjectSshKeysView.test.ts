// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectSshKeysView from "./SettingsProjectSshKeysView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { SshKey } from "@/api/types";

const listSshKeys = vi.fn();
const importSshKey = vi.fn();
const renameSshKey = vi.fn();
const revokeSshKey = vi.fn();

vi.mock("@/api/repositories", () => ({
  listSshKeys: (...args: unknown[]) => listSshKeys(...args),
  importSshKey: (...args: unknown[]) => importSshKey(...args),
  renameSshKey: (...args: unknown[]) => renameSshKey(...args),
  revokeSshKey: (...args: unknown[]) => revokeSshKey(...args)
}));

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
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push("/project/settings/ssh-keys?project_id=prj-1");
  await router.isReady();
  const mounted = mountShell(SettingsProjectSshKeysView, {}, router);
  await flushPromises();
  return mounted;
}

describe("SettingsProjectSshKeysView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listSshKeys.mockResolvedValue({ items: [], total: 0 });
  });

  it("空列表传达空态提示", async () => {
    document.body.innerHTML = "";
    const { wrapper } = await mountView();
    expect(wrapper.find('[data-testid="ssh-key-empty"]').text()).toContain(
      "还没有密钥"
    );
  });

  it("有数据时展示名称/指纹/默认标记", async () => {
    document.body.innerHTML = "";
    listSshKeys.mockResolvedValue({
      items: [
        sshKey({}),
        sshKey({ id: "key-2", label: "gitlab", is_default: false })
      ],
      total: 2
    });
    const { wrapper } = await mountView();
    const items = wrapper.findAll('[data-testid="ssh-key-item"]');
    expect(items).toHaveLength(2);
    expect(wrapper.text()).toContain("github");
    expect(wrapper.text()).toContain("gitlab");
    expect(wrapper.text()).toContain("SHA256:");
    expect(wrapper.text()).toContain("默认");
  });

  it("粘贴导入：trim 后提交，空名称 / 空输入不得提交", async () => {
    document.body.innerHTML = "";
    importSshKey.mockResolvedValue(sshKey({}));
    const { wrapper } = await mountView();

    await wrapper
      .find('[data-testid="ssh-key-name"]')
      .setValue(" github-main ");
    await wrapper
      .find('[data-testid="ssh-key-input"]')
      .setValue(
        "  -----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----\n  "
      );
    await wrapper.find('[data-testid="ssh-key-submit"]').trigger("click");
    await flushPromises();

    expect(importSshKey).toHaveBeenCalledWith(
      "prj-1",
      "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----",
      "github-main"
    );

    await wrapper.find('[data-testid="ssh-key-name"]').setValue("  ");
    await wrapper
      .find('[data-testid="ssh-key-input"]')
      .setValue("-----BEGIN OPENSSH PRIVATE KEY-----");
    await wrapper.find('[data-testid="ssh-key-submit"]').trigger("click");
    await flushPromises();
    expect(importSshKey).toHaveBeenCalledTimes(1);
  });

  it("改名：行内编辑保存走 rename 契约，成功后刷新列表", async () => {
    document.body.innerHTML = "";
    listSshKeys.mockResolvedValue({ items: [sshKey({})], total: 1 });
    renameSshKey.mockResolvedValue(sshKey({ label: "github-main" }));
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="rename-btn-key-1"]').trigger("click");
    const input = wrapper.find('[data-testid="rename-input-key-1"]');
    expect(input.exists()).toBe(true);
    await input.setValue(" github-main ");
    await wrapper.find('[data-testid="rename-save-key-1"]').trigger("click");
    await flushPromises();

    expect(renameSshKey).toHaveBeenCalledWith("key-1", "github-main");
  });

  it("改名撞既有 label（409 label_taken）→ 本地化提示，不伪装成功", async () => {
    document.body.innerHTML = "";
    listSshKeys.mockResolvedValue({ items: [sshKey({})], total: 1 });
    renameSshKey.mockRejectedValue({
      data: { error: "label_taken" },
      message: "ssh key label already in use"
    });
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="rename-btn-key-1"]').trigger("click");
    await wrapper.find('[data-testid="rename-input-key-1"]').setValue("gitlab");
    await wrapper.find('[data-testid="rename-save-key-1"]').trigger("click");
    await flushPromises();

    expect(renameSshKey).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain("已被同项目另一把密钥使用");
  });

  it("删除两段式：第一击武装、第二击按 key 执行；409 key_in_use 提示先解绑", async () => {
    document.body.innerHTML = "";
    listSshKeys.mockResolvedValue({
      items: [
        sshKey({}),
        sshKey({ id: "key-2", label: "gitlab", is_default: false })
      ],
      total: 2
    });
    revokeSshKey.mockResolvedValue({ id: "key-2" });
    const { wrapper } = await mountView();

    const items = wrapper.findAll('[data-testid="ssh-key-item"]');
    const deleteBtn = items[1]!
      .findAll("button")
      .find(b => b.text().includes("删除"))!;
    await deleteBtn.trigger("click");
    expect(deleteBtn.text()).toContain("确认删除");
    await deleteBtn.trigger("click");
    await flushPromises();
    expect(revokeSshKey).toHaveBeenCalledWith("key-2");

    // 被引用拒绝：稳定码 → 本地化文案
    revokeSshKey.mockRejectedValue({
      data: { error: "key_in_use" },
      message: "ssh key is still referenced by a verified repository"
    });
    const items2 = wrapper.findAll('[data-testid="ssh-key-item"]');
    const btn = items2[1]!
      .findAll("button")
      .find(b => b.text().includes("删除"))!;
    await btn.trigger("click");
    await btn.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("仍被仓库绑定引用");
  });
});
