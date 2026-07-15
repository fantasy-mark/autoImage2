## ADDED Requirements

### Requirement: List Dockerfile backups
The system SHALL expose `GET /api/dockerfile/backups` returning JSON `{"backups": [{"name": "Dockerfile.bak.YYYYMMDD-HHMMSS", "size": <bytes>, "created_at": "<RFC3339>"}]}`. Entries SHALL be sorted by filename descending (newest first).

#### Scenario: Backups exist
- **WHEN** the working tree contains one or more `Dockerfile.bak.*` files
- **THEN** the server returns HTTP 200 with the list sorted newest first

#### Scenario: No backups
- **WHEN** no `Dockerfile.bak.*` files exist
- **THEN** the server returns HTTP 200 with `{"backups": []}`

### Requirement: View a single backup
The system SHALL expose `GET /api/dockerfile/backups/:name` returning the backup content. The `:name` MUST match the regex `^Dockerfile\.bak\.\d{8}-\d{6}(\.\d+)?$`; otherwise the server returns HTTP 400. Path traversal segments (`..`, `/`) MUST be rejected.

#### Scenario: Read a valid backup
- **WHEN** client requests `GET /api/dockerfile/backups/Dockerfile.bak.20260714-153012`
- **THEN** the server returns HTTP 200 with the file content as JSON `{"name": "...", "content": "..."}`

#### Scenario: Path traversal blocked
- **WHEN** client requests `GET /api/dockerfile/backups/..%2FDockerfile`
- **THEN** the server returns HTTP 400 with `{"error": "invalid backup name"}`

### Requirement: Backup filename format
Backups MUST be named `Dockerfile.bak.YYYYMMDD-HHMMSS` in the local timezone, with optional `.<n>` suffix to resolve same-second collisions. The base filename MUST start with `Dockerfile.bak.` and the timestamp MUST be generated from `chrono::Local::now()`.

#### Scenario: Timestamp uses local time
- **WHEN** a save occurs at 2026-07-14 15:30:12 in the server's local timezone
- **THEN** the backup filename begins with `Dockerfile.bak.20260714-153012`

#### Scenario: Collision suffix
- **WHEN** two saves occur at the same local second
- **THEN** the second backup uses `Dockerfile.bak.YYYYMMDD-HHMMSS.2`
