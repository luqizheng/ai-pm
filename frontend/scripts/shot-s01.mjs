/**
 * S01 前端页面截图脚本（本地预览用）。
 *
 * 用法：node scripts/shot-s01.mjs
 * 前置：dev server 已启动（pnpm dev），VITE_USE_MOCK=1。
 */
import { chromium } from "playwright";
import { mkdirSync } from "node:fs";

const BASE = "http://127.0.0.1:8848";
const OUT = ".preview";
mkdirSync(OUT, { recursive: true });

const PAGES = [
  { name: "01-project-creation", path: "/projects/new" },
  { name: "02-repositories", path: "/projects/prj-shield/repositories-v2" },
  { name: "03-cognition", path: "/projects/prj-shield/cognition" },
  { name: "04-baseline", path: "/projects/prj-shield/baseline" }
];

const browser = await chromium.launch();
const context = await browser.newContext({
  viewport: { width: 1440, height: 960 },
  locale: "zh-CN"
});

// 预置登录态：按 token-storage.ts 的 KEYS 分别写 localStorage
await context.addInitScript(() => {
  const user = {
    id: "usr-admin",
    username: "admin",
    nickname: "管理员",
    email: null,
    phone: null,
    avatar: null,
    roles: ["project_owner"],
    permissions: []
  };
  localStorage.setItem("prism_access_token", "mock-access-token");
  localStorage.setItem("prism_refresh_token", "mock-refresh-token");
  localStorage.setItem("prism_expires_at", String(Date.now() + 86400 * 1000));
  localStorage.setItem("prism_user", JSON.stringify(user));
});

const page = await context.newPage();
const errors = [];
page.on("console", msg => {
  if (msg.type() === "error") errors.push(msg.text());
});
page.on("pageerror", err => errors.push(String(err)));

for (const target of PAGES) {
  await page.goto(`${BASE}${target.path}`, { waitUntil: "networkidle" });
  await page.waitForTimeout(1200);
  await page.screenshot({
    path: `${OUT}/${target.name}.png`,
    fullPage: true
  });
  console.log(`[shot] ${target.name} <- ${target.path}`);
}

await browser.close();

if (errors.length > 0) {
  console.log("\n[console errors]");
  for (const e of [...new Set(errors)].slice(0, 20)) console.log(" -", e);
} else {
  console.log("\n[console] 无错误");
}
