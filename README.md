# cvm

[中文](README_CN.md)

`cvm` is a per-user version manager for LLVM and GCC toolchains.

It installs compiler versions from source, switches the active compiler in the
current shell, records persistent defaults, and removes installed toolchains
without modifying system compilers.

## Version

Current release: `v0.0.1`

## Installation

Install from a local checkout:

```sh
git clone https://github.com/QGrain/cvm.git
cd cvm
./install.sh
```

Install from a release tag:

```sh
curl -fsSL https://raw.githubusercontent.com/QGrain/cvm/v0.0.1/install.sh | bash
```

The installer places `cvm` under `$HOME/.cvm/bin`, writes `$HOME/.cvm/cvm.sh`,
and appends a small loader to the detected shell profile. Set
`PROFILE=/dev/null` to skip profile edits.

When `install.sh` is run from a local checkout, it builds that checkout with
`cargo build --release` and does not download release assets. When it is run
from a downloaded script, it first tries the matching binary release asset for
the selected tag, then falls back to the GitHub source archive and builds it
locally with Cargo.

Source builds require Rust/Cargo 1.65 or newer.

Override the release tag with either form:

```sh
curl -fsSL https://raw.githubusercontent.com/QGrain/cvm/v0.0.1/install.sh | CVM_VERSION=v0.0.1 bash
curl -fsSL https://raw.githubusercontent.com/QGrain/cvm/main/install.sh | bash -s -- --version v0.0.1
```

## Usage

Install compilers:

```sh
cvm install llvm 21.1.8 -j8
cvm install gcc 15.1.0 -j8
```

Use a compiler in the current shell:

```sh
cvm use llvm 21.1.8
```

For scripts or shells that do not source `$CVM_HOME/cvm.sh`, evaluate the
printed environment explicitly:

```sh
eval "$(cvm use llvm 21.1.8)"
```

Set persistent defaults:

```sh
cvm alias default llvm 21.1.8
cvm alias default gcc 15.1.0
```

List and inspect versions:

```sh
cvm ls
cvm current
cvm version
```

Uninstall a toolchain:

```sh
cvm uninstall llvm 17.0.6
```

## Commands

```text
cvm install <llvm|gcc> <version> [-jN|--jobs N]
cvm ls [llvm|gcc]
cvm use <llvm|gcc> [version]
cvm alias default <llvm|gcc> <version>
cvm current [llvm|gcc]
cvm env <llvm|gcc> [version]
cvm uninstall <llvm|gcc> <version>
cvm init
cvm version
```

## Shell Behavior

The installer writes a profile snippet that sources `$HOME/.cvm/cvm.sh`.
After that, `cvm use ...` works like nvm in interactive shells.

`cvm use` and `cvm alias default` require the selected version to be installed.
This avoids pointing `PATH` at a missing toolchain and accidentally falling back
to a system compiler.

For one-off shells, use:

```sh
eval "$(cvm use llvm 21.1.8)"
```

`cvm alias default` writes a persistent default under `$CVM_HOME/defaults`.
Defaults are applied when `$CVM_HOME/cvm.sh` is sourced in new shells.

When switching versions, cvm clears the compiler variables it owns
(`CC`, `CXX`, `LD`, `LLVM`, `HOSTCC`, `HOSTCXX`) before exporting the selected
toolchain. It does not clear unrelated user-managed variables such as
`CROSS_COMPILE`.

## Project Defaults

Create a `.cvmrc` file to select compiler versions for a project:

```text
llvm 21.1.8
gcc 15.1.0
```

When no version is passed to `cvm use` or `cvm env`, `.cvmrc` takes precedence
over the global default alias.

## Storage

`cvm` installs toolchains under:

```text
$CVM_HOME/toolchains/llvm/<version>
$CVM_HOME/toolchains/gcc/<version>
```

If `CVM_HOME` is unset, cvm uses `$HOME/.cvm`.

Default layout:

```text
$HOME/.cvm/bin/cvm
$HOME/.cvm/cvm.sh
$HOME/.cvm/toolchains/llvm/<version>
$HOME/.cvm/toolchains/gcc/<version>
$HOME/.cvm/defaults/{llvm,gcc}
$HOME/.cvm/scripts/
```

## Build Backends

The source build scripts live in `scripts/` and are embedded into the Rust
binary at compile time:

- `scripts/build_llvm-project.sh`
- `scripts/build_gcc.sh`

`cvm install` materializes the selected backend under `$CVM_HOME/scripts` and
invokes it with a versioned prefix.

On Debian/Ubuntu systems, the backend scripts run `sudo apt update` and
`sudo apt install` for required build packages before compiling. `sudo` prompts
normally in the terminal when credentials are needed. In non-interactive
environments, install the dependencies first or provide passwordless sudo.

## Development

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
bash -n scripts/build_llvm-project.sh
bash -n scripts/build_gcc.sh
```

## Repository

https://github.com/QGrain/cvm

## Release Assets

`install.sh` first tries to download binary release assets named:

```text
cvm-x86_64-unknown-linux-gnu.tar.gz
cvm-aarch64-unknown-linux-gnu.tar.gz
cvm-x86_64-apple-darwin.tar.gz
cvm-aarch64-apple-darwin.tar.gz
```

Each archive must contain an executable named `cvm` at the archive root. Binary
assets are optional: if an asset is not available, the installer downloads the
tagged GitHub source archive and builds cvm locally with Cargo.

## Uninstalling

Remove the profile snippet that sources `$CVM_HOME/cvm.sh`, then remove:

```sh
rm -rf "$HOME/.cvm"
```

## License

Apache-2.0. See [LICENSE](LICENSE).
