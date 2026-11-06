# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: team-onboarding.spec.ts >> Team QA · Onboarding 流程 >> B09-20 刷新/深链进入邀请页 → 仍能发出邀请（团队上下文须自行加载）
- Location: e2e\team-onboarding.spec.ts:130:3

# Error details

```
Error: expect(locator).toContainText(expected) failed

Locator: getByTestId('onboarding-created-banner')
Expected substring: "Reload Team"
Received string:    "团队创建成功你现在是「新团队」的 Owner。下一步邀请成员加入。"
Timeout: 15000ms

Call log:
  - Expect "toContainText" getByTestId('onboarding-created-banner') with timeout 15000ms
  - waiting for getByTestId('onboarding-created-banner')
    33 × locator resolved to <div class="success" data-v-943c77ae="" data-testid="onboarding-created-banner">…</div>
       - unexpected value "团队创建成功你现在是「新团队」的 Owner。下一步邀请成员加入。"

```

```yaml
- paragraph: 团队创建成功
- paragraph: 你现在是「新团队」的 Owner。下一步邀请成员加入。
```

# Test source

```ts
  34  | 
  35  | // ─────────────────────────────────────────────── 首登与建团（Screen 01-05）
  36  | 
  37  | test.describe("Team QA · Onboarding 流程", () => {
  38  |   test("B09-01 首登无团队 → 登录中转页分流到 /onboarding", async ({ context, page }) => {
  39  |     await signIn(context, "ui-first", 101);
  40  |     await page.goto("/auth/signing-in");
  41  |     await expect(page).toHaveURL(/\/onboarding$/, { timeout: 20_000 });
  42  |     await expect(page.getByTestId("onboarding-choice-create")).toBeVisible();
  43  |     expect((await me("ui-first", 101)).onboarding_required).toBe(true);
  44  |   });
  45  | 
  46  |   test("B09-02 建团 → 邀请页横幅；后端创建者=owner、团队档案落库", async ({ context, page }) => {
  47  |     await signIn(context, "ui-founder", 102);
  48  |     await page.goto("/onboarding");
  49  |     await page.getByTestId("onboarding-choice-create").click();
  50  |     await expect(page).toHaveURL(/\/onboarding\/create$/);
  51  | 
  52  |     await field(page.getByTestId("onboarding-team-name")).fill("QA 团队 102");
  53  |     await field(page.getByTestId("onboarding-team-slug")).fill("qa-team-102");
  54  |     await field(page.getByTestId("onboarding-team-description")).fill("created by e2e");
  55  |     await page.getByTestId("onboarding-create-submit").click();
  56  | 
  57  |     await expect(page).toHaveURL(/\/onboarding\/invite/);
  58  |     await expect(page.getByTestId("onboarding-created-banner")).toContainText("QA 团队 102");
  59  | 
  60  |     const body = await me("ui-founder", 102);
  61  |     expect(body.onboarding_required).toBe(false);
  62  |     expect(body.tenant_role).toBe("owner");
  63  |     expect(body.team).toMatchObject({ name: "QA 团队 102", slug: "qa-team-102" });
  64  |   });
  65  | 
  66  |   test("B09-03 slug 被占用 → 页面显示错误且不跳转", async ({ context, page }) => {
  67  |     await claimTeam("ui-squatter", 103, "Squatter", "taken-slug");
  68  |     await signIn(context, "ui-late", 104);
  69  |     await page.goto("/onboarding/create");
  70  |     await field(page.getByTestId("onboarding-team-name")).fill("Late Team");
  71  |     await field(page.getByTestId("onboarding-team-slug")).fill("taken-slug");
  72  |     await page.getByTestId("onboarding-create-submit").click();
  73  | 
  74  |     await expect(page.getByTestId("onboarding-create-error")).toBeVisible();
  75  |     await expect(page).toHaveURL(/\/onboarding\/create$/);
  76  |     expect((await me("ui-late", 104)).onboarding_required).toBe(true);
  77  |   });
  78  | 
  79  |   test("B09-04 建团后连续流程：邀请成员 → 生成一次性链接；后端 PENDING 邀请落库", async ({ context, page }) => {
  80  |     await signIn(context, "ui-inviter", 105);
  81  |     await page.goto("/onboarding/create");
  82  |     await field(page.getByTestId("onboarding-team-name")).fill("Inviter Team");
  83  |     await page.getByTestId("onboarding-create-submit").click();
  84  |     await expect(page).toHaveURL(/\/onboarding\/invite/);
  85  | 
  86  |     await page.getByTestId("onboarding-invite-row-0").locator("input").first().fill("newbie@team-qa.test");
  87  |     await page.getByTestId("onboarding-invite-submit").click();
  88  |     const issued = page.getByTestId("onboarding-issued");
  89  |     await expect(issued).toContainText("newbie@team-qa.test");
  90  |     await expect(issued).toContainText("/invitation/accept?token=");
  91  | 
  92  |     const token = signToken("ui-inviter", 105);
  93  |     const tenantId = (await me("ui-inviter", 105)).tenant_id;
  94  |     const list = await api("GET", `/v1/pm/tenants/${tenantId}/invitations`, token);
  95  |     expect(list.body).toEqual(
  96  |       expect.arrayContaining([
  97  |         expect.objectContaining({ email: "newbie@team-qa.test", role: "member", status: "PENDING" })
  98  |       ])
  99  |     );
  100 |   });
  101 | 
  102 |   test("B09-05 建团后连续流程：跳过邀请 → 链接页生成链接 → 完成进入团队首页", async ({ context, page }) => {
  103 |     await signIn(context, "ui-linker", 106);
  104 |     await page.goto("/onboarding/create");
  105 |     await field(page.getByTestId("onboarding-team-name")).fill("Linker Team");
  106 |     await page.getByTestId("onboarding-create-submit").click();
  107 |     await page.getByTestId("onboarding-invite-skip").click();
  108 |     await expect(page).toHaveURL(/\/onboarding\/link$/);
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
> 134 |     await expect(page.getByTestId("onboarding-created-banner")).toContainText("Reload Team");
      |                                                                 ^ Error: expect(locator).toContainText(expected) failed
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
  209 |     await expect(page.getByTestId("accept-result")).toHaveAttribute("data-state", "tenantMismatch");
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
```