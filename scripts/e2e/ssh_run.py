"""Paramiko SSH helper for 192.168.2.x automation (runbook-documented test credentials)."""
import os
import sys
import paramiko

HOST = sys.argv[1] if len(sys.argv) > 1 else "192.168.2.10"
USER = "kudi"
PASSWORD = "Ewq321#@!"


def client(host: str = None) -> paramiko.SSHClient:
    c = paramiko.SSHClient()
    c.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    c.connect(host or HOST, username=USER, password=PASSWORD, timeout=15)
    return c


def run(cmd: str, timeout: int = 300) -> int:
    c = client()
    _, out, err = c.exec_command(cmd, timeout=timeout)
    o = out.read().decode("utf-8", "replace")
    e = err.read().decode("utf-8", "replace")
    rc = out.channel.recv_exit_status()
    if o:
        print(o, end="" if o.endswith("\n") else "\n")
    if e:
        print(e, end="" if e.endswith("\n") else "\n", file=sys.stderr)
    c.close()
    return rc


def put(local: str, remote: str) -> None:
    local = os.path.abspath(local)  # paramiko 5.x: 相对路径在 Windows 下 open 失败
    c = client()
    sftp = c.open_sftp()
    sftp.put(local, remote)
    sftp.close()
    c.close()
    print(f"uploaded {local} -> {HOST}:{remote}")


if __name__ == "__main__":
    mode = sys.argv[2] if len(sys.argv) > 2 else "run"
    if mode == "run":
        sys.exit(run(sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 300))
    if mode == "put":
        put(sys.argv[3], sys.argv[4])
        sys.exit(0)
    print(f"unknown mode {mode}")
    sys.exit(2)
