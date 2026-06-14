# Contributing to WinSweep

Thank you for your interest in contributing! This document covers everything
you need to get started.

---

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [Making Changes](#making-changes)
5. [Pull Request Checklist](#pull-request-checklist)
6. [Bug Reports](#bug-reports)
7. [Feature Requests](#feature-requests)
8. [Security Issues](#security-issues)
9. [License](#license)

---

## Code of Conduct

Be respectful and constructive. We follow the
[Contributor Covenant](https://www.contributor-covenant.org/) v2.1.

---

## Getting Started

1. Fork the repository and clone your fork:
   ```powershell
   git clone https://github.com/<your-username>/winsweep.git
   cd winsweep
   ```
2. Create a branch for your change:
   ```powershell
   git checkout -b feat/my-awesome-feature
   ```
3. Make your changes (see below).
4. Push and open a Pull Request against `main`.

---

## Development Setup

### Requirements

| Tool | Version | Install |
|---|---|---|
| Rust | 1.75+ | [rustup.rs](https://rustup.rs/) |
| Target | x86_64-pc-windows-gnu | `rustup target add x86_64-pc-windows-gnu` |
| MinGW-w64 | Any | [winlibs.com](https://winlibs.com/) or via scoop/chocolatey |
| NSIS | 3.x | [nsis.sourceforge.io](https://nsis.sourceforge.io/) — for installer only |

The project is Windows-only; all development should be done on Windows 10 or 11.

### Build Commands

```powershell
cargo build                             # debug, all crates
cargo build -p winsweep-gui             # GUI only
cargo build -p winsweep-gui --features system-tray  # with tray icon
cargo build --release -p winsweep-gui --features system-tray
cargo build --release -p winsweep-cli
```

### Running Tests

```powershell
cargo test --workspace                  # all tests
cargo test -p winsweep-core             # core only
cargo test --test integration_tests     # integration suite
```

### Linting

```powershell
cargo fmt --all                         # auto-format
cargo clippy --all-targets --all-features -- -D warnings
```

---

## Making Changes

### Coding Conventions

- **Formatting**: `rustfmt` with default settings.  Run `cargo fmt --all` before committing.
- **Lints**: zero clippy warnings (`-D warnings` in CI).
- **Error handling**: `anyhow::Result` in binaries / GUI, `thiserror` in library crates.
- **Logging**: `tracing::{debug, info, warn, error}` — never `println!` in library code.
- **Unsafe**: allowed only for Windows API calls that have no safe wrapper.
  Document every `unsafe` block.
- **Doc comments**: every public item in library crates needs `///` documentation.
- **Tests**: every new function / module should have unit tests. Use
  `tempfile::TempDir` for filesystem tests.

### Adding a Package Manager

See [Developer Guide — Adding a Package Manager](docs/developer-guide.md#adding-a-package-manager).

### Adding a New View

See [Developer Guide — Adding a New View](docs/developer-guide.md#adding-a-new-view).

### Modifying the NEVER_DELETE List

**This requires extra care.** The `NEVER_DELETE` set in `winsweep-common`
protects critical system paths from accidental deletion. Any change here must:

1. Have an extremely strong justification in the PR description.
2. Be reviewed by at least two maintainers.
3. Include a test in `integration_tests.rs` that explicitly verifies the new
   entry is protected.

---

## Pull Request Checklist

Before submitting a PR, confirm all of these:

- [ ] `cargo fmt --all` — no diffs
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` — no warnings
- [ ] `cargo test --workspace` — all tests pass
- [ ] New functionality has tests
- [ ] Public API changes are documented in `docs/api-reference.md`
- [ ] User-visible changes are noted in `CHANGELOG.md`
- [ ] PR description explains *what* changed and *why*

---

## Bug Reports

Use the [GitHub Issues](https://github.com/N1KH1LT0X1N/WinSweep/issues) template.
Please include:

- WinSweep version (`winsweep-gui --version`)
- Windows version and build number
- Steps to reproduce
- Expected vs. actual behaviour
- Logs from `%LocalAppData%\WinSweep\logs\winsweep.log` (if relevant)

---

## Feature Requests

Open a [GitHub Discussion](https://github.com/N1KH1LT0X1N/WinSweep/discussions)
before opening an issue for a new feature. Discuss the use-case and design
first — this saves time for everyone.

---

## Security Issues

**Do not open public issues for security vulnerabilities.**

Use GitHub's **"Report a vulnerability"** button (Security → Advisories → New draft security advisory) instead of opening a public issue. Include:
- A description of the vulnerability
- Steps to reproduce
- Potential impact

We will respond within 48 hours and work with you on a coordinated disclosure.

---

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE).
