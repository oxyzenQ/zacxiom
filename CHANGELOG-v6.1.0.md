# Zacxiom v6.1.0 — Developer Awareness Release

**Date:** 2026-06-22  
**Build:** fmt ✅ / clippy ✅ / test 69/69 ✅

## Summary

v6.1.0 expands Zacxiom's default scan coverage from 5 directories to 21,
adding Rust, Docker, AI/ML, gaming, Flatpak, Snap, and developer tooling caches.
The default scan now finds 5-10× more reclaimable storage than v5.4.0.

## Changes

### Expanded Default Scan Roots

**Before (v5.4.0):** 5 directories
```
~/.cache/  ~/.local/share/Trash/  /var/cache/  /tmp/  ~/.mozilla/
```

**After (v6.1.0):** 21 directories
```
Developer:  ~/.cargo/  ~/.rustup/  ~/.npm/  ~/.docker/  ~/.gradle/  ~/.m2/repository/
Gaming:     ~/.steam/  ~/.local/share/Steam/
Desktop:    ~/Downloads/  ~/.local/share/Trash/
Container:  ~/.var/app/  ~/snap/
System:     /var/cache/  /var/lib/docker/  /tmp/
```

### New Domain Classifications (10 patterns added)

| Category | New Patterns |
|---|---|
| **Rust** | `.rustup/toolchains/`, `.cargo/git/` |
| **Docker/Podman** | `.docker/overlay2`, `.docker/buildkit`, `/var/lib/docker/`, `.local/share/containers/` |
| **AI/ML** | `.cache/huggingface/`, `.cache/torch/`, `.cache/ollama/`, `.cache/modelscope/` |
| **Python** | `.cache/uv/` |
| **Node.js** | `.cache/pnpm/` |
| **Gaming** | Steam shadercache, downloading, compatdata, DXVK, VKD3D, Lutris, Heroic |
| **Desktop** | `.local/share/Trash/` |
| **Flatpak** | `.var/app/*/cache/` |
| **Snap** | `snap/*/.cache/` |
| **Arch AUR** | `.cache/yay/`, `.cache/paru/` |

### Enhanced Domain Summaries

Sub-domain recognition in report output:
- "Cargo Registry & Build Cache" vs generic "Developer Cache"
- "AI/ML Model Cache" — HuggingFace, Ollama, Torch
- "Docker/Container Cache" — Docker, Podman
- "Steam Shader Cache" / "Proton Compat Data"
- "DXVK/VKD3D Shader Cache"
- "Desktop Trash"

### Tests

- 69 tests (was 58) — 11 new classification tests
- All domain patterns verified with real-world paths
- Classification priority order verified (specific before general)

## Impact

| Metric | v5.4.0 | v6.1.0 |
|---|---|---|
| Default scan roots | 5 | 21 |
| Domain patterns | ~15 | ~35 |
| Recognizes Docker? | ❌ | ✅ |
| Recognizes Steam? | ❌ | ✅ |
| Recognizes AI caches? | ❌ | ✅ |
| Recognizes Rustup? | ❌ | ✅ |
| Reclaimable storage found | ~100 MB | 5-10× more |

## Known Limitations (→ v6.2.0)

- No dry-run / preview mode yet
- No age-based abandonment detection
- No `zacxiom weekly` habit command
- Confidence tiers not yet displayed in output
