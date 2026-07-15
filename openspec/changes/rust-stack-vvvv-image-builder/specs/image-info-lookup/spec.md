## ADDED Requirements

### Requirement: Query image information via proxy.vvvv.ee
The system SHALL expose `POST /api/image/info` that accepts an image name and proxies the request to `${PROXY_BASE_URL}/api/image/info?image=<name>` (default `https://proxy.vvvv.ee`), returning the upstream response body and content-type to the client.

#### Scenario: Successful info lookup
- **WHEN** client posts `{"image": "alpine"}` to `/api/image/info`
- **THEN** the server forwards the request to `https://proxy.vvvv.ee/api/image/info?image=alpine`
- **AND** the upstream JSON body is returned with HTTP 200 to the client

#### Scenario: Missing image field rejected
- **WHEN** client posts a body without the `image` field
- **THEN** the server returns HTTP 400 with a JSON error `{"error": "missing image"}`

#### Scenario: Upstream error forwarded
- **WHEN** the upstream returns a non-2xx status
- **THEN** the server returns the same status code and body to the client and emits a `tracing::warn!` log

#### Scenario: Request timeout
- **WHEN** the upstream does not respond within 15 seconds
- **THEN** the server returns HTTP 504 with `{"error": "upstream timeout"}`

### Requirement: Validate image name format
The system MUST reject image names that contain characters outside `[a-zA-Z0-9._\-/:@]` to prevent URL injection.

#### Scenario: Disallowed characters
- **WHEN** client posts `{"image": "ali pne"}` (contains space)
- **THEN** the server returns HTTP 400 with `{"error": "invalid image name"}`
