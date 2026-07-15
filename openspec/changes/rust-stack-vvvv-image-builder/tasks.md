## 1. Project bootstrap

- [x] 1.1 Create `Cargo.toml` at repo root with crate name `autoimage` and binary `autoimage`
- [x] 1.2 Add dependencies: `axum`, `tokio` (full), `tower`, `tower-http` (ServeDir / cors / trace), `serde`, `serde_json`, `toml`, `reqwest` (json, rustls-tls), `chrono`, `anyhow`, `thiserror`, `tracing`, `tracing-subscriber`, `regex`
- [x] 1.3 Create `rust-toolchain.toml` pinning `stable`
- [x] 1.4 Create `src/main.rs` skeleton that boots `tokio` runtime and starts the Axum server on the configured bind
- [x] 1.5 Add `static/` directory with `index.html`, `app.js`, `app.css`
- [x] 1.6 Verify `cargo check` succeeds and `cargo run` serves the editor page at `127.0.0.1:8080/`

## 2. Configuration module

- [x] 2.1 Add `src/config.rs` with a `Config` struct (bind, proxy_base_url, github.owner/repo/workflow_file/default_branch, target.repo/namespace) and a `load()` function that reads `config.toml` then applies env overrides
- [x] 2.2 Implement env precedence for: `APP_BIND`, `PROXY_BASE_URL`, `GH_TOKEN`, `GH_OWNER`, `GH_REPO`, `GH_WORKFLOW`, `GH_DEFAULT_BRANCH`, `TARGET_REPO`, `TARGET_NAMESPACE`; reject `config.toml` containing `GH_TOKEN` field
- [x] 2.3 Wire `Config` into `main.rs` and emit a `tracing::info!` summary of the loaded config (without secrets) on startup
- [x] 2.4 Add a default `config.toml` to the repo (no `GH_TOKEN`) so the binary boots in a sane state

## 3. Static UI shell

- [x] 3.1 Implement `GET /` and `GET /static/*` via `tower_http::services::ServeDir` serving from the `static/` directory
- [x] 3.2 Write `index.html` with three sections: image info, image download, Dockerfile editor (textarea + Save / Refresh / Trigger Build buttons)
- [x] 3.3 Write `app.js` with `fetch` wrappers for `/api/image/info`, `/api/image/download`, `/api/dockerfile`, `/api/dockerfile/backups`, `/api/build`
- [x] 3.4 Write `app.css` with minimal layout (header, panels, monospace textarea, button row, status line)
- [x] 3.5 Page assets load and respond 200 (verified via `curl`; visual browser test is a manual follow-up)

## 4. Dockerfile editor API

- [x] 4.1 Add `src/handlers/dockerfile.rs` with `GET /api/dockerfile` returning `{content, size, updated_at}` and 404 if the file is missing
- [x] 4.2 Implement `PUT /api/dockerfile` accepting `{content}` (validated as UTF-8) and returning 400 on invalid bytes
- [x] 4.3 Implement backup creation in `src/backup.rs`: write `Dockerfile.bak.YYYYMMDD-HHMMSS` using `chrono::Local::now()`, with collision suffix `.2`, `.3`, ... when the target exists
- [x] 4.4 Wire `PUT` to call the backup helper atomically (write backup first, then overwrite `Dockerfile`; on any error leave the original untouched)
- [x] 4.5 Wire both routes into the Axum router in `main.rs`
- [x] 4.6 Verified with `curl`: two saves in the same second produced `Dockerfile.bak.20260714-182731` and `Dockerfile.bak.20260714-182731.2`

## 5. Backups API

- [x] 5.1 Implement `GET /api/dockerfile/backups` returning a JSON list sorted by filename desc with `{name, size, created_at}` (mtime → RFC3339)
- [x] 5.2 Implement `GET /api/dockerfile/backups/:name` that matches `^Dockerfile\.bak\.\d{8}-\d{6}(\.\d+)?$` and returns the content
- [x] 5.3 Reject any name containing `..` or `/` with HTTP 400 (verified)
- [x] 5.4 Add UI section that lists backups and lets the user open a backup content in a read-only modal (panel + click handler in `app.js`)

## 6. proxy.vvvv.ee proxy endpoints

