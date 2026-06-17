# Contribution Guide

Thanks for your interest in contributing to cvm.

## Development Checks

Run these before opening a PR:

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

## Code Guidelines

- Keep behavior small and predictable.
- Prefer existing patterns over new abstractions.
- Keep user-facing commands close to nvm-style conventions when practical.
- Do not add network dependencies to ordinary compiler switching paths.
- Keep runtime functionality separate from maintainer tooling.

## Documentation Guidelines

- Update `README.md` and `README_CN.md` together for user-facing changes.
- Keep root READMEs focused on quick start and common commands.
- Put design, release, and maintainer details under `docs/`.
- Keep examples short and runnable.

## Commit and PR Guidelines

- Use concise imperative commit subjects, for example `Add release workflow`.
- Keep unrelated changes in separate commits or PRs.
- Include tests for behavior changes.
- For release index changes, let the synchronization workflow open the PR when
  possible.

For small releases, a single release-focused commit is acceptable:

```text
v0.0.X: short release-focused summary
```

When a release contains multiple logical changes, split commits by concern.
Keep pure version metadata in a dedicated bump commit:

```text
bump: prepare v0.0.X release
```

That commit may include `Cargo.toml`, `Cargo.lock`, `install.sh`, README
version badges or install examples, and `manifests/remote-index.json`.

Use module-style prefixes for feature commits:

```text
profiles: add compiler build profile support
install: pass build profile environment to backend scripts
docs: document build profiles
logos: add cvm logo assets
agents: add project instructions for coding agents
```

Avoid mixing unrelated feature, documentation, logo, workflow, and release bump
changes in the same commit when they can be reviewed independently.

## Release Notes

Use the GitHub release title as the tag only:

```text
v0.0.X
```

Put the release theme in the annotated tag message and release note body, not
in the GitHub release title:

```sh
git tag -a v0.0.X -m "v0.0.X: short release theme"
```

Release notes should stay concise and include:

- `What's Changed`
- `Documentation`, when user-facing docs changed
- `Verification`, listing local checks that passed
- `Verifying Packages`, when release assets have `.sig` files

The `Verifying Packages` section should show users how to download the cvm
release signing key, import it with `gpg`, download the matching asset and
`.sig`, and run `gpg --verify <asset>.sig <asset>`.

## CI Expectations

PRs should pass formatting, tests, clippy, shell syntax checks, Python syntax
checks, and manifest JSON validation.
