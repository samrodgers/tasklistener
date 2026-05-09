# Installers

The reproducible build path is **GitHub Actions** —
`.github/workflows/release.yml` builds both installers from a clean runner on
every push to a `v*` tag (and uploads them to the matching GitHub Release).

You can also build locally; see prerequisites below.

## What's produced

| Platform | Artifact | Built by |
|---|---|---|
| macOS 13+  | `TaskListener-Release.dmg`              | `scripts/build-macos.sh`   |
| Windows 10+ x64 | `TaskListener-0.1.0-x64-Setup.exe` | `scripts/build-windows.ps1` |

The macOS DMG contains a drag-to-Applications `TaskListener.app` with the Rust
dylib embedded under `Contents/Frameworks`. The Windows installer is a
self-contained Inno Setup `.exe`; no admin required (per-user install by
default; Inno Setup can elevate via the wizard if the user picks "all users").

## CI build (recommended)

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow runs on `macos-14` (Xcode 15 preinstalled) and
`windows-2022` (Visual Studio 2022 preinstalled). Artifacts attach to the GitHub
Release automatically. PRs and `workflow_dispatch` runs upload the same
artifacts but don't create a release.

### Codesigning

CI builds are **unsigned** by default. To sign:

- **macOS** — base64-encode your `.p12` Developer ID Application certificate,
  store as `MACOS_CERT_P12` secret. Add a job step that imports it into a
  temporary keychain, then set `DEVELOPER_ID` env on `build-macos.sh`. Add
  `NOTARIZATION_PROFILE` once you've stored notarytool credentials with
  `xcrun notarytool store-credentials`.
- **Windows** — for an EV / OV certificate, store the `.pfx` as `WINDOWS_CERT_PFX`
  and call `signtool` on `dist/*.exe` after Inno Setup runs. Inno Setup also
  supports `SignTool=` directives if you prefer.

The workflow scaffolding has the hook points but skips signing if the secrets
aren't set — this is intentional, so first-time forks get builds without
needing Apple/MS developer accounts.

## Local build — macOS

Prerequisites (one-time):

```bash
# Full Xcode (the App Store app, not just Command Line Tools).
xcode-select -p   # must point at /Applications/Xcode.app

# Tooling.
brew install xcodegen create-dmg
rustup default stable
```

Build:

```bash
./scripts/build-macos.sh           # produces dist/TaskListener-Release.dmg
./scripts/build-macos.sh --debug   # debug build
```

If you have a Developer ID cert installed in your keychain:

```bash
DEVELOPER_ID="Developer ID Application: Your Name (TEAMID)" \
NOTARIZATION_PROFILE="my-notary-profile" \
./scripts/build-macos.sh
```

## Local build — Windows

Prerequisites:

- Visual Studio 2022 with **.NET Desktop Development** + **Windows App SDK**
- .NET 8 SDK
- Rust + `x86_64-pc-windows-msvc` target (`rustup target add x86_64-pc-windows-msvc`)
- Inno Setup 6 (`iscc` on PATH) — install from <https://jrsoftware.org/isinfo.php>

Build:

```pwsh
./scripts/build-windows.ps1 -Configuration Release -Platform x64
```

Output: `dist/TaskListener-0.1.0-x64-Setup.exe`.

## Verifying a build (macOS)

```bash
hdiutil attach dist/TaskListener-Release.dmg
codesign -dv --verbose=4 "/Volumes/TaskListener/TaskListener.app"
spctl --assess --type execute --verbose "/Volumes/TaskListener/TaskListener.app"
```

Without notarisation, `spctl` will reject. Open the .app once with right-click
→ Open to register the user-level approval, after which it launches normally
on that machine.
