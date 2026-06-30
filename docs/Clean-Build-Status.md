# WinSweep Clean Build Status

**Date:** 2026-06-30

## Summary

The WinSweep workspace compiles with **zero Clippy warnings** and **all tests passing**.

## Verification Commands

```bash
# Zero warnings / zero errors
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# All tests pass
cargo test --workspace --all-features

# Release builds
cargo build --release -p winsweep-gui --features system-tray
cargo build --release -p winsweep-cli
```

## Results

| Check | Status | Details |
|-------|--------|---------|
| Format (`--check`) | ✅ Clean | 0 diffs |
| Clippy (`-D warnings`) | ✅ Clean | 0 warnings, 0 errors |
| Tests (`--all-features`) | ✅ Passing | 157 passed, 1 ignored |
| Release GUI + tray | ✅ Built | target/x86_64-pc-windows-gnu/release/winsweep-gui.exe |
| Release CLI | ✅ Built | target/x86_64-pc-windows-gnu/release/winsweep.exe |

### Test Breakdown

| Crate | Passed | Ignored |
|-------|--------|---------|
| `winsweep` (root) | 0 | 0 |
| `winsweep-common` | 16 | 0 |
| `winsweep-core` (lib) | 93 | 1 |
| `winsweep-core` (scanner_tests) | 9 | 0 |
| `winsweep-cli` | 0 | 0 |
| `winsweep-gui` | 27 | 0 |
| Integration (`integration_tests.rs`) | 12 | 0 |
| **Total** | **157** | **1** |

## Notes

- All critical (C2–C4), CLI handler (H1–H11), and medium (M1–M7) gaps from the
  completion plan are implemented and covered by tests.
- The one ignored test (`ipc::tests::test_ipc_server_client`) requires admin
  privileges and a stable named-pipe handle setup; it is intentionally skipped
  in normal CI runs.
- The IPC server now forwards non-Ping messages to a caller-accessible
  `incoming_receiver()` channel instead of silently dropping them.
- The self-updater correctly queries the `N1KH1LT0X1N/WinSweep` GitHub repository.
- WSL distribution detection uses real `HKLM\...\Lxss` registry subkey enumeration
  with a known-distro fallback.
