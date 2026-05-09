import SwiftUI
import TaskListenerCore

struct SettingsRoot: View {
    var body: some View {
        TabView {
            IntegrationsTab()
                .tabItem { Label("Integrations", systemImage: "link") }
            GeneralTab()
                .tabItem { Label("General", systemImage: "gearshape") }
        }
        .padding()
    }
}

struct GeneralTab: View {
    @EnvironmentObject var store: AppStore
    var body: some View {
        Form {
            Toggle("Listen always-on", isOn: $store.listening)
            HStack {
                Text("Audio pipeline:")
                Text(store.audioIsReal ? "real" : "stubbed (build with --features audio)")
                    .foregroundColor(store.audioIsReal ? .green : .orange)
            }
        }
        .padding()
    }
}
