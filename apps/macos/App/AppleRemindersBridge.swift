import Foundation
import EventKit
import TaskListenerCore

/// Apple Reminders integration. Lives in the Swift app because EventKit is
/// macOS-only and runs in-process; the Rust core doesn't see it. Subscribes
/// to `taskCreated` events, pushes via EKEventStore, then records the result
/// back to core through `recordExternalPush`.
@MainActor
final class AppleRemindersBridge {
    private let store = EKEventStore()

    func requestAccess() async -> Bool {
        do {
            if #available(macOS 14, *) {
                return try await store.requestFullAccessToReminders()
            } else {
                return try await withCheckedThrowingContinuation { cont in
                    store.requestAccess(to: .reminder) { granted, err in
                        if let err { cont.resume(throwing: err) }
                        else { cont.resume(returning: granted) }
                    }
                }
            }
        } catch {
            return false
        }
    }

    /// All visible reminder lists. Used by the connect-sheet target picker.
    func availableLists() -> [ProviderTarget] {
        store.calendars(for: .reminder).map { cal in
            ProviderTarget(id: cal.calendarIdentifier, label: cal.title)
        }
    }

    /// Push a task to the configured Reminders list, then record back to core.
    func push(task: Task, provider: ProviderView) {
        guard let listId = provider.target_id,
              let calendar = store.calendars(for: .reminder)
                .first(where: { $0.calendarIdentifier == listId }) else {
            TaskListener.shared.recordExternalPush(
                taskId: task.id,
                providerId: provider.id,
                error: "no Reminders list picked"
            )
            return
        }

        let reminder = EKReminder(eventStore: store)
        reminder.title = task.text
        reminder.calendar = calendar
        if let snippet = task.source_snippet {
            reminder.notes = "Captured by TaskListener\n\n> \(snippet)"
        }
        // due_hint is free-form natural language for now — Reminders doesn't
        // parse it. We stash it in the notes; v3 will resolve to a real date.
        if let due = task.due_hint {
            reminder.notes = (reminder.notes ?? "") + "\n\nDue hint: \(due)"
        }

        do {
            try store.save(reminder, commit: true)
            TaskListener.shared.recordExternalPush(
                taskId: task.id,
                providerId: provider.id,
                externalId: reminder.calendarItemIdentifier,
                externalURL: nil
            )
        } catch {
            TaskListener.shared.recordExternalPush(
                taskId: task.id,
                providerId: provider.id,
                error: error.localizedDescription
            )
        }
    }
}
