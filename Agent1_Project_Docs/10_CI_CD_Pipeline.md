# CI/CD Pipeline

## CI Goals

- Verify builds.
- Run tests.
- Check formatting.
- Check linting.
- Validate docs.
- Build release artifacts.

## Required CI Jobs

## Rust Format

```bash
cargo fmt --all -- --check
```

## Rust Lint

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

## Rust Tests

```bash
cargo test --workspace
```

## SQLx Check

```bash
cargo sqlx prepare --workspace --check
```

## Frontend Tests

```bash
npm test
```

## Frontend Build

```bash
npm run build
```

## Tauri Build

```bash
cargo tauri build
```

## Security Checks

Recommended:

```bash
cargo audit
cargo deny check
```

## Release Pipeline

On version tag:

1. Run all CI checks.
2. Build Linux artifact.
3. Build Windows artifact.
4. Build macOS artifact.
5. Generate checksums.
6. Publish GitHub release.
7. Attach artifacts.
8. Publish docs.

## Branch Rules

- `main` must always build.
- Pull requests require tests.
- Releases are created from tags.
- Migrations require review.

## Version Tag Format

```text
v0.1.0
```

## Artifacts

Release artifacts:

- Windows installer
- macOS dmg/app
- Linux AppImage/deb
- CLI binaries
- Checksums
- Source archive
