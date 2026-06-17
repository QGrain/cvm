# Source Cache

cvm caches downloaded compiler source archives under:

```sh
$CVM_HOME/cache/sources/<llvm|gcc>/<version>/
```

If `CVM_HOME` is unset, cvm uses `$HOME/.cvm`.

## Commands

```sh
cvm cache dir
cvm cache list
cvm cache prune
cvm cache prune --older-than 14d
```

`cvm install` reuses a cached source archive when present. When no cached
archive exists, cvm downloads the archive into the cache before invoking the
backend build script.

## Lifecycle

The default cache lifetime is 14 days. cvm does not install cron jobs, start
daemons, or modify system services. Instead, stale source archives are pruned
lazily when `cvm install` or `cvm cache prune` runs.

Use `cvm cache prune` when disk usage grows, or remove `$CVM_HOME/cache`
directly if you want to clear all cached downloads. Installed toolchains live
under `$CVM_HOME/toolchains` and are not removed by cache pruning.
