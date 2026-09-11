#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
derived_data="${1:-/tmp/ainb-fleet-macos-e02-release-derived-data}"
binary="$derived_data/Build/Products/Release/AINBFleet.app/Contents/MacOS/AINBFleet"
plist="$derived_data/Build/Products/Release/AINBFleet.app/Contents/Info.plist"
public_key="$(openssl genpkey -algorithm Ed25519 | openssl pkey -pubout -outform DER | tail -c 32 | base64)"

xcodebuild build \
  -project "$repo_root/apps/ainb-fleet-macos/AINBFleet.xcodeproj" \
  -scheme AINBFleet \
  -configuration Release \
  -destination 'platform=macOS' \
  -derivedDataPath "$derived_data" \
  CODE_SIGNING_ALLOWED=NO ARCHS='arm64 x86_64' ONLY_ACTIVE_ARCH=NO SPARKLE_PUBLIC_ED_KEY="$public_key"

test -x "$binary"
lipo "$binary" -verify_arch arm64 x86_64
test "$(plutil -extract SUFeedURL raw "$plist")" = "https://github.com/stevengonsalvez/agents-in-a-box/releases/latest/download/appcast.xml"
test "$(plutil -extract SUPublicEDKey raw "$plist")" = "$public_key"
test "$(plutil -extract SUEnableAutomaticChecks raw "$plist")" = "true"
test "$(plutil -extract SURequireSignedFeed raw "$plist")" = "true"
! strings "$binary" | rg -F -- '--fleet-test-read-range'
! strings "$binary" | rg -F -- '--fleet-test-open-window'
