#!/usr/bin/env node
/**
 * 把 Vditor 的运行时资源同步到 public/。
 *
 * 为什么需要：Vditor 的 Markdown 解析引擎（lute）与内容主题是在运行时用
 * `<script src>` / `<link href>` 动态加载的，地址由 `options.cdn` 拼出，
 * 默认指向 `https://unpkg.com/vditor@<version>`。内网/离线环境下该地址不可达，
 * 编辑器会停在「解析引擎未加载」的状态，无法初始化。
 *
 * 因此构建与本地开发前把这几类资源落到 `frontend/public/vditor/dist/`，
 * 组件侧把 `cdn` 指向同源路径即可。产物随依赖版本自动刷新，升级 vditor 后重跑本脚本。
 *
 * 用法：node scripts/sync-vditor-assets.js
 */
const { cpSync, existsSync, mkdirSync, rmSync } = require("node:fs");
const { join, dirname } = require("node:path");

const ROOT = join(__dirname, "..");
const SRC_DIST = join(ROOT, "frontend", "node_modules", "vditor", "dist");
const DEST_ROOT = join(ROOT, "frontend", "public", "vditor", "dist");

/**
 * 只同步实际用到的子集：
 *   js/lute        —— Markdown 解析引擎，缺了编辑器起不来
 *   css/content-theme —— 正文排版主题
 *   images/emoji   —— 正文 emoji 图标
 * 公式、图表、脑图等重型渲染器不在当前功能范围内，不同步。
 */
const ENTRIES = [
  "js/lute/lute.min.js",
  "css/content-theme",
  "images/emoji"
];

function main() {
  if (!existsSync(SRC_DIST)) {
    console.error(
      "[sync-vditor] 未找到 vditor 依赖，请先在 frontend 下执行 pnpm install"
    );
    process.exit(1);
  }

  rmSync(DEST_ROOT, { recursive: true, force: true });
  mkdirSync(DEST_ROOT, { recursive: true });

  let copied = 0;
  for (const entry of ENTRIES) {
    const from = join(SRC_DIST, entry);
    if (!existsSync(from)) {
      console.warn(`[sync-vditor] 跳过缺失资源：${entry}`);
      continue;
    }
    const to = join(DEST_ROOT, entry);
    mkdirSync(dirname(to), { recursive: true });
    cpSync(from, to, { recursive: true });
    copied += 1;
  }

  console.log(`[sync-vditor] 已同步 ${copied}/${ENTRIES.length} 项到 public/vditor`);
}

main();
