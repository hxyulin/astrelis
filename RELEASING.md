# Releasing Astrelis

Astrelis uses one synchronized version for its complete crate graph. The first
rewritten release is `0.3.0-rc.1`.

## Preparation

From a clean release commit, run:

```sh
cargo fmt --all --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
./scripts/release-astrelis.sh package
```

`package` creates and inspects every archive without registry-dependent build
verification. The publish command performs Cargo's full verification as each
dependency layer becomes available.

## Publishing

```sh
./scripts/release-astrelis.sh status
./scripts/release-astrelis.sh self-test
./scripts/release-astrelis.sh publish
```

The script publishes in dependency order and pauses for confirmation between
layers. It checks the exact version from a neutral directory before every
upload so a matching local workspace package cannot be mistaken for a
published crate. Rerunning after a rate limit or interrupted registry
propagation skips completed packages. An upload error stops immediately and is
never retried automatically.

After every package is visible, test a new application outside the workspace
using only crates.io dependencies. Then tag the published commit as
`v0.3.0-rc.1` and create the matching GitHub prerelease. Never tag before the
complete registry graph succeeds.
