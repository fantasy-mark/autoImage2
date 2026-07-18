## ADDED Requirements

### Requirement: List recent Dockerfile revisions
The app MUST call `GET /repos/{owner}/{repo}/commits?path=Dockerfile&per_page=20` and display the SHA, message, author, and ISO timestamp for each entry.

#### Scenario: Successful fetch
- **WHEN** the user opens the history view
- **THEN** up to 20 commits that touched `Dockerfile` are listed newest-first

#### Scenario: Empty history
- **WHEN** the repo has no `Dockerfile` commits yet
- **THEN** the UI shows "no history yet"

### Requirement: View a past revision
The user MUST be able to click a revision and see its content via `GET /repos/{owner}/{repo}/contents/Dockerfile?ref={sha}`.

#### Scenario: Click a revision
- **WHEN** the user clicks a commit entry
- **THEN** the textarea is replaced by a read-only view of that revision's `Dockerfile`

### Requirement: Diff two revisions
The app MUST be able to show a simple unified diff between the current local editor and any past revision, OR between two past revisions.

#### Scenario: Diff current vs picked revision
- **WHEN** the user picks a revision and clicks "Compare to current"
- **THEN** the UI renders a per-line diff (added lines prefixed `+`, removed `-`, unchanged ` `)
- **AND** uses a unified diff library — `diff` npm package — called entirely client-side
