#!/usr/bin/env python3
"""Apply acp09_reset.sql with resolved hashes (idempotent deterministic reset)."""
import hashlib
import subprocess
import sys

lease_token = "e2e0f1e2d3c4b5a69788796a5b4c3d2e1f0ab"
token_hash = hashlib.sha256(lease_token.encode()).hexdigest()
cpk = "cpk_" + "0f1e2d3c4b5a69788796a5b4c3d2e1f0" * 2
cpk_hash = hashlib.sha256(cpk.encode()).hexdigest()

sql = open("scripts/e2e/acp09_reset.sql", encoding="utf-8").read()
sql = sql.replace("@@token_hash", token_hash).replace("@@cpk_hash", cpk_hash)
# 占位符原文 ':token_hash' / ':cpk_hash' 自带 SQL 单引号，替换后即 'hash' 字面量。

r = subprocess.run(
    ["docker", "exec", "-i", "infrastructure-aipm-pg-1", "psql", "-U", "aipm",
     "-d", "e2e_acp09", "-v", "ON_ERROR_STOP=1", "-q"],
    input=sql, capture_output=True, text=True)
print("reset:", "OK" if r.returncode == 0 else r.stderr[:400])
print("cpk for E2E_CPK:", cpk)
sys.exit(r.returncode)
