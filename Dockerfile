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
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY apps apps
COPY migrations migrations
COPY policies policies
RUN cargo build --release --bins

FROM debian:bookworm-slim AS runtime-base
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

FROM runtime-base AS runtime-pm-api
COPY --from=build /build/target/release/pm-api /app/pm-api
ENV PM_API_PORT=8090
EXPOSE 8090
ENTRYPOINT ["/app/pm-api"]

FROM runtime-base AS runtime-pm-worker
COPY --from=build /build/target/release/pm-worker /app/pm-worker
ENTRYPOINT ["/app/pm-worker"]

FROM runtime-base AS runtime-ai-pm-worker
COPY --from=build /build/target/release/ai-pm-worker /app/ai-pm-worker
COPY --from=build /build/policies /app/policies
ENTRYPOINT ["/app/ai-pm-worker"]
