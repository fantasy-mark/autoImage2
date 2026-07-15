## ADDED Requirements

### Requirement: Trigger image download via proxy.vvvv.ee
The system SHALL expose `POST /api/image/download` that accepts `image`, `mode` (default `prepare`), `compressed` (default `true`), and `platform` (default `linux/amd64`), and forwards the request to `${PROXY_BASE_URL}/api/image/download` with those parameters as query string. The upstream response body and content-type SHALL be returned to the client.

#### Scenario: Default parameters
- **WHEN** client posts `{"image": "alpine"}` to `/api/image/download`
- **THEN** the server forwards to `https://proxy.vvvv.ee/api/image/download?image=alpine&mode=prepare&compressed=true&platform=linux%2Famd64`
- **AND** returns the upstream JSON body with HTTP 200

#### Scenario: Custom platform
- **WHEN** client posts `{"image": "alpine", "platform": "linux/arm64"}`
- **THEN** the server URL-encodes the platform and forwards it; the response is returned unchanged

#### Scenario: Missing image
- **WHEN** client posts a body without `image`
- **THEN** the server returns HTTP 400 with `{"error": "missing image"}`

#### Scenario: Upstream non-2xx
- **WHEN** the upstream returns a non-2xx status
- **THEN** the server returns the same status and body to the client

### Requirement: Do not persist downloaded image on the server
The system MUST NOT write the downloaded image content to the local filesystem or to the git working tree.

#### Scenario: No artifact on disk
- **WHEN** `/api/image/download` is invoked successfully
- **THEN** the working tree contains no new files (e.g., no `*.tar` or `tmp_*` directories)
