import SwiftUI

@MainActor
final class WorkspaceStore: ObservableObject {
    @Published private(set) var raw = ""
    @Published private(set) var ready = false
    @Published private(set) var saving = false
    @Published var error: String?
    private let disk: WorkspaceDisk

    init(url: URL? = nil) {
        let target = url ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("TenRate/workspace.json")
        disk = WorkspaceDisk(url: target)
    }
    func load() async {
        guard !ready, !saving else { return }
        saving = true
        defer { saving = false }
        do {
            if let stored = try await disk.load() {
                // Validate without replacing a damaged or future-format file.
                let object = try Core.object(stored)
                guard (object["schemaVersion"] as? Int) == 15,
                      object["slots"] is [[String: Any]] else { throw CoreError.invalidData }
                _ = try Core.call(stored, ["action": "validate"])
                raw = stored
            } else {
                let initial = try Core.call("", ["action": "default"])
                try await disk.save(initial)
                raw = initial
            }
            ready = true
        } catch { self.error = error.localizedDescription }
    }
    @discardableResult
    func mutate(_ command: [String: Any]) async -> Bool {
        guard ready, !saving else { return false }
        saving = true
        defer { saving = false }
        do {
            let next = try Core.call(raw, command)
            try await disk.save(next)
            raw = next
            return true
        } catch { self.error = error.localizedDescription; return false }
    }
    func timers(at date: Date) -> [TimerView] {
        guard ready,
              let projected = try? Core.call(raw, ["action": "project"], at: Int64(date.timeIntervalSince1970 * 1000)),
              let result = try? Core.decode(TimerProjection.self, projected) else { return [] }
        let positions = Dictionary(uniqueKeysWithValues: result.slotOrder.enumerated().map { ($0.element, $0.offset) })
        return result.slots.sorted { positions[$0.id, default: 0] < positions[$1.id, default: 0] }
    }
    private var object: [String: Any] { (try? Core.object(raw)) ?? [:] }
    private func array<T: Decodable>(_ key: String, as type: T.Type) -> [T] {
        guard let value = object[key], let data = try? JSONSerialization.data(withJSONObject: value) else { return [] }
        return (try? JSONDecoder().decode([T].self, from: data)) ?? []
    }
    var categories: [Category] { array("categories", as: Category.self) }
    var sessions: [TimerSession] { array("sessions", as: TimerSession.self).sorted { $0.endedAtEpochMillis > $1.endedAtEpochMillis } }
    var archives: [ArchivedTask] { array("archivedTasks", as: ArchivedTask.self) }
    var notes: [Note] { (object["notes"] as? [[String: Any]] ?? []).map { Note(value: $0) } }
    func categoryName(_ id: String?) -> String { categories.first { $0.id == id }?.name ?? "未分类" }
}
