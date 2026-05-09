# Building TaskListener

This is a v0.1 build. The Rust core, providers, push queue, FFI, and Swift
wrapper are real and pass tests. The audio + ML pipeline is **stubbed** behind
the `audio` feature flag — see `crates/core/src/audio/mod.rs`. End-to-end you
can capture tasks manually and push them to Todoist / Notion / Things /
Webhook / Apple Reminders.

## Prerequisites

- Rust (`rustup` — `stable` channel)
- macOS 13+ for the Mac app
- Xcode 15+ with command-line tools (for the Mac app)
- `xcodegen` for generating the Xcode project: `brew install xcodegen`

## Rust core

```bash
# Build everything.
cargo build

# Run the test suite (8 tests, no network).
cargo test -p tasklistener-core

# Release build of the dylib (used by the Mac app).
cargo build -p tasklistener-ffi --release
```

## macOS app

```bash
# 1. Build the Rust dylib (the Xcode build phase also does this, but doing it
#    once up-front means the first Xcode build is faster).
cargo build -p tasklistener-ffi

# 2. Generate the Xcode project.
cd apps/macos
xcodegen generate

# 3. Open and run.
open TaskListener.xcodeproj
```

The app appears in the menu bar (no dock icon — `LSUIElement = true`). Type
into the input to add a task; connect a provider in **Settings → Integrations**
to push tasks out.

### What works in v0.1

- Manual task entry (the audio pipeline is stubbed).
- Push to **Todoist** (paste a personal API token from
  `https://app.todoist.com/app/settings/integrations/developer`).
- Push to **Notion** (create an integration at `https://www.notion.so/my-integrations`,
  share a database with it, paste the token).
- Push to **Things 3** via the local URL scheme (mac only).
- Push to **Apple Reminders** via EventKit (mac only — Swift-side, not via core).
- Push to a **generic webhook** (URL + optional bearer).
- Push state badges per task (queued / pushing / pushed / failed).
- Retry with exponential backoff (1m → 5m → 30m → 2h, then dead-letter).
- Per-task right-click → "Push to …" force-push.
- Tokens stored in macOS Keychain.

### What's stubbed / deferred

- **Audio + ML pipeline** — see `crates/core/src/audio/mod.rs`. Wire up cpal,
  Silero VAD, ECAPA-TDNN, whisper.cpp, and Qwen2.5-3B in a follow-up. The FFI
  surface is already shaped right: the real pipeline calls
  `Engine::capture(NewTask {...})` exactly as the manual-entry path does.
- **WinUI 3 app** — see `apps/windows/`. The skeleton + P/Invoke bindings are
  in place; needs a Windows machine with Visual Studio 2022 + Windows App SDK
  to compile. Apple Reminders / Things providers are mac-only.
- **Two-way sync** — v3.

## Windows app

See `apps/windows/README.md`. Requires:
- Visual Studio 2022 with **.NET Desktop** + **Windows App SDK** workloads
- Rust with the `x86_64-pc-windows-msvc` or `aarch64-pc-windows-msvc` target

```pwsh
cargo build -p tasklistener-ffi --release --target x86_64-pc-windows-msvc
cd apps\windows\TaskListener
dotnet build
```

## Troubleshooting

- **"libtasklistener.dylib not found"** — rerun `cargo build -p tasklistener-ffi`
  (the Xcode build also does this in a pre-build phase, but a clean checkout
  needs it once).
- **Reminders permission denied** — System Settings → Privacy & Security →
  Reminders → enable TaskListener, then reconnect from Settings → Integrations.
- **Todoist 401** — token revoked or expired. Reconnect.
- **Notion "Not allowed"** — your integration token was created but the target
  database wasn't shared with the integration. Open the database in Notion →
  ⋯ → Connections → add your integration.
