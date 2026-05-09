import SwiftUI
import TaskListenerCore

struct IntegrationsTab: View {
    @EnvironmentObject var store: AppStore
    @State private var connecting: ProviderKind?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Connected").font(.headline)
            if store.providers.isEmpty {
                Text("No integrations yet.")
                    .foregroundColor(.secondary)
            } else {
                ForEach(store.providers) { p in
                    ConnectedRow(provider: p)
                }
            }

            Divider().padding(.vertical, 8)

            Text("Add integration").font(.headline)
            ForEach(ProviderKind.allCases, id: \.self) { kind in
                AddRow(kind: kind, onTap: { connecting = kind })
            }
        }
        .padding()
        .sheet(item: $connecting) { kind in
            ConnectSheet(kind: kind, onClose: { connecting = nil })
                .frame(width: 520, height: 420)
        }
    }
}

struct ConnectedRow: View {
    @EnvironmentObject var store: AppStore
    let provider: ProviderView

    var body: some View {
        HStack {
            Image(systemName: ProviderKind(rawValue: provider.kind)?.icon ?? "link")
            VStack(alignment: .leading) {
                Text(provider.display_name).bold()
                Text(provider.target_label.map { "→ \($0)" } ?? "no target picked")
                    .font(.caption)
                    .foregroundColor(.secondary)
                if let masked = provider.token_masked {
                    Text("Token: ••••\(masked)")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            }
            Spacer()
            Button(role: .destructive) {
                _ = TaskListener.shared.deleteProvider(id: provider.id)
                store.refreshProviders()
            } label: {
                Image(systemName: "trash")
            }
            .buttonStyle(.plain)
        }
        .padding(.vertical, 4)
    }
}

struct AddRow: View {
    let kind: ProviderKind
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack {
                Image(systemName: kind.icon).frame(width: 20)
                Text(kind.displayName)
                Spacer()
                Image(systemName: "plus.circle")
                    .foregroundColor(.accentColor)
            }
        }
        .buttonStyle(.plain)
        .padding(.vertical, 4)
    }
}
