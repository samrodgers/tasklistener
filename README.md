# TaskListener

A lightweight desktop app that listens to your voice, extracts tasks and
commitments from what you say, and pushes them to Apple Reminders, Todoist,
Notion, Things, or a generic webhook.

> **v0.1 status.** The Rust core, providers, push queue, FFI, SwiftUI shell,
> and WinUI skeleton are real and working. The audio + ML pipeline is stubbed
> behind a Cargo feature flag — manual entry exercises the rest end-to-end.

See [SPEC.md](SPEC.md) for the full design and [BUILDING.md](BUILDING.md) for
build instructions.

## Layout

```
crates/
  core/   Rust — storage, providers, push queue, audio pipeline (stubbed)
  ffi/    C ABI shared library consumed by both UIs
apps/
  macos/
    TaskListenerCore/  Swift package wrapping the C ABI
    App/               SwiftUI menu-bar app + Apple Reminders bridge
    project.yml        xcodegen project definition
  windows/
    TaskListener/      WinUI 3 / .NET 8 tray app + P/Invoke bindings
```

## Quick start (macOS)

```bash
cargo test -p tasklistener-core   # 8 tests, no network
cargo build -p tasklistener-ffi   # produces target/debug/libtasklistener.dylib
brew install xcodegen
cd apps/macos && xcodegen generate && open TaskListener.xcodeproj
```
