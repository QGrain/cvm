# Release Process

## Automated Release Assets

Pushing a `vX.Y.Z` tag runs `.github/workflows/release.yml`.

The workflow creates the GitHub Release with generated release notes when the
release does not already exist, builds cvm for each supported target, and
uploads these assets:

```text
cvm-x86_64-unknown-linux-musl.tar.gz
cvm-aarch64-unknown-linux-musl.tar.gz
cvm-x86_64-apple-darwin.tar.gz
cvm-aarch64-apple-darwin.tar.gz
```

Each archive contains a single executable named `cvm` at the archive root.
The names must match `install.sh`.
Linux assets use musl targets to avoid tying the executable to the glibc
version installed on the GitHub Actions runner.

The same workflow can be run manually with `workflow_dispatch` to backfill
assets for an existing tag such as `v0.0.1`. Provide the tag input, and the
workflow will check out that tag and upload assets to the matching release.

When signed assets are available, each archive should have a matching `.sig`
asset with the same name plus `.sig`.

## Installer Behavior

`install.sh` first tries to download the matching binary asset for the selected
tag. If no asset is available, it downloads the tagged source archive and builds
cvm locally with Cargo.

Re-running the installer replaces `$CVM_HOME/bin/cvm` and regenerates
`$CVM_HOME/cvm.sh`. It does not remove installed toolchains or defaults.

## Checklist

Before tagging a release:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
bash -n install.sh
bash -n scripts/build_llvm-project.sh
bash -n scripts/build_gcc.sh
python3 -m py_compile tools/update_remote_index.py
python3 -m json.tool manifests/remote-index.json
```

Then create an annotated tag and push it:

```sh
git tag -a vX.Y.Z -m "vX.Y.Z: short release theme"
git push origin vX.Y.Z
```

Use the GitHub release title as the tag only, for example `v0.0.6`. Put the
release theme and details in the release note body.

## Signing Setup

Release assets are signed with a cvm-specific GPG key. Configure these
repository secrets before publishing a signed release:

```text
CVM_RELEASE_GPG_PRIVATE_KEY
CVM_RELEASE_GPG_PASSPHRASE
CVM_RELEASE_GPG_KEY_ID
```

Publish the matching public key at:

```text
assets/keys/cvm-release-signing-key.asc
```

See [Release Signing](signing.md) for key generation and secret setup.

## Release Note Template

````markdown
## What's Changed

- ...

## Documentation

- ...

## Verifying Packages

Download the cvm release signing key, import it, then verify the asset:

```sh
curl -fsSLO https://raw.githubusercontent.com/QGrain/cvm/vX.Y.Z/assets/keys/cvm-release-signing-key.asc
gpg --import cvm-release-signing-key.asc
curl -fsSLO https://github.com/QGrain/cvm/releases/download/vX.Y.Z/<asset>.tar.gz
curl -fsSLO https://github.com/QGrain/cvm/releases/download/vX.Y.Z/<asset>.tar.gz.sig
gpg --verify <asset>.tar.gz.sig <asset>.tar.gz
```
````
