# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: team-onboarding.spec.ts >> Team QA · 接受邀请 >> B09-10 跨租户接受 → tenantMismatch
- Location: e2e\team-onboarding.spec.ts:203:3

# Error details

```
Error: expect(locator).toHaveAttribute(expected) failed

Locator:  getByTestId('accept-result')
Expected: "tenantMismatch"
Received: "invalid"
Timeout:  15000ms

Call log:
  - Expect "toHaveAttribute" getByTestId('accept-result') with timeout 15000ms
  - waiting for getByTestId('accept-result')
    33 × locator resolved to <div data-v-f1a23e19="" data-state="invalid" class="accept__result" data-testid="accept-result">…</div>
       - unexpected value "invalid"

```

```yaml
- heading "邀请无效" [level=1]
- paragraph: 邀请码不存在或链接不完整，请核对后重试。
```

# Test source

```ts
  109 | 
  110 |     await page.getByTestId("onboarding-link-create").click();
  111 |     await expect(page.getByTestId("onboarding-link-result")).toContainText("type=link");
  112 | 
  113 |     const token = signToken("ui-linker", 106);
  114 |     const tenantId = (await me("ui-linker", 106)).tenant_id;
  115 |     const links = await api("GET", `/v1/pm/tenants/${tenantId}/invitation-links`, token);
  116 |     expect(links.body).toHaveLength(1);
  117 |     expect(links.body[0]).toMatchObject({ status: "ACTIVE", default_role: "member" });
  118 | 
  119 |     await page.getByTestId("onboarding-link-finish").click();
  120 |     await expect(page).toHaveURL(/\/dashboard$/);
  121 |   });
  122 | 
  123 |   test("B09-19 链接页「默认角色」字段显示 MEMBER", async ({ context, page }) => {
  124 |     await claimTeam("ui-roleview", 107, "Role View Team");
  125 |     await signIn(context, "ui-roleview", 107);
  126 |     await page.goto("/onboarding/link");
  127 |     await expect(field(page.getByTestId("onboarding-link-role"))).toHaveValue("MEMBER");
  128 |   });
  129 | 
  130 |   test("B09-20 刷新/深链进入邀请页 → 仍能发出邀请（团队上下文须自行加载）", async ({ context, page }) => {
  131 |     const { tenantId, token } = await claimTeam("ui-reload", 108, "Reload Team");
  132 |     await signIn(context, "ui-reload", 108);
  133 |     await page.goto("/onboarding/invite");
  134 |     await expect(page.getByTestId("onboarding-created-banner")).toContainText("Reload Team");
  135 |     await page.getByTestId("onboarding-invite-row-0").locator("input").first().fill("deep@team-qa.test");
  136 |     await page.getByTestId("onboarding-invite-submit").click();
  137 |     await expect(page.getByTestId("onboarding-issued")).toContainText("deep@team-qa.test");
  138 |     const list = await api("GET", `/v1/pm/tenants/${tenantId}/invitations`, token);
  139 |     expect(list.body).toHaveLength(1);
  140 |   });
  141 | });
  142 | 
  143 | // ─────────────────────────────────────────────── 接受邀请（Screen 06/07）
  144 | 
  145 | test.describe("Team QA · 接受邀请", () => {
  146 |   test("B09-06 被邀者预览并接受 → success；后端激活为 member", async ({ context, page }) => {
  147 |     const { tenantId, token } = await claimTeam("ui-host", 110, "Host Team");
  148 |     const inv = await api("POST", `/v1/pm/tenants/${tenantId}/invitations`, token, {
  149 |       email: "guest@team-qa.test",
  150 |       role: "member"
  151 |     });
  152 |     await signIn(context, "ui-guest", 110, "guest@team-qa.test");
  153 |     await page.goto(`/invitation/accept?token=${inv.body.token}&type=invite`);
  154 |     await expect(page.getByText("Host Team").first()).toBeVisible();
  155 |     await page.getByTestId("accept-confirm").click();
  156 |     await expect(page.getByTestId("accept-result")).toHaveAttribute("data-state", "success");
  157 | 
  158 |     const body = await me("ui-guest", 110);
  159 |     expect(body.is_active).toBe(true);
  160 |     expect(body.memberships[0]).toMatchObject({ role: "member", team_name: "Host Team" });
  161 | 
  162 |     await page.getByTestId("accept-continue").click();
  163 |     await expect(page).toHaveURL(/\/dashboard$/);
  164 |   });
  165 | 
  166 |   test("B09-07 已撤销邀请 → 预览即阻断（revoked）", async ({ context, page }) => {
  167 |     const { tenantId, token } = await claimTeam("ui-host2", 111, "Revoker Team");
  168 |     const inv = await api("POST", `/v1/pm/tenants/${tenantId}/invitations`, token, {
  169 |       email: "late@team-qa.test",
  170 |       role: "member"
  171 |     });
  172 |     await api("POST", `/v1/pm/tenants/${tenantId}/invitations/${inv.body.id}/revoke`, token);
  173 |     await signIn(context, "ui-late2", 111);
  174 |     await page.goto(`/invitation/accept?token=${inv.body.token}&type=invite`);
  175 |     await expect(page.getByTestId("accept-preview-blocked")).toBeVisible();
  176 |     await expect(page.getByTestId("accept-confirm")).toHaveCount(0);
  177 |   });
  178 | 
  179 |   test("B09-08 登录邮箱与邀请不符 → emailMismatch，后端未激活", async ({ context, page }) => {
  180 |     const { tenantId, token } = await claimTeam("ui-host3", 112, "Mail Team");
  181 |     const inv = await api("POST", `/v1/pm/tenants/${tenantId}/invitations`, token, {
  182 |       email: "right@team-qa.test",
  183 |       role: "member"
  184 |     });
  185 |     await signIn(context, "ui-wrong", 112, "wrong@team-qa.test");
  186 |     await page.goto(`/invitation/accept?token=${inv.body.token}&type=invite`);
  187 |     await page.getByTestId("accept-confirm").click();
  188 |     await expect(page.getByTestId("accept-result")).toHaveAttribute("data-state", "emailMismatch");
  189 |     expect((await me("ui-wrong", 112)).is_active).toBe(false);
  190 |   });
  191 | 
  192 |   test("B09-09 名额耗尽的链接 → 预览即阻断（exhausted）", async ({ context, page }) => {
  193 |     const { tenantId, token } = await claimTeam("ui-host4", 113, "Quota Team");
  194 |     const link = await api("POST", `/v1/pm/tenants/${tenantId}/invitation-links`, token, { max_uses: 1 });
  195 |     await api("POST", "/v1/pm/invitation-links/accept", signToken("ui-first-taker", 113), {
  196 |       token: link.body.token
  197 |     });
  198 |     await signIn(context, "ui-second", 113);
  199 |     await page.goto(`/invitation/accept?token=${link.body.token}&type=link`);
  200 |     await expect(page.getByTestId("accept-preview-blocked")).toBeVisible();
  201 |   });
  202 | 
  203 |   test("B09-10 跨租户接受 → tenantMismatch", async ({ context, page }) => {
  204 |     const { tenantId, token } = await claimTeam("ui-host5", 114, "Foreign Team");
  205 |     const link = await api("POST", `/v1/pm/tenants/${tenantId}/invitation-links`, token, {});
  206 |     await signIn(context, "ui-outsider", 115);
  207 |     await page.goto(`/invitation/accept?token=${link.body.token}&type=link`);
  208 |     await page.getByTestId("accept-confirm").click();
> 209 |     await expect(page.getByTestId("accept-result")).toHaveAttribute("data-state", "tenantMismatch");
      |                                                     ^ Error: expect(locator).toHaveAttribute(expected) failed
  210 |   });
  211 | 
  212 |   test("B09-11 无效 token → invalid", async ({ context, page }) => {
  213 |     await signIn(context, "ui-random", 116);
  214 |     await page.goto("/invitation/accept?token=not-a-real-token&type=invite");
  215 |     await expect(page.getByTestId("accept-result")).toHaveAttribute("data-state", "invalid");
  216 |   });
  217 | });
  218 | 
  219 | // ─────────────────────────────────────────────── 团队设置与权限（细则 #1/#2）
  220 | 
  221 | test.describe("Team QA · 团队设置与角色", () => {
  222 |   test("B09-12 TeamSwitcher 显示当前团队名", async ({ context, page }) => {
  223 |     await claimTeam("ui-switch", 120, "Switcher Team");
  224 |     await signIn(context, "ui-switch", 120);
  225 |     await page.goto("/dashboard");
  226 |     await expect(page.getByTestId("team-switcher")).toContainText("Switcher Team");
  227 |   });
  228 | 
  229 |   test("B09-13 admin 改团队资料成功；看不到 Danger Zone", async ({ context, page }) => {
  230 |     const { tenantId, token } = await claimTeam("ui-own", 121, "Admin Edit Team");
  231 |     await joinAs(token, tenantId, "ui-admin", 121, "admin");
  232 |     await signIn(context, "ui-admin", 121);
  233 |     await page.goto("/settings/team/general");
  234 |     await field(dataTest(page, "team-general-description")).fill("edited by admin");
  235 |     await dataTest(page, "team-general-save").click();
  236 |     await expect
  237 |       .poll(async () => (await api("GET", `/v1/pm/tenants/${tenantId}`, token)).body.description)
  238 |       .toBe("edited by admin");
  239 |     await expect(dataTest(page, "team-dissolve-open")).toHaveCount(0);
  240 |   });
  241 | 
  242 |   test("B09-14 member 只读：通用设置无保存、名册只读、无管理按钮", async ({ context, page }) => {
  243 |     const { tenantId, token } = await claimTeam("ui-own2", 122, "Readonly Team");
  244 |     await joinAs(token, tenantId, "ui-member", 122, "member");
  245 |     await signIn(context, "ui-member", 122);
  246 | 
  247 |     await page.goto("/settings/team/general");
  248 |     await expect(page.locator("#team-name")).toBeVisible();
  249 |     await expect(dataTest(page, "team-general-save")).toHaveCount(0);
  250 | 
  251 |     await page.goto("/settings/team/members");
  252 |     await expect(page.getByTestId("team-members-readonly-note")).toBeVisible();
  253 |     await expect(page.getByText("ui-own2").first()).toBeVisible();
  254 |     await expect(page.locator('[data-test^="team-member-remove-"]')).toHaveCount(0);
  255 |   });
  256 | 
  257 |   test("B09-15 owner 在邀请管理页创建并撤销邀请", async ({ context, page }) => {
  258 |     const { tenantId, token } = await claimTeam("ui-own3", 123, "Invites Team");
  259 |     await signIn(context, "ui-own3", 123);
  260 |     await page.goto("/settings/team/invitations");
  261 |     await field(dataTest(page, "invitations-email")).fill("ui-invited@team-qa.test");
  262 |     await dataTest(page, "invitations-submit").click();
  263 | 
  264 |     const row = page.locator('[data-test^="invitation-"]', { hasText: "ui-invited@team-qa.test" });
  265 |     await expect(row).toBeVisible();
  266 |     await row.getByRole("button").click();
  267 |     await expect(row).toHaveCount(0);
  268 | 
  269 |     const list = await api("GET", `/v1/pm/tenants/${tenantId}/invitations`, token);
  270 |     expect(list.body).toEqual(
  271 |       expect.arrayContaining([expect.objectContaining({ email: "ui-invited@team-qa.test", status: "REVOKED" })])
  272 |     );
  273 |   });
  274 | 
  275 |   test("B09-16 owner 在名册中移除成员 → 后端成员行消失", async ({ context, page }) => {
  276 |     const { tenantId, token } = await claimTeam("ui-own4", 124, "Remove Team");
  277 |     await joinAs(token, tenantId, "ui-leaver", 124, "member");
  278 |     const leaverId = (await me("ui-leaver", 124)).principal_id;
  279 |     await signIn(context, "ui-own4", 124);
  280 |     await page.goto("/settings/team/members");
  281 |     await dataTest(page, `team-member-remove-${leaverId}`).click();
  282 |     await dataTest(page, "team-member-remove-confirm").click();
  283 |     await expect(dataTest(page, `team-member-remove-${leaverId}`)).toHaveCount(0);
  284 | 
  285 |     const roster = await api("GET", `/v1/pm/tenants/${tenantId}/members`, token);
  286 |     const ids = (roster.body.items ?? roster.body).map((m: any) => m.principal_id);
  287 |     expect(ids).not.toContain(leaverId);
  288 |   });
  289 | 
  290 |   test("B09-17 owner 解散：名称不符不解锁；确认后登出并回到 onboarding 门", async ({ context, page }) => {
  291 |     const { tenantId } = await claimTeam("ui-own5", 125, "Doomed Team");
  292 |     await signIn(context, "ui-own5", 125);
  293 |     await page.goto("/settings/team/general");
  294 | 
  295 |     const open = dataTest(page, "team-dissolve-open");
  296 |     await field(dataTest(page, "team-dissolve-confirm")).fill("Wrong Name");
  297 |     await expect(open).toBeDisabled();
  298 |     await field(dataTest(page, "team-dissolve-confirm")).fill("Doomed Team");
  299 |     await expect(open).toBeEnabled();
  300 |     await open.click();
  301 |     await dataTest(page, "team-dissolve-confirm-button").click();
  302 | 
  303 |     await expect(page).toHaveURL(/\/auth\/login/, { timeout: 20_000 });
  304 |     const body = await me("ui-own5", 125);
  305 |     expect(body.team).toBeNull();
  306 |     expect(body.onboarding_required).toBe(true);
  307 |     const profile = await api("GET", `/v1/pm/tenants/${tenantId}`, signToken("ui-own5", 125));
  308 |     expect(profile.status).not.toBe(500);
  309 |   });
```