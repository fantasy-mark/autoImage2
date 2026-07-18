## 1. Bootstrap gh-pages branch

- [x] 1.1 Create `ghpages/` directory at repo root with `index.html`, `app.css`, `app.js` placeholders
- [x] 1.2 Set up orphan branch `gh-pages` from `ghpages/`: `git checkout --orphan gh-pages && git checkout ghpages/ . && git commit`
- [x] 1.3 Add `.nojekyll` empty file to skip Jekyll processing

## 2. PAT auth (`github-pat-auth`)

- [x] 2.1 Add a `<input type="password">` for the GH_TOKEN; render top of page on first visit
- [x] 2.2 Persist to `localStorage` under key `autoimage.ghToken`
- [x] 2.3 Add `owner` + `repo` inputs alongside, persisted to `localStorage` too
- [x] 2.4 Add a "warning" banner explaining the token is plaintext in the browser
- [x] 2.5 On 401 from any GitHub API, clear the token and force the user back to the input form

## 3. Contents API read (`dockerfile-contents-api` + `dockerfile-editor`)

- [x] 3.1 `GET /repos/{owner}/{repo}/contents/Dockerfile` with `Authorization: Bearer <token>`
- [x] 3.2 Decode base64 `content` and put it into the textarea
- [x] 3.3 On 404: leave textarea empty, show "no Dockerfile yet"
- [x] 3.4 On 401: trigger the auth re-entry flow

## 4. Variable substitution UI (`dockerfile-editor`)

- [x] 4.1 Scan editor content for `${VAR}` / `${VAR:-default}` with regex `\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}/g`
- [x] 4.2 Render one row per distinct variable below the editor; show default as placeholder
- [x] 4.3 Add a "Use defaults for all" button that fills inputs with their placeholders
- [x] 4.4 On save, substitute values into content (using `:-` default if value empty) before PUT

## 5. Contents API write (`dockerfile-contents-api`)

- [x] 5.1 Implement `PUT /repos/{owner}/{repo}/contents/Dockerfile` with base64 content + commit message
- [x] 5.2 Include the SHA from the most recent GET as `sha` for optimistic locking
- [x] 5.3 On 409 (conflict), show "file changed on the server, please refresh"
- [x] 5.4 On success, update local "last saved SHA" state and clear "unsaved changes" indicator

## 6. Workflow dispatch (`workflow-build-trigger`)

- [x] 6.1 Implement `POST /repos/{owner}/{repo}/actions/workflows/build.yml/dispatches` with `{"ref":"main","inputs":{...}}`
- [x] 6.2 Wire `image` / `version` (default `latest`) inputs; mirror them across all four UI inputs (existing logic from `feat(editor): mirror`)
- [x] 6.3 On 403 with "Resource not accessible by personal access token", show error body and prompt user to update PAT scope
- [x] 6.4 Show "dispatched: build.yml (image:tag on <sha>)" on 204

## 7. Pull command synthesis (`ghcr-pull-tar`)

- [x] 7.1 Render `podman pull --platform <p> proxy.vvvv.ee/ghcr.io/<owner>/<i>:<v>` with version defaulting to `latest`
- [x] 7.2 Add platform `<select>` with options: linux/amd64, linux/arm64, linux/arm/v7, linux/386, linux/ppc64le, linux/s390x
- [x] 7.3 Implement Copy button via `navigator.clipboard.writeText` with `execCommand('copy')` fallback
- [x] 7.4 Render `proxy.vvvv.ee` as a static anchor text near the command

## 8. Commit history viewer (`commit-history-diff`)

- [x] 8.1 `GET /repos/{owner}/{repo}/commits?path=Dockerfile&per_page=20` to list recent Dockerfile commits
- [x] 8.2 Render each entry with short SHA, message, author, timestamp
- [x] 8.3 Click handler: `GET /repos/{owner}/{repo}/contents/Dockerfile?ref={sha}` to fetch that revision
- [x] 8.4 Show a read-only `<pre>` of the historical content
- [x] 8.5 Add "Compare to current" button; implement Myers diff algorithm (~30 LOC) inline

## 9. Deploy

- [x] 9.1 Build a tarball of `ghpages/` (no source maps in production)
- [x] 9.2 Push to `gh-pages` branch
- [x] 9.3 Enable GitHub Pages with source = `gh-pages` / root
- [x] 9.4 Visit the deployed URL, verify token entry, editor load, save, dispatch, download

## 10. Clean up main branch (optional)

- [x] 10.1 Move `static/` to `static-legacy/` and add a deprecation notice in `README.md`
- [x] 10.2 (Do NOT delete the Rust code — keep it for local development)
