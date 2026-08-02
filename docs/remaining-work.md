# Remaining work

The authoritative scope is the unattended P0/P1 specification. The current gap analysis is in `docs/full-development-audit.md`.

## Active stage

Complete the remaining P0 timeline/branch/configuration/Git surfaces and App Server approval relay after the v3 persistence, incremental watcher, Context Compiler v2, Codex Profile, App Server, and canonical Continuation foundation.

## Last verified baseline

- `npm run typecheck`
- `npm test -- --run` — 7 files / 10 tests
- `npm run test:e2e` — 3 tests
- `cargo test --lib` — 30 passed, 1 real-Codex test ignored by default
- `cargo test --lib real_app_server_fresh_continuation_creates_binds_and_indexes_a_session -- --ignored --nocapture` — real Fresh acceptance passed on 2026-08-01
- `cargo clippy --all-targets --all-features -- -D warnings`
- `npm run build`
- `npm run tauri:build` — release EXE + NSIS installer

Full real-Fresh evidence and artifact hashes are recorded in `docs/fresh-continuation-acceptance-2026-08-01.md`.

## Environment boundary

- Current machine has Codex CLI/Desktop 0.146.0 only; App Server v2 handshake and a full real Fresh session are verified.
- Claude Code, Gemini CLI, OpenCode, and other Agent CLIs will not be installed or tested.
- Workspace is not a Git repository, so checkpoint commits are unavailable.

## Remaining P0 gaps

- Relay App Server approval requests into Continuum UI before enabling `on-request` and `untrusted` Profiles on that transport.
- Persist/consume the App Server notification stream directly; keep JSONL ingestion as the deduplicating persistence verifier.
- Add a richer visual session-chain display; local timeline search/filter/pagination and raw/copy/pin controls are complete.
- Expose branch comparison and deterministic selected-node merge in the UI; rename/archive/restore/delete and safe deletion constraints are complete.
- Complete branch/Continuation-scoped Skills and MCP bindings, detail views, duplicate/dependency warnings, and safe config editing.
- Add the dedicated Git workspace and context-health reminder actions.
- Add fake-process App Server timeout/exit/error integration tests. The non-destructive isolated real Fresh acceptance now passes and remains ignored during ordinary test runs because it creates a persistent local Codex session.

## P1 not yet complete

- Continuation templates and reusable presets UI.
- Activity/error center and richer diagnostics log persistence.
- Multi-Agent adapters beyond the intentionally bounded Codex implementation.
- Performance profiling for very large session stores and long unified timelines.
