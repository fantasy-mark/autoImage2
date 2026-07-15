## ADDED Requirements

### Requirement: Read current Dockerfile
The system SHALL expose `GET /api/dockerfile` that returns the current `Dockerfile` content as JSON `{"content": "...", "size": <bytes>, "updated_at": "<RFC3339>"}`. If the file does not exist, the server returns HTTP 404 with `{"error": "dockerfile not found"}`.

#### Scenario: Read existing file
- **WHEN** the working tree contains a `Dockerfile`
- **THEN** the server returns HTTP 200 with the file content and metadata

#### Scenario: File missing
- **WHEN** the working tree does not contain a `Dockerfile`
- **THEN** the server returns HTTP 404 with `{"error": "dockerfile not found"}`

### Requirement: Save Dockerfile with automatic backup
The system SHALL expose `PUT /api/dockerfile` that accepts `{"content": "..."}`. Before overwriting the existing `Dockerfile`, the server SHALL create a backup file `Dockerfile.bak.YYYYMMDD-HHMMSS` (local time) in the same directory containing the current content. The PUT MUST be atomic with respect to the backup: if either step fails, the original file remains unchanged.

#### Scenario: Save creates one backup
- **WHEN** client sends `PUT /api/dockerfile` with new content
- **THEN** a single `Dockerfile.bak.YYYYMMDD-HHMMSS` file is created with the previous content
- **AND** `Dockerfile` is overwritten with the new content

#### Scenario: Save on a clean tree
- **WHEN** no `Dockerfile` exists yet and the client sends `PUT /api/dockerfile`
- **THEN** the server creates `Dockerfile` with the new content without creating any backup

#### Scenario: Concurrent same-second saves
- **WHEN** two saves land in the same second
- **THEN** the second backup receives a numeric suffix `Dockerfile.bak.YYYYMMDD-HHMMSS.2` to avoid collision
- **AND** both backups are preserved

#### Scenario: Non-UTF8 content rejected
- **WHEN** client sends content that is not valid UTF-8
- **THEN** the server returns HTTP 400 with `{"error": "content must be utf-8"}` and `Dockerfile` is unchanged

### Requirement: Provide editor UI page
The system SHALL serve a single-page editor at `GET /` consisting of an HTML form that loads the current `Dockerfile` content into a text area and exposes Save / Refresh / Trigger Build buttons. The page MUST call `/api/dockerfile`, `/api/image/info`, `/api/image/download`, and `/api/build` via `fetch`.

#### Scenario: Editor loads existing content
- **WHEN** a user opens `/` in a browser
- **THEN** the page fetches `GET /api/dockerfile` and populates the text area with the current content

#### Scenario: Save button persists content
- **WHEN** a user clicks Save
- **THEN** the page sends `PUT /api/dockerfile` with the text area content
- **AND** on HTTP 200, the page displays "saved" and the backup list is refreshed
