import SwiftUI
import TaskListenerCore

@main
struct TaskListenerApp: App {
    @StateObject private var store = AppStore()

    init() {
        TaskListener.shared.start()
    }

    var body: some Scene {
        MenuBarExtra {
            MenuBarRoot()
                .environmentObject(store)
                .frame(width: 380, height: 540)
        } label: {
            Image(systemName: store.iconName)
        }
        .menuBarExtraStyle(.window)

        Settings {
            SettingsRoot()
                .environmentObject(store)
                .frame(width: 620, height: 480)
        }
    }
}
