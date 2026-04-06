# Multi-stage build: Node (frontend) → Rust (server) → Runtime (Chrome + Xvfb)
# All Debian stages use trixie to avoid glibc mismatch.

# --- Stage 1: Build frontend ---
FROM node:22-slim AS frontend
WORKDIR /app/ruscore-server/web
COPY ruscore-server/web/package.json ruscore-server/web/package-lock.json* ./
RUN npm ci
COPY ruscore-server/web/ .
RUN npm run build

# --- Stage 2: Build Rust binary ---
FROM rust:slim-trixie AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
COPY --from=frontend /app/ruscore-server/web/out/ ruscore-server/web/out/
RUN cargo build --release --bin ruscore-server

# --- Stage 3: Runtime (same Debian as builder) ---
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    wget \
    gnupg \
    xvfb \
    && wget -q -O - https://dl.google.com/linux/linux_signing_key.pub | gpg --dearmor -o /usr/share/keyrings/google-chrome.gpg \
    && echo "deb [arch=amd64 signed-by=/usr/share/keyrings/google-chrome.gpg] http://dl.google.com/linux/chrome/deb/ stable main" > /etc/apt/sources.list.d/google-chrome.list \
    && apt-get update && apt-get install -y --no-install-recommends \
    google-chrome-stable \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ruscore-server /usr/local/bin/ruscore-server

# Create non-root user
RUN useradd -m -s /bin/bash ruscore
USER ruscore
WORKDIR /home/ruscore

# Xvfb wrapper script — Chrome runs "headed" inside a virtual framebuffer
COPY --chmod=755 <<'EOF' /usr/local/bin/entrypoint.sh
#!/bin/bash
set -e
Xvfb :99 -screen 0 1920x1080x24 -nolisten tcp &
export DISPLAY=:99
sleep 1
exec ruscore-server
EOF

ENV RUSCORE_PORT=3000
ENV RUSCORE_DATA_DIR=/home/ruscore/data

VOLUME /home/ruscore/data

EXPOSE 3000

HEALTHCHECK --interval=10s --timeout=3s --start-period=15s \
    CMD wget -q --spider http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
