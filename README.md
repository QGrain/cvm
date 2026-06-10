# cvm

[中文](README_CN.md)

`cvm` is a per-user compiler version manager for LLVM and GCC.

It installs compiler toolchains from source, switches the active compiler in
the current shell, records persistent defaults, and keeps system compilers
untouched.

## Installation

Install a release:

```sh
curl -fsSL https://raw.githubusercontent.com/QGrain/cvm/v0.0.2/install.sh | bash
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

## Quick Start

```sh
cvm install llvm 21.1.8 -j8
cvm install gcc 15.1.0 -j8

cvm ls-remote llvm
cvm ls

cvm use llvm 21.1.8
cvm alias default llvm 21.1.8

cvm version
cvm upgrade --dry-run
```

When the first managed version of a compiler family is installed, cvm sets it
as the persistent default automatically.

## Commands

```text
cvm install <llvm|gcc> <version> [-jN|--jobs N]
cvm ls-remote [llvm|gcc]
cvm ls [llvm|gcc]
cvm use <llvm|gcc> [version]
cvm alias default <llvm|gcc> <version>
cvm current [llvm|gcc]
cvm env <llvm|gcc> [version]
cvm uninstall <llvm|gcc> <version>
cvm upgrade [version] [--dry-run]
cvm init
cvm version
```

In interactive shells that source `$CVM_HOME/cvm.sh`, `cvm use ...` updates the
current shell like `nvm`. In scripts or one-off shells, use:

```sh
eval "$(cvm use llvm 21.1.8)"
```

## Documentation

- [Design notes](docs/design.md)
- [Release process](docs/release.md)
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
