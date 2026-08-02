# Fresh Continuation real acceptance — 2026-08-01

Environment: Windows 11, Codex CLI/App Server `0.146.0`, Continuum `0.1.0-alpha.1`.

## Result

Passed. The ignored Rust acceptance test created a real clean Codex session through App Server in an isolated temporary read-only workspace.

- Imported a pre-existing Codex-format source session into a temporary Continuum database.
- Compiled and persisted a Context Snapshot and project-local Markdown bootstrap.
- Started `codex app-server`, completed `initialize`, `thread/start`, and `turn/start`.
- Received and persisted a newly created Codex session ID (redacted in the public report).
- Bound the session directly to the original unified project and branch.
- Located the actual persisted JSONL by the returned session ID.
- Incrementally indexed a non-empty assistant message into the unified timeline.
- Reopened SQLite and verified that the Continuation remained `listening` with the same target session ID.
- Did not modify the Continuum workspace or delete any existing Codex session.

Command:

```powershell
cd src-tauri
cargo test --lib real_app_server_fresh_continuation_creates_binds_and_indexes_a_session -- --ignored --nocapture
```

Observed test time: approximately 34 seconds.

## Defects found and fixed during acceptance

1. The first test design attempted to import the complete local Codex archive before launch and exceeded its outer timeout. The real test now imports only its source fixture, then locates the returned target ID and scopes incremental ingestion to that session's date directory.
2. `where.exe codex` returned both the npm CLI shim and a Microsoft Store internal desktop binary. The latter was visible but rejected direct process creation with Windows error 5. Runtime resolution now prefers `codex.cmd`, then `.exe`, then `.ps1`.
3. App Server response waits now use one absolute deadline, so unrelated notifications cannot extend a request forever.
4. If `thread/start` succeeds but the initial `turn/start` fails, Continuum persists the partial thread ID and refuses automatic retry, preventing duplicate empty sessions.

## Regression matrix

- TypeScript typecheck: passed.
- Vitest: 7 files, 10 tests passed.
- Playwright: 3 tests passed with one local worker.
- Rust: 30 passed, 1 real-Codex test ignored by default.
- Strict Clippy (`-D warnings`): passed.
- Vite production build: passed.
- Tauri release and NSIS packaging: passed.

Release artifacts:

- `src-tauri/target/release/continuum-desktop.exe` — SHA-256 `6DD6567D45F9C5D4EC52B069E2DB9FE8F1212CB3111D5003879475EE9E30F4BB`
- `src-tauri/target/release/bundle/nsis/Continuum_0.1.0-alpha.1_x64-setup.exe` — SHA-256 `1EA49A619D81FB7FA2FCCAF702FCC7B1161B60024C441F5AFA41B904BE8EA791`

The release linker emitted only Microsoft's normal import-library creation message; strict Clippy remained warning-free.
