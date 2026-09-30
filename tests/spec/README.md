# Vendored TOON specification conformance fixtures

These files are copied verbatim from the official TOON specification repository:

- Repository: https://github.com/toon-format/spec
- Tag: `v4.1.1`
- Commit: `62f16b369408180f1faf1cba7da1b46d1f336f12`
- Paths: `tests/fixtures/**` and `tests/fixtures.schema.json`

They are licensed under the MIT License, Copyright (c) Johann Schopplich (see `LICENSE-MIT`).

They are exercised by `tests/spec_conformance.rs`. To update, replace the
`fixtures/` directory with the one from a newer spec tag, update the tag and
commit above, and bump `SPEC_VERSION` in `src/lib.rs` if the targeted spec
version changed.
