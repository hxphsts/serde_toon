## Description

Brief description of changes.

## Checklist

Each item matches a CI job; the command is what to run locally.

- [ ] Tests pass (`cargo test --all`)
- [ ] Clippy passes (`cargo clippy --all-targets -- -D warnings`)
- [ ] Formatting checked (`cargo fmt --all -- --check`)
- [ ] Docs build (`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`)
- [ ] No public API break (`cargo semver-checks check-release --release-type minor`)
- [ ] MSRV holds (`cargo +1.70 check --lib`, see the `msrv` job in `ci.yml` for the lockfile recipe)
- [ ] Crate packages (`cargo package`)
- [ ] Documentation updated
- [ ] CHANGELOG.md updated

## Testing

Describe tests run and how to reproduce.
