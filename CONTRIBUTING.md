# Contributing

Thanks for helping out. This document covers the checks your change has to
pass, the conventions that are not obvious from reading the code, and how
releases happen.

## Prerequisites

- Rust stable (edition 2024 — the project is developed against 1.94).
- Linux with **cgroup v2** (unified hierarchy) if you want the full test suite
  to do anything. Tests that need a live cgroup skip themselves elsewhere, so
  they pass on macOS but verify nothing there. Check with:

  ```bash
  stat -fc %T /sys/fs/cgroup   # must print: cgroup2fs
  ```

## The checks

All three must pass before a pull request can merge. Run them locally first —
it is faster than waiting for CI.

**Lint.** This is the one CI enforces, and it is stricter than a bare
`cargo clippy`:

```bash
cargo clippy --all-targets --all-features --locked -- -D warnings
```

`--all-targets` matters because every test lives in `tests/`; without it clippy
never looks at them. `-D warnings` turns a lint into a failure instead of
something that scrolls past in the log.

**Format:**

```bash
cargo fmt --all
```

**Tests:**

```bash
cargo test
```

If clippy suggests a rewrite that would change behaviour, do not just take it.
`rate()` in `src/utils/parse.rs` is guarded with `!elapsed_secs.is_finite()`
rather than `elapsed_secs <= 0.0` precisely because the latter is `false` for
NaN; clippy has flagged that shape before. Prefer restructuring so the lint is
satisfied *and* the behaviour is right, over adding `#[allow]`.

## Project layout

```
src/
  cli.rs            argument parsing, metric selection
  main.rs           argument dispatch and exit codes
  utils/
    parse.rs        cgroup file-format parsers, rate arithmetic
    bytes.rs        IEC byte formatting
    cgfile.rs       file presence, device naming, path normalisation
  task/
    mod.rs          Stats, collection orchestration, sampling
    metrics.rs      data model and the four collectors
    format.rs       human / JSON / table renderers
tests/
  common/mod.rs     fixtures shared between test binaries
  <unit>.rs         one file per unit under test
```

Pure helpers belong in `src/utils/`. `src/task/` is for the data model, the
collectors that talk to the kernel, and the renderers.

## Commits

The project uses [Conventional Commits](https://www.conventionalcommits.org/).
The version bump is derived from your commit messages, so the prefix decides
what ships:

| Prefix | Effect | Changelog section |
|---|---|---|
| `feat:` | minor bump | ✨ Features |
| `fix:` | patch bump | 🐛 Bug Fixes |
| `perf:` | patch bump | ⚡️ Performance Improvements |
| `revert:` | patch bump | ⏪️ Reverts |
| `docs:` | no bump | hidden |
| `chore:`, `ci:`, `test:`, `refactor:`, `style:` | no bump | hidden |

A `!` after the type (`feat!:`) or a `BREAKING CHANGE:` footer triggers a major
bump.

Write the body to explain *why*, not what — the diff already says what.

## Pull requests

- Branch from `main` and target `main`.
- **Do not bump the version by hand.** `release-it` writes it into `Cargo.toml`
  and `CITATION.cff` automatically; a manual bump will conflict.
- Put `[skip-release]` in the PR **title** if the change should merge without
  cutting a release.
- Keep the working tree clean of generated files. `docs/` is gitignored on
  purpose.