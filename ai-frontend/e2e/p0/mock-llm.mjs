#!/usr/bin/env node
/**
 * P0 E2E · 确定性 mock LLM（OpenAI 兼容 /chat/completions，`openai_compatible`
 * provider 的 endpoint 指向本服务）。
 *
 * 分流依据（pm-ai 两个调用方的请求形态）：
 *  - 分析链 `openai_adapter.rs`：messages 带 **system**（SYSTEM_PROMPT），
 *    content 必须能反序列化成 AnalysisResult（summary/findings/proposals，
 *    proposals.command = ProjectCommand adjacent tag `{"kind","data"}`）。
 *  - 回复链 `conversation_model.rs`：只有 user/assistant 轮次，content 为
 *    纯文本即可（非空）。
 */
import { createServer } from "node:http";

const port = Number(process.argv[2] ?? "18497");

const ANALYSIS_RESULT = {
  summary:
    "mock 分析：识别到任务管理系统的核心能力（创建/分配/跟踪/进度）与一个待确认问题（AI 拆解边界）。",
  findings: [
    {
      skill: "requirement_analysis",
      severity: "info",
      description: "核心能力集：任务创建、任务分配、状态跟踪、进度查看。",
      evidence_refs: ["inline:mock-input"]
    },
    {
      skill: "requirement_analysis",
      severity: "info",
      description: "待确认：AI 辅助拆解任务的边界与人工确认点。",
      evidence_refs: ["inline:mock-input"]
    }
  ],
  proposals: [
    {
      command: {
        kind: "create_requirements",
        data: {
          requirements: [
            {
              key: "R1",
              code: "REQ-MOCK-001",
              title: "任务创建与分配",
              markdown:
                "# 任务创建与分配\n\n系统支持团队成员创建任务，管理员可将任务分配给指定成员。",
              priority: "high",
              confidence: "high",
              evidence: []
            }
          ]
        }
      },
      rationale: "mock 分析生成的待确认需求提案（必须人工确认后才落地）。",
      risk_level: "low",
      confidence: "high",
      evidence: ["inline:mock-input"]
    }
  ]
};

createServer((req, res) => {
  if (!req.url || !req.url.endsWith("/chat/completions")) {
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: { message: "not found" } }));
    return;
  }
  let raw = "";
  req.on("data", chunk => {
    raw += chunk;
  });
  req.on("end", () => {
    let isAnalysis = false;
    try {
      const body = JSON.parse(raw);
      isAnalysis = (body.messages ?? []).some(m => m.role === "system");
    } catch {
      // 保持默认：按回复链处理
    }
    const content = isAnalysis
      ? JSON.stringify(ANALYSIS_RESULT)
      : "mock 回复：基于当前项目上下文，建议先完成目标文档与仓库接入，再进入需求拆解。";
    res.writeHead(200, { "content-type": "application/json" });
    res.end(
      JSON.stringify({
        id: "chatcmpl-mock",
        object: "chat.completion",
        model: "mock-e2e",
        choices: [{ index: 0, message: { role: "assistant", content }, finish_reason: "stop" }]
      })
    );
  });
}).listen(port, "127.0.0.1", () => {
  console.log(`mock-llm listening on http://127.0.0.1:${port}`);
});
