# ai-pm 平台镜像构建（三服务共用一个构建阶段，按 --target 出三个镜像）：
#   docker build --target runtime-pm-api       -t aipm/pm-api:0.1.0 .
#   docker build --target runtime-pm-worker    -t aipm/pm-worker:0.1.0 .
#   docker build --target runtime-ai-pm-worker -t aipm/ai-pm-worker:0.1.0 .
# 构建上下文 = 仓库根（sqlx::migrate! 在编译期嵌入 migrations/，必须整体可见）。
# 外网代理（可选）：--build-arg CARGO_PROXY=http://host.docker.internal:10808

FROM rust:1-slim AS build
WORKDIR /build
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
# crates.io 代理（可选；空值不影响直连）
ARG CARGO_PROXY=""
ENV http_proxy=${CARGO_PROXY} \
    https_proxy=${CARGO_PROXY}
# 仓库 cargo 配置：私有 registry（kellnr：digua_common_jwt 等 lmms 基座 crate）
# 与 crates.io 镜像源（rsproxy）。不带进镜像则 cargo 报
# "registry index was not found in any configuration: `kellnr`"。
# ⚠️ 内网构建须能访问 192.168.2.12:16444（kellnr 匿名读可用）。
COPY .cargo .cargo
COPY Cargo.toml Cargo.lock ./
COPY crates crates
# vendor/digua-common-jwt：临时源码快照（Cargo.toml 的 digua_common_jwt path 依赖，
# 调试期替代 kellnr 1.6.0；回退时同步删除本行）
COPY vendor vendor
COPY apps apps
COPY migrations migrations
COPY policies policies
# skills/：apps/pm-api/build.rs 把仓库根 skills/ 白名单打包成 zip 内嵌进二进制
# （缺目录 → build.rs panic "skills package entry ... unreadable"）
COPY skills skills
# --config 'build.rustc-wrapper=""'：仓内 .cargo/config.toml 的 sccache wrapper
# 只对本机有效，容器内无 sccache → 必须覆盖掉，否则 cargo 报 "sccache not found"。
RUN cargo build --release --bins --config 'build.rustc-wrapper=""'

# ⚠️ 运行时必须与构建镜像同 Debian 大版本：rust:1-slim 现在是 Debian 13 trixie
#    （GLIBC 2.41），而 bookworm 只有 GLIBC 2.36 → 容器启动即报
#    "weak version `GLIBC_2.39' not found"。
FROM debian:trixie-slim AS runtime-base
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

FROM runtime-base AS runtime-pm-api
# 知识库是 Git-backed（Markdown 真源）：pm_submit_document / pm_discovery_submit_proposal
# 走知识写入管线时在容器内 shell 出 `git`。2026-11-03 两机实测发现运行时镜像缺
# git → 知识写入全部失败（本机 e2e 跑宿主二进制没暴露）。
# openssh-client：deploy key 生成（ssh-keygen）与 git-over-ssh（ssh -i）必需——
# `apt-get install --no-install-recommends git` 不会带 ssh-client 推荐（checkpoint-1 实测）。
RUN apt-get update \
    && apt-get install -y --no-install-recommends git openssh-client \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/pm-api /app/pm-api
ENV PM_API_PORT=8090
EXPOSE 8090
ENTRYPOINT ["/app/pm-api"]

FROM runtime-base AS runtime-pm-worker
# checkpoint-1：KB 仓库快照分析本体在 worker 消费端 shell 出 `git`
# （clone --bare / rev-parse / log / ls-tree / show）+ git-over-ssh 需 openssh-client。
# 镜像缺 git 时全部仓库输入的 intake 分析失败（processing.failed）。
RUN apt-get update \
    && apt-get install -y --no-install-recommends git openssh-client \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/pm-worker /app/pm-worker
ENTRYPOINT ["/app/pm-worker"]

FROM runtime-base AS runtime-ai-pm-worker
RUN apt-get update \
    && apt-get install -y --no-install-recommends git openssh-client \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/ai-pm-worker /app/ai-pm-worker
COPY --from=build /build/policies /app/policies
ENTRYPOINT ["/app/ai-pm-worker"]

# ── ai-frontend 生产镜像（PRISM 壳：含 /project/environments Runner UI 等）──
#   两段式：dist 由宿主构建（pnpm install / build:packages / vite build，AI-前端
#   构建链在容器内受网络与 pnpm 版本约束，见 e2e 报告 §5 注）→ 镜像只装 nginx。
#   docker build --target runtime-web -t aipm/ai-frontend-web:0.1.0 .
#   前置：ai-frontend/dist 已在宿主产出；vite 产物含 hash，重建即刷新。
# API 面由 nginx 代理：/api → pm-api:8090，/gw → api-gateway:3000（同 compose 网络）。
FROM alpine:3.21 AS runtime-web
RUN apk add --no-cache nginx
COPY ai-frontend/dist /usr/share/nginx/html
COPY ai-frontend/docker/nginx.conf /etc/nginx/http.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
