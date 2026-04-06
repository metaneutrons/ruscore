# Multi-stage build: Node (frontend) → Rust (server) → Runtime (Chrome + Xvfb)

# --- Stage 1: Build frontend ---
FROM node:22-slim AS frontend
WORKDIR /app/ruscore-server/web
COPY ruscore-server/web/package.json ruscore-server/web/package-lock.json* ./
RUN npm ci
COPY ruscore-server/web/ .
RUN npm run build

# --- Stage 2: Build Rust binary ---
FROM rust:1.85-bookworm AS builder
WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY ruscore-core/Cargo.toml ruscore-core/Cargo.toml
COPY ruscore-cli/Cargo.toml ruscore-cli/Cargo.toml
COPY ruscore-server/Cargo.toml ruscore-server/Cargo.toml
RUN mkdir -p ruscore-core/src ruscore-cli/src ruscore-server/src \
    && echo "pub fn dummy() {}" > ruscore-core/src/lib.rs \
    && echo "fn main() {}" > ruscore-cli/src/main.rs \
    && echo "fn main() {}" > ruscore-server/src/main.rs \
    && cargo build --release --bin ruscore-server 2>/dev/null || true

# Copy real source + built frontend
COPY ruscore-core/ ruscore-core/
COPY ruscore-cli/ ruscore-cli/
COPY ruscore-server/src/ ruscore-server/src/
COPY --from=frontend /app/ruscore-server/web/out/ ruscore-server/web/out/
RUN cargo build --release --bin ruscore-server

# --- Stage 3: Runtime with Chrome + Xvfb ---
FROM debian:bookworm-slim AS runtime

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
# Start Xvfb on display :99
Xvfb :99 -screen 0 1920x1080x24 -nolisten tcp &
export DISPLAY=:99
# Wait for Xvfb
sleep 1
exec ruscore-server
EOF

ENV RUSCORE_PORT=3000
ENV RUSCORE_REDIS_URL=redis://redis:6379
ENV RUSCORE_DATA_DIR=/home/ruscore/data

EXPOSE 3000

HEALTHCHECK --interval=10s --timeout=3s --start-period=15s \
    CMD wget -q --spider http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
