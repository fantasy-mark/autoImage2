## ADDED Requirements

### Requirement: Pull command synthesis
Given an `image`, `version` (defaulting to `latest` when blank), and `platform` (defaulting to `linux/amd64`), the app MUST render a `podman pull --platform <p> proxy.vvvv.ee/ghcr.io/<owner>/<i>:<v>` string and copy it to the clipboard via a "Copy" button.

#### Scenario: Generate with overrides
- **WHEN** the user has typed `image=alpine`, `version=3.20`, and chose `linux/arm64` in the dropdown
- **THEN** the rendered command is `podman pull --platform linux/arm64 proxy.vvvv.ee/ghcr.io/fantasy-mark/alpine:3.20`

#### Scenario: Generate with defaults
- **WHEN** the user has only `image=alpine` filled in and `version` is blank
- **THEN** the rendered command uses `latest` for version

### Requirement: Copy via Clipboard API
The Copy button MUST use `navigator.clipboard.writeText()` with a fallback to `document.execCommand('copy')` on the selection if the API is not available.

#### Scenario: Modern browser
- **WHEN** the user clicks "Copy" in a Chromium-based browser
- **THEN** the command string is written to the clipboard and a "copied!" toast is shown

### Requirement: Static link to the proxy
The page MUST show `proxy.vvvv.ee` as a regular anchor text near the pull command so the user can navigate to it and understand the routing context.

#### Scenario: Link present
- **WHEN** the user scrolls to the pull command section
- **THEN** a clickable `proxy.vvvv.ee` link is rendered

### Requirement: No docker save tar streaming
The page MUST NOT attempt to download an image tarball. The user runs the displayed pull command on their target host instead.

#### Scenario: No tar download button
- **WHEN** the user inspects the page after a successful build
- **THEN** there is no "Download .tar" button or any link to `/api/registry/download`
