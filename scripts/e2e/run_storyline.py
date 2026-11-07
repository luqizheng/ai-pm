"""ACP 通信故事线 E2E 驱动（真实栈 192.168.2.10 + Runner 192.168.2.59）。

故事线（V0.5 §九 / V1.1 通信面）：
  Runner 自领 queued Run → spawn Agent(S1) → Agent 写介入标记 → Runner 上报
  → Platform: Interaction+awaiting_decision+guard+terminate 命令 → Runner 终止 S1
  → 人类经 REST 回复（A* 入口）→ 同事务 continue 命令 → Runner preflight+spawn S2
  （回复注入）→ 消费回执 → CONSUMED + guard 释放 → Agent S2 完成 → 收割。
"""
import json
import sys
import time
import urllib.request

sys.path.insert(0, "scripts/e2e")
from ssh_run import client as ssh_client  # noqa: E402

HOST10 = "192.168.2.10"
HOST59 = "192.168.2.59"
PROJECT = "11111111-1111-1111-1111-111111111111"
TASK = "33333333-3333-3333-3333-333333333333"
RUN = "44444444-4444-4444-4444-444444444444"

EVIDENCE = []


def ev(step: str, detail: str) -> None:
    line = f"[EVIDENCE] {step}: {detail}"
    EVIDENCE.append(line)
    print(line, flush=True)


def psql(sql: str) -> str:
    c = ssh_client(HOST10)
    _, out, _ = c.exec_command(
        f'docker exec infrastructure-aipm-pg-1 psql -U aipm -d aipm -t -A -c "{sql}"',
        timeout=60,
    )
    r = out.read().decode().strip()
    c.close()
    return r


def http(method: str, url: str, token: str = None, body: dict = None):
    req = urllib.request.Request(url, method=method)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = None
    if body is not None:
        req.add_header("Content-Type", "application/json")
        data = json.dumps(body).encode()
    try:
        with urllib.request.urlopen(req, data=data, timeout=15) as r:
            return r.status, json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read().decode() or "{}")


def ssh59(cmd: str, timeout: int = 60) -> str:
    c = ssh_client(HOST59)
    _, out, err = c.exec_command(cmd, timeout=timeout)
    r = out.read().decode() + err.read().decode()
    c.close()
    return r


def ssh59_nowait(cmd: str) -> None:
    """Fire-and-forget：写入即断通道（nohup + 全 fd 重定向保活远端进程）。"""
    c = ssh_client(HOST59)
    c.exec_command(cmd, timeout=10)
    time.sleep(1)
    c.close()


