import { createHash } from "node:crypto";

import { expect, test } from "@playwright/test";

import { PM_API } from "./team/stack";
import { buildOwner, type TestOwner } from "./p0/fixtures";

/**
 * P1-04 · FR-085 Codex / MCP Bootstrap（docs/e2e-spec/19 号 P1-04）
 *
 * 锚点（R82/R83）：/install 目录 + /install/{tool}（未知 slug 不 404）+
 * release-manifest.json（package/version/format/sha256/integrity_only/contents
 * 六 SKILL 与脚本）+ /install/packages/{file}（pm-ai-skills-<version>.zip）。
 * 「S27 临时代理」= 需求层术语（18 号 R59），无运行时组件 → 不设代理断言。
 * D53：/install 公开性待定 → 本组统一带凭据调用（公开面裁决后补无凭据分支）。
 */

test.describe.configure({ mode: "serial" });

const TID = 9531;
let owner: TestOwner;

interface ReleaseManifest {
  package: string;
  version: string;
  format: string;
  sha256: string;
  min_platform_api_version: string;
  protocol_version: string;
  integrity_only: boolean;
  contents: string[];
}

test.beforeAll(async () => {
  owner = await buildOwner("p1-fr085-a", TID, "team-p1-fr085", "p1b1");
});

/** TC-P1-04-01/02 · 工具目录与分工具指引（未知 slug 不 404） */
test("TC-01/02 /install 目录 + 指引 + 未知 slug 兜底", async ({ request }) => {
  const catalog = await request.get(`${PM_API}/install`, {
    headers: { Authorization: `Bearer ${owner.token}` }
  });
  expect(catalog.status()).toBe(200);

  const guide = await request.get(`${PM_API}/install/codex`, {
    headers: { Authorization: `Bearer ${owner.token}` }
  });
  expect(guide.status()).toBe(200);

  const unknown = await request.get(`${PM_API}/install/definitely-not-a-tool`, {
    headers: { Authorization: `Bearer ${owner.token}` }
  });
  expect(unknown.status()).toBe(200); // 未知 slug 落通用条目，不 404（onboarding.rs:133）
});

/** TC-P1-04-03 · manifest 结构与信任根边界 */
test("TC-03 release-manifest：四 SKILL + 脚本 + SHA-256 + integrity_only", async ({ request }) => {
  const res = await request.get(`${PM_API}/install/packages/release-manifest.json`, {
    headers: { Authorization: `Bearer ${owner.token}` }
  });
  expect(res.status()).toBe(200);
  const manifest = (await res.json()) as ReleaseManifest;

  expect(manifest.package).toBe("pm-ai-skills");
  expect(manifest.format).toBe("zip");
  expect(manifest.sha256).toMatch(/^[0-9a-f]{64}$/);
  expect(manifest.integrity_only).toBe(true); // 完整性验证 ≠ 发布者认证（评审 P1-4）
  expect(manifest.version).toBeTruthy();

  for (const skill of [
    "skills/pm-ai-bootstrap/SKILL.md",
    "skills/pm-task-execution/SKILL.md",
    "skills/pm-knowledge/SKILL.md",
    "skills/pm-learn/SKILL.md"
  ]) {
    expect(manifest.contents, skill).toContain(skill);
  }
  expect(manifest.contents).toContain("skills/scripts/verify-install.ps1");
  expect(manifest.contents).toContain("skills/scripts/verify-install.sh");
});

/** TC-P1-04-04/05 · ZIP 下载 → 本地 SHA-256 与 manifest 一致 + 无凭证泄漏 */
test("TC-04/05 zip 完整性（sha256 == manifest）且不含凭证样式串", async ({ request }) => {
  const manifestRes = await request.get(`${PM_API}/install/packages/release-manifest.json`, {
    headers: { Authorization: `Bearer ${owner.token}` }
  });
  const manifest = (await manifestRes.json()) as ReleaseManifest;

  const zipRes = await request.get(
    `${PM_API}/install/packages/${manifest.package}-${manifest.version}.zip`,
    { headers: { Authorization: `Bearer ${owner.token}` } }
  );
  expect(zipRes.status()).toBe(200);
  const bytes = Buffer.from(await zipRes.body());

  const digest = createHash("sha256").update(bytes).digest("hex");
  expect(digest).toBe(manifest.sha256); // 下载完整性（同源验证）

  const text = bytes.toString("latin1");
  expect(text).not.toMatch(/sk-[A-Za-z0-9]{16,}/); // API key 样式
  expect(text).not.toContain("BEGIN PRIVATE KEY");
  expect(text).not.toContain("BEGIN RSA PRIVATE KEY");
});

/** TC-P1-04-06 · 认证边界（撤销闭环的设备/凭据层在 tests/pm_devices.rs 已自动化） */
test("TC-06 无凭据业务调用被拒（认证边界）", async ({ request }) => {
  const res = await request.get(`${PM_API}/v1/pm/me`);
  expect(res.status()).toBe(401);
});
