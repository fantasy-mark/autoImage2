## ADDED Requirements

### Requirement: Direct workflow dispatch from browser
The "Trigger build" button MUST call `POST /repos/{owner}/{repo}/actions/workflows/build.yml/dispatches` with `{"ref": "main", "inputs": {…}}` straight from the browser using the PAT.

#### Scenario: Successful dispatch
- **WHEN** the user has filled in `image` and clicks "Trigger build"
- **THEN** the browser sends the dispatch and the response is 204 No Content
- **AND** the UI shows "dispatched: build.yml (image:tag on <first 7 of SHA>)"

#### Scenario: Dispatch denied
- **WHEN** the user's PAT lacks `actions: write`
- **THEN** GitHub responds 403 "Resource not accessible by personal access token"
- **AND** the UI displays the error body and prompts the user to update their token's scope

### Requirement: Required input validation
The browser MUST refuse to send the dispatch if `image` is empty (the workflow's `image` input has `required: true`).

#### Scenario: Empty image
- **WHEN** the user clicks "Trigger build" with no `image` filled in
- **THEN** the browser does NOT send a request; it shows "image is required" inline

### Requirement: Refresh digest isn't called between save and dispatch
On Trigger build, the browser MUST first PUT the Dockerfile (so the on-disk file is up to date) and only then dispatch. There is no separate "git commit" service call.

#### Scenario: Trigger with unsaved changes
- **WHEN** the editor has unsaved edits and the user clicks "Trigger build"
- **THEN** the browser PUTs the new content first, then dispatches the workflow
- **AND** the on-disk file at the dispatched commit matches the editor
