# Troubleshooting

## Linux glibc and musl

cvm Linux release assets use musl targets:

```text
cvm-x86_64-unknown-linux-musl.tar.gz
cvm-aarch64-unknown-linux-musl.tar.gz
```

This avoids requiring the target host to provide the same glibc version as the
GitHub Actions runner that built the binary. If an older cvm binary reports a
`GLIBC_x.y not found` error, reinstall a newer cvm release.

## Proxy Settings

cvm honors standard proxy environment variables for HTTP requests:

```sh
export http_proxy=http://127.0.0.1:7890
export https_proxy=http://127.0.0.1:7890
```

Use the lowercase and uppercase variants if your environment requires both.
`cvm ls-remote`, `cvm version`, and `cvm upgrade` read the remote index through
this network path.

Compiler source archives downloaded by `cvm install` are cached under
`$CVM_HOME/cache/sources`. If a download succeeds but the build later fails,
re-running the same install can reuse the cached archive.

## Cache Disk Usage

Source archives can be hundreds of megabytes each. Inspect and prune them with:

```sh
cvm cache list
cvm cache prune
cvm cache prune --older-than 14d
```

Cache pruning only removes downloaded source archives. It does not remove
installed toolchains under `$CVM_HOME/toolchains`.

## PATH Priority

The installer writes `cvm` to `$CVM_HOME/bin/cvm` and loads `$CVM_HOME/cvm.sh`
from your shell profile. The loader puts `$CVM_HOME/bin` and active toolchain
`bin` directories before system compiler paths.

Run:

```sh
cvm version
cvm current
cvm which llvm
cvm which gcc
```

If system compilers are still selected, ensure the cvm profile snippet appears
after other PATH setup in your shell profile.

## Profile Not Loaded

If `cvm use ...` prints shell code instead of switching the current shell, the
shell loader is not loaded. Open a new shell or run:

```sh
export CVM_HOME="${CVM_HOME:-$HOME/.cvm}"
. "$CVM_HOME/cvm.sh"
```

For one-off scripts, use:

```sh
eval "$(cvm use llvm 21)"
```

## Completion Not Loaded

`cvm init` registers bash/zsh completion through `$CVM_HOME/cvm.sh`. If tab
completion is not available, open a new shell or source the loader again:

```sh
. "$CVM_HOME/cvm.sh"
```

To inspect the generated completion scripts:

```sh
cvm completion bash
cvm completion zsh
```

These commands only print shell code to stdout. They do not modify files or
fetch network resources.

## Source Build Dependencies

cvm builds LLVM and GCC from source. On Debian and Ubuntu systems the backend
scripts bootstrap required packages with `apt`. Root containers call `apt`
directly. Non-root users need `sudo` because the scripts run `sudo apt update`
and `sudo apt install`.

Starting with `v0.0.7`, cvm verifies downloaded LLVM and GCC source archives
with GPG detached signatures before building. cvm downloads the upstream key
bundle into `$CVM_HOME/cache/keys/<tool>/`, imports it with `gpg`, and then
verifies the source archive.

If dependency installation fails:

- Confirm the user can run `sudo`.
- Confirm apt repositories are reachable through your proxy or mirror.
- In minimal root containers, confirm `apt` exists; `sudo` is not required when
  running as root.
- Confirm `gpg` is installed. Minimal containers may omit it.
- Confirm the upstream key bundle exists under `$CVM_HOME/cache/keys/<tool>/`
  and `gpg --verify <archive>.sig <archive>` works for the upstream source
  archive.
- Re-run the failing `cvm install ... --dry-run` command to inspect the backend
  script invocation.
- Install missing build tools manually in locked-down containers.
