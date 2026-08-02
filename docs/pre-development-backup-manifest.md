# Pre-full-development baseline

Recorded on 2026-07-31 before the unattended P0/P1 implementation pass.

The workspace is not a Git repository, so no branch or checkpoint commit could be created. Existing files were left in place. This manifest is the recovery index for the last known buildable baseline.

## Baseline verification

- `npm run typecheck`: passed
- `npm test -- --reporter=dot`: 6 files, 9 tests passed
- `npm run build`: passed
- Release executable existed at `src-tauri/target/release/continuum-desktop.exe`
- NSIS installer existed at `src-tauri/target/release/bundle/nsis/Continuum_0.1.0-alpha.1_x64-setup.exe`

## Configuration hashes (SHA-256)

- `package.json`: `0FF298AADF588488319E8674E469E0ECE858F74F844D396FB5D6022B3F8FEA86`
- `package-lock.json`: `38078F7835C614ECA1A1D9F778701EC3B7BFF56C65BD861B8EEF441FF0BD1E95`
- `src-tauri/Cargo.toml`: `4810C2A82C037CA7F09F032487D59CF5C037397D98E671B9680F78CF0C00998C`
- `src-tauri/Cargo.lock`: `0F0277D8529493CDCBD8F01600E3EF706DA6A27F541483DA53F3ED482DA5D7D5`
- `src-tauri/tauri.conf.json`: `496D3E95CE6FE09F1CB149E570CBB764A84B694861242A5CED16080D6CB3CB81`
- `src-tauri/src/database.rs`: `3CEA09D6CD0297041C6F41C084E8377992E8CC588A12419720E43452C5AC6C51`

## Database baseline

The pre-pass schema is preserved verbatim in `docs/database-schema-baseline-v2.sql`. Application data is not copied into the repository because it can contain user session bindings. The live database discovered by the release smoke test is under the Tauri application data directory for `studio.continuum.desktop`.

## Safety notes

- No real Codex session file was copied, modified, or deleted.
- No source repository was reset, cleaned, committed, or pushed.
- No other Agent CLI was installed.
