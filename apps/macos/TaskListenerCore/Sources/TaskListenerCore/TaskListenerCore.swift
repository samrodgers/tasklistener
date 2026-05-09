import Foundation
import Combine
import CTaskListener

// MARK: - Models

public enum TaskStatus: String, Codable, Sendable {
    case open, done, dismissed
}

public enum DestinationState: String, Codable, Sendable {
    case pending, pushing, pushed, failed, dead_letter
}

public struct Task: Codable, Identifiable, Sendable, Equatable {
    public let id: String
    public let text: String
    public let due_hint: String?
    public let source_snippet: String?
    public let captured_at: Date
    public let status: TaskStatus
    public let confidence: Float
}

public struct TaskDestination: Codable, Identifiable, Sendable, Equatable {
    public let id: String
    public let task_id: String
    public let provider: String
    public let external_id: String?
    public let external_url: String?
    public let pushed_at: Date?
    public let last_error: String?
    public let state: DestinationState
    public let attempts: Int
}

public struct ProviderView: Codable, Identifiable, Sendable, Equatable {
    public let id: String
    public let kind: String
    public let display_name: String
    public let enabled: Bool
    public let config_json: String
    public let min_confidence: Float
    public let auto_push: Bool
    public let target_id: String?
    public let target_label: String?
    public let token_masked: String?
}

public struct ProviderTarget: Codable, Identifiable, Sendable, Hashable {
    public let id: String
    public let label: String
    public init(id: String, label: String) {
        self.id = id
        self.label = label
    }
}

public enum CoreEvent: Sendable {
    case taskCreated(String)
    case taskUpdated(String)
    case taskDeleted(String)
    case destinationStateChanged(taskId: String, provider: String, state: DestinationState)
    case providerConnected(String)
    case providerDisconnected(String)
}

// MARK: - Core API

@_cdecl("taskListenerEventCallback")
private func taskListenerEventCallback(jsonPtr: UnsafePointer<CChar>?, ctx: UnsafeMutableRawPointer?) {
    guard let jsonPtr else { return }
    let json = String(cString: jsonPtr)
    DispatchQueue.main.async {
        if let ev = TaskListener.decodeEvent(json) {
            TaskListener.shared.events.send(ev)
        }
    }
}

@MainActor
public final class TaskListener: ObservableObject {
    public static let shared = TaskListener()

    public let events = PassthroughSubject<CoreEvent, Never>()
    @Published public private(set) var audioIsReal: Bool = false

    private var started = false

    private init() {}

    public func start(dbPath: String? = nil) {
        guard !started else { return }
        let result = (dbPath?.withCString { tl_start($0) }) ?? tl_start(nil)
        if result != 0 {
            assertionFailure("tl_start failed: \(result)")
            return
        }
        Self.installEventBridge()
        audioIsReal = tl_audio_is_real() != 0
        started = true
    }

    private static var bridgeInstalled = false
    private static func installEventBridge() {
        guard !bridgeInstalled else { return }
        bridgeInstalled = true
        _ = tl_subscribe(taskListenerEventCallback, nil)
    }

    fileprivate static func decodeEvent(_ json: String) -> CoreEvent? {
        struct Header: Decodable { let kind: String }
        guard let data = json.data(using: .utf8),
              let header = try? JSONDecoder().decode(Header.self, from: data) else { return nil }
        switch header.kind {
        case "task_created":
            struct E: Decodable { let task_id: String }
            return (try? JSONDecoder().decode(E.self, from: data)).map { .taskCreated($0.task_id) }
        case "task_updated":
            struct E: Decodable { let task_id: String }
            return (try? JSONDecoder().decode(E.self, from: data)).map { .taskUpdated($0.task_id) }
        case "task_deleted":
            struct E: Decodable { let task_id: String }
            return (try? JSONDecoder().decode(E.self, from: data)).map { .taskDeleted($0.task_id) }
        case "destination_state_changed":
            struct E: Decodable { let task_id: String; let provider: String; let state: DestinationState }
            return (try? JSONDecoder().decode(E.self, from: data)).map {
                .destinationStateChanged(taskId: $0.task_id, provider: $0.provider, state: $0.state)
            }
        case "provider_connected":
            struct E: Decodable { let provider: String }
            return (try? JSONDecoder().decode(E.self, from: data)).map { .providerConnected($0.provider) }
        case "provider_disconnected":
            struct E: Decodable { let provider: String }
            return (try? JSONDecoder().decode(E.self, from: data)).map { .providerDisconnected($0.provider) }
        default: return nil
        }
    }

    // MARK: - Tasks

    @discardableResult
    public func captureManual(_ text: String) -> String? {
        text.withCString { ptr in
            guard let cstr = tl_capture_manual(ptr) else { return nil }
            defer { tl_string_free(cstr) }
            return String(cString: cstr)
        }
    }

    public func listTasks(includeDone: Bool = false, limit: Int = 200) -> [Task] {
        guard let cstr = tl_list_tasks(includeDone ? 1 : 0, Int64(limit)) else { return [] }
        defer { tl_string_free(cstr) }
        struct Wrapper: Decodable { let tasks: [Task] }
        return decode(cstr, as: Wrapper.self)?.tasks ?? []
    }

