# Changelog

## [1.3.0](https://github.com/metaneutrons/ruscore/compare/ruscore-v1.2.1...ruscore-v1.3.0) (2026-04-07)


### Features

* detect partial capture (PRO+ paywall) with warnings ([b34d959](https://github.com/metaneutrons/ruscore/commit/b34d959856739061b24707049d3cd22b6f1312e2))
* per-row delete/retry buttons + retry API endpoint ([e261e8e](https://github.com/metaneutrons/ruscore/commit/e261e8e8950c2d8cbf3f6e268b8a3b56c2f8bb28))


### Bug Fixes

* **docker:** declare VOLUME for SQLite persistence ([d3f6bbd](https://github.com/metaneutrons/ruscore/commit/d3f6bbdfc312fb403e75915ff9a629b151f41a86))
* **docker:** robust Xvfb + Chrome container startup ([9b3105a](https://github.com/metaneutrons/ruscore/commit/9b3105ab2dc9632cdbbfba19ad86e2d3dd5f3c27))

## [1.2.1](https://github.com/metaneutrons/ruscore/compare/ruscore-v1.2.0...ruscore-v1.2.1) (2026-04-06)


### Bug Fixes

* **docker:** match glibc — use trixie for both builder and runtime ([e1f2673](https://github.com/metaneutrons/ruscore/commit/e1f267394e2bfc7a67a024655ee794ec9efe7ffd))

## [1.2.0](https://github.com/metaneutrons/ruscore/compare/ruscore-v1.1.2...ruscore-v1.2.0) (2026-04-06)


### Features

* all 15 SVGs captured and converted to PDF! ([de087bc](https://github.com/metaneutrons/ruscore/commit/de087bc3443dbbeac03f7452c86945190fdc2ad7))
* **ci:** push Docker images to ghcr.io instead of Docker Hub ([ba9a797](https://github.com/metaneutrons/ruscore/commit/ba9a7976d4631c2b62ea7652620b5d4520c025fd))
* delete jobs with bulk selection + confirmation guard ([fd5e9eb](https://github.com/metaneutrons/ruscore/commit/fd5e9ebc99049e76c489a3db7a407cecc3649ca6))
* Dockerfile + docker-compose.yaml ([407ec1e](https://github.com/metaneutrons/ruscore/commit/407ec1ef57edc7c2521cc8132ec18baa0d889836))
* full-text search across metadata ([35f11d3](https://github.com/metaneutrons/ruscore/commit/35f11d3b827a528119ffb704b7fe7724a06d04f4))
* initial ruscore implementation ([8e0c5c7](https://github.com/metaneutrons/ruscore/commit/8e0c5c732683af8a198a4da38b36b9b7cb73264a))
* job timeout, stale recovery, Chrome recycling, WAL mode ([3ed65e7](https://github.com/metaneutrons/ruscore/commit/3ed65e77a9a7fb7c7ab31523a94f1a82e17e1fa4))
* Next.js frontend + rust-embed static file serving ([7214685](https://github.com/metaneutrons/ruscore/commit/721468545f483d25db7a8bae1e4b38c3310ae874))
* PDF metadata embedding, detail view layout, screenshots ([c3c869f](https://github.com/metaneutrons/ruscore/commit/c3c869f70b0a7dd8d045dcc498a540e0113b0464))
* persistent Chrome session + retry with backoff ([613bf04](https://github.com/metaneutrons/ruscore/commit/613bf0475beab605c224e220211ed925e994899f))
* POST /api/v1/jobs/cleanup endpoint ([6f1e25d](https://github.com/metaneutrons/ruscore/commit/6f1e25d1e911d2e16c3d9e4f9aebbf47957bef91))
* replace chromiumoxide with raw CDP WebSocket client ([6edf5e7](https://github.com/metaneutrons/ruscore/commit/6edf5e7528805635ec555c3cd2a4fe7d43b602d6))
* resilient page-by-page scroll with per-page confirmation ([6c36312](https://github.com/metaneutrons/ruscore/commit/6c363120dbdc668acedaa0251ed5ee6c6e5fad1d))
* ruscore-server with axum API, SQLite, Redis, background worker ([c95d760](https://github.com/metaneutrons/ruscore/commit/c95d7602c58bb5616b497745f6c4c629a7c22809))
* sortable job list + full metadata in detail view ([6f58b04](https://github.com/metaneutrons/ruscore/commit/6f58b04906601c71333377bad42357716a88970c))
* typeahead search with FTS5 suggest endpoint ([11bb82d](https://github.com/metaneutrons/ruscore/commit/11bb82da0dcc83714e265945727aba94092ecd7f))
* upgrade search to SQLite FTS5 with BM25 ranking ([cdf9cde](https://github.com/metaneutrons/ruscore/commit/cdf9cdea21f5aa0d01f876e48db7f1b619812b1b))
* **web:** rewrite frontend as SPA with merged main view ([f722f94](https://github.com/metaneutrons/ruscore/commit/f722f94c25cc8b6864cfa1684fb3b7afee4b67dc))


### Bug Fixes

* 409 duplicate returns job data (not ProblemDetail) ([afa9269](https://github.com/metaneutrons/ruscore/commit/afa92692ab4b7af0e097688a2993464bafefefc7))
* case-insensitive composer prefix stripping, add 'Composed by' ([4354207](https://github.com/metaneutrons/ruscore/commit/4354207d0a51fa08aec34512b2ba557c2004e4f4))
* **ci:** build frontend before Rust (rust-embed needs web/out/) ([e98653d](https://github.com/metaneutrons/ruscore/commit/e98653d73bb00f8cb81ebd81a0d40633e9ed0cc1))
* **ci:** release-please reads version from workspace root Cargo.toml ([71aaed4](https://github.com/metaneutrons/ruscore/commit/71aaed4990d1db3f0fc1b9e42705b3d3cc2b6b1f))
* **ci:** simplify Dockerfile, amd64-only Docker (Chrome has no ARM Linux) ([59d523a](https://github.com/metaneutrons/ruscore/commit/59d523afb2b6aaa881c41710ead7532d178c1b10))
* **ci:** use config/manifest files for release-please (inline params ignored) ([96a3426](https://github.com/metaneutrons/ruscore/commit/96a3426f078ec3d3d04dbb7d7e9ad9b1b925d958))
* **ci:** use inline release-please config (simple type, no manifest) ([f3eb81f](https://github.com/metaneutrons/ruscore/commit/f3eb81f450825de18b9f42d5b5de217c09719436))
* **ci:** use simple release type with version.txt + toml extra-file ([7b52894](https://github.com/metaneutrons/ruscore/commit/7b528947d08585c646af24f663624a1a741531a6))
* descriptive error messages for all failure modes ([3a145cd](https://github.com/metaneutrons/ruscore/commit/3a145cde9d492e9726d6a8d7a5ff0e8540cf6bc1))
* **docker:** use rust:latest (deps need newer than 1.85) ([e91aa83](https://github.com/metaneutrons/ruscore/commit/e91aa83189c510169492baea2ee56832333973ac))
* extract metadata from all JSON-LD blocks + alt text fallback ([64aabec](https://github.com/metaneutrons/ruscore/commit/64aabec5a62cf59700e455ff2a180bdad693d480))
* remove unused as_str method on JobStatus ([1bb1c37](https://github.com/metaneutrons/ruscore/commit/1bb1c378c04f11da0cbe9affa53c13d2d5c6fdec))
* REST API audit — all issues resolved ([93ef272](https://github.com/metaneutrons/ruscore/commit/93ef272d187f09b50b51a7fcaf92312b297ea0c1))
* rewrite PDF generation using svg2pdf::to_pdf + lopdf merge ([9126b4f](https://github.com/metaneutrons/ruscore/commit/9126b4fa98891a92de699959378bdf897d64351c))
* RFC 7807 on all extractor rejections ([5cc4119](https://github.com/metaneutrons/ruscore/commit/5cc411964e720007bd807eff61f93af8a719abdd))
* scroll in short increments, suppress Chrome noise, add automation flag bypass ([9f96d34](https://github.com/metaneutrons/ruscore/commit/9f96d345c775e2b26c66cc65f6d75e9722f4142a))
* strip 'Words & Music by' prefix from composer ([ad96489](https://github.com/metaneutrons/ruscore/commit/ad96489c1001673ff65eec3acd25c475885714ef))
* strip 'Written by' prefix from composer field ([10ef45a](https://github.com/metaneutrons/ruscore/commit/10ef45aae1a60a5e18b27240551981da880d1996))
* wait for score viewer children, drain events during scroll ([98fcd16](https://github.com/metaneutrons/ruscore/commit/98fcd166f33974eb96295ce8e70159b47dcf300b))

## [1.1.2](https://github.com/metaneutrons/ruscore/compare/v1.1.1...v1.1.2) (2026-04-06)


### Bug Fixes

* **docker:** use rust:latest (deps need newer than 1.85) ([e91aa83](https://github.com/metaneutrons/ruscore/commit/e91aa83189c510169492baea2ee56832333973ac))

## [1.1.1](https://github.com/metaneutrons/ruscore/compare/v1.1.0...v1.1.1) (2026-04-06)


### Bug Fixes

* **ci:** simplify Dockerfile, amd64-only Docker (Chrome has no ARM Linux) ([59d523a](https://github.com/metaneutrons/ruscore/commit/59d523afb2b6aaa881c41710ead7532d178c1b10))

## [1.1.0](https://github.com/metaneutrons/ruscore/compare/v1.0.0...v1.1.0) (2026-04-06)


### Features

* **ci:** push Docker images to ghcr.io instead of Docker Hub ([ba9a797](https://github.com/metaneutrons/ruscore/commit/ba9a7976d4631c2b62ea7652620b5d4520c025fd))
* PDF metadata embedding, detail view layout, screenshots ([c3c869f](https://github.com/metaneutrons/ruscore/commit/c3c869f70b0a7dd8d045dcc498a540e0113b0464))

## 1.0.0 (2026-04-06)


### Features

* all 15 SVGs captured and converted to PDF! ([de087bc](https://github.com/metaneutrons/ruscore/commit/de087bc3443dbbeac03f7452c86945190fdc2ad7))
* delete jobs with bulk selection + confirmation guard ([fd5e9eb](https://github.com/metaneutrons/ruscore/commit/fd5e9ebc99049e76c489a3db7a407cecc3649ca6))
* Dockerfile + docker-compose.yaml ([407ec1e](https://github.com/metaneutrons/ruscore/commit/407ec1ef57edc7c2521cc8132ec18baa0d889836))
* full-text search across metadata ([35f11d3](https://github.com/metaneutrons/ruscore/commit/35f11d3b827a528119ffb704b7fe7724a06d04f4))
* initial ruscore implementation ([8e0c5c7](https://github.com/metaneutrons/ruscore/commit/8e0c5c732683af8a198a4da38b36b9b7cb73264a))
* job timeout, stale recovery, Chrome recycling, WAL mode ([3ed65e7](https://github.com/metaneutrons/ruscore/commit/3ed65e77a9a7fb7c7ab31523a94f1a82e17e1fa4))
* Next.js frontend + rust-embed static file serving ([7214685](https://github.com/metaneutrons/ruscore/commit/721468545f483d25db7a8bae1e4b38c3310ae874))
* persistent Chrome session + retry with backoff ([613bf04](https://github.com/metaneutrons/ruscore/commit/613bf0475beab605c224e220211ed925e994899f))
* POST /api/v1/jobs/cleanup endpoint ([6f1e25d](https://github.com/metaneutrons/ruscore/commit/6f1e25d1e911d2e16c3d9e4f9aebbf47957bef91))
* replace chromiumoxide with raw CDP WebSocket client ([6edf5e7](https://github.com/metaneutrons/ruscore/commit/6edf5e7528805635ec555c3cd2a4fe7d43b602d6))
* resilient page-by-page scroll with per-page confirmation ([6c36312](https://github.com/metaneutrons/ruscore/commit/6c363120dbdc668acedaa0251ed5ee6c6e5fad1d))
* ruscore-server with axum API, SQLite, Redis, background worker ([c95d760](https://github.com/metaneutrons/ruscore/commit/c95d7602c58bb5616b497745f6c4c629a7c22809))
* sortable job list + full metadata in detail view ([6f58b04](https://github.com/metaneutrons/ruscore/commit/6f58b04906601c71333377bad42357716a88970c))
* typeahead search with FTS5 suggest endpoint ([11bb82d](https://github.com/metaneutrons/ruscore/commit/11bb82da0dcc83714e265945727aba94092ecd7f))
* upgrade search to SQLite FTS5 with BM25 ranking ([cdf9cde](https://github.com/metaneutrons/ruscore/commit/cdf9cdea21f5aa0d01f876e48db7f1b619812b1b))
* **web:** rewrite frontend as SPA with merged main view ([f722f94](https://github.com/metaneutrons/ruscore/commit/f722f94c25cc8b6864cfa1684fb3b7afee4b67dc))


### Bug Fixes

* 409 duplicate returns job data (not ProblemDetail) ([afa9269](https://github.com/metaneutrons/ruscore/commit/afa92692ab4b7af0e097688a2993464bafefefc7))
* case-insensitive composer prefix stripping, add 'Composed by' ([4354207](https://github.com/metaneutrons/ruscore/commit/4354207d0a51fa08aec34512b2ba557c2004e4f4))
* **ci:** build frontend before Rust (rust-embed needs web/out/) ([e98653d](https://github.com/metaneutrons/ruscore/commit/e98653d73bb00f8cb81ebd81a0d40633e9ed0cc1))
* **ci:** release-please reads version from workspace root Cargo.toml ([71aaed4](https://github.com/metaneutrons/ruscore/commit/71aaed4990d1db3f0fc1b9e42705b3d3cc2b6b1f))
* **ci:** use inline release-please config (simple type, no manifest) ([f3eb81f](https://github.com/metaneutrons/ruscore/commit/f3eb81f450825de18b9f42d5b5de217c09719436))
* **ci:** use simple release type with version.txt + toml extra-file ([7b52894](https://github.com/metaneutrons/ruscore/commit/7b528947d08585c646af24f663624a1a741531a6))
* descriptive error messages for all failure modes ([3a145cd](https://github.com/metaneutrons/ruscore/commit/3a145cde9d492e9726d6a8d7a5ff0e8540cf6bc1))
* extract metadata from all JSON-LD blocks + alt text fallback ([64aabec](https://github.com/metaneutrons/ruscore/commit/64aabec5a62cf59700e455ff2a180bdad693d480))
* remove unused as_str method on JobStatus ([1bb1c37](https://github.com/metaneutrons/ruscore/commit/1bb1c378c04f11da0cbe9affa53c13d2d5c6fdec))
* REST API audit — all issues resolved ([93ef272](https://github.com/metaneutrons/ruscore/commit/93ef272d187f09b50b51a7fcaf92312b297ea0c1))
* rewrite PDF generation using svg2pdf::to_pdf + lopdf merge ([9126b4f](https://github.com/metaneutrons/ruscore/commit/9126b4fa98891a92de699959378bdf897d64351c))
* RFC 7807 on all extractor rejections ([5cc4119](https://github.com/metaneutrons/ruscore/commit/5cc411964e720007bd807eff61f93af8a719abdd))
* scroll in short increments, suppress Chrome noise, add automation flag bypass ([9f96d34](https://github.com/metaneutrons/ruscore/commit/9f96d345c775e2b26c66cc65f6d75e9722f4142a))
* strip 'Words & Music by' prefix from composer ([ad96489](https://github.com/metaneutrons/ruscore/commit/ad96489c1001673ff65eec3acd25c475885714ef))
* strip 'Written by' prefix from composer field ([10ef45a](https://github.com/metaneutrons/ruscore/commit/10ef45aae1a60a5e18b27240551981da880d1996))
* wait for score viewer children, drain events during scroll ([98fcd16](https://github.com/metaneutrons/ruscore/commit/98fcd166f33974eb96295ce8e70159b47dcf300b))
