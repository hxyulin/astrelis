# Contributing to Astrelis

Astrelis is a Rust rendering API built directly on wgpu, plus an optional winit
integration crate. Read the [guide](docs/guide.md) first, then the contract for
the area you are changing. Small fixes, new examples and clear bug reports are
all useful contributions.

## Set up

- Rust stable with `rustfmt` and `clippy`. `rust-toolchain.toml` selects these.
  The minimum supported version is the `rust-version` in [Cargo.toml](Cargo.toml),
  currently 1.98.1, and CI checks it.
- A GPU with a Metal, Vulkan or DX12 driver. Tests render headlessly and read
  pixels back. On a Linux machine without a GPU, Mesa's software Vulkan driver
  (lavapipe, in the `mesa-vulkan-drivers` package) is enough; CI uses it.
- [prek](https://prek.j178.dev) for Git hooks. It reads [prek.toml](prek.toml).
  [typos](https://github.com/crate-ci/typos) runs through prek, so you don't
  need to install it separately.

```sh
git clone https://github.com/hxyulin/astrelis.git
cd astrelis
prek install
cargo test --workspace
cargo run -p astrelis-winit --example runner_triangle
```

`prek install` sets up the hooks for both commit and push. On commit, they fix
whitespace and line endings, check TOML, YAML and JSON syntax, check spelling,
and run `cargo fmt`. On push, they run clippy with warnings denied.
`prek run --all-files` runs the commit hooks over the whole tree.

## Find the right source

| Location | Purpose |
| --- | --- |
| `crates/astrelis/src/` | Context, targets, frames and passes, every renderer, Painter, text and the WGSL shaders |
| `crates/astrelis/examples/` | Standalone, copyable programs, one window lifecycle per file |
| `crates/astrelis/benches/` | Headless benchmarks that check their output before timing |
| `crates/astrelis/tests/fonts/` | Licensed test fonts with pinned sources and hashes |
| `crates/astrelis-winit/` | `WindowContext`, the desktop `Runner` and their examples and benchmarks |
| `docs/` | The guide, per-area contracts and the foundation criteria |
| `docs/performance/` | Benchmark reports and the raw runs behind them |

## Validate a change

Run these before opening a pull request. CI runs the same checks:

```sh
prek run --all-files                                    # formatting, typos, file checks
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Public items need documentation (`missing_docs` is on), and `unsafe` code is
forbidden in the workspace. When you fix a bug, add a test that fails without
the fix. Rendering tests should read pixels back rather than restate the
implementation. Compile-fail doc tests are the place to check lifetime and
borrowing rules.

For a visible change, run the examples it affects on your platform and compare
before and after. Include the backend and GPU when you report the result.

## Performance changes

Benchmarks live in each crate's `benches/` and run headless, for example:

```sh
cargo bench -p astrelis --bench primitives -- --counts 100,1000,10000 --samples 40 --warmup 8
```

When a change touches a measured path, rerun its benchmark and update the
matching report under `docs/performance/`. Commit the raw CSV and log files with
the machine metadata, as the existing reports do. Name the machine and backend,
compare against the previous result, and keep the measurement setup unchanged
unless the change is about the measurement itself.

## Examples and documentation

An example is one file that someone can copy into their own project. It owns its
window, resize, redraw scheduling and surface-loss handling, with no shared
support module and no smoke-test mode. Keep `winit` and `pollster` as
development dependencies of the core crate.

Update the contract in `docs/` when the behavior it describes changes. The
README is the entry point. Put detailed material in [docs/guide.md](docs/guide.md)
or the per-area documents.

## Issues and pull requests

Use the issue forms for bugs and feature requests. For a rendering bug, include
the smallest reproducing code, the expected and actual result, a screenshot, and
your OS, GPU, driver and wgpu backend.

Keep pull requests focused. Describe the problem and the resulting behavior, list
the checks you ran, and attach before and after screenshots for visible changes.
Explain any tradeoff a reviewer has to weigh. The pull request template gives
the structure; delete sections that don't apply.

## Licensing

Astrelis is dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
Unless you explicitly state otherwise, any contribution you intentionally submit
for inclusion, as defined in the Apache-2.0 license, is dual licensed under both,
without any additional terms or conditions. Third-party material keeps its own
license; the OFL test fonts are an example.
