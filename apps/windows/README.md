# TaskListener — Windows app (skeleton)

Status: **skeleton only**. Code compiles on a Windows machine with the right
toolchain; cannot be built from macOS. Mirrors the SwiftUI macOS app with
WinUI 3 + a system-tray flyout.

## Toolchain

- Visual Studio 2022 with workloads:
  - .NET Desktop Development
  - Windows App SDK
- .NET 8 SDK
- Rust with `x86_64-pc-windows-msvc` (or `aarch64-pc-windows-msvc`) target

## Build

```pwsh
# 1. Build the Rust dylib for Windows.
cargo build -p tasklistener-ffi --release --target x86_64-pc-windows-msvc

# 2. Restore + build the C# app. The csproj copies the dll into the output
#    directory via a target.
cd apps\windows\TaskListener
dotnet restore
dotnet build -c Release
```

## What's here

- `TaskListener.csproj` — WinUI 3 project, .NET 8, copies `tasklistener.dll`
  next to the binary at build time.
- `App.xaml` / `App.xaml.cs` — application entry, creates the tray icon.
- `MainWindow.xaml` / `.cs` — the popover-style task-list flyout.
- `Interop/NativeBindings.cs` — full P/Invoke binding to the C ABI in
  `crates/ffi/include/tasklistener.h`. Mirrors `TaskListenerCore.swift`.
- `ViewModels/AppStore.cs` — same role as the Mac `AppStore`: subscribes to
  core events, mirrors task & provider state into observables.
- `Views/IntegrationsPage.xaml` / `ConnectDialog.xaml` — settings UI.

## What is **not** built here

- Apple Reminders & Things providers (mac-only).
- Audio + ML pipeline (Rust feature flag).
- Code signing / packaging (msix / msi). Add via the Windows App SDK packaging
  project once the app actually runs.

## Maintenance note

Keep `Interop/NativeBindings.cs` in lockstep with
`crates/ffi/include/tasklistener.h`. Any new exported function on the Rust side
needs a `[DllImport]` here.
