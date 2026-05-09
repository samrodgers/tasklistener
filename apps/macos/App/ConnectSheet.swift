import SwiftUI
import TaskListenerCore

enum ProviderKind: String, CaseIterable, Identifiable {
    case reminders, things, todoist, notion, webhook
    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .reminders: return "Apple Reminders"
        case .things: return "Things 3"
        case .todoist: return "Todoist"
        case .notion: return "Notion"
        case .webhook: return "Webhook"
        }
    }
    var icon: String {
        switch self {
        case .reminders: return "checklist"
        case .things: return "checkmark.square"
        case .todoist: return "list.bullet.rectangle"
        case .notion: return "n.square"
        case .webhook: return "arrow.up.right.square"
        }
    }
    var needsToken: Bool {
        switch self {
        case .todoist, .notion: return true
        case .webhook: return true   // optional bearer
        case .things, .reminders: return false
        }
    }
    var tokenHelp: String? {
        switch self {
        case .todoist:
            return "In Todoist: Settings → Integrations → Developer → copy your API token."
        case .notion:
            return "Create an integration at notion.so/my-integrations, share the destination database with it, then paste the integration token."
        case .webhook:
            return "Optional bearer token sent as Authorization: Bearer <token>. Leave blank for unauthenticated webhooks."
        default: return nil
        }
    }
    var docURL: URL? {
        switch self {
        case .todoist: return URL(string: "https://app.todoist.com/app/settings/integrations/developer")
        case .notion: return URL(string: "https://www.notion.so/my-integrations")
        default: return nil
        }
    }
}

/// Force-pick destination flow: enter token (if any) → list targets → pick one → save.
struct ConnectSheet: View {
    @EnvironmentObject var store: AppStore
    let kind: ProviderKind
    let onClose: () -> Void

    @State private var token: String = ""
    @State private var webhookURL: String = ""
    @State private var targets: [ProviderTarget] = []
    @State private var pickedTarget: ProviderTarget?
    @State private var status: ConnectStatus = .idle
    @State private var providerId: String = ""

    enum ConnectStatus: Equatable {
        case idle
        case validating
        case picking
        case saving
        case error(String)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Image(systemName: kind.icon).font(.title2)
                Text("Connect \(kind.displayName)").font(.title2.bold())
                Spacer()
                Button("Cancel", action: onClose)
            }

            if let help = kind.tokenHelp {
                Text(help).font(.caption).foregroundColor(.secondary)
                if let docURL = kind.docURL {
                    Link("Open \(kind.displayName) settings", destination: docURL)
                        .font(.caption)
                }
            }

            if kind == .webhook {
                Text("Webhook URL")
                TextField("https://example.com/hook", text: $webhookURL)
                    .textFieldStyle(.roundedBorder)
                    .autocorrectionDisabled()
                Text("Bearer token (optional)").padding(.top, 6)
                SecureField("token", text: $token)
                    .textFieldStyle(.roundedBorder)
            } else if kind.needsToken {
                Text("API token")
                SecureField("paste token here", text: $token)
                    .textFieldStyle(.roundedBorder)
            } else if kind == .reminders {
                Text("TaskListener will request permission to access Reminders. macOS will show a system prompt.")
                    .font(.caption).foregroundColor(.secondary)
            } else if kind == .things {
                Text("Things 3 must be installed. No auth required.")
                    .font(.caption).foregroundColor(.secondary)
            }

            switch status {
            case .picking:
                Text("Pick a destination").font(.headline).padding(.top, 6)
                Picker("Target", selection: $pickedTarget) {
                    Text("Choose…").tag(ProviderTarget?.none)
                    ForEach(targets) { t in
                        Text(t.label).tag(Optional(t))
                    }
                }
                .pickerStyle(.menu)
            case .error(let msg):
                Text(msg).foregroundColor(.red).font(.callout)
            default: EmptyView()
            }

