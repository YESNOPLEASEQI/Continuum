# Nightly worklog

## 2026-07-31 — baseline audit

- Read the unattended P0/P1 specification and established a persistent development goal.
- Confirmed the workspace is not a Git repository; created a hash-based recovery manifest and copied the v2 structural schema.
- Verified actual Codex CLI 0.133.0 help and executable location.
- Verified baseline frontend typecheck, 9 Vitest tests, and production build.
- Started strict Rust Clippy verification.
- Audited routes, IPC, models, migrations, parser, continuation detector, context compiler, project graph, configuration discovery, and current tests.
- Wrote `docs/full-development-audit.md` with the actual implementation boundary and ordered remediation.

Next: versioned SQLite migration and backend foundation.

## 2026-07-31 — P0 persistence, watcher, profiles, state machine

- Added transactional SQLite schema v3 migration with pre-migration backup, integrity checks, manual backup/restore, project/session cursor fields, continuation recovery fields, profile and diagnostics tables.
- Replaced repeated full-session polling with durable byte offsets, physical line cursors, partial-line recovery, per-file parse isolation, strict source identities, and an append-only hash chain.
- Added normalized project paths, duplicate prevention, relocate/rename/archive/restore/delete-record operations, safe bind/unbind/rebind, and missing-path visibility.
- Replaced hard-coded Codex assumptions with executable/version/help detection against the installed CLI and cached capability invalidation.
- Added Codex Profile persistence, validation, import/export, global/project/branch defaults, full IPC bridge, and a dedicated desktop editor.
- Upgraded Fresh Continuation to canonical persisted states, legal-transition checks, idempotency, launch/detection deadlines, durable candidates, retry/cancel/recovery/context cleanup, and independent Resume/Fork activity records.
- Corrected the UI main action to the required “压缩后新建会话” wording and removed the interactive fake “other Agent” action.
- Verified frontend typecheck and 6 Vitest files / 9 tests after these changes. Rust unit/integration suite is being rerun after canonical state migration.

Next: finish the Fresh state regression, connect profile selection to continuation launch, then complete context inspector/search/diagnostics P0 surfaces.

## 2026-08-01 — Context Compiler, Codex App Server, diagnostics, release

- Upgraded the Context Compiler taxonomy and health model, added deterministic item/content SHA-256 values, old-snapshot compatibility, rule-v2 persistence fields, per-item overrides, permanent items, and snapshot diff support.
- Added full Context Inspector controls for keep/compress/retrieve-only/exclude, priority/permanence, stale/incorrect handling, hashes, and two-snapshot comparison.
- Added global search/command navigation, complete Codex Profiles UI, Diagnostics, database backup/restore controls, settings validation, and canonical context-health badges.
- Refreshed the official Codex manual and verified the installed `codex-cli 0.146.0` App Server surface, generated its current v2 JSON Schema, and completed a real initialize handshake without creating a session.
- Added a Rust stdio JSON-RPC App Server adapter. Fresh Continuation now prefers `thread/start` + `turn/start` for explicit no-approval Profiles, binds the returned thread ID directly, and retains strict CLI/session-file detection as the compatibility fallback.
- Added App Server capability cache versioning, Diagnostics probe, Profile capability display, launch transport persistence, and direct unified-timeline switch events.
- Removed non-functional online compression provider placeholders so the UI/API cannot imply unavailable network behavior.
- Added/updated browser page objects and critical route tests. Local Playwright uses one worker to avoid a reproducible Windows page-fixture race; CI uses two.
- Added unified-timeline search, type/session filters, newest-first pagination, raw/copy/pin controls, persistent branch switching, and safe rename/archive/restore/delete branch management.
- Hardened App Server startup with a true total response deadline and partial-thread preservation so a failed first turn cannot cause an automatic duplicate-session retry.
- Fixed Windows Codex resolution to prefer the usable npm `codex.cmd` shim over an access-restricted Microsoft Store desktop binary returned by `where.exe`.
- Completed a non-destructive real Fresh acceptance in an isolated read-only workspace: App Server created a new session, returned its ID, Continuum bound it, found and incrementally indexed the actual JSONL and assistant response, and retained the binding after reopening SQLite.
- Verified 30 regular Rust unit/integration tests plus 1 explicitly ignored real-Codex acceptance test, 10 Vitest tests, 3 Playwright tests, TypeScript typecheck, strict Clippy with `-D warnings`, Vite production build, release EXE startup, and the NSIS bundle.
- Release artifacts: `src-tauri/target/release/continuum-desktop.exe` and `src-tauri/target/release/bundle/nsis/Continuum_0.1.0-alpha.1_x64-setup.exe`.

Next: finish interactive App Server approval handling, App Server notification persistence, branch compare/merge UI, Skills/MCP detail and binding scopes, dedicated Git workspace, and remaining P1 items.
