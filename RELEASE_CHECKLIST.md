# Release Checklist — zacxiom

Check each item before tagging a release.

## Pre-Release

- [ ] `cargo fmt --check` — no formatting violations
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` — no lint errors
- [ ] `cargo test` — all tests pass
- [ ] `cargo build --release --locked` — release build succeeds
- [ ] `cargo audit` — no security advisories
- [ ] `cargo deny check` — license and dependency checks pass
- [ ] `codespell` — no spelling errors
- [ ] `./scripts/gatekeeper.sh` — all gates pass
- [ ] `./scripts/install.sh` — user install succeeds
- [ ] `./scripts/uninstall.sh` — user uninstall succeeds
- [ ] Manual smoke test: `scan`, `clean --dry-run`, `explain`, `status`
- [ ] All exit codes verified (plan blocked=0, undo fail=1, etc.)

## Documentation

- [ ] `README.md` — up to date with current version and features
- [ ] `TRADEMARK.md` — present and accurate
- [ ] `LICENSE` — unchanged and valid
- [ ] `docs/ARCHITECTURE.md` — reflects current module layout
- [ ] `docs/RULES.md` — safety rules current

## Packaging

- [ ] `./scripts/release.sh` — generates release archive and checksum
- [ ] `tar -tzf target/zacxiom-v*.tar.gz` — contents verified
- [ ] `sha512sum -c target/zacxiom-v*.tar.gz.sha512` — checksum matches

## GitHub Release

- [ ] `git push origin main` — all commits pushed
- [ ] `git tag vX.Y.Z` — tag created and pushed
- [ ] GitHub Release created from tag
- [ ] Release archive and checksum uploaded
- [ ] CI workflow is green on tag

## Post-Release

- [ ] Release downloaded and verified on clean system
- [ ] `--check-update` returns correct latest version
