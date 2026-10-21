# Contributing

## Git and GitHub

- Keep `main` working.
- Use a short-lived branch for each meaningful task: `feat/open-image`, `fix/zoom-anchor`, or `chore/ci`.
- Each commit should contain one logical change. Use concise English titles such as `feat: open local images`.
- Once the GitHub remote is connected, merge changes through pull requests.
- PR descriptions should explain the result, validation, and known limitations.
- Prefer squash merges after checks pass; delete completed branches.
- Tag releases using versions such as `v0.1.0`. Release packaging will be added later.

The initial setup commits were created directly on `main`. The private GitHub remote is connected, and Windows CI runs on pushes to `main` and on pull requests. Future changes should use feature branches and pull requests. Branch protection and automatic merging have not been configured.

## Validation

Before opening a PR, run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`.
GitHub Actions runs the same checks on Windows. Add meaningful behavior tests as features are implemented; check the graphical interface manually as well.

## Collaboration

Start with one agent. Use subagents when requested by the user. Keep tasks small and reviewable. Explain technical decisions and relevant Rust concepts briefly during development.

Keep source code, UI text, documentation, commit messages, and PR content in English. Conversation with the project owner continues in Turkish.
