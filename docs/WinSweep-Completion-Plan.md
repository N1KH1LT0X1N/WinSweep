# WinSweep — Codebase Completion Plan

> **Status:** Active · **Author:** Engineering · **Date:** 2026-06-21
> **Scope:** Full-codebase audit against the [Final Implementation Plan v5.0](archive/WinSweep-Implementation-Plan-vFinal.md), fix every bug/stub/missing feature, expand tests, refresh docs.

---

## 0. Executive Summary

A line-by-line audit of all 80 Rust source files (~25k LOC) was performed against the
v5.0 plan and the v1.0.0 CHANGELOG claims. The **build is green** and **111 tests pass**,
but the audit surfaced **one class of high-impact defects the release notes hid**:

1. **The CLI TUI is ~40% functional.** Most action handlers (`start_scan`,
   `refresh_package_managers`, `clean_*`, `toggle_service`, `save_config`,
   `compact_wsl_distribution`, `cleanup_docker`, `cleanup_windows_update`) are
   **stubs** that set a "…ing" flag and log a line but never call the backend.
2. **Two correctness bugs can corrupt safety/identity guarantees** — a
   case-sensitivity hole in the `NEVER_DELETE` parent check, and a `CleanupResult`
   that always fabricates a fresh `scan_id` instead of echoing the scan it cleaned.
3. **An over-broad IPC pipe DACL** (`Authenticated Users`) widens the attack surface
   on the cross-privilege channel.
4. **`cargo` cleaner runs `cargo clean` inside WinSweep's own working directory** —
   wrong target; a latent foot-gun.
5. **Several plan-mandated behaviors are missing**: WSL2 5-retry/backoff +
   process-kill + sparse detection, full service lifecycle (delete / re-enable /
   real start-type), and two file-version detector stubs.
6. **Clippy is not clean** (~18 warnings) despite the status doc claiming zero.

The marginal cost of finishing all of this is low and the fixes are well-bounded.
This document is the execution plan; every item below will be implemented, tested,
and documented in this pass.

### Severity rollup

| Sev | Count | Theme |
|-----|-------|-------|
| 🔴 Critical | 4 | NEVER_DELETE case bug, IPC DACL, CleanupResult scan_id, cargo clean wrong dir |
| 🟠 High | 11 | CLI action stubs (scan, pkg-mgr, services, config, wsl, docker, update) |
| 🟡 Medium | 14 | Plan gaps: WSL retry/kill/sparse, service lifecycle, detector stubs, handle leaks, config dead fields |
| 🔵 Low | ~10 | pkg-mgr init/env-var nits, regex perf, clippy warnings, doc drift |

---

## 1. Verified Current State

```text
cargo build --workspace                                  → OK (exit 0)
cargo clippy --workspace --all-targets --all-features    → 0 errors, ~18 warnings
cargo test  --workspace --all-features                   → 111 passed, 1 ignored
```

- The ignored test (`ipc::tests::test_ipc_server_client`) requires admin + a stable
  named-pipe handle and is intentionally skipped.
- The reported "runtime exit 1" for `winsweep-cli` and `winsweep-gui` is a
  PowerShell artifact (stderr routed through `2>&1` is treated as a native error);
  both binaries actually launch correctly.

---

## 2. Findings & Fix Plan

Legend: ☐ pending · ☑ done. Each fix lands with a test where testable.

### 2.1 🔴 Critical — Safety & Security

