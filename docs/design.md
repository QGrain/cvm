# Design Notes

`cvm` keeps all user-owned state under one root:

```text
$CVM_HOME/bin/cvm
$CVM_HOME/cvm.sh
$CVM_HOME/toolchains/llvm/<version>
$CVM_HOME/toolchains/gcc/<version>
$CVM_HOME/cache/sources/<llvm|gcc>/<version>
$CVM_HOME/defaults/{llvm,gcc}
$CVM_HOME/scripts/
```

If `CVM_HOME` is unset, the default is `$HOME/.cvm`.

## Shell Integration

The installer writes `$CVM_HOME/cvm.sh` and adds a profile snippet that sources
it. The loader puts `$CVM_HOME/bin` first in `PATH`, applies persistent default
toolchains, and wraps `cvm use` so it can update the current shell.

`cvm use` and `cvm alias default` require the selected version to be installed.
This avoids pointing `PATH` at missing toolchains and silently falling back to
system compilers.

When switching versions, cvm clears the compiler variables it owns:
`CC`, `CXX`, `LD`, `LLVM`, `HOSTCC`, and `HOSTCXX`. It does not clear unrelated
user-managed variables such as `CROSS_COMPILE`.

## Remote Index

`cvm ls-remote`, `cvm version`, and `cvm upgrade` read
`manifests/remote-index.json` from the cvm repository. Set
`CVM_REMOTE_INDEX_URL` to point cvm at a mirrored index in restricted networks.

The index is synchronized by `.github/workflows/synchronize-remote-index.yml`
using `tools/update_remote_index.py`.

## Build Backends

Compiler source builds are delegated to embedded backend scripts:

- `scripts/build_llvm-project.sh`
- `scripts/build_gcc.sh`

At runtime, cvm materializes the selected backend under `$CVM_HOME/scripts` and
invokes it with a versioned install prefix.

## Source Cache

`cvm install` downloads compiler source archives and detached `.sig` files into
`$CVM_HOME/cache/sources/<tool>/<version>/`. It also caches upstream release key
bundles under `$CVM_HOME/cache/keys/<tool>/`, imports the key bundle with
`gpg --import`, verifies the source archive with `gpg --verify`, and passes the
cached archive to the backend script. Backend scripts still support direct
standalone use; when run outside cvm, they download sources into the current
directory as before.

The cache uses lazy pruning with a default 14-day lifetime. cvm does not install
cron jobs or background services. Cache pruning never removes installed
toolchains under `$CVM_HOME/toolchains`.

## Build Profiles

Build profiles are optional TOML files for advanced compiler source builds.
The default profile is persistent: if
`$CVM_HOME/profiles/build/<tool>/default.toml` exists, `cvm install` uses it
unless an explicit `--profile PATH` is provided.

Default profiles live under:

```text
$CVM_HOME/profiles/build/llvm/default.toml
$CVM_HOME/profiles/build/gcc/default.toml
```

The data flow is:

```text
TOML profile -> Rust validation -> CVM_* environment variables -> backend script
```

This keeps shell scripts directly runnable while avoiding arbitrary argument
passthrough. Generate a commented starting point with `cvm profile template`.