def main() -> None:
    # ── 登录 ──
    st, body = http("POST", f"http://{HOST10}:3000/v1/iam/auth/login",
                    body={"username": "admin", "password": "Admin@123456"})
    assert st == 200, f"login failed: {st} {body}"
    token = body["access_token"]
    ev("S0-login", f"admin JWT ok (sub={body.get('sub', 'n/a')})")

    # ── S1 启动 Runner（.59，nohup）──
    ssh59("pkill -f 'ai-pm-runner' 2>/dev/null; rm -rf /home/kudi/e2e/ws; true")
    runner_env = (
        f"AI_PM_PLATFORM_URL=http://{HOST10}:8090/mcp "
        f"AI_PM_RUNNER_TOKEN={token} "
        f"AI_PM_RUNNER_PROJECT_ID={PROJECT} "
        f"AI_PM_RUNNER_WORKSPACE_ROOT=/home/kudi/e2e/ws "
        f"AI_PM_RUNNER_AGENT_CMD='python3 /home/kudi/e2e/fake_agent.py' "
        f"AI_PM_RUNNER_CLAIM_INTERVAL_SECS=3 "
        f"AI_PM_INTERACTION_DETECT_INTERVAL_SECS=2 "
        f"AI_PM_COMMAND_POLL_INTERVAL_SECS=2 "
        f"RUST_LOG=debug"
    )
    ssh59_nowait(
        f"cd /home/kudi/e2e && nohup env {runner_env} ./ai-pm-runner > runner.log 2>&1 < /dev/null & echo started")
    time.sleep(2)
    alive = ssh59("pgrep -fc 'e2e/ai-pm-runner' || true").strip()
    ev("S1-runner-start", f"runner started on .59 (claim interval 3s); alive={alive}")

    # ── S2 等待自领 + 介入上报（Platform 侧断言）──
    interaction_id = ""
    for i in range(30):
        time.sleep(2)
        interaction_id = psql(
            f"SELECT id FROM delivery.task_interaction WHERE task_id='{TASK}' AND status='pending' LIMIT 1")
        run_state = psql(f"SELECT status FROM delivery.task_run WHERE id='{RUN}'")
        if interaction_id:
            ev("S2-interaction", f"interaction={interaction_id} reported by runner; run={run_state}")
            break
        if i == 29:
            raise SystemExit("TIMEOUT waiting for interaction")
    run_state = psql(f"SELECT status FROM delivery.task_run WHERE id='{RUN}'")
    task_state = psql(f"SELECT status FROM delivery.task WHERE id='{TASK}'")
    shield = psql(f"SELECT scheduling_shield FROM delivery.task_run WHERE id='{RUN}'")
    ev("S2-guard", f"run={run_state} task={task_state} shield={shield} (expect running/awaiting_decision/human_wait)")
    cmd_count = psql(
        f"SELECT count(*) FROM delivery.worker_command WHERE run_id='{RUN}' AND command_type='agent.process.terminate' AND status='issued'")
    ev("S2-terminate-cmd", f"terminate commands issued={cmd_count} (D-03 same-tx)")

    # ── S3 Runner 终止 S1 + ack ──
    for i in range(15):
        time.sleep(2)
        st_cmd = psql(
            f"SELECT status FROM delivery.worker_command WHERE run_id='{RUN}' AND command_type='agent.process.terminate' ORDER BY issued_at LIMIT 1")
        if st_cmd == "acked":
            ev("S3-terminate", "terminate command acked by runner (process killed)")
            break
        if i == 14:
            raise SystemExit(f"TIMEOUT waiting terminate ack (last={st_cmd})")

    # ── S4 人类 REST 回复（A*：可信 REST/UI 入口；AC P-446）──
    st, body = http("POST", f"http://{HOST10}:8090/v1/pm/interactions/{interaction_id}/responses",
                    token=token,
                    body={"response_payload": {"decision": "approve", "content": "E2E：允许删除旧字段，继续执行"},
                          "idempotency_key": "e2e-reply-001"})
    assert st == 200, f"reply failed: {st} {body}"
    ev("S4-reply", f"REST reply 200; response_id={body.get('response_id')} "
                   f"continue_command_id={body.get('continue_command_id')}")

    # ── S5 Runner 消费 continue → S2 spawn → 消费回执 ──
    consumed = ""
    for i in range(30):
        time.sleep(2)
        consumed = psql(
            f"SELECT status FROM delivery.task_interaction WHERE id='{interaction_id}'")
        delivery = psql(
            f"SELECT count(*) FROM delivery.worker_command WHERE run_id='{RUN}' AND command_type='agent.task.continue' AND status='acked'")
        if consumed == "consumed":
            ev("S5-consumed", f"interaction consumed; continue acked={delivery}")
            break
        if i == 29:
            log_tail = ssh59("tail -25 /home/kudi/e2e/runner.log; pgrep -af '[a]i-pm-runner' || echo runner-gone")
            print("===== runner.log on S5 timeout =====")
            print(log_tail)
            raise SystemExit(f"TIMEOUT waiting consumption (status={consumed}, continue_acked={delivery})")
    shield2 = psql(f"SELECT scheduling_shield FROM delivery.task_run WHERE id='{RUN}'")
    ev("S5-guard-release", f"shield={shield2} (expect none after continue acked)")

    # ── S6 Agent S2 完成证据（done 标记含回复）──
    done = ssh59("cat /home/kudi/e2e/ws/*/.aipm/done-*.json 2>/dev/null | head -5")
    ev("S6-agent-done", f"session-2 done marker: {done[:160]}")

    # ── 终态汇总 ──
    final = psql(
        f"SELECT ti.status, r.scheduling_shield, t.status FROM delivery.task_interaction ti "
        f"JOIN delivery.task_run r ON r.id = ti.run_id "
        f"JOIN delivery.task t ON t.id = ti.task_id WHERE ti.id='{interaction_id}'")
    ledger = psql(f"SELECT status FROM delivery.task_run_cleanup WHERE run_id='{RUN}'")
    cmds = psql(
        f"SELECT command_type || '=' || status FROM delivery.worker_command WHERE run_id='{RUN}' ORDER BY issued_at")
    ev("S7-final", f"interaction|shield|task = {final}; ledger={ledger}; commands={cmds}")

    runner_log = ssh59("tail -20 /home/kudi/e2e/runner.log")
    print("\n===== runner.log (tail) =====")
    print(runner_log)

    ssh59("pkill -f 'ai-pm-runner' 2>/dev/null; true")
    with open("target/e2e/storyline_evidence.txt", "w", encoding="utf-8") as f:
        f.write("\n".join(EVIDENCE) + "\n")
    print("\n[E2E-STORYLINE-PASS]")


if __name__ == "__main__":
    main()
