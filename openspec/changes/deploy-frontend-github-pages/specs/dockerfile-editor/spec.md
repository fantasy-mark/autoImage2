## ADDED Requirements

### Requirement: Single-editor surface
The page MUST provide ONE `<textarea>` for editing the Dockerfile. There MUST NOT be a second editor pane or a separate "raw vs structured" view.

#### Scenario: Initial render
- **WHEN** the page loads with a valid token and the Dockerfile was successfully fetched from Contents API
- **THEN** the textarea is populated with the file content

### Requirement: Variable substitution
While editing, the system MUST scan for `${VAR}` and `${VAR:-default}` and surface one input row per unique variable so the user can override defaults before saving.

#### Scenario: Variable detected
- **WHEN** the editor content contains `${TARGETPLATFORM:-linux/amd64}` and `${TURBO_VERSION:-2.9.18}`
- **THEN** the UI shows two rows: `TARGETPLATFORM` (default `linux/amd64`) and `TURBO_VERSION` (default `2.9.18`)

#### Scenario: User overrides a variable
- **WHEN** the user types `linux/arm64` into the `TARGETPLATFORM` input
- **THEN** clicking "Save" sends the editor content with that variable substituted

#### Scenario: User leaves variable blank
- **WHEN** the user clears an override input
- **THEN** "Save" sends the content with the default value (or leaves `${VAR}` unsubstituted if no `:-`)

### Requirement: Unsaved-changes indication
If the textarea content differs from the last successfully saved version (per the GitHub SHA returned by Contents API), the UI MUST show an "unsaved changes" hint.

#### Scenario: Edit without saving
- **WHEN** the user types in the editor
- **THEN** the indicator is visible while the local content differs from the last-saved SHA

#### Scenario: Successful save
- **WHEN** Contents API PUT succeeds and returns a new `commit.sha`
- **THEN** the indicator clears and the new SHA is shown
