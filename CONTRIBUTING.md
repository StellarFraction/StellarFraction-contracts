# Contributing
## Before You Start — Please Star the Repository

**We ask all contributors to star the repository before opening a pull request.**

Starring the repo is a small gesture that helps surface the project to more developers, shows you have engaged with the codebase, and helps maintainers prioritise active contributors in code review.

> **How to star:** Click the star button at the top-right of the GitHub repository page before you fork or open a PR. PRs from accounts that have not starred the repo may be deprioritised during review cycles.

---
 to StellarFraction

We are thrilled that you are interested in contributing to StellarFraction! This guide contains instructions on how to participate in our open-source ecosystem.

---

## Code of Conduct

Please be respectful, professional, and collaborative in all communication and contributions.

## How to Contribute

### 1. Find or Report an Issue
- Browse our open issues or create a new one to discuss changes you wish to introduce.
- Ensure that no one else is currently working on the same task.

### 2. Fork and Branch
- Fork the repository.
- Create a new feature or bugfix branch off the `main` branch:
  ```bash
  git checkout -b feature/your-feature-name
  # Or:
  git checkout -b fix/bug-description
  ```

### 3. Development Workflow

#### Rust (Smart Contracts)
- **Code Style:** Always format your code with rustfmt:
  ```bash
  cargo fmt
  ```
- **Linting:** Check for warnings and common mistakes with clippy:
  ```bash
  cargo clippy --all-targets -- -D warnings
  ```
- **Testing:** Any new logic or contract function *must* be accompanied by unit tests in the `src/test.rs` file. Verify that all tests pass:
  ```bash
  cargo test
  ```
- **Security auditing:** Before opening a PR, scan dependencies for known
  advisories and policy violations. Configuration lives in `.cargo/audit.toml`
  and `deny.toml`:
  ```bash
  cargo install cargo-audit cargo-deny   # one-time
  cargo audit                            # RustSec advisory scan
  cargo deny check                       # advisories, licenses, bans, sources
  ```

#### React (Frontend)
- **Code Style:** We use ESLint and Prettier. Run the linter to verify formatting:
  ```bash
  npm run lint
  ```
- **Simulations:** Ensure that any modifications to the layout are fully responsive and preserve the dark theme styling.

---

### 4. Commits Guidelines

We follow **Conventional Commits** for clean git history. Your commit messages should use this format:

- `feat: add landlord fee calculations to distribution contract`
- `fix: resolve precision rounding error in client claim`
- `docs: update setup steps in README`
- `test: add unit test for zero-shares distribute edge case`

---

### 5. Submitting a Pull Request (PR)

1. Push your branch to your forked repository.
2. Open a Pull Request pointing to the `main` branch of the official repo.
3. In the description, clearly outline:
   - What problems your PR solves.
   - Design choices or mathematical models implemented.
   - Proof that your tests are passing (copy test logs).
4. Address any feedback raised by repository maintainers during code review.

Thank you for helping make property investments accessible to everyone!
