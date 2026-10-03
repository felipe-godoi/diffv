# Contributing to diffv

Thank you for your interest in contributing to `diffv`! We welcome contributions of all kinds: bug reports, documentation improvements, feature requests, and code contributions.

Please take a moment to review this document to make the contribution process smooth and effective for everyone.

---

## 📜 Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md). Please report any unacceptable behavior to the project maintainers.

---

## 🌿 Branching Model & Release Strategy

We follow a Trunk-Based development model with an automated release pipeline powered by **Google Release Please**:

- **`main`**: The primary branch for daily development. Pull requests should target `main`. Changes merged here automatically publish continuous `nightly` builds.
- **`beta`**: The staging branch used for Release Candidates (RCs) and release stabilization.
- **Feature Branches**: Create your branch off `main` with a descriptive name (e.g. `feat/syntax-caching` or `fix/tmux-popup-bounds`).

For an in-depth breakdown of the release lifecycle, see [docs/release-flow.md](docs/release-flow.md).

---

## 💬 Commit Message Convention

`diffv` uses **[Conventional Commits](https://www.conventionalcommits.org/)** to automate semantic versioning and changelog generation via Release Please.

All commit messages in your pull request must follow this specification:

```text
<type>(<optional scope>): <description>

[optional body]

[optional footer(s)]
```

### Supported Types:
- `feat`: A new user-facing feature (triggers a `minor` version bump).
- `fix`: A bug fix (triggers a `patch` version bump).
- `perf`: A code change that improves performance.
- `docs`: Documentation changes only.
- `refactor`: A code change that neither fixes a bug nor adds a feature.
- `test`: Adding missing tests or correcting existing tests.
- `ci`: Changes to CI/CD workflows and deployment scripts.
- `chore`: Maintenance tasks, dependency bumps, or tool configuration.

### Examples:
- `feat(ui): add visual indicators for staged hunks`
- `fix(watcher): debounce file events to prevent double reload`
- `docs: update installation instructions for Arch Linux`

---

## 🛠️ Local Development Setup

### Prerequisites
- **Rust**: Latest stable toolchain (install via [rustup](https://rustup.rs/)):
  ```bash
  rustup update stable
  rustup component add clippy rustfmt
  ```
- **Git** 2.20+

### Building & Running
```bash
# Clone the repository
git clone https://github.com/felipe-godoi/diffv.git
cd diffv

# Build in debug mode
cargo build

# Run against current directory
cargo run -- --no-update

# Run on a specific commit or branch
cargo run -- HEAD~1
```

### Pre-commit Verification
Before opening a pull request, ensure that your code passes all local automated checks:

```bash
# 1. Run unit and integration tests
cargo test --locked

# 2. Run Clippy linter with strict warning checks
cargo clippy --all-targets --locked -- -D warnings

# 3. Check code formatting
cargo fmt --check
```

---

## 🎨 Design Principles

When contributing code to `diffv`, keep these core architectural goals in mind:

1. **Instant Startup & Responsiveness**: `diffv` is meant to be opened dozens of times a day via terminal popups and hotkeys. Startup time should stay minimal (< 10ms).
2. **Zero-Allocation Hot Paths**: Diff rendering, intra-line token calculation, and tree layout should minimize heap allocations during scrolling and resize events.
3. **Non-Blocking Architecture**: File watching, update checks, and git operations must never freeze the terminal event loop or stutter rendering.
4. **Resilient Terminal Handling**: Always ensure the terminal state (alternate screen, raw mode, mouse capture) is restored gracefully on exit or panic.

---

## 🚀 Submitting a Pull Request

1. **Fork & Branch**: Fork the repo and create your feature branch from `main`.
2. **Implement & Test**: Write clean, idiomatic Rust code and include unit/integration tests for any new behavior or bug fixes.
3. **Format & Lint**: Run `cargo fmt` and verify `cargo clippy` has zero warnings.
4. **Commit**: Use Conventional Commit messages.
5. **Open PR**: Submit your pull request targeting the `main` branch. Fill out the pull request template with a clear explanation of what changed and how you tested it.
