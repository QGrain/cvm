# Build Profiles

Build profiles let advanced users customize LLVM and GCC source builds without
adding options to the normal `cvm install` workflow.

The recommended workflow is to generate the default profile, edit it, and then
run `cvm install` normally.

## Quick Start

```sh
cvm profile template llvm
vim "${CVM_HOME:-$HOME/.cvm}/profiles/build/llvm/default.toml"
cvm install llvm 21
```

For GCC:

```sh
cvm profile template gcc
vim "${CVM_HOME:-$HOME/.cvm}/profiles/build/gcc/default.toml"
cvm install gcc 15
```

Default build profiles live at:

```text
$CVM_HOME/profiles/build/llvm/default.toml
$CVM_HOME/profiles/build/gcc/default.toml
```

If `CVM_HOME` is unset, cvm uses `$HOME/.cvm`.

`cvm profile template <llvm|gcc>` prints the commented template to the
terminal, writes it to the default path, and refuses to overwrite an existing
file unless `--force` is used.

List existing profiles with:

```sh
cvm profile list
```

This command reports TOML files under `$CVM_HOME/profiles`. It does not parse
or validate their contents.

## Install Priority

When installing a compiler, cvm selects build configuration in this order:

```text
1. A file passed with --profile PATH
2. $CVM_HOME/profiles/build/<tool>/default.toml
3. cvm's built-in kernel-oriented defaults
```

Use `--profile` when a one-off build should use a profile outside `$CVM_HOME`:

```sh
cvm profile template llvm ./llvm-custom.toml
cvm install llvm 21 --profile ./llvm-custom.toml
```

The `--profile` value is a file path. It is not a named profile.

## LLVM Example

```toml
[llvm]
targets = "X86;AArch64"
projects = "clang;lld;compiler-rt"
runtimes = "libcxx;libcxxabi;libunwind"
build_type = "Release"

[llvm.cmake_defines]
LLVM_ENABLE_ASSERTIONS = "ON"
LLVM_ENABLE_ZSTD = "ON"
```

`targets = "X86"` is enough for common x86 Linux kernel builds. Add targets
only when you need them, because extra backends increase build time and disk
usage.

## GCC Example

```toml
[gcc]
languages = "c,c++"
multilib = false
bootstrap = false
configure_args = [
  "--enable-plugin",
  "--enable-lto",
]
```

The default `multilib = false` and `bootstrap = false` match cvm's faster
kernel-oriented GCC build. Enabling them can improve coverage for some GCC
development tasks, but increases build cost.

## Compatibility

Build profiles can change compiler behavior. If you use cvm for Linux kernel
or syzkaller workflows, start from the generated default template and change
one option at a time.

`--targets` is an older `cvm install llvm ...` shortcut that is forwarded to
`scripts/build_llvm-project.sh --targets`. It remains supported for quick LLVM
target changes, but it cannot be combined with an active build profile. This
includes both an explicit `--profile PATH` and
`$CVM_HOME/profiles/build/llvm/default.toml`. If a default LLVM profile exists
and you want persistent target changes, put them in that profile.

cvm converts profile fields into internal `CVM_LLVM_*` and `CVM_GCC_*`
environment variables before launching the backend scripts. Users normally do
not need to set these variables by hand. They exist so the Rust CLI can parse
and validate TOML while the Bash scripts remain directly runnable and simple.
