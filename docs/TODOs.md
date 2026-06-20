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

