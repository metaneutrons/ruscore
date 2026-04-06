# 🎵 ruscore

[![CI](https://github.com/metaneutrons/ruscore/actions/workflows/ci.yaml/badge.svg)](https://github.com/metaneutrons/ruscore/actions/workflows/ci.yaml)
[![Release](https://github.com/metaneutrons/ruscore/actions/workflows/release.yaml/badge.svg)](https://github.com/metaneutrons/ruscore/actions/workflows/release.yaml)
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](https://github.com/metaneutrons/ruscore/pkgs/container/ruscore)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Rust: 1.85+](https://img.shields.io/badge/rust-1.85%2B%20(edition%202024)-orange.svg)](https://www.rust-lang.org)

**Scrape MuseScore sheet music and convert to high-quality vector PDF.**

<p align="center">
  <img src="assets/ruscore-web-ui.png" alt="ruscore web interface" width="700" />
</p>

<p align="center">
  <img src="assets/ruscore-job-detail.png" alt="ruscore job detail view" width="700" />
</p>

ruscore navigates MuseScore score pages using a real Chrome instance via CDP (Chrome DevTools Protocol), captures all SVG pages through network interception, extracts rich metadata, and generates multi-page vector PDFs — all without triggering Cloudflare bot detection.

<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Axum-000000?style=for-the-badge" alt="Axum" />
  <img src="https://img.shields.io/badge/Next.js-000000?style=for-the-badge&logo=next.js&logoColor=white" alt="Next.js" />
  <img src="https://img.shields.io/badge/Tailwind_CSS-06B6D4?style=for-the-badge&logo=tailwindcss&logoColor=white" alt="Tailwind" />
  <img src="https://img.shields.io/badge/SQLite-003B57?style=for-the-badge&logo=sqlite&logoColor=white" alt="SQLite" />
  <img src="https://img.shields.io/badge/Docker-2496ED?style=for-the-badge&logo=docker&logoColor=white" alt="Docker" />
</p>

---

## ✨ Features

- **Vector PDF output** — SVGs converted via `usvg` + `svg2pdf` + `lopdf`, not rasterized screenshots
- **PDF metadata** — title, composer, arranger, instruments embedded in PDF Info dictionary
- **Cloudflare bypass** — raw CDP WebSocket, no `Page.enable`, no `navigator.webdriver` flag
- **Rich metadata extraction** — title, composer, arranger, instruments, description from JSON-LD
- **Full-text search** — SQLite FTS5 with BM25 ranking, prefix matching, phrase search, typeahead
- **Web service** — REST API (RFC 7807 errors, pagination Link headers, URL validation)
- **Job queue** — persistent Chrome session, retry with backoff, per-job timeout, stale recovery
- **Embedded frontend** — Next.js 15 + Tailwind v4 SPA, baked into a single binary via `rust-embed`
- **Cross-platform** — macOS, Windows, Linux (x86-64 & ARM64)
- **Docker ready** — Chrome + Xvfb in a single container, multi-arch images

## 📦 Architecture

```
ruscore/
├── ruscore-core/     # Library: Chrome CDP, scraping, PDF generation
├── ruscore-cli/      # CLI: ruscore <url> [output.pdf]
└── ruscore-server/   # Web service: axum API + SQLite + embedded frontend
    └── web/          # Next.js 15 + Tailwind v4 + TypeScript
```

| Component | Technology |
|-----------|-----------|
| CDP client | Raw WebSocket (`tokio-tungstenite`) — no automation detection |
| PDF engine | `usvg` + `svg2pdf` + `lopdf` — pure Rust, vector output |
| Web framework | `axum` + `tower-http` |
| Database | SQLite (`rusqlite`) — jobs, metadata, PDF blobs, FTS5 search |
| Frontend | Next.js 15, Tailwind CSS v4, TypeScript — static export |
| Embedding | `rust-embed` — single binary deployment |

## 🚀 Quick Start

### CLI

```bash
# Requires Chrome/Chromium installed
cargo install --path ruscore-cli

ruscore "https://musescore.com/user/2017661/scores/5507029" score.pdf
```

### Web Service (local)

```bash
cargo run --release --bin ruscore-server
# → http://localhost:3000
```

### Docker

```bash
docker compose up --build
# → http://localhost:3000
```

## 🌐 API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/v1/jobs` | Submit URL for conversion |
| `GET` | `/api/v1/jobs` | List jobs (paginated, filterable, sortable, searchable) |
| `GET` | `/api/v1/jobs/suggest?q=` | Typeahead search suggestions |
| `GET` | `/api/v1/jobs/:id` | Job status + metadata |
| `GET` | `/api/v1/jobs/:id/pdf` | Download generated PDF |
| `DELETE` | `/api/v1/jobs/:id` | Delete a job (requires `X-Confirm: yes` header) |
| `POST` | `/api/v1/jobs/batch/delete` | Bulk delete jobs (requires `X-Confirm: yes` header) |
| `POST` | `/api/v1/jobs/cleanup?max_age_hours=24` | Delete old jobs (requires `X-Confirm: yes` header) |
| `GET` | `/health` | Health check |

### Query Parameters for `GET /api/v1/jobs`

| Param | Example | Description |
|-------|---------|-------------|
| `q` | `beethoven` | FTS5 search (supports `prefix*`, `"exact phrase"`, `OR`) |
| `status` | `completed` | Filter by status |
| `sort` | `title` | Sort by: `title`, `composer`, `pages`, `status`, `created_at` |
| `order` | `asc` | Sort direction: `asc`, `desc` |
| `page` | `1` | Page number |
| `per_page` | `20` | Items per page (max 100) |

### API Design

- **RFC 7807** Problem Details on all errors (`application/problem+json`)
- **Location** header on `202 Accepted` and `409 Conflict`
- **Link** headers for pagination (`rel=next/prev/first/last`)
- **URL validation** — rejects non-musescore.com URLs with `422`
- **Confirmation guard** — destructive operations require `X-Confirm: yes` header
- **Content-Length** + title-based filename on PDF downloads

## ⚙️ Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `RUSCORE_PORT` | `3000` | HTTP server port |
| `RUSCORE_DATA_DIR` | `./data` | SQLite database directory |
| `RUST_LOG` | `info` | Log level (`debug`, `info`, `warn`, `error`) |

## 🔧 How It Works

1. **Launch Chrome** with `--remote-debugging-port` and `--disable-blink-features=AutomationControlled`
2. **Connect via raw WebSocket** — only enable `Runtime` + `Network` CDP domains (deliberately skip `Page.enable` to avoid `navigator.webdriver = true`)
3. **Navigate** via `Runtime.evaluate("location.href = ...")` — no `Page.navigate`
4. **Wait** for the score viewer to render (React hydration)
5. **Fire-and-forget scroll** inside the browser to trigger lazy loading
6. **Intercept SVGs** via `Network.responseReceived` + `Network.getResponseBody` (S3 presigned URLs)
7. **Convert to PDF** using `usvg` → `svg2pdf::to_pdf()` → `lopdf` merge (vector, not rasterized)
8. **Embed metadata** — title, composer, arranger, instruments in PDF Info dictionary
9. **Extract metadata** from `<script type="application/ld+json">` (MusicComposition schema.org)

### Resilience

- **Persistent Chrome** — session reused across jobs, Cloudflare `cf_clearance` cookie persists
- **Retry with backoff** — Cloudflare blocks and Chrome crashes trigger restart + retry (up to 3×)
- **Per-job timeout** — 5 minutes, kills Chrome if stuck
- **Stale recovery** — on startup, resets orphaned "processing" jobs back to "queued"
- **Chrome recycling** — proactive restart every 50 jobs to prevent memory leaks
- **SQLite WAL mode** — concurrent reads during worker writes

## 🐳 Docker Details

The Docker image runs Chrome **headed** inside an Xvfb virtual framebuffer — this is required because Cloudflare detects `--headless` mode. Multi-arch images are built natively (no QEMU emulation).

```yaml
# docker-compose.yaml
services:
  ruscore:
    image: ghcr.io/metaneutrons/ruscore:latest
    ports: ["3000:3000"]
    volumes: [ruscore-data:/home/ruscore/data]
```

## 📄 License

[AGPL-3.0](LICENSE)
