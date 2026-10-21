# Contributing

## Workflow

- Keep `main` working and submit changes through pull requests.
- Use short-lived branches such as `feat/open-image`, `fix/zoom-anchor`, or `chore/ci`.
- Keep commits focused on one logical change, with concise English titles.
- Describe user-visible changes, validation, and known limitations in pull requests.
- Prefer squash merges after checks pass and delete completed branches.
- Use version tags such as `v0.1.0` for releases.
- Summarize user-visible changes under `Unreleased` in [CHANGELOG.md](CHANGELOG.md).
- Label feature and fix PRs with `enhancement` or `bug`; use `ignore-for-release`
  for changes that should be omitted from the generated release notes.
- Write source code, UI text, documentation, commit messages, and pull requests in English.

## License

Original contributions are accepted under GPL-3.0-only. Retain existing copyright
and license notices, and identify the origin and license of any third-party code
or assets. See [LICENSE](LICENSE) and [BRANDING.md](BRANDING.md).

## Validation

Run the following before submitting changes:

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Windows CI runs the same checks on pull requests and pushes to `main`.
Add behavior tests where appropriate and manually verify graphical changes.
See [the release guide](docs/development.md#releases) for previewing packages and
preparing draft releases.
