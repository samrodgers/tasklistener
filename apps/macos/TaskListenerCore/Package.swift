// swift-tools-version: 5.9
import PackageDescription

// Swift wrapper around the Rust C ABI. Linked against the prebuilt
// `libtasklistener.dylib` produced by `cargo build -p tasklistener-ffi`.
//
// Set TASKLISTENER_RUST_ROOT to the workspace root, or rely on the default
// "../../" path resolution (this package lives at apps/macos/TaskListenerCore).

let package = Package(
    name: "TaskListenerCore",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "TaskListenerCore", targets: ["TaskListenerCore"]),
    ],
    targets: [
        .systemLibrary(
            name: "CTaskListener",
            path: "Sources/CTaskListener"
        ),
        .target(
            name: "TaskListenerCore",
            dependencies: ["CTaskListener"],
            path: "Sources/TaskListenerCore",
            linkerSettings: [
                // Resolved at app-bundle time; the .app should embed
                // libtasklistener.dylib in Contents/Frameworks.
                .unsafeFlags(["-L../../target/debug", "-L../../target/release"]),
                .linkedLibrary("tasklistener"),
            ]
        ),
    ]
)
