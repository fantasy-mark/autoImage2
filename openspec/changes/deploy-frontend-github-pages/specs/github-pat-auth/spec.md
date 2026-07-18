## ADDED Requirements

### Requirement: Token storage
The system MUST persist the user's GitHub PAT in the browser's `localStorage` under a known key and MUST read it back on every subsequent page load.

#### Scenario: First-time visit
- **WHEN** the page loads and `localStorage` has no token for the app
- **THEN** the UI shows a token input field and the rest of the app is hidden / disabled until a token is entered

#### Scenario: Returning visit
- **WHEN** the page loads and `localStorage` already has a token for the app
- **THEN** the UI hides the token input and proceeds to render the editor + dispatcher

#### Scenario: User updates token
- **WHEN** the user types a new token and clicks "Save token"
- **THEN** the new value overwrites the `localStorage` entry and a confirmation is shown
- **AND** the next outgoing API call uses the new token

### Requirement: Authenticated fetch wrapper
All outbound calls to `api.github.com` MUST include `Authorization: Bearer <token>` and `Accept: application/vnd.github+json`.

#### Scenario: Fetch with valid token
- **WHEN** the app needs to talk to the API
- **THEN** the `Authorization: Bearer …` header MUST be attached to the request

#### Scenario: Fetch with empty token
- **WHEN** `localStorage` returns no token (or empty)
- **THEN** the fetch MUST be prevented from firing; the UI MUST show the token input again

### Requirement: Token exposure warning
The UI MUST display, in plain text, a warning that the token is stored unencrypted in the browser and visible to anyone with DevTools access.

#### Scenario: First-time save
- **WHEN** the user saves a token for the first time in this browser
- **THEN** the page shows the warning text before the save action takes effect
