#!/usr/bin/env python3
"""E2E 假 Agent（确定性驱动 ACP 通信故事线，不依赖真实 LLM CLI）。

会话语义（Runner 注入的环境变量区分代次）：
- Session-1（无 AI_PM_HUMAN_REPLY）：启动 3s 后写 interaction 标记请求人工介入，
  然后长眠等待（Runner 收到 terminate 命令后终止本进程——D-03 立即终止）。
- Session-2（有 AI_PM_HUMAN_REPLY = Continuation）：读回复、写 done 标记
  （含回复摘要），模拟"继续执行并完成"，随后主动退出。

标记文件协议（V1.1 §1.2）：schema aipm.interaction.v1 / event_id 键控 /
原子写（tmp+rename）/ 只创建新事件不改写已提交事件。
"""
import json
import os
import time

AIPM = os.path.join(os.environ.get("AI_PM_WORKSPACE", "."), ".aipm")
RUN_ID = os.environ.get("AI_PM_RUN_ID", "unknown-run")


def atomic_write(path: str, payload: bytes) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(payload)
    os.replace(tmp, path)


def marker(event_id: str, reason: str, question: str, options=None) -> bytes:
    event = {
        "schema_version": "aipm.interaction.v1",
        "event_id": event_id,
        "reason": reason,
        "title": "E2E 人工介入请求",
        "question": question,
        "options": options or [],
        "required_response": True,
        "checkpoint": {"path": ".aipm/checkpoint.json", "available": True},
    }
    return json.dumps(event, ensure_ascii=False).encode("utf-8")


def main() -> None:
    reply = os.environ.get("AI_PM_HUMAN_REPLY")
    if not reply:
        # Session-1：请求人工介入后等待终止（D-03）。
        # P2-1 收敛（检查点 2 I6）：标记文件名含 event_id（双键一致）。
        event_id = f"evt-{RUN_ID[:8]}-{int(time.time())}"  # 平台 UNIQUE(run_id,event_id)：跨轮唯一
        atomic_write(
            os.path.join(AIPM, f"interaction.{event_id}.json"),
            marker(
                event_id,
                "needs_decision",
                "E2E：是否允许删除旧字段？",
                [{"value": "approve", "label": "允许"},
                 {"value": "reject", "label": "保留"}],
            ),
        )
        print(f"[fake-agent] session-1 interaction marker written; run={RUN_ID}", flush=True)
        while True:
            time.sleep(1)
    else:
        # Session-2（Continuation）：消费回复 → 交付（commit+push+submit）→ 完成标记 → 退出。
        reply_obj = json.loads(reply)
        atomic_write(
            os.path.join(AIPM, f"done-{RUN_ID[:8]}.json"),
            json.dumps(
                {
                    "run_id": RUN_ID,
                    "consumed_reply": reply_obj,
                    "note": "session-2 completed with human reply",
                },
                ensure_ascii=False,
            ).encode("utf-8"),
        )
        print(f"[fake-agent] session-2 consumed reply and finished; run={RUN_ID}", flush=True)
        # ── 交付段（V-05-2：Agent 正常 commit/push + 申报 SHA，平台远端只读验证）──
        delivery_outcome = deliver()
        print(f"[fake-agent] delivery outcome: {delivery_outcome}", flush=True)
        time.sleep(2)


def deliver() -> str:
    """Agent 交付：workspace.json → commit/push → pm_submit_result（MCP）。

    凭据纪律（ADR-0002）：lease_token 只经环境变量持有于内存，绝不落盘；
    git push 走用户会话凭据（deploy key），平台侧只读验证申报 SHA（Q-RC-1）。
    """
    import subprocess
    import urllib.request

    ws_path = os.path.join(AIPM, "workspace.json")
    if not os.path.exists(ws_path):
        return "skipped: no workspace.json"
    raw = json.loads(open(ws_path, encoding="utf-8").read())
    if isinstance(raw, str):  # runner 落盘为 env 字符串（双重编码）——稳健解码
        raw = json.loads(raw)
    repos = raw if isinstance(raw, list) else [raw]
    if not repos:
        return "skipped: empty workspace map"
    repo = repos[0]
    local, base = repo["local_path"], repo["base_commit"]

    def git(*args: str) -> str:
        out = subprocess.run(["git", "-C", local, *args], capture_output=True, text=True, timeout=60)
        if out.returncode != 0:
            raise RuntimeError(f"git {args[0]} failed: {out.stderr.strip()[:200]}")
        return out.stdout.strip()

    delivery_file = os.path.join(local, f"delivery-{RUN_ID[:8]}.md")
    with open(delivery_file, "w", encoding="utf-8") as f:
        f.write(f"# R4 boundary-1 delivery ({RUN_ID})\n\n- base: {base}\n- reply consumed ✓\n")
    git("add", "-A")
    git("-c", "user.name=agent-59", "-c", "user.email=agent59@e2e.local",
        "commit", "-q", "-m", f"R4 delivery for run {RUN_ID[:8]}")
    branch = f"aipm/{RUN_ID[:8]}-{int(time.time())}"  # 每轮唯一分支：push 非快进互不阻塞
    git("push", "-q", "origin", f"HEAD:refs/heads/{branch}")
    commit_sha = git("rev-parse", "HEAD")

    mcp_url = os.environ.get("AI_PM_MCP_URL", "")
    token = os.environ.get("AI_PM_LEASE_TOKEN", "")
    gen = os.environ.get("AI_PM_LEASE_GENERATION", "1")
    project = os.environ.get("AI_PM_PROJECT_ID", "")
    body = {
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "pm_submit_result", "arguments": {
            "project_id": project, "run_id": RUN_ID,
            "lease_token": token, "lease_generation": int(gen),
            "integration_status": "not_run",
            "repositories": [{"repository_id": repo["repository_id"], "base_commit": base,
                              "commit_sha": commit_sha, "changed_files": 1, "tests": []}],
            "summary": f"R4 boundary-1 delivery for run {RUN_ID[:8]}",
            "knowledge_proposals": [],
        }},
    }
    req = urllib.request.Request(mcp_url, method="POST",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json",
                 "Accept": "application/json, text/event-stream",
                 "Authorization": "Bearer " + os.environ.get("AI_PM_RUNNER_TOKEN", "")})
    with urllib.request.urlopen(req, timeout=60) as r:
        raw = r.read().decode()
    line = next((l[6:] for l in raw.splitlines() if l.startswith("data: ")), raw)
    inner = json.loads(line).get("result", {})
    text = inner.get("content", [{}])[0].get("text", "")
    return f"submit({text[:160]}) commit={commit_sha[:8]} branch={branch}"


if __name__ == "__main__":
    main()
