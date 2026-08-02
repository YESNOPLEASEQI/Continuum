# Remaining work

The authoritative live ledger is `AGENTS.md`; the detailed handoff is `docs/HANDOFF.md`. The older `docs/full-development-audit.md` is a historical baseline and contains state that has since changed.

## Active stage

Finish the remaining Codex P0 reliability and product surfaces before beginning the P1 workflow layer.

## Last verified baseline

- `npm run typecheck` — passed on 2026-08-02.
- `npm test -- --run` — 9 files / 15 tests passed on 2026-08-02.
- `cargo test --lib` — 45 passed / 1 real-Codex test ignored on 2026-08-02.
- `npm run test:e2e` — 4 passed on 2026-08-02.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed on 2026-08-02.
- `npm run build` — passed on 2026-08-02; Vite still reports the main JavaScript chunk above 500 kB.
- `npm run tauri:build` — release EXE and NSIS passed on 2026-08-02.
- The rebuilt release client was visually inspected against the real v4 database with 177 source sessions; the no-sidebar archive UI, full-screen menu, Sessions view, and hover/focus contrast were checked and adjusted.
- The isolated real Fresh App Server acceptance passed on 2026-08-01.

## Environment and repository

- Codex CLI/Desktop 0.146.0 has been verified locally.
- App Server v2 handshake and a full real Fresh session have been verified.
- Other Agent CLIs are not installed or verified.
- Public repository: <https://github.com/YESNOPLEASEQI/Continuum>.
- Default branch: `main`.
- Current app version: `0.1.0-alpha.2`.
- Current database schema: v4; App Server lifecycle notifications are normalized directly and JSONL is the read-only verifier/fallback.

## Remaining P0

- Add richer Conversation Graph and session-chain visualization.
- Expose backend branch comparison and deterministic selected-node merge in the UI.
- Complete branch/Continuation-scoped Skills and MCP bindings, detail views, duplicate/dependency warnings, and safe config editing/rollback.
- Add the dedicated read-only Git workspace.
- Add complete Context Health reminder actions and per-project reminder controls.
- Add paged/on-demand Raw Data access without restoring whole-session IPC payloads.
- Add safe migration and compaction for legacy giant SQLite databases.
- Refresh desktop acceptance for alpha.2 install, startup, restart, and binding persistence.

## Remaining P1

- Automatic rotation reminders and user thresholds.
- Complete Continuation recovery center and logs.
- Context history retrieval with pin/stale/incorrect actions.
- Context conflict detection and resolution.
- Continuation templates and reusable presets.
- Windows tray, background scanning, and notifications.
- Project activity timeline and error center.
- Safe Continuum project import/export.
- Performance profiling and virtualization for very large stores and timelines.

Multi-Agent adapters remain deferred until the Codex P0/P1 path is stable and can be verified honestly.
