---
title: "Release process"
---

How a release is cut, tagged, built, and shipped.

## What this page will contain

- Version bumping (Cargo.toml + package.json)
- CHANGELOG.md update
- Tag + push
- Release workflow run
- Homebrew tap auto-update
- Verifying the release

## Fleet macOS distribution

Fleet shares the stable CLI release version. The release workflow passes its
`version` input into both `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION`, so
the Git tag, cask, app bundle, and Sparkle appcast identify one release.
Prerelease tags do not ship Fleet, because the app's stable update feed must not
offer a prerelease to ordinary users.

Stable releases stop before tagging if a Sparkle update secret is absent. They
archive an unsigned universal app, create an EdDSA-signed Sparkle appcast,
attach both to the GitHub release, and write `Casks/ainb-fleet.rb` into
`stevengonsalvez/homebrew-agents-in-a-box`.

Fleet is distributed only through the custom tap, not the Mac App Store or the
official Homebrew cask repository. `brew install --cask ainb-fleet` installs the
app, but macOS can require a first-launch Gatekeeper override because the app is
unsigned. Right-click the app and choose Open, or choose Open Anyway in System
Settings > Privacy & Security.

### Required repository secrets

| Secret | Purpose |
|---|---|
| `SPARKLE_PUBLIC_ED_KEY` | Base64 Ed25519 public key embedded in Fleet `Info.plist`. |
| `SPARKLE_PRIVATE_ED_KEY` | Matching Ed25519 private key used only to sign appcast enclosures. |

`HOMEBREW_TAP_TOKEN` already authorizes writes to the existing tap. Keep the
private Sparkle key in Bitwarden and the matching GitHub repository secret.
Never commit it.

## See also

- [Docs hub](/readme)
