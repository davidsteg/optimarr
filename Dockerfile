# syntax=docker/dockerfile:1

# ---- web UI ----
FROM node:22-bookworm-slim AS ui
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ---- server ----
FROM rust:1-bookworm AS server
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY backend ./backend
COPY --from=ui /src/frontend/build ./frontend/build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked && cp target/release/optimarr /usr/local/bin/optimarr

# ---- runtime ----
FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="Optimarr" \
      org.opencontainers.image.description="Media library compatibility and quality analyzer for Sonarr, Radarr and Jellyfin" \
      org.opencontainers.image.source="https://github.com/davidsteg/optimarr" \
      org.opencontainers.image.licenses="MIT"

RUN apt-get update \
    && apt-get install -y --no-install-recommends mediainfo ca-certificates curl gosu tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 1000 optimarr && useradd -u 1000 -g optimarr -M -s /usr/sbin/nologin optimarr \
    && mkdir -p /app/config /app/data /app/logs

COPY --from=server /usr/local/bin/optimarr /usr/local/bin/optimarr
COPY --chmod=755 docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

WORKDIR /app
ENV OPTIMARR_BIND=0.0.0.0:8080 \
    OPTIMARR_DATA_DIR=/app/data \
    OPTIMARR_CONFIG_DIR=/app/config \
    OPTIMARR_LOG_DIR=/app/logs \
    PUID=1000 \
    PGID=1000
EXPOSE 8080
VOLUME ["/app/data", "/app/logs"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://localhost:8080/api/system/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/docker-entrypoint.sh"]
CMD ["optimarr"]