- [x] 6.1 Add `src/handlers/proxy.rs` with a shared `reqwest::Client` (15s timeout, rustls)
- [x] 6.2 Implement image-name validation regex `[A-Za-z0-9._\-/:@]+` and reject others with 400
- [x] 6.3 Implement `POST /api/image/info` accepting `{image}` and forwarding to `${PROXY_BASE_URL}/api/image/info?image=...`; pass through upstream status, body, and content-type
- [x] 6.4 Implement `POST /api/image/download` accepting `{image, mode?, compressed?, platform?}` with defaults `prepare/true/linux/amd64` and forwarding query string
- [x] 6.5 Map upstream timeouts to HTTP 504 with `{"error": "upstream timeout"}` and non-2xx upstream statuses to the same code returned to the client
- [x] 6.6 Wire both routes into the router
- [x] 6.7 Verified: `image=alpine` returned real upstream JSON (`digest`, `mediaType`, `platforms`); `image=ali pne` rejected with 400

## 7. GitHub Actions trigger

- [x] 7.1 Add `src/handlers/build.rs` with a `GithubClient` that owns the `reqwest` client and the loaded `Config`
- [x] 7.2 Implement `POST /api/build` accepting optional `{image, version}` overrides; build the `inputs` object from config + overrides
- [x] 7.3 Send `POST https://api.github.com/repos/{owner}/{repo}/actions/workflows/{workflow_file}/dispatches` with `Authorization: Bearer ${GH_TOKEN}`, `Accept: application/vnd.github+json`, body `{"ref": "<default_branch>", "inputs": {...}}`
- [x] 7.4 Return HTTP 202 `{accepted: true, workflow: "<file>"}` on success, propagate non-2xx from GitHub, return 500 when `GH_TOKEN` is unset
- [x] 7.5 Wire the route into the router
- [x] 7.6 Smoke test: with a real-looking `GH_TOKEN` the dispatch request was constructed and sent to `api.github.com`; GitHub returned an error (expected for a fake token). End-to-end with a real PAT requires the user to run it on a machine that has the right token and network access.

## 8. GitHub workflow update

- [x] 8.1 Update `.github/workflows/build.yml` to declare `workflow_dispatch` with `inputs`: `repo` (default `ghcr.io`), `namespace` (default `${{ github.repository_owner }}`), `image` (required), `version`
- [x] 8.2 Replace the Dockerfile-first-line parsing with `inputs.*` reads in the build job (now a single `docker/build-push-action@v5` step)
- [x] 8.3 Add top-level `permissions: { contents: read, packages: write }` to the workflow
- [x] 8.4 Replace the aliyun `docker login` and `docker tag/push` steps: log in to `${repo}` (default `ghcr.io`) with username `${{ github.actor }}` and password `${{ secrets.GITHUB_TOKEN }}`; push to `${repo}/${namespace}/${image}:${version}`
- [x] 8.5 Remove the `DOCKER_USERNAME` / `DOCKER_PASSWORD` secret references from the workflow
- [x] 8.6 Manual `gh workflow run` against the live repo is a user action; the workflow file is ready.

## 9. Cleanup of old Python code

- [x] 9.1 Delete `main.py`, `utils/docker_pull.py`, `github_api.py`, and `config.json` (also `utils/__init__.py` and `utils/xjson.py` removed; the `utils/` directory is gone)
- [x] 9.2 Update `.gitignore` to include `Dockerfile.bak.*` and `target/` (Cargo build output) and exclude `Cargo.lock` only if desired (we keep it for reproducible builds; the spec asked for "if not already ignored" so it is intentionally not added to the ignore list)
- [x] 9.3 Update `Readme.md` to describe the Rust workflow (`cargo run --release`), required env vars, the new trigger path, and that built images are published to `ghcr.io/<github-owner>/<image>:<version>`
- [x] 9.4 The application Dockerfile is at `app/Dockerfile` (multi-stage builder + runtime). The `Dockerfile` at the repo root remains the user-editable source the workflow builds from, per the editor spec.

## 10. Verification

- [x] 10.1 `cargo build --release` succeeds with no warnings
- [x] 10.2 `cargo run` starts; `GET /` returns the editor HTML; `GET /api/dockerfile` returns the seeded Dockerfile
- [x] 10.3 Editing and saving the Dockerfile creates a timestamped backup visible in the backups list
- [x] 10.4 Triggering Build calls GitHub with the expected `inputs` (verified by request construction; live run requires a real PAT)
- [x] 10.5 Image info and download endpoints return the upstream JSON for `alpine`
- [x] 10.6 `openspec-cn validate rust-stack-vvvv-image-builder --type change --strict` passes
