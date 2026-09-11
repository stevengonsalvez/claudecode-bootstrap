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

Stable releases stop before tagging if any macOS distribution secret is absent.
They archive a universal app, sign it with Developer ID, notarize and staple the
DMG, create an EdDSA-signed Sparkle appcast, attach both to the GitHub release,
and write `Casks/ainb-fleet.rb` into `stevengonsalvez/homebrew-agents-in-a-box`.

### Required repository secrets

| Secret | Purpose |
|---|---|
| `APPLE_DEVELOPER_ID_CERTIFICATE` | Base64 Developer ID Application `.p12` certificate. |
| `APPLE_DEVELOPER_ID_CERTIFICATE_PASSWORD` | Password for that `.p12` certificate. |
| `APPLE_TEAM_ID` | Apple Developer Team ID used while signing. |
| `APPLE_NOTARY_ISSUER_ID` | App Store Connect API key issuer UUID for notarization. |
| `APPLE_NOTARY_KEY_ID` | App Store Connect API key ID for notarization. |
| `APPLE_NOTARY_PRIVATE_KEY` | Base64 App Store Connect API `.p8` key. |
| `SPARKLE_PUBLIC_ED_KEY` | Base64 Ed25519 public key embedded in Fleet `Info.plist`. |
| `SPARKLE_PRIVATE_ED_KEY` | Matching Ed25519 private key used only to sign appcast enclosures. |

`HOMEBREW_TAP_TOKEN` already authorizes writes to the existing tap. Keep all
private certificate, notarization, and Sparkle key material in repository
secrets only. Never commit it.

## See also

- [Docs hub](/readme)
