# TODOs

This document tracks ideas that have been discussed but are not part of the
current release. It is a planning aid, not a commitment.

## Candidate Features

- Persistent system default command: design a command that clears one or all
  persistent `alias default` entries, so users can make system compilers the
  default after opening a new shell.
- `cvm info <llvm|gcc> [version-or-prefix]`: print install prefix, key binary
  paths, default status, cache status, source URL, and verification status.
- Better diagnostics for dependency failures, missing `gpg`, signature
  verification failures, and ambiguous version prefixes.
- Maintainer installation test matrix for representative Linux and macOS
  environments.

## Cross Toolchains

Cross-compilation support has been requested, including Linux cross GCC
toolchains and MinGW-w64 GCC. These are not supported by the current release.

Current scope and constraints:

- LLVM can already be built with additional backends through `--targets` or a
  build profile, such as `AArch64`, `ARM`, or `RISCV`. This only enables LLVM
  code generation backends; it is not a complete cross toolchain by itself.
- A complete cross toolchain also needs a target triple, sysroot, target
  headers/libraries, linker/binutils or LLD configuration, and runtime support.
- GCC cross toolchains are more involved than native GCC builds because they
  usually require binutils, libc headers, startup files, and staged compiler
  builds.
- MinGW-w64 GCC should be treated as a cross toolchain target, commonly
  `x86_64-w64-mingw32` or `i686-w64-mingw32`. Linux-to-Windows MinGW use can be
  supported before full native Windows cvm support.

Possible implementation path:

- Document LLVM backend-oriented profiles for users who only need Clang to emit
  code for additional architectures.
- Add a way to register and switch existing external cross toolchains without
  rebuilding them, preserving cvm's environment management model.
- Add managed downloads for common prebuilt cross toolchains before attempting
  full source-built cross GCC support.
- Prioritize common targets such as `aarch64-linux-gnu`, `arm-linux-gnueabihf`,
  `riscv64-linux-gnu`, `x86_64-w64-mingw32`, and `i686-w64-mingw32`.
