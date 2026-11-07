#!/usr/bin/env bash
# E2E 真 CLI Agent 适配层（ACP-08 Adapter 契约的 V1 实现）：
# - 协议面（标记文件/reply 注入）由本 wrapper 确定性承载——Adapter 的职责边界；
# - Agent 智能面由真实 LLM CLI（opencode，免费模型）执行——LLM 真实产出落盘。
# 会话语义与 fake_agent.py 一致：
#   Session-1（无 AI_PM_HUMAN_REPLY）：写介入标记 → opencode 做阻塞期分析 → 长眠待终止
#   Session-2（有 AI_PM_HUMAN_REPLY）：opencode 做收尾工作 → 写 done 标记 → 退出
set -u
AIPM="${AI_PM_WORKSPACE:-.}/.aipm"
RUN_ID="${AI_PM_RUN_ID:-unknown}"
mkdir -p "$AIPM"

MODEL="opencode/deepseek-v4.1-flash-free"

if [ -z "${AI_PM_HUMAN_REPLY:-}" ]; then
  # ── Session-1：人工介入请求 ──
  EVENT_ID="evt-${RUN_ID:0:8}"
  cat > "$AIPM/interaction.json" <<EOF
{"schema_version":"aipm.interaction.v1","event_id":"$EVENT_ID","reason":"needs_decision","title":"E2E 人工介入请求","question":"是否允许删除旧字段？","options":[{"value":"approve","label":"允许"},{"value":"reject","label":"保留"}],"required_response":true,"checkpoint":{"path":".aipm/checkpoint.json","available":true}}
EOF
  echo "[wrapper] session-1 marker written ($EVENT_ID); launching opencode for blocked-period analysis"
  timeout 180 opencode run --model "$MODEL" \
    "任务：ACP E2E 通信故事线。你当前被阻塞，等待人类决策（是否删除旧字段）。\
请把你的阻塞期分析（当前状态、等待的决策、风险）写入 $AI_PM_WORKSPACE/analysis-s1.md，最多 10 行。" \
    > "$AIPM/opencode-s1.log" 2>&1
  echo "[wrapper] opencode session-1 exit=$?; analysis file:"
  ls -la "$AI_PM_WORKSPACE/analysis-s1.md" 2>/dev/null || echo "[wrapper] analysis-s1.md MISSING (LLM 未产出)"
  # 长眠等待 Runner 的 terminate 命令（D-03 立即终止）
  sleep 600
else
  # ── Session-2（Continuation）：消费回复 → 收尾工作 → 完成标记 ──
  echo "[wrapper] session-2 reply consumed: $AI_PM_HUMAN_REPLY"
  timeout 180 opencode run --model "$MODEL" \
    "任务继续（Continuation）。人类已批准：$AI_PM_HUMAN_REPLY。\
请把收尾总结（批准内容、后续动作）写入 $AI_PM_WORKSPACE/analysis-s2.md，最多 10 行。" \
    > "$AIPM/opencode-s2.log" 2>&1
  echo "[wrapper] opencode session-2 exit=$?; summary file:"
  ls -la "$AI_PM_WORKSPACE/analysis-s2.md" 2>/dev/null || echo "[wrapper] analysis-s2.md MISSING (LLM 未产出)"
  # 完成标记（done-<run>.json）：消费回执的证据面（Adapter 职责）
  printf '{"run_id":"%s","consumed_reply":%s,"note":"session-2 completed with human reply (opencode)"}' \
    "$RUN_ID" "$AI_PM_HUMAN_REPLY" > "$AIPM/done-${RUN_ID:0:8}.json"
  echo "[wrapper] done marker written; session-2 exiting"
fi
