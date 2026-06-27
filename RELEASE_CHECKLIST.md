# RELEASE CHECKLIST — zacxiom v10.0.0

## Pre-release

- [ ] `./scripts/build.sh check-all` passes (fmt + clippy + build + test)
- [ ] `./scripts/golden-check.sh` passes
- [ ] `./scripts/gatekeeper.sh` passes
- [ ] `cargo build --release --locked` succeeds
- [ ] `CHANGELOG.md` updated with release notes
- [ ] Version bumped in `Cargo.toml`
- [ ] All `#[allow(dead_code)]` reviewed

## Post-release

- [ ] Archive uploaded to GitHub Releases
- [ ] Checksum verified (`sha512sum -c`)
- [ ] `cargo publish --dry-run` succeeds
- [ ] Tag pushed: `git tag v<VERSION> && git push origin v<VERSION>`
