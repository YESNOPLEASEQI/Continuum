# Continuum full-development audit

Audit date: 2026-07-31  
Workspace: local Continuum development workspace  
Repository state: the directory is **not** a Git repository. No branch or checkpoint commit can be created. The baseline hashes and schema copy are recorded in `docs/pre-development-backup-manifest.md`.

## Baseline actually verified

- React 19 + TypeScript + Vite application builds successfully.
- Tauri 2 / Rust / rusqlite application and NSIS configuration exist.
- `npm run typecheck` passed.
- `npm test -- --reporter=dot` passed: 6 test files, 9 tests.
- `npm run build` passed.
- Existing release EXE and NSIS installer are present.
- The installed Codex executable is the npm PowerShell shim in the current user's roaming npm directory.
- Actual Codex version is `codex-cli 0.133.0`.
- Actual top-level help advertises `resume`, `fork`, `-C/--cd`, model, profile, sandbox, approval, and inline prompt support.
- A previous real smoke run created a Codex session, and its persisted JSONL was matched by marker, cwd, session metadata, and creation time. This is useful evidence but does not by itself satisfy the new complete P0 acceptance matrix.

## Product direction and runtime truth

The active routes and navigation are Continuum-first. Legacy AgentPack source modules and tests remain in the tree, but package routes are not in the active router. Runtime browser fallbacks return empty states or desktop-only errors; they do not inject demonstration sessions into the desktop application.

Current complete-Agent claim: **Codex only**. Claude/Gemini/OpenCode are enum values or future adapter scaffolding, not installed, not parsed, and not verified.

## Features already connected end to end

- Create a Unified Project with one main branch.
- Scan configured/default Codex JSON/JSONL directories recursively.
- Parse a useful subset of Codex messages, tool calls, commands, file changes, errors, cwd, timestamps, and raw JSON.
- Bind one or several scanned sessions to a project branch without rewriting source JSONL.
- Build a unified timeline with source-session traceability.
- Create a branch from a timeline node.
- Compile deterministic RuleBased context, save a Context Snapshot, write a project-local Markdown bootstrap, hash it, and launch a fresh Codex process.
- Detect a candidate using Codex type, creation time, normalized cwd, first-user marker, and unbound status.
- Bind one high-confidence result automatically or return several candidates.
- Poll a bound source and append new message nodes.
- Persist projects, branches, nodes, snapshots, continuations, skills/MCP inventory, and bindings in SQLite.

## UI-only or partial features

- Context Inspector renders actions and can map node status/importance changes, but it lacks complete per-item override persistence, snapshot diff, conflict resolution, permanent exclusion, and source-message navigation.
- Context Health exists but uses the older five labels (`healthy`, `growing`, `compress`, `new_session`, `high_risk`) and fewer metrics than required.
- Skills and MCP inventory can be scanned and project-bound. Branch and continuation bindings, dependency diagnostics, duplicate diagnostics, secure configuration diff/edit/rollback, and environment-name-only modeling are missing.
- Project archive exists. Restore, delete-record-only, relocation, rename, recent/open tracking, and path-missing recovery are missing.
- Branch creation/switching exists. Rename, archive/restore/delete, comparison, deterministic ContextItem merge, and graph integrity checks are missing.
- Timeline renders messages, tool nodes, file changes, errors, and switch nodes. Full filtering/search, Raw Data, copy/pin/stale/incorrect/exclude controls, pagination/virtualization, and command/test-specific rendering are incomplete.
- Settings persist a small subset of the required fields and do not provide path validation diagnostics.
- Resume/Fork buttons launch native Codex commands, but capabilities are hard-coded and no operation/event record is persisted.
- Fresh Continuation has a working abbreviated state flow (`prepared`, `launching`, `waiting_detection`, `needs_confirmation`, `listening`, `launch_failed`) but not the required durable, idempotent state machine.

## Backend-only features not fully connected to UI

- Git inspector reads branch, HEAD, status, working diff, staged diff, timeout, and errors, but has no dedicated inspector page.
- Security scanner redacts parsed values, but coverage and report/export/log guarantees need expansion and tests.
- Agent adapter traits include future extension methods, but only Codex has meaningful runtime support.

## Mock and fixture boundary

- Vitest and Rust unit/integration tests use fixtures and temporary directories, as allowed.
- Playwright currently verifies only the browser shell and empty state. It does not cover the full requested desktop workflow.
- The browser API deliberately returns empty settings/dashboard placeholders when Tauri IPC is absent. These are not used as desktop session data.
- No formal production mock-session generator is present.

## Current data model and migration state

SQLite schema version records 1 and 2. Most Continuum tables were added in one large `MIGRATION_SQL` block. Continuation columns are also added by best-effort `ALTER TABLE` statements whose errors are ignored.

Important gaps relative to P0:

