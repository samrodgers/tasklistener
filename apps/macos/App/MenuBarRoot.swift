import SwiftUI
import TaskListenerCore

struct MenuBarRoot: View {
    @EnvironmentObject var store: AppStore
    @State private var draftText: String = ""
    @FocusState private var inputFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            captureRow
            Divider()
            taskList
            Divider()
            footer
        }
        .background(.regularMaterial)
    }

    private var header: some View {
        HStack {
            Text("TaskListener")
                .font(.headline)
            Spacer()
            Button {
                store.listening.toggle()
            } label: {
                Image(systemName: store.listening ? "pause.circle" : "play.circle")
                    .font(.title3)
            }
            .buttonStyle(.plain)
            .help(store.listening ? "Pause listening" : "Resume listening")

            Button {
                if let url = URL(string: "tasklistener://settings") {
                    NSWorkspace.shared.open(url)
                }
                NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
            } label: {
                Image(systemName: "gear")
                    .font(.title3)
            }
            .buttonStyle(.plain)
            .help("Settings")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    private var captureRow: some View {
        HStack {
            Image(systemName: "plus.circle")
                .foregroundColor(.secondary)
            TextField("Add a task", text: $draftText, onCommit: submit)
                .textFieldStyle(.plain)
                .focused($inputFocused)
            if !draftText.isEmpty {
                Button("Add", action: submit)
                    .keyboardShortcut(.return, modifiers: [])
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
    }

    private func submit() {
        store.captureManual(draftText)
        draftText = ""
    }

    private var taskList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                if store.tasks.isEmpty {
                    Text(store.audioIsReal
                         ? "Listening… nothing captured yet."
                         : "Audio pipeline is stubbed. Type a task above to test the rest of the app.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .padding(20)
                        .frame(maxWidth: .infinity)
                }
                ForEach(store.tasks) { task in
                    TaskRow(task: task,
                            destinations: store.destinationsByTask[task.id] ?? [])
                    Divider()
                }
            }
        }
    }

    private var footer: some View {
        HStack {
            if store.audioIsReal {
                Label("Listening", systemImage: "ear")
                    .font(.caption)
                    .foregroundColor(.secondary)
            } else {
                Label("Audio: stubbed", systemImage: "exclamationmark.bubble")
                    .font(.caption)
                    .foregroundColor(.orange)
            }
            Spacer()
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .font(.caption)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
    }
}

struct TaskRow: View {
    @EnvironmentObject var store: AppStore
    let task: Task
    let destinations: [TaskDestination]

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Button {
                store.setStatus(task, .done)
            } label: {
                Image(systemName: task.status == .done ? "checkmark.circle.fill" : "circle")
                    .foregroundColor(task.status == .done ? .green : .secondary)
            }
            .buttonStyle(.plain)
            .padding(.top, 2)

            VStack(alignment: .leading, spacing: 4) {
                Text(task.text)
                    .strikethrough(task.status == .done)
                if let due = task.due_hint {
                    Label(due, systemImage: "calendar")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                if !destinations.isEmpty {
                    HStack(spacing: 6) {
                        ForEach(destinations) { d in
                            DestinationBadge(destination: d)
                        }
                    }
                }
            }

            Spacer()

            Menu {
                Button("Delete", role: .destructive) { store.delete(task) }
                Divider()
                ForEach(store.providers.filter { $0.enabled }) { p in
                    Button("Push to \(p.display_name)") {
                        store.pushNow(task: task, provider: p)
                    }
                }
            } label: {
                Image(systemName: "ellipsis")
            }
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .frame(width: 16)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }
}

struct DestinationBadge: View {
    let destination: TaskDestination

    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: icon)
                .font(.system(size: 9, weight: .semibold))
            Text(destination.provider)
                .font(.system(size: 10))
        }
        .padding(.horizontal, 5)
        .padding(.vertical, 2)
        .background(background)
        .foregroundColor(foreground)
        .clipShape(Capsule())
        .help(destination.last_error ?? destination.state.rawValue)
    }

    private var icon: String {
        switch destination.state {
        case .pushed: return "checkmark"
        case .pushing: return "arrow.up.circle"
        case .pending: return "clock"
        case .failed, .dead_letter: return "exclamationmark.triangle"
        }
    }

    private var background: Color {
        switch destination.state {
        case .pushed: return .green.opacity(0.18)
        case .pushing, .pending: return .secondary.opacity(0.15)
        case .failed, .dead_letter: return .orange.opacity(0.22)
        }
    }

    private var foreground: Color {
        switch destination.state {
        case .pushed: return .green
        case .pushing, .pending: return .secondary
        case .failed, .dead_letter: return .orange
        }
    }
}