| # | File:Line | Defect | Fix |
|---|-----------|--------|-----|
| C1 | [never_delete.rs](../crates/winsweep-common/src/never_delete.rs) | Parent-of check `never_path_buf.starts_with(path)` compares an **original-case** `PathBuf` against a **lowercased** `path` → `C:\WINDOWS` won't match `c:\windows`. | Lowercase both sides before `starts_with`; add regression test for mixed-case parents. |
| C2 | [ipc.rs](../crates/winsweep-core/src/ipc.rs#L33) | `PIPE_SDDL = "D:(A;;GA;;;AU)"` grants **all Authenticated Users** full access to the elevated IPC pipe. | Restrict to the interactive owner + SYSTEM + Administrators: `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)` with explicit owner SID; document rationale. |
| C3 | [cleanup.rs](../crates/winsweep-core/src/cleanup.rs) | `CleanupResult { scan_id: Uuid::new_v4() }` — comment says "set by caller" but it never is; result can't be correlated to its scan. | Add `scan_id` parameter to `cleanup()` and thread it through. Update callers + integration test asserting the id is preserved. |
| C4 | [cargo.rs](../crates/winsweep-core/src/package_manager/cargo.rs) | `cargo clean` is invoked with `current_dir = std::env::current_dir()` — i.e. **WinSweep's own CWD**, deleting an unrelated `target/`. | Remove the misdirected `cargo clean` block; rely on the already-correct registry/git cache directory removal (`$CARGO_HOME/registry`, `/git`). |

### 2.2 🟠 High — CLI TUI action handlers (all currently stubs)

All live in [crates/winsweep-cli/src/app.rs](../crates/winsweep-cli/src/app.rs). The
app owns a `tokio::runtime::Runtime`; Docker already uses `block_on` successfully, so
each handler will be wired the same way (synchronous `block_on` into the real backend),
respecting `--dry-run`.

| # | Handler | Current | Fix |
|---|---------|---------|-----|
| H1 | `start_scan()` | sets `scanning=true`, logs, never scans. | `block_on(Scanner::scan(paths))`, store results, honor `min_age_days`, populate result list, clear progress. |
| H2 | task-receiver loop | `if task_tx.is_some() { /* would have a receiver */ }` | Remove dead plumbing or implement a real `mpsc` receiver drain; simplest correct path: drop unused sender, drive ops synchronously (matches existing Docker pattern). |
| H3 | `refresh_package_managers()` | `cache_size:0, installed:false` hardcoded. | For each manager `block_on(get_cache_info())`; set real `installed`, `cache_size`, `version`. |
| H4 | `clean_selected_package_manager()` | no-op. | `block_on(manager.clean_all_caches())`; report freed bytes; refresh. |
| H5 | `clean_all_package_managers()` | no-op. | Iterate registry, clean each, sum freed; refresh. |
| H6 | `show_package_manager_info()` | no-op. | Populate a detail string with per-path sizes from `get_cache_info()`. |
| H7 | `save_config()` | no-op. | `self.config.save()?`; surface success/error in status. |
| H8 | `toggle_service()` | no-op. | `block_on` via `ServiceManager` start/stop based on current state. |
| H9 | `compact_wsl_distribution()` | no-op. | `block_on(HomeEditionCompat::compact_wsl_vhdx(dist))`; show method + freed. |
| H10 | `cleanup_docker()` | no-op. | `block_on(DockerClient::cleanup_all())`; show reclaimed. |
| H11 | `cleanup_windows_update()` | no-op. | `block_on` Windows Update cache cleanup through the elevated/service path. |

### 2.3 🟡 Medium — Plan-mandated behavior & robustness

| # | File | Gap (vs v5.0 plan) | Fix |
|---|------|--------------------|-----|
| M1 | [home_edition_compat.rs](../crates/winsweep-core/src/home_edition_compat.rs) | `compact_wsl_vhdx` has **no 5-retry exponential backoff**, **no process-kill**, **no sparse-VHD detection** (all explicit plan requirements). | Add retry loop (5×, 2s·2^n backoff), `shutdown_wsl()` before retries, optional VHDX-signature sparse check; return method + attempts in result. |
| M2 | [service_manager.rs](../crates/winsweep-core/src/service_manager.rs) | No `delete_service`, no `re_enable_service`, `start_type` always `Unknown`. | Implement `DeleteService`, `re_enable_service`, and `QueryServiceConfigW`-based start-type read; wire `can_delete` correctly. |
| M3 | [tool_detector.rs](../crates/winsweep-core/src/tool_detector.rs) | `get_file_version()` and `get_nuget_version()` always return `None`. | Implement `GetFileVersionInfoW`/`VerQueryValueW` for file version; parse `nuget`/`nuget help` banner for its version. |
| M4 | [windows_api.rs](../crates/winsweep-core/src/windows_api.rs) | `get_final_path_name` leaks the `CreateFileW` HANDLE on error paths. | RAII guard (`OwnedHandle`/`scopeguard`) so the handle always closes. |
| M5 | [junction_detector.rs](../crates/winsweep-core/src/junction_detector.rs) | `CreateFileW` handles in reparse/target reads not always closed; raw pointer math unbounded. | Same RAII guard; bounds-check reparse buffer before pointer arithmetic. |
| M6 | [scanner.rs](../crates/winsweep-core/src/scanner.rs) | `dir_size_sync` blocks the tokio worker and swallows IO errors; `items_scanned` computed then discarded. | Wrap the recursive walk in `spawn_blocking`; surface `items_scanned` in the returned summary/log; keep permission errors as debug-logged skips. |
| M7 | [config.rs](../crates/winsweep-common/src/config.rs) | `max_file_size` validated but never used by the scanner; duplicate flat/nested fields. | Wire `max_file_size` into the scanner's size filter; document the canonical (nested) fields and deprecate the flat duplicates in docs. |
| M8 | [updater.rs](../crates/winsweep-core/src/updater.rs) | Confirm `WinVerifyTrust` is actually invoked on the downloaded payload (plan: "verified self-update"). | Verify/observe the call path; add a negative test (unsigned file ⇒ error) — already partially present. |

### 2.4 🔵 Low — Package-manager polish & hygiene

| # | File | Issue | Fix |
|---|------|-------|-----|
| L1 | [maven.rs](../crates/winsweep-core/src/package_manager/maven.rs) | Reads `MAVEN_OPTS` (JVM flags) as a home dir — dead heuristic. | Read `MAVEN_HOME` / `M2_HOME` instead. |
| L2 | [go_modules.rs](../crates/winsweep-core/src/package_manager/go_modules.rs) | Fallback adds `%TEMP%\go-build*` (wrong; real path is `%LOCALAPPDATA%\go-build`). | Drop the TEMP fallback; trust `go env GOCACHE`. |
| L3 | [conda.rs](../crates/winsweep-core/src/package_manager/conda.rs) | `shellexpand::full(...).unwrap_or_default()` silently yields empty paths. | Log + skip on expansion failure. |
| L4 | [vscode.rs](../crates/winsweep-core/src/package_manager/vscode.rs) | Ignores `VSCODE_PORTABLE`. | Honor `VSCODE_PORTABLE` when set. |
| L5 | npm/pip cargo path fields | `*_path: Option` fields never initialized (works via fallback, but the subprocess prune never runs). | Initialize via `which(...)` in `new()` so the official prune/clean runs first. |
| L6 | workspace | ~18 clippy warnings (`unused import`, `needless default`, `collapsible if`, `&PathBuf`→`&Path`, `too many arguments`, etc.). | Fix all; restore **`-D warnings`** cleanliness. |

---

## 3. Test Plan

New/updated tests accompany each fix. Targets:

- **never_delete:** mixed-case parent path (`C:\WINDOWS\System32` vs scan of `c:\`),
  ensure protection triggers (C1).
- **cleanup:** `scan_id` round-trips from input scan to `CleanupResult` (C3).
- **cargo cleaner:** does **not** depend on CWD; asserts only registry/git dirs targeted (C4).
- **service_manager:** start-type parsing + `can_delete` truthfulness (M2).
- **tool_detector:** `get_file_version` returns `Some` for a known system exe; nuget
  version parsing of a sample banner (M3).
- **scanner:** `max_file_size` filter excludes oversized files (M7); `items_scanned`
  is reported (M6).
- **config:** serialization round-trip still green after field wiring (M7).
- **pkg managers:** maven `M2_HOME`, conda expansion skip, vscode portable path (L1/L3/L4).
- **CLI:** keep NDJSON test; add a headless unit test that `start_scan` populates
  results from a temp tree (H1) without entering raw-mode TUI.

Gate (must pass):

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test  --workspace --all-features
cargo build --release -p winsweep-gui --features system-tray
cargo build --release -p winsweep-cli
```

---

## 4. Documentation Updates

- **CHANGELOG.md** — add an *Unreleased* / `1.0.1` section enumerating every fix
  (security, CLI completeness, WSL/service lifecycle, detector implementations).
- **README.md** — correct the test count and the "33 package managers" / structure
  blurbs to match reality; note CLI feature parity.
- **docs/Clean-Build-Status.md** — refresh once clippy is truly zero again.
- **docs/api-reference.md / developer-guide.md** — document the new
  `cleanup(scan_id, …)` signature, `ServiceManager` lifecycle methods, and the
  hardened IPC DACL.
- Retire/annotate inaccurate claims so the docs stop overstating completeness.

---

## 5. Execution Order

1. Critical correctness/security (C1–C4) + their tests.
2. winsweep-core medium gaps (M1–M8) + tests.
3. Package-manager polish (L1–L5).
4. CLI handler wiring (H1–H11) + headless test.
5. Clippy zero-out (L6) and `cargo fmt`.
6. Tests green across the matrix; release builds.
7. Documentation refresh (§4).
8. Final full-gate verification.

---

## 6. Definition of Done

- ✅ `cargo fmt --check`, `clippy -D warnings`, full test suite, and both release
  builds all pass.
- ✅ Zero `// In a real implementation` / `// Implementation would` stubs remain in
  shipping code paths.
- ✅ Every CLI action performs real work (or a faithful `--dry-run` preview).
- ✅ Plan-mandated WSL/service/updater behaviors implemented.
- ✅ NEVER_DELETE is case-correct; IPC DACL is least-privilege; `CleanupResult`
  carries the right `scan_id`; cargo cleaner targets the cache, not the CWD.
- ✅ CHANGELOG + README + status docs reflect reality.
