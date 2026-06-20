<img src="assets/logos/cvm-logo-color.svg" alt="Compiler Version Manager" width="160">

<h1>Compiler Version Manager</h1>

<img src="https://img.shields.io/badge/version-v0.1.0-orange.svg" alt="Release Version"> <img src="https://img.shields.io/badge/license-Apache--2.0-green" alt="License"> <img src="https://img.shields.io/github/actions/workflow/status/QGrain/cvm/release.yml" alt="Release Workflow Status"> <img src="https://img.shields.io/github/downloads/QGrain/cvm/total" alt="Total Downloads">

[[中文]](README_CN.md)

`cvm` is a cross-platform C/C++ compiler version manager for LLVM and GCC.

Inspired by tools like nvm, rustup, and rbenv, cvm lets you install multiple
compiler toolchain versions, switch the active compiler in the current shell,
and set persistent defaults without overwriting system compilers.

cvm is designed for Linux kernel development, syzkaller workflows, compiler
testing, CI, and any C/C++ project that needs reproducible LLVM or GCC version
switching.

## Installation

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/QGrain/cvm/v0.1.0/install.sh | bash
```

Install from a local checkout:

```sh
git clone https://github.com/QGrain/cvm.git
cd cvm
./install.sh
```

The installer writes `cvm` to `$HOME/.cvm/bin/cvm`, generates
`$HOME/.cvm/cvm.sh`, and appends a small loader to the detected shell profile.
Set `PROFILE=/dev/null` to skip profile edits.

Running the installer again replaces only the cvm binary and regenerates
`cvm.sh`; installed toolchains and defaults under `$HOME/.cvm` are preserved.

`cvm install llvm ...` and `cvm install gcc ...` download the upstream release
key bundle into `$CVM_HOME/cache/keys`, import it with `gpg`, and verify source
archives with detached GPG signatures before building. The `install.sh`
bootstrapper does not verify cvm release assets automatically; users who want
to audit the bootstrap binary can manually verify the matching `.sig` asset
from the GitHub Release and compare it with the installed binary.

Compiler source builds bootstrap Debian/Ubuntu build dependencies with `apt`.
When cvm is not running as root, the backend scripts use `sudo apt update` and
`sudo apt install`; in root containers they call `apt` directly. Review the
backend scripts before running `cvm install` in locked-down environments.

## Quick Start

```sh
cvm install llvm 21 -j8
cvm install gcc 15 -j8

cvm ls-remote llvm 21
cvm ls
cvm cache list

cvm use llvm 21
cvm which llvm
cvm alias default llvm 21.1.8
cvm use system
cvm deactivate

cvm version
cvm upgrade --dry-run

cvm profile template llvm
cvm install llvm 21
```

When the first managed version of a compiler family is installed, cvm sets it
as the persistent default automatically.

## Commands

```text
cvm install <llvm|gcc> <version-or-prefix> [-jN|--jobs N] [--profile PATH] [--targets LIST]
cvm cache dir
cvm cache list
cvm cache prune [--older-than 14d]
cvm profile template <llvm|gcc> [PATH] [--force]
cvm profile list
cvm ls-remote [llvm|gcc] [prefix]
cvm ls [llvm|gcc]
cvm use <llvm|gcc|system> [version-or-prefix]
cvm alias default <llvm|gcc> <version-or-prefix>
cvm current [llvm|gcc]
cvm env <llvm|gcc> [version-or-prefix]
cvm which <llvm|gcc> [version-or-prefix]
cvm uninstall <llvm|gcc> <version-or-prefix>
cvm deactivate
cvm upgrade [version] [--dry-run]
cvm init
cvm version
```

In interactive shells that source `$CVM_HOME/cvm.sh`, `cvm use ...` updates the
current shell like `nvm`. The same shell loader registers bash/zsh completion
when the shell supports it. `cvm use system` and `cvm deactivate` return the
current shell to system compiler resolution without changing persistent
defaults. In scripts or one-off shells, use:

```sh
eval "$(cvm use llvm 21)"
eval "$(cvm use system)"
```

## Documentation

- [Design notes](docs/design.md)
- [Build profiles](docs/build-profiles.md)
- [Source cache](docs/cache.md)
- [Release process](docs/release.md)
- [Release signing](docs/signing.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Contribution guide](docs/contribution.md)

## Uninstalling

Remove the profile snippet that sources `$CVM_HOME/cvm.sh`, then remove:

```sh
rm -rf "$HOME/.cvm"
```

## Contributing

Contributions are welcome. Please read the
[contribution guide](docs/contribution.md) before opening a PR.

## License

Apache-2.0. See [LICENSE](LICENSE).
