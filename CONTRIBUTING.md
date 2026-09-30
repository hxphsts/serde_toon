# Contributing

## Checks

CI (`.github/workflows/ci.yml`) runs these on every pull request; run them
locally before opening one:

```sh
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo semver-checks check-release --release-type minor   # no public API break
cargo package
```

The MSRV is Rust 1.70; the `msrv` job in `ci.yml` shows how to resolve a
lockfile that 1.70 can build.

**The public API never breaks.** `cargo-semver-checks` runs with
`--release-type minor`, so even a 0.x minor bump must be non-breaking. New
behavior goes behind additive API (new functions, new option constructors,
new trait impls), and `tests/api_surface.rs` fails to compile if an existing
item changes.

## Releasing

Releases are cut by pushing a version tag; `.github/workflows/release.yml`
does the rest.

1. On a branch, bump `version` in `Cargo.toml` and add a
   `## [X.Y.Z] - YYYY-MM-DD` section to `CHANGELOG.md`. That section becomes
   the GitHub release notes verbatim, so write it for users.
2. Merge the pull request into `main`.
3. Tag the merge commit on `main` and push the tag:

   ```sh
   git switch main && git pull
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

The workflow then:

1. **gate**: stops unless the tagged commit is on `main` and the tag equals
   `v` + the `Cargo.toml` version.
2. **verify**, **semver**, **msrv** (in parallel): fmt, clippy, tests, docs and
   `cargo package`; the non-breaking API check against the latest release on
   crates.io; the Rust 1.70 build.
3. **publish**: in the `release` environment, publishes to crates.io using
   Trusted Publishing. A version already on crates.io is skipped, so
   re-running a tag is harmless.
4. **github-release**: creates the GitHub release from the tag, with the
   version's `CHANGELOG.md` section as notes and links to crates.io and docs.rs.

If a job fails, fix the cause on `main`, delete the tag (locally and with
`git push origin :refs/tags/vX.Y.Z`), and tag again. Nothing is published
before every check has passed.

### One-time setup

Publishing uses [crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing):
the workflow exchanges GitHub's OIDC token for a short-lived crates.io token,
so no registry token is stored in the repository.

- **crates.io**: on the `serde_toon` crate, Settings → Trusted Publishing →
  Add → GitHub, with owner `hxphsts`, repository `serde_toon`, workflow
  `release.yml`, environment `release`.
- **GitHub**: repository Settings → Environments → New environment named
  `release`. Under deployment branches and tags, allow only tags matching
  `v*`. Optionally add required reviewers to approve each publish.
- A `CARGO_REGISTRY_TOKEN` secret is not needed; remove any old one.
