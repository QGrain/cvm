# cvm Coding Agent Instructions

This document provides guidance for AI coding agents working on cvm.

## Overview

cvm is a per-user compiler version manager for LLVM and GCC. It is implemented
as a Rust CLI plus a generated shell loader. Compiler toolchains are built from
source by backend shell scripts and installed under `$CVM_HOME`, which defaults
to `$HOME/.cvm`.

## Core Architecture

- `src/lib.rs`: command parsing, version resolution, environment generation,
  remote index loading, build profile parsing, and command implementations.
- `install.sh`: nvm-style installer that installs cvm under `$HOME/.cvm`,
  generates `cvm.sh`, and updates the detected shell profile.
- `scripts/`: backend source build scripts for LLVM and GCC. Keep their
  defaults kernel-oriented and directly runnable outside cvm.
- `manifests/remote-index.json`: compiler release metadata and latest cvm
  release metadata used by `ls-remote`, `version`, and `upgrade`.
- `tools/update_remote_index.py`: maintainer tool used by the remote index
  synchronization workflow.
- `tests/`: Rust integration tests for CLI behavior, docs contracts, scripts,
  and core parsing.
- `docs/`: design, release, troubleshooting, build profile, and contribution
  documentation.

## Command Model

Primary user commands include:

- `cvm install <llvm|gcc> <version-or-prefix>`
- `cvm use <llvm|gcc> [version-or-prefix]`
- `cvm env <llvm|gcc> [version-or-prefix]`
- `cvm alias default <llvm|gcc> <version-or-prefix>`
- `cvm ls [llvm|gcc]`
- `cvm ls-remote [llvm|gcc] [prefix]`
- `cvm which <llvm|gcc> [version-or-prefix]`
- `cvm current [llvm|gcc]`
- `cvm version`
- `cvm upgrade [version] [--dry-run]`
- `cvm profile template <llvm|gcc> [PATH] [--force]`
- `cvm profile list`
- `cvm init`

Interactive shells source `$CVM_HOME/cvm.sh`, which wraps `cvm use` so it can
modify the current shell. Scripts can use `eval "$(cvm use llvm 21)"`.

## Build Backends

Build scripts are embedded into the Rust binary and materialized under
`$CVM_HOME/scripts` at install time. They should remain portable Bash scripts
with conservative defaults:

- LLVM defaults to X86, `clang;lld;compiler-rt`, and kernel-oriented utilities.
- GCC defaults to C/C++, no multilib, and no bootstrap.
- User customization should go through `$CVM_HOME/profiles/build/<tool>/default.toml`,
  explicit `--profile PATH`, and documented environment contracts, not arbitrary
  `--` passthrough.

## Testing

Before submitting changes, run:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
bash -n install.sh
bash -n scripts/build_llvm-project.sh
bash -n scripts/build_gcc.sh
python3 -m py_compile tools/update_remote_index.py
python3 -m json.tool manifests/remote-index.json
```

Prefer tests for user-visible behavior over brittle implementation assertions.
Avoid tests that require exact GitHub Actions runner labels, exact action
versions, or incidental README wording unless those are deliberate contracts.

## Contribution Notes

- Keep unrelated changes in separate commits.
- Update `README.md` and `README_CN.md` together for user-facing changes.
- Keep root READMEs concise; put detailed design and maintainer notes under
  `docs/`.
- Do not add network dependencies to ordinary compiler switching paths.
- Preserve installed toolchains and user defaults during cvm upgrades.
