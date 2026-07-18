## ADDED Requirements

### Requirement: Read Dockerfile via Contents API
The app MUST fetch the Dockerfile via `GET /repos/{owner}/{repo}/contents/Dockerfile` and decode the base64 `content` field.

#### Scenario: Successful read
- **WHEN** the user opens the page with a valid token and the file exists in the repo
- **THEN** the editor is populated with the file content

#### Scenario: 404 (file doesn't exist yet)
- **WHEN** Contents API returns 404
- **THEN** the editor starts empty and a "no Dockerfile yet" status is shown

### Requirement: Write Dockerfile via Contents API PUT
The "Save" button MUST `PUT /repos/{owner}/{repo}/contents/Dockerfile` with the current editor content (after variable substitution), base64-encoded, in a commit with a user-visible message.

#### Scenario: Successful save
- **WHEN** Contents API PUT returns 200/201 with a new `commit.sha`
- **THEN** the UI updates the "last saved" state and shows the new SHA

#### Scenario: Save failure
- **WHEN** Contents API PUT returns 401 (token revoked) or 4xx
- **THEN** the UI shows the error body and does NOT update the last-saved state

### Requirement: Conflict detection via SHA
The PUT MUST include the SHA of the file at fetch time as the `sha` field to detect concurrent edits.

#### Scenario: No concurrent edit
- **WHEN** the file SHA matches the last-fetched SHA
- **THEN** the PUT succeeds normally

#### Scenario: Concurrent edit on the server
- **WHEN** someone else pushed a change since the user fetched
- **THEN** Contents API returns 409 and the UI shows "file changed on the server, please refresh"

### Requirement: Configuration of target repo
The UI MUST expose fields for `owner` and `repo` (with defaults pre-filled from `octocat/hello-world` or a user-configured value) and persist these in `localStorage` alongside the token.

#### Scenario: User changes target repo
- **WHEN** the user types `acme-co` into the owner field and clicks Save
- **THEN** subsequent API calls go to `api.github.com/repos/acme-co/<repo>/…`
- **AND** the value persists in `localStorage` across reloads
