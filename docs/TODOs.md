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
- User guide documentation with quick start examples and advanced workflows.

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

## Fortran, OpenMP, and MPI

Parallel and scientific-computing environments have been requested, especially
`gfortran`, OpenMP, and MPI. These are related to C/C++ toolchains but should be
added in layers instead of being folded into the compiler version model without
clear boundaries.

Possible implementation path:

- Treat `gfortran` as part of GCC support. Add documented build profile examples
  for `languages = "c,c++,fortran"` and make `cvm use gcc ...` export Fortran
  compiler variables such as `FC`, `F77`, and `F90` when `gfortran` is present.
- Treat OpenMP as compiler runtime support. GCC uses `libgomp`; LLVM/Clang uses
  the llvm-project `openmp` runtime. Build profiles should make this explicit.
- Treat MPI as a separate parallel environment that is usually built against a
  specific compiler version. Future support should record the compiler binding
  instead of treating OpenMPI or MPICH as compiler versions themselves.
- Consider OpenMPI and MPICH first if MPI management is added.

## Documentation Ecosystem

The root README files should stay concise. Detailed usage should move into a
small documentation set under `docs/`, with a static documentation site later if
the project grows.

Possible implementation path:

- Add a user guide such as `docs/usage.md` with quick start, common workflows,
  build profiles, source cache, switching behavior, and advanced examples.
- Keep `docs/troubleshooting.md` separate from the user guide so the primary
  guide does not become too long.
- Add documentation-site generation later, for example with mdBook, MkDocs, or
  Read the Docs plus GitHub Pages.

## Community Workflow

Prepare repository metadata for external contributors before the project grows.

Possible implementation path:

- Add issue labels for feature requests, bugs, documentation, good first issue,
  help wanted, cross toolchains, parallel computing, installation, release, and
  platform support.
- Add issue templates for bug reports, feature requests, and toolchain support
  requests.
- Add a long-lived contributor onboarding issue only when there are concrete,
  scoped starter tasks to avoid creating an empty call for help.
