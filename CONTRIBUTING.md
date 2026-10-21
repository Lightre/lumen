# Contributing

## Workflow

- Keep `main` working and submit changes through pull requests.
- Use short-lived branches such as `feat/open-image`, `fix/zoom-anchor`, or `chore/ci`.
- Keep commits focused on one logical change, with concise English titles.
- Describe user-visible changes, validation, and known limitations in pull requests.
- Prefer squash merges after checks pass and delete completed branches.
- Use version tags such as `v0.1.0` for releases.
- Write source code, UI text, documentation, commit messages, and pull requests in English.

## Validation

Run the following before submitting changes:

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Windows CI runs the same checks on pull requests and pushes to `main`.
Add behavior tests where appropriate and manually verify graphical changes.
