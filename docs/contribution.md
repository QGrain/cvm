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

## CI Expectations

PRs should pass formatting, tests, clippy, shell syntax checks, Python syntax
checks, and manifest JSON validation.