            Spacer()

            HStack {
                Spacer()
                if status == .picking {
                    Button("Save") { save() }
                        .keyboardShortcut(.defaultAction)
                        .disabled(pickedTarget == nil)
                } else {
                    Button("Next") { next() }
                        .keyboardShortcut(.defaultAction)
                        .disabled(!canProceed)
                }
            }
        }
        .padding()
    }

    private var canProceed: Bool {
        switch kind {
        case .webhook: return !webhookURL.isEmpty
        case .todoist, .notion: return !token.isEmpty
        case .things, .reminders: return true
        }
    }

    private func next() {
        // Reminders is handled entirely Swift-side via EventKit; we save the
        // provider record so the AppStore can route pushes to it, but we don't
        // call into core for target listing.
        if kind == .reminders {
                providerId = "reminders:default"
                _Concurrency.Task { @MainActor in
                    let granted = await store.remindersBridge.requestAccess()
                    if !granted {
                        status = .error("Reminders access not granted.")
                        return
                    }
                    let cfg = ProviderConfigInput(
                        id: providerId,
                        kind: "reminders",
                        displayName: "Apple Reminders",
                        enabled: true,
                        configJson: "{}",
                        minConfidence: 0.7,
                        autoPush: true,
                        targetId: nil,
                        targetLabel: nil
                    )
                    _ = TaskListener.shared.setProvider(cfg, token: nil)
                    targets = store.remindersBridge.availableLists()
                    status = targets.isEmpty
                        ? .error("No Reminders lists found.")
                        : .picking
                }
                status = .validating
                return
        }

        // Save provider config first (the targets API needs it persisted to read the keychain).
        providerId = "\(kind.rawValue):default"
        let configJson: String
        switch kind {
        case .webhook:
            configJson = #"{"url":"\#(webhookURL.replacingOccurrences(of: "\"", with: "\\\""))"}"#
        default:
            configJson = "{}"
        }
        let cfg = ProviderConfigInput(
            id: providerId,
            kind: kind.rawValue,
            displayName: kind.displayName,
            enabled: true,
            configJson: configJson,
            minConfidence: 0.7,
            autoPush: true,
            targetId: nil,
            targetLabel: nil
        )
        let tokenToStore: String? = kind.needsToken ? token : nil
        guard TaskListener.shared.setProvider(cfg, token: tokenToStore) else {
            status = .error("Failed to save credentials.")
            return
        }
        status = .validating
        DispatchQueue.global().async {
            // Special-case: Apple Reminders is implemented Swift-side via EventKit.
            // For v0.1 we don't ship a Reminders provider in core — skip target list.
            if kind == .reminders {
                DispatchQueue.main.async {
                    targets = [ProviderTarget(id: "default", label: "Reminders (default list)")]
                    status = .picking
                }
                return
            }
            let fetched = TaskListener.shared.listTargets(providerId: providerId)
            DispatchQueue.main.async {
                if fetched.isEmpty {
                    status = .error("Couldn't fetch targets. Check your token / shared databases / network.")
                } else {
                    targets = fetched
                    status = .picking
                }
            }
        }
    }

    private func save() {
        guard let target = pickedTarget else { return }
        status = .saving
        let cfg = ProviderConfigInput(
            id: providerId,
            kind: kind.rawValue,
            displayName: kind.displayName,
            enabled: true,
            configJson: kind == .webhook
                ? #"{"url":"\#(webhookURL.replacingOccurrences(of: "\"", with: "\\\""))"}"#
                : "{}",
            minConfidence: 0.7,
            autoPush: true,
            targetId: target.id,
            targetLabel: target.label
        )
        let tokenToStore: String? = kind.needsToken ? token : nil
        if TaskListener.shared.setProvider(cfg, token: tokenToStore) {
            store.refreshProviders()
            onClose()
        } else {
            status = .error("Failed to save target.")
        }
    }
}