- `projects` lacks normalized/display path separation, default profile, last-opened time, and a uniqueness constraint on normalized path.
- `conversation_branches` lacks current session and archive timestamp.
- `conversation_nodes` lacks source message column, explicit flags, import timestamp, and a composite uniqueness constraint.
- `source_sessions` lacks external ID separation, normalized cwd, import cursor/line, file hash, bound IDs, status, and raw metadata.
- `continuations` lacks source session, complete timestamps, failure code/message, completion timestamp, update timestamp, and required operation/state values.
- `context_snapshots` lacks original estimate, compiler version, content hash, and metadata columns.
- `context_items` lacks action reason naming, priority, stale/incorrect flags, and conflict group.
- No Codex profile, branch configuration binding, activity-event, reminder, template, watcher-error, diagnostic, or backup metadata tables exist.
- Migration rollback/backup, integrity check, crash recovery, orphan detection, and restore validation are missing.

The pre-pass structural schema is copied in `docs/database-schema-baseline-v2.sql`.

## Current scan and parse behavior

- Recursively enumerates `.jsonl` and `.json` files using WalkDir.
- Invalid individual JSONL lines are skipped and summarized.
- A file with no parseable record is rejected; one file failure is logged and does not stop the scan.
- The parser redacts raw JSON before returning/storing it.
- Parsed fields are heuristic public fields only; inaccessible model hidden state is not represented.
- Full rescans re-read each entire file.
- Parse errors are not persisted in a dedicated readable scan-error table.
- Raw unknown values are kept in the serialized session detail, but raw metadata is not normalized into the required source-session field.

## Current incremental behavior

There is no durable byte cursor or filesystem watcher. Unified Chat polls every five seconds, reparses bound files in full, and deduplicates appended messages by deterministic node ID. This prevents many duplicates but does not satisfy half-line, file rename, lock retry, directory recreation, or restart-resumed cursor requirements.

## Current Fresh Continuation identification

The detector currently checks:

- agent is Codex;
- parsed creation time is later than `started_at`;
- normalized cwd equals the project cwd;
- first visible user message contains the unique `CONTINUATION_ID` marker;
- session is not already bound.

Missing or incomplete checks:

- file modified time after launch;
- explicit source-session exclusion;
- launch-prompt signature beyond the marker;
- parse-health confidence details;
- timeout state and retry/re-detect controls;
- durable candidate rows with first-message preview;
- duplicate-process detection.

## Existing test coverage

Rust currently has 15 tests covering parser basics, invalid JSONL tolerance, continuation marker/normalized cwd, deterministic context, configuration scan, Git parsing, security redaction, package compatibility/integrity, filesystem hashing, project source traceability, and a temporary-directory continuation integration flow.

React currently has 9 tests across 6 files. Two files still cover inactive legacy package pages. Fresh/Resume/Fork distinction has one test.

Playwright currently has 2 browser-shell tests. It does not yet exercise project creation, binding, context inspection, candidates, profiles, diagnostics, or recovery center.

## Real Codex evidence

Existing evidence:

- marker-based fresh session creation was executed against the actual installed Codex CLI;
- persisted JSONL session metadata and cwd were checked;
- Codex read the generated file and checked the workspace;
- the real session history was retained.

Still required for this development pass:

- isolated test Git repository;
- actual app-compatible capability cache;
- full real data-layer bind/listen/reopen evidence with recorded snapshot ID;
- native Resume verification;
- native Fork verification because 0.133.0 advertises Fork;
- sanitized test report artifacts.

## Current technical debt

- Many React pages are compressed into very long source lines, making focused maintenance difficult.
- Legacy package code increases backend/API/test surface even though it is not routed.
- Agent capability values are hard-coded.
- The installed `codex.ps1` shim needs robust Windows resolution for every operation.
- Full-file reparsing does not scale to long sessions.
- Timeline ordering relies on timestamps plus SQLite row order rather than an explicit sequence.
- Several required destructive metadata actions lack confirm dialogs and backend safety constraints.
- No central diagnostics or activity log exists.

## Security risks

- npm audit reports two high advisories in React Router's RSC action path. Continuum is a static client and does not enable RSC/SSR actions; the currently suggested fixed 8.3.0 line is not published in the active registry. Do not silently downgrade to the older vulnerable line.
- Parsed session values are redacted, but logs, diagnostics, exports, MCP environment references, and context snapshots need one shared sanitizer.
- MCP scanning must avoid persisting environment values; this needs explicit schema and tests.
- Launch-command previews may expose user paths and prompt metadata and therefore must be sanitized in exported diagnostics.

## Implementation order

1. Introduce reliable versioned migrations, normalized path helpers, complete P0 core columns/tables, integrity/backup primitives, and model updates.
2. Add real Codex executable/version/help capability detection and cache.
3. Replace full-file-only syncing with durable incremental cursors and a resilient watcher/polling service.
4. Complete project, binding, branch, graph, timeline, search, and annotation operations.
5. Complete deterministic Context Compiler V2, Inspector diff/overrides, Health metrics, profiles, Skills/MCP, and read-only Git integration.
6. Replace the abbreviated Fresh Continuation flow with a durable idempotent state machine and recovery operations.
7. Add Settings validation, Diagnostics, database backup/restore, global search, command palette, and recovery center.
8. Implement P1 reminders, templates, conflict workflows, project activity, safe import/export, and tray/background behavior where stable.
9. Expand Rust, React, integration, Playwright, real Codex, release, NSIS, and desktop visual acceptance.

This audit is a baseline, not a completion claim. Development continues immediately after it.
