## ADDED Requirements

### Requirement: Trigger GitHub Actions workflow
The system SHALL expose `POST /api/build` that calls GitHub's `POST /repos/{owner}/{repo}/actions/workflows/{workflow_file}/dispatches` with `ref` from the configured default branch and `inputs` containing `repo` (the push registry hostname, default `ghcr.io`), `namespace` (the ghcr namespace, default `github.owner`), `image`, and `version` (all read from `config.toml`; `image` and `version` overridable via request body). The endpoint requires no client credentials; it reads the GitHub token from the `GH_TOKEN` environment variable.

#### Scenario: Successful dispatch
- **WHEN** client posts `{"image": "alpine", "version": "3.20"}` to `/api/build`
- **THEN** the server sends `workflow_dispatch` to GitHub with `inputs` including `repo=ghcr.io, namespace=<github-owner>, image=alpine, version=3.20`
- **AND** returns HTTP 202 with `{"accepted": true, "workflow": "<workflow_file>"}`

#### Scenario: Missing GH_TOKEN
- **WHEN** the `GH_TOKEN` environment variable is not set
- **THEN** the server returns HTTP 500 with `{"error": "GH_TOKEN not configured"}`

#### Scenario: GitHub returns non-2xx
- **WHEN** GitHub returns a 4xx or 5xx response
- **THEN** the server returns the same status to the client and logs the GitHub response body

### Requirement: Workflow file accepts workflow_dispatch inputs
The `.github/workflows/build.yml` file MUST declare `workflow_dispatch` as a trigger and accept `inputs` for `repo`, `namespace`, `image`, `version` (all strings, `image` required, others with defaults from `config.toml` semantics; `repo` defaults to `ghcr.io`, `namespace` defaults to the GitHub owner). The build job MUST read those inputs instead of parsing the Dockerfile comment, declare `permissions: { contents: read, packages: write }`, log in to `${repo}` using `${GITHUB_TOKEN}` as the password and `${github.actor}` as the username, and push to `${repo}/${namespace}/${image}:${version}`.

#### Scenario: Workflow triggered manually
- **WHEN** a `workflow_dispatch` event fires with `inputs.image=nacos/nacos-server, inputs.version=v2.5.2`
- **THEN** the build job logs in to `ghcr.io` with the GitHub token
- **AND** runs `docker build -t ${image}:${version} .`
- **AND** runs `docker push ghcr.io/<github-owner>/${image}:${version}`

#### Scenario: Workflow permissions insufficient
- **WHEN** the workflow does not declare `packages: write`
- **THEN** the `docker push` step fails with a 403 from the registry
- **AND** the run is marked failed in the GitHub Actions UI

### Requirement: No local git push from the application
The application process MUST NOT execute `git add`, `git commit`, or `git push` to trigger the build. Triggering is exclusively through the GitHub API.

#### Scenario: Trigger does not touch the working tree
- **WHEN** `/api/build` is called
- **THEN** the working tree's git status is unchanged
