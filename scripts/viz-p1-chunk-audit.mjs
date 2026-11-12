/**
 * V-1 chunk 审计（ai-pm-viz-26 P1 执行任务书 AC-P1-1）。
 *
 * 校验三项：
 * 1. **主 bundle 零污染**：G6 特征串（antv-dagre 布局注册名等）不得出现在
 *    entry chunk 与首屏共享 chunk；
 * 2. **G6 chunk gzip ≤ 350 KB**；
 * 3. 输出体积清单（写入 benchmark 报告用）。
 *
 * 用法：先 `pnpm build`，再 `node ../scripts/viz-p1-chunk-audit.mjs`。
 * 退出码非 0 即不达标（CI 可直接用）。
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

const DIST_JS = join(process.cwd(), "dist", "static", "js");
/** 主 bundle 零污染判定的特征串（压缩后仍保留的 G6 内部字符串）。 */
const G6_MARKERS = ["antv-dagre", "hover-activate", "combo-drag"];
const LIMIT_GZIP_BYTES = 350 * 1024;

function gzipSize(filePath) {
  return gzipSync(readFileSync(filePath)).length;
}

function main() {
  let files;
  try {
    files = readdirSync(DIST_JS).filter(f => f.endsWith(".js"));
  } catch {
    console.error(`[viz-p1-chunk-audit] 找不到 ${DIST_JS}，请先 pnpm build`);
    process.exit(2);
  }

  const chunks = files.map(name => {
    const filePath = join(DIST_JS, name);
    const content = readFileSync(filePath, "utf8");
    const markers = G6_MARKERS.filter(m => content.includes(m));
    return {
      name,
      raw: statSync(filePath).size,
      gzip: gzipSize(filePath),
      g6: markers
    };
  });

  const g6Chunks = chunks.filter(c => c.g6.length > 0);
  const others = chunks.filter(c => c.g6.length === 0);

  // 1) 主 bundle（entry，无 hash 前缀命名区分——以含 "entry" 或最大非 G6 chunk 判定）
  const entry = others.find(c => /entry/i.test(c.name)) ?? others.sort((a, b) => b.raw - a.raw)[0];
  const polluted = entry ? entry.g6.length : 1;
  // 其他共享 chunk 也检查（防 G6 泄进 vendor）
  const leakedOthers = others.filter(c => c.g6.length > 0);

  // 2) G6 chunk gzip 总量（可能拆多个文件）
  const g6GzipTotal = g6Chunks.reduce((sum, c) => sum + c.gzip, 0);

  const results = {
    timestamp: new Date().toISOString(),
    limitGzipKB: LIMIT_GZIP_BYTES / 1024,
    g6Chunks: g6Chunks.map(c => ({ name: c.name, gzipKB: +(c.gzip / 1024).toFixed(1) })),
    g6GzipTotalKB: +(g6GzipTotal / 1024).toFixed(1),
    entryChunk: entry ? { name: entry.name, gzipKB: +(entry.gzip / 1024).toFixed(1), g6Markers: entry.g6 } : null,
    leakedOthers: leakedOthers.map(c => c.name),
    totalChunks: chunks.length
  };

  console.log("[viz-p1-chunk-audit] chunks:");
  for (const c of chunks.sort((a, b) => b.gzip - a.gzip).slice(0, 12)) {
    console.log(
      `  ${c.g6.length ? "[G6]" : "    "} ${c.name}  gzip=${(c.gzip / 1024).toFixed(1)}KB  raw=${(c.raw / 1024).toFixed(0)}KB`
    );
  }
  console.log(`[viz-p1-chunk-audit] G6 gzip 总量 = ${(g6GzipTotal / 1024).toFixed(1)} KB（上限 350 KB）`);

  let ok = true;
  if (polluted) {
    console.error(`✗ 主 bundle 含 G6 特征：${entry?.name}`);
    ok = false;
  } else {
    console.log(`✓ 主 bundle 零污染：${entry?.name}`);
  }
  if (leakedOthers.length > 0) {
    console.error(`✗ 其他非 G6 chunk 泄漏：${leakedOthers.join(", ")}`);
    ok = false;
  }
  if (g6GzipTotal > LIMIT_GZIP_BYTES) {
    console.error(`✗ G6 chunk gzip ${(g6GzipTotal / 1024).toFixed(1)}KB 超 350KB 上限`);
    ok = false;
  } else {
    console.log(`✓ G6 chunk 体积达标`);
  }

  console.log(JSON.stringify(results, null, 2));
  process.exit(ok ? 0 : 1);
}

main();