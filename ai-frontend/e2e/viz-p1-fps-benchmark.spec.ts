import { expect, test } from "@playwright/test";

/**
 * V-1 帧率测量（ai-pm-viz-26 P1 执行任务书 AC-P1-1）。
 *
 * 固定测试条件：
 * - 浏览器：系统 MS Edge（channel msedge，headless）；
 * - 视口：1440×900（设计基线桌面档位）；
 * - 数据：150 节点 / 209 边（分层 DAG，含中英文 label）；
 * - 交互操作集：hover 扫掠（触发 hover-activate 重绘）+ 滚轮缩放；
 * - 采样：requestAnimationFrame 时间戳，持续 6s；
 * - 方法：avg = frames / duration；p50/p95 = 帧间隔分位数。
 *
 * 阈值（ADR-VIZ-002 / V-1）：≥ 50 fps。
 * 运行：`npx playwright test viz-p1-fps-benchmark.spec.ts`（需 dev server）。
 * 结果写入 docs/visualization/viz-p1-g6-benchmark-v1.md。
 */

const NODE_COUNT = 150;
const SAMPLE_MS = 6000;
const FPS_THRESHOLD = 50;
/** 分流实验：VIZ_FPS_MODE=hover|wheel|both（默认 both）。 */
const MODE = (process.env.VIZ_FPS_MODE as "hover" | "wheel" | "both") ?? "both";

test("V-1 帧率：150 节点交互 ≥ 50 fps", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/");
  // 等应用初始化与路由重定向（登录守卫）稳定，避免 evaluate 期间
  // 导航销毁执行上下文（G6 模块编译耗时数秒，必须先静止）
  await page.waitForLoadState("load");
  await page.waitForTimeout(3000);

  const result = await test.step("构造 150 节点 spec 并渲染", async () => {
    return page.evaluate(
      async ({ nodeCount }) => {
        const mod = await import("/src/lib/viz/renderers/g6Adapter.ts");
        const container = document.createElement("div");
        container.id = "viz-bench-root";
        container.style.cssText = "position:fixed;inset:0;width:1440px;height:900px;";
        document.body.appendChild(container);

        // 分层 DAG：5 层 × 30 节点，层内全连相邻层（密度可调）
        const nodes = Array.from({ length: nodeCount }, (_, i) => ({
          id: `n${i}`,
          label: `节点 ${i} Task`,
          kind: i % 3 === 0 ? "task" : "requirement_revision",
          status: i % 2 === 0 ? "ready" : "completed"
        }));
        const edges = [];
        for (let i = 0; i < nodeCount - 30; i++) {
          edges.push({
            id: `e${i}`,
            from: `n${i}`,
            to: `n${i + 30}`,
            relation: i % 2 === 0 ? "task.depends_on_task" : "design.covers_requirement"
          });
        }
        // spec 形状与 Live 契约一致（layout_hint 驱动 dagre）
        const spec = {
          spec_version: "1.0",
          slot_key: "bench",
          view: { kind: "task-dag", focus: [], lenses: [], layout_hint: "layered-lr" },
          nodes,
          edges,
          meta: { budget: "balanced" }
        };

        const started = performance.now();
        const renderer = await mod.createG6Renderer({ container, spec });
        const renderMs = Math.round(performance.now() - started);
        return { renderMs, renderer, container };
      },
      { nodeCount: NODE_COUNT }
    );
  });

  expect(result.renderMs, "渲染应在合理时间内完成").toBeLessThan(15000);

  // hover 扫掠 + 滚轮缩放（真实指针事件，触发 hover-activate / zoom-canvas 重绘）
  // 交互锚点用渲染容器（SVG renderer 下无 canvas；裸 mouse 事件仍能命中图元）
  const canvas = page.locator("#viz-bench-root");
  const box = await canvas.boundingBox();
  if (box) {
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  }
  const sampling = page.evaluate(
    async sampleMs => {
      const stamps: number[] = [];
      let running = true;
      const loop = (t: number) => {
        stamps.push(t);
        if (running) requestAnimationFrame(loop);
      };
      requestAnimationFrame(loop);
      await new Promise(r => setTimeout(r, sampleMs));
      running = false;
      return stamps;
    },
    SAMPLE_MS
  );

  // 交互轨迹：按 MODE 分流（hover 扫掠 / wheel 缩放）
  if (box) {
    if (MODE !== "wheel") {
      for (let round = 0; round < 6; round++) {
        await page.mouse.move(box.x + 10, box.y + box.height / 2);
        for (let x = 10; x < box.width - 10; x += 40) {
          await page.mouse.move(box.x + x, box.y + box.height / 2 + (round % 2 === 0 ? 40 : -40));
        }
        await page.waitForTimeout(200);
      }
    }
    if (MODE === "wheel" || MODE === "both") {
      for (let round = 0; round < 6; round++) {
        await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
        await page.mouse.wheel(0, round % 2 === 0 ? -240 : 240);
        await page.waitForTimeout(200);
      }
    }
  }


  const stamps = await sampling;
  const deltas: number[] = [];
  for (let i = 1; i < stamps.length; i++) deltas.push(stamps[i] - stamps[i - 1]);
  deltas.sort((a, b) => a - b);
  const percentile = (p: number) => deltas[Math.min(deltas.length - 1, Math.floor(deltas.length * p))];
  const durationS = (stamps[stamps.length - 1] - stamps[0]) / 1000;
  const avgFps = stamps.length / durationS;
  const p50 = percentile(0.5);
  const p95 = percentile(0.95);

  const report = {
    nodeCount: NODE_COUNT,
    renderMs: result.renderMs,
    frames: stamps.length,
    durationS: +durationS.toFixed(2),
    avgFps: +avgFps.toFixed(1),
    p50DeltaMs: +p50.toFixed(1),
    p95DeltaMs: +p95.toFixed(1),
    threshold: FPS_THRESHOLD,
    viewport: "1440x900",
    browser: "msedge-headless"
  };
  console.log("[V-1 FPS]", JSON.stringify(report, null, 2));

  // 采样窗口需足够帧数才有统计意义
  expect(stamps.length).toBeGreaterThan(100);
  expect(avgFps, `150 节点交互帧率 ${avgFps.toFixed(1)} fps 应 ≥ ${FPS_THRESHOLD}`).toBeGreaterThanOrEqual(
    FPS_THRESHOLD
  );
});