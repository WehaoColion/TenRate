import Foundation

enum CoreError: LocalizedError {
    case rejected, invalidData, notReady
    var errorDescription: String? {
        switch self {
        case .rejected: return "操作未能完成，原有数据已保留。"
        case .invalidData: return "数据无法读取。请保留原文件并从备份恢复。"
        case .notReady: return "数据尚未载入，请稍后重试。"
        }
    }
}

enum Core {
    static var now: Int64 { Int64(Date().timeIntervalSince1970 * 1000) }
    static func call(_ raw: String, _ command: [String: Any], at now: Int64 = Core.now) throws -> String {
        let commandData = try JSONSerialization.data(withJSONObject: command)
        guard let commandString = String(data: commandData, encoding: .utf8) else { throw CoreError.invalidData }
        return try raw.withCString { rawPointer in
            try commandString.withCString { commandPointer in
                guard let result = tenrate_apply(rawPointer, commandPointer, now) else { throw CoreError.rejected }
                defer { tenrate_string_free(result) }
                return String(cString: result)
            }
        }
    }
    static func object(_ raw: String) throws -> [String: Any] {
        guard let data = raw.data(using: .utf8),
              let value = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw CoreError.invalidData }
        return value
    }
    static func decode<T: Decodable>(_ type: T.Type, _ raw: String) throws -> T {
        try JSONDecoder().decode(type, from: Data(raw.utf8))
    }
    static func duration(_ milliseconds: Int64) -> String {
        let seconds = max(0, milliseconds / 1000)
        return String(format: "%02lld:%02lld:%02lld", seconds / 3600, seconds / 60 % 60, seconds % 60)
    }
}

struct Category: Decodable, Identifiable { let id: String; let name: String }
struct TimerView: Decodable, Identifiable {
    let id: Int
    let title: String
    let categoryId: String?
    let accumulatedMillis: Int64
    let isRunning: Bool
    let microBreakPhase: String
}
struct TimerProjection: Decodable { let slots: [TimerView]; let slotOrder: [Int] }
struct TimerSession: Decodable, Identifiable {
    let id: String
    let slotTitle: String
    let durationMillis: Int64
    let endedAtEpochMillis: Int64
}
struct ArchivedTask: Decodable, Identifiable {
    let id: String
    let title: String
    let accumulatedMillis: Int64
}
struct Note: Identifiable {
    let value: [String: Any]
    var id: String { value["id"] as? String ?? "" }
    var title: String { value["title"] as? String ?? "" }
    var content: String { value["content"] as? String ?? "" }
    var deleted: Bool { value["deletedAtEpochMillis"] is NSNumber }
    var editable: Bool {
        let document = value["document"] as? [String: Any] ?? [:]
        let blocks = document["blocks"] as? [[String: Any]] ?? []
        let plainBlocks = blocks.isEmpty || (blocks.count == 1
            && blocks[0]["id"] as? String == "legacy-text"
            && blocks[0]["type"] as? String == "TEXT"
            && (blocks[0]["knowledge"] == nil || blocks[0]["knowledge"] is NSNull)
            && (blocks[0]["attachmentId"] == nil || blocks[0]["attachmentId"] is NSNull))
        return (value["encryption"] == nil || value["encryption"] is NSNull)
            && (value["kind"] as? String ?? "STICKY") == "STICKY"
            && plainBlocks && (value["attachments"] as? [Any] ?? []).isEmpty
            && (document["richTextEnabled"] as? Bool != true)
            && (document["knowledge"] == nil || document["knowledge"] is NSNull)
    }
}

/// Serializes writes off the UI thread. A failed write never publishes new state.
actor WorkspaceDisk {
    let url: URL
    init(url: URL) { self.url = url }
    func load() throws -> String? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        let data = try Data(contentsOf: url)
        guard let text = String(data: data, encoding: .utf8) else { throw CoreError.invalidData }
        return text
    }
    func save(_ raw: String) throws {
        let directory = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try Data(raw.utf8).write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }
}
