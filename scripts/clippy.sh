#!/usr/bin/env bash
# scripts/clippy.sh —— 本地 clippy 内环：只验改动面 + 专用 O0 profile。
#
# 为什么存在：
#   `cargo clippy --workspace --all-targets` 会编译整个工作区的所有 target，
#   而一次改动通常只碰一两个 crate。脚本做两件事：
#   - profile：固定 `--profile clippy`（根 Cargo.toml 定义，debug = 0）。
#     dev 默认带完整 debuginfo，clippy 产物却直接丢弃，那笔编译/落盘账
#     不该由静态分析付。lint 在 HIR/MIR 层，opt/debug 不影响结论。
#   - 改动面：无参数时从 git 工作区自动提取改动的 crate，只 lint 它们。
#     全量 lint 交 CI / `--all`。
#
# 用法：
#   bash scripts/clippy.sh                  # 自动检测改动 crate（日常）
#   bash scripts/clippy.sh pm-ai pm-api     # 显式指定
#   bash scripts/clippy.sh --all            # 全量
#
# 首次使用：`.target-clippy/` 是独立产物目录，首次会全量重建一次（O0 依赖
# 树），之后走标准增量。
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

# clippy 用独立 target 目录：dev 产物与 lint 产物混在一起会让两边都变慢。
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/.target-clippy}"

ALL_CRATES=()
for d in crates/*/ apps/*/; do
    [ -f "$d/Cargo.toml" ] || continue
    ALL_CRATES+=("$(basename "$d")")
done

usage() {
    echo "用法: bash scripts/clippy.sh [--all | <crate>...]" >&2
    echo "  无参数 = 自动检测改动 crate；--all = 全量（${ALL_CRATES[*]}）" >&2
    exit 2
}

run_crates() {
    local args=()
    for c in "$@"; do
        [ -d "crates/$c" ] || [ -d "apps/$c" ] || {
            echo "[clippy] 未知 crate：$c（crates/$c 与 apps/$c 都不存在）" >&2
            exit 2
        }
        args+=(-p "$c")
    done
    local cmd=(cargo clippy --profile clippy "${args[@]}" --all-targets -- -D warnings)
    echo "[clippy] ${cmd[*]}"
    "${cmd[@]}"
}

all=false
crates=()
for arg in "$@"; do
    case "$arg" in
        --all) all=true ;;
        -h | --help) usage ;;
        -*) echo "[clippy] 未知选项：$arg" >&2; usage ;;
        *) crates+=("$arg") ;;
    esac
done

if $all; then
    run_crates "${ALL_CRATES[@]}"
    echo "[clippy] PASS"
    exit 0
fi

if [ ${#crates[@]} -eq 0 ]; then
    changed_files="$(
        {
            git diff --name-only HEAD
            git diff --cached --name-only
            git ls-files --others --exclude-standard
        } | sort -u
    )"
    # 只认 `crates/<name>/...` 与 `apps/<name>/...`；docs/scripts/frontend
    # 改动不产生 clippy 目标。
    while IFS= read -r c; do
        [ -n "$c" ] && crates+=("$c")
    done < <(
        printf '%s\n' "$changed_files" \
            | sed -n -e 's#^crates/\([^/]*\)/.*#\1#p' -e 's#^apps/\([^/]*\)/.*#\1#p' \
            | sort -u
    )
    if [ ${#crates[@]} -eq 0 ]; then
        echo "[clippy] 无改动 crate（docs/scripts/frontend 改动不产生 clippy 目标）——跳过。"
        echo "[clippy] 要全量请跑：bash scripts/clippy.sh --all"
        exit 0
    fi
    echo "[clippy] 检测到改动 crate：${crates[*]}"
fi

run_crates "${crates[@]}"
echo "[clippy] PASS"