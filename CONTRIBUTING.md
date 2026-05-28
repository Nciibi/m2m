# Contributing to M2M

Thank you for your interest in contributing to M2M Secure Messenger! We welcome contributions that align with our zero-trust, privacy-first vision.

## Security First
If you discover a security vulnerability, please do **NOT** open a public issue. Email the repository owner directly.

## Development Workflow

1. **Fork the Repository**: Clone your fork locally.
2. **Branching**: Create a feature branch (`git checkout -b feature/your-feature-name`).
3. **Dependencies**: 
   - Ensure `libsodium-dev` is installed on your system if you're not on Windows.
   - Run `pnpm install` in the root.
4. **Testing**: 
   - Ensure the Rust backend passes all tests: `cd src-tauri && cargo test`
   - Run the linter: `cargo clippy -- -D warnings`
5. **Commit Messages**: Write clear, descriptive commit messages.

## Code Style

- **Rust**: We follow standard `rustfmt` formatting. Run `cargo fmt` before committing.
- **Frontend**: We use modern React with Vite. Keep components functional and use hooks. Maintain the custom CSS styling variables in `App.css`.

## Architecture Principles

When submitting new features, ensure they do not violate the [Threat Model](docs/threat-model.md):
- Do not introduce telemetry or tracking.
- Do not write unencrypted sensitive data to the disk.
- Keys must be zeroized upon drop.

## Pull Requests

1. Keep pull requests focused on a single change or feature.
2. Ensure CI passes.
3. Request review from a maintainer.

Happy hacking! 🦀
