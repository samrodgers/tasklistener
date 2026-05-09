import Foundation
import Combine
import TaskListenerCore

/// Single observable that mirrors core state into SwiftUI. Subscribes to the
/// core event bus and re-fetches affected views.
@MainActor
final class AppStore: ObservableObject {
    @Published var tasks: [Task] = []
    @Published var providers: [ProviderView] = []
    @Published var destinationsByTask: [String: [TaskDestination]] = [:]
    @Published var listening: Bool = true
    @Published var audioIsReal: Bool = false

    private var bag = Set<AnyCancellable>()
    let remindersBridge = AppleRemindersBridge()

    init() {
        refreshTasks()
        refreshProviders()
        audioIsReal = TaskListener.shared.audioIsReal

        TaskListener.shared.events
            .receive(on: DispatchQueue.main)
            .sink { [weak self] event in
                guard let self else { return }
                switch event {
                case .taskCreated(let id):
                    self.refreshTasks()
                    self.routeRemindersPush(taskId: id)
                case .taskDeleted:
                    self.refreshTasks()
                case .taskUpdated(let id):
                    self.refreshTask(id: id)
                case .destinationStateChanged(let taskId, _, _):
                    self.refreshTask(id: taskId)
                case .providerConnected, .providerDisconnected:
                    self.refreshProviders()
                }
            }
            .store(in: &bag)
    }

    /// Apple Reminders is handled entirely Swift-side via EventKit.
    /// On every new task, fan it out to any enabled "reminders" provider.
    private func routeRemindersPush(taskId: String) {
        let remindersProviders = providers.filter {
            $0.kind == "reminders" && $0.enabled && $0.auto_push && $0.target_id != nil
        }
        guard !remindersProviders.isEmpty,
              let (task, _) = TaskListener.shared.taskWithDestinations(id: taskId) else { return }
        for p in remindersProviders {
            remindersBridge.push(task: task, provider: p)
        }
    }

    var iconName: String {
        if !listening { return "mic.slash" }
        return audioIsReal ? "ear" : "ear.badge.checkmark"
    }

    func refreshTasks() {
        tasks = TaskListener.shared.listTasks(includeDone: false, limit: 200)
        for t in tasks {
            refreshTask(id: t.id)
        }
    }

    func refreshTask(id: String) {
        if let (_, dests) = TaskListener.shared.taskWithDestinations(id: id) {
            destinationsByTask[id] = dests
        }
        tasks = TaskListener.shared.listTasks(includeDone: false, limit: 200)
    }

    func refreshProviders() {
        providers = TaskListener.shared.listProviders()
    }

    func captureManual(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        TaskListener.shared.captureManual(trimmed)
    }

    func setStatus(_ task: Task, _ status: TaskStatus) {
        TaskListener.shared.setStatus(id: task.id, status: status)
    }

    func delete(_ task: Task) {
        TaskListener.shared.delete(id: task.id)
    }

    func pushNow(task: Task, provider: ProviderView) {
        TaskListener.shared.pushNow(taskId: task.id, providerId: provider.id)
    }
}
