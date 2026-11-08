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
        event_id = f"evt-{RUN_ID[:8]}"
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
        # Session-2（Continuation）：消费回复 → 完成标记 → 退出。
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
        time.sleep(2)


if __name__ == "__main__":
    main()
