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

There is a git hook that runs this for you. Enable it once per clone:

```bash
git config core.hooksPath .githooks
```

It formats the tree and re-stages the Rust files it changed. A file that has
*unstaged* edits as well as staged ones is formatted but deliberately left
unstaged, so your uncommitted work is never swept into a commit — the hook
prints which files that applied to.

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

## Conventions

These are enforced by review, and several exist because of bugs that already
shipped once.

**Tests live in `tests/`, never in `#[cfg(test)]` modules.** There are no unit
test modules in `src/`; please do not add any. Note the consequence: `tests/`
sees only the public API, so a new private helper cannot be unit-tested
directly — test it through the public function that calls it.

**Never `.unwrap()` or `.expect()` on anything derived from a cgroup file.**
These files can vanish mid-read when a container dies, and their contents come
from the kernel, not from you. Return a `Result` and let the caller decide.

**A limit is `Option<T>`, where `None` means unlimited.** This maps onto the v2
files, where an absent limit is the literal string `max`. It holds in every
struct and every renderer, and serialises to `null` in JSON.

**A metric field is `Option<Result<T, String>>`.** `None` means the metric was
not requested, `Err` means it was requested but is unavailable, and `Ok` is a
reading. All three render differently, and that distinction is the point — see
the next item.

**Check that a metric's files exist before reading through `cgroups-rs`.** The
crate cannot report a failed read: `memory_stat()` returns `usage_in_bytes == 0`
for a missing `memory.current`, and `get_mem()` maps a failed read of
`memory.max` onto `MaxValue::Max`, which is byte-identical to a genuine
"unlimited". Without the `require()` precheck the root cgroup reports a
confident `0 / unlimited` instead of `n/a`.

**`anyhow` and `humansize` are deliberately excluded.** Errors use
`Box<dyn std::error::Error>`; byte formatting is hand-written in
`src/utils/bytes.rs`. Please do not reintroduce either.

**Exit codes:** `1` for a runtime failure, `2` for an argument error (clap's
default — do not remap it).

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

## Releases

Merging a pull request into `main` runs `.github/workflows/release.yml`, which
derives the version from the commit history, creates a draft GitHub release,
then builds and attaches:

- macOS tarballs for `aarch64-apple-darwin` and `x86_64-apple-darwin`
- Linux tarballs and `.deb` packages for `x86_64` and `aarch64`
- an apt repository published to the `gh-pages` branch

The release is only published once every one of those succeeds.

## Reporting bugs

Include the output of the command you ran, plus:

```bash
stat -fc %T /sys/fs/cgroup && cat /sys/fs/cgroup/cgroup.controllers
```

If the problem involves a specific cgroup, the contents of its `cpu.max`,
`cpu.stat`, `memory.current`, `memory.high`, `memory.max` and `io.stat` are
what make it reproducible.