    public func taskWithDestinations(id: String) -> (Task, [TaskDestination])? {
        return id.withCString { idPtr in
            guard let cstr = tl_get_task(idPtr) else { return nil }
            defer { tl_string_free(cstr) }
            struct Wrapper: Decodable { let task: Task; let destinations: [TaskDestination] }
            guard let w = decode(cstr, as: Wrapper.self) else { return nil }
            return (w.task, w.destinations)
        }
    }

    @discardableResult
    public func updateTaskText(id: String, text: String) -> Bool {
        id.withCString { i in
            text.withCString { t in
                tl_update_task_text(i, t) == 0
            }
        }
    }

    @discardableResult
    public func setStatus(id: String, status: TaskStatus) -> Bool {
        id.withCString { i in
            status.rawValue.withCString { s in
                tl_set_task_status(i, s) == 0
            }
        }
    }

    @discardableResult
    public func delete(id: String) -> Bool {
        id.withCString { tl_delete_task($0) == 0 }
    }

    // MARK: - Providers

    public func listProviders() -> [ProviderView] {
        guard let cstr = tl_list_providers() else { return [] }
        defer { tl_string_free(cstr) }
        struct Wrapper: Decodable { let providers: [ProviderView] }
        return decode(cstr, as: Wrapper.self)?.providers ?? []
    }

    @discardableResult
    public func setProvider(_ provider: ProviderConfigInput, token: String?) -> Bool {
        guard let json = try? JSONEncoder().encode(provider),
              let jsonString = String(data: json, encoding: .utf8) else { return false }
        return jsonString.withCString { j in
            if let token, !token.isEmpty {
                return token.withCString { t in tl_set_provider(j, t) == 0 }
            } else {
                return tl_set_provider(j, nil) == 0
            }
        }
    }

    public func listTargets(providerId: String) -> [ProviderTarget] {
        providerId.withCString { p in
            guard let cstr = tl_list_targets(p) else { return [] }
            defer { tl_string_free(cstr) }
            struct Wrapper: Decodable { let targets: [ProviderTarget] }
            return decode(cstr, as: Wrapper.self)?.targets ?? []
        }
    }

    @discardableResult
    public func deleteProvider(id: String) -> Bool {
        id.withCString { tl_delete_provider($0) == 0 }
    }

    @discardableResult
    public func pushNow(taskId: String, providerId: String) -> Bool {
        taskId.withCString { t in
            providerId.withCString { p in
                tl_push_now(t, p) == 0
            }
        }
    }

    @discardableResult
    public func recordExternalPush(
        taskId: String,
        providerId: String,
        externalId: String? = nil,
        externalURL: String? = nil,
        error: String? = nil
    ) -> Bool {
        func withOpt<R>(_ s: String?, _ body: (UnsafePointer<CChar>?) -> R) -> R {
            if let s { return s.withCString { body($0) } }
            return body(nil)
        }
        return taskId.withCString { tid in
            providerId.withCString { pid in
                withOpt(externalId) { eid in
                    withOpt(externalURL) { url in
                        withOpt(error) { err in
                            tl_record_external_push(tid, pid, eid, url, err) == 0
                        }
                    }
                }
            }
        }
    }

    private func decode<T: Decodable>(_ cstr: UnsafeMutablePointer<CChar>, as type: T.Type) -> T? {
        let s = String(cString: cstr)
        guard let data = s.data(using: .utf8) else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        // captured_at comes back as a unix timestamp from the core; tolerate both.
        decoder.dateDecodingStrategy = .custom { d in
            let c = try d.singleValueContainer()
            if let n = try? c.decode(Double.self) {
                return Date(timeIntervalSince1970: n)
            }
            if let str = try? c.decode(String.self) {
                let f = ISO8601DateFormatter()
                f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
                if let dt = f.date(from: str) { return dt }
                f.formatOptions = [.withInternetDateTime]
                if let dt = f.date(from: str) { return dt }
            }
            throw DecodingError.dataCorruptedError(in: c, debugDescription: "unrecognised date")
        }
        return try? decoder.decode(T.self, from: data)
    }
}

public struct ProviderConfigInput: Codable, Sendable {
    public var id: String
    public var kind: String
    public var display_name: String
    public var enabled: Bool
    public var config_json: String
    public var min_confidence: Float
    public var auto_push: Bool
    public var target_id: String?
    public var target_label: String?

    public init(
        id: String,
        kind: String,
        displayName: String,
        enabled: Bool = true,
        configJson: String = "{}",
        minConfidence: Float = 0.7,
        autoPush: Bool = true,
        targetId: String? = nil,
        targetLabel: String? = nil
    ) {
        self.id = id
        self.kind = kind
        self.display_name = displayName
        self.enabled = enabled
        self.config_json = configJson
        self.min_confidence = minConfidence
        self.auto_push = autoPush
        self.target_id = targetId
        self.target_label = targetLabel
    }
}
