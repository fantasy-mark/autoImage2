## ADDED Requirements

### Requirement: Branch and Pages configuration
The documentation MUST describe the setup: create a `gh-pages` branch containing only `index.html` + `app.js` + `app.css`, then enable GitHub Pages with source = `gh-pages` branch.

#### Scenario: One-time setup
- **WHEN** a maintainer first deploys
- **THEN** they push the static branch, open repo Settings → Pages, and pick the branch as source

### Requirement: Static-only bundle
The `gh-pages` branch MUST contain only static assets — no `Cargo.toml`, no compiled binaries, no Docker-related files except as inert markup.

#### Scenario: Branch contains only static files
- **WHEN** the maintainer runs `git ls-tree gh-pages`
- **THEN** the only entries are HTML, CSS, JS, and an optional `.nojekyll` file (to bypass Jekyll processing)

### Requirement: Pages-style relative paths
All asset URLs (CSS/JS) MUST be relative (e.g. `app.css` not `/app.css`) so the site works under both `<user>.github.io/<repo>/` and a future custom domain.

#### Scenario: Asset loads under subpath
- **WHEN** the page is served at `https://<user>.github.io/autoimage2/`
- **THEN** the browser successfully resolves `app.css` and `app.js` relative to that base
