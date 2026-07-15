## ADDED Requirements

### Requirement: Load configuration from config.toml
The system SHALL load a `config.toml` file from the working directory at startup and parse it into a typed structure containing `bind` (default `127.0.0.1:8080`), `proxy_base_url` (default `https://proxy.vvvv.ee`), `github.owner`, `github.repo`, `github.workflow_file`, `github.default_branch` (default `main`), `target.repo` (default `ghcr.io`), `target.namespace` (default equal to `github.owner`). Missing optional fields use the documented defaults.

#### Scenario: Config file present
- **WHEN** a valid `config.toml` exists in the working directory
- **THEN** the server starts with those values

#### Scenario: Default push target
- **WHEN** `config.toml` does not set `target.repo` or `target.namespace`
- **THEN** `target.repo` defaults to `ghcr.io` and `target.namespace` defaults to `github.owner`
- **AND** the built image is pushed to `ghcr.io/<github-owner>/<image>:<version>`

#### Scenario: Config file missing
- **WHEN** no `config.toml` exists
- **THEN** the server starts with default values for optional fields and returns an error if required fields are unset (e.g., `github.owner` is required when `/api/build` is first called; absence yields a clear startup warning but server still starts)

#### Scenario: Invalid TOML
- **WHEN** `config.toml` cannot be parsed
- **THEN** the server exits with a non-zero code and a clear error message

### Requirement: Environment variables override config file
The system MUST allow the following environment variables to override file values: `APP_BIND`, `PROXY_BASE_URL`, `GH_TOKEN`, `GH_OWNER`, `GH_REPO`, `GH_WORKFLOW`, `GH_DEFAULT_BRANCH`, `TARGET_REPO`, `TARGET_NAMESPACE`. `GH_TOKEN` is read exclusively from the environment and MUST NOT be loaded from `config.toml`.

#### Scenario: GH_TOKEN override
- **WHEN** `GH_TOKEN` is set in the environment
- **THEN** the server uses it for GitHub API calls

#### Scenario: Bind override
- **WHEN** `APP_BIND=0.0.0.0:9000` is set
- **THEN** the server listens on `0.0.0.0:9000` regardless of `config.toml`

### Requirement: Configuration is read-only at runtime
The configuration object MUST be immutable after startup. The application MUST NOT rewrite `config.toml` or persist runtime state to disk.

#### Scenario: Config not persisted
- **WHEN** the server processes a `/api/build` request
- **THEN** the contents of `config.toml` remain unchanged on disk
