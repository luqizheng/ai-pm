#!/usr/bin/env bash
# scripts/cargo-test.sh —— 测试运行器：nextest 并行调度 + 独立 TEMP 目录。
#
# 为什么存在：
#   1. 执行器：`cargo test` 逐个测试二进制串行执行；nextest 用全局并行池
#      跨测试二进制调度，本仓有 pm-application / pm-infrastructure / pm-mcp
#      等多个集成测试目标，运行期提速明显。nextest 0.9.x 默认遇失败提前
#      收工，门禁要完整失败清单，故全量模式显式 --no-fail-fast。
#      nextest 不跑 doctest，全量模式在 nextest 之后补 `cargo test --doc`。
#   2. 临时目录：每次运行用**系统临时目录下的随机子目录**，跑完即清——
#      多会话并行跑测试不会互踩，且不会把临时产物落进工作区。
#      注意**不能**把 TEMP 指到仓库内：pm-application 的 knowledge 测试
#      会拒绝落在 git 仓库内的知识根（"point AI_PM_KNOWLEDGE_ROOT at a
#      standalone repository"）。
#
# 用法：
#   bash scripts/cargo-test.sh                    # 全量（nextest + doctest）
#   bash scripts/cargo-test.sh -p pm-ai           # 透传 nextest 参数
#   bash scripts/cargo-test.sh --no-capture       # 透传任意 nextest 选项
#
# 数据库：集成测试默认连本机开发库，可用 DATABASE_URL 覆盖。
#
# 模板库：`#[sqlx::test]` 每个用例都会从头跑 20 个迁移（约 1.9s/用例，全仓
#   298 个 DB 用例）。因此跑测试前先把迁移预跑进 template1，每个用例的建库
#   就退化成文件拷贝（sqlx 建库是裸 `create database "<name>"`，默认模板即
#   template1），迁移段被 sqlx 的 `_sqlx_migrations` 记账直接跳过。
#   见 crates/pm-infrastructure/examples/seed_test_template.rs。
#   预建失败（库没起等）只告警不阻断：慢路径仍然正确。AIPM_TEST_SKIP_TEMPLATE=1 可跳过。
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root" || exit 1

export DATABASE_URL="${DATABASE_URL:-postgres://aipm:aipm@127.0.0.1:5433/aipm}"

if ! command -v cargo-nextest >/dev/null 2>&1; then
    echo "[test] 未找到 cargo-nextest，先跑：cargo install cargo-nextest --locked" >&2
    exit 1
fi

if [ "${AIPM_TEST_SKIP_TEMPLATE:-0}" != "1" ]; then
    if ! cargo run -q -p pm-infrastructure --example seed_test_template; then
        echo "[test] 模板库预建失败，回退为逐用例重跑迁移（测试仍正确，只是更慢）" >&2
    fi
fi

tmp_root="${TMPDIR:-/tmp}"
mkdir -p "$tmp_root"
tmp_dir="$(mktemp -d "$tmp_root/aipm-test-XXXXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

# Windows API（GetTempPathW）只认 Win32 路径，MSYS 路径需转换。
if command -v cygpath >/dev/null 2>&1; then
    tmp_win="$(cygpath -w "$tmp_dir")"
else
    tmp_win="$tmp_dir"
fi
export TEMP="$tmp_win"
export TMP="$tmp_win"

if [ "$#" -eq 0 ]; then
    cargo nextest run --workspace --no-fail-fast && cargo test --workspace --doc
    exit $?
fi
exec cargo nextest run "$@"