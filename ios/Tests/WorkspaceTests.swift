import XCTest
@testable import TenRate

@MainActor
final class WorkspaceTests: XCTestCase {
    func testPlainNoteRemainsEditableAfterSaveAndRelaunch() async throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: folder) }
        let url = folder.appendingPathComponent("workspace.json")
        let store = WorkspaceStore(url: url)
        await store.load()
        let created = await store.mutate(["action": "saveNote", "note": ["id": "plain", "kind": "STICKY", "content": "First note"]])
        XCTAssertTrue(created)
        let note = try XCTUnwrap(store.notes.first)
        XCTAssertTrue(note.editable)
        var updated = note.value
        updated["content"] = "Edited note"
        let saved = await store.mutate(["action": "saveNote", "note": updated])
        XCTAssertTrue(saved)
        let reopened = WorkspaceStore(url: url)
        await reopened.load()
        XCTAssertEqual(reopened.notes.first?.content, "Edited note")
        XCTAssertEqual(reopened.notes.first?.editable, true)
    }
    func testRelaunchRetainsNamedTimer() async throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: folder) }
        let url = folder.appendingPathComponent("workspace.json")
        let store = WorkspaceStore(url: url)
        await store.load()
        XCTAssertTrue(store.ready)
        XCTAssertEqual(store.timers(at: .now).count, 14)
        let saved = await store.mutate(["action": "title", "slotId": 1, "text": "阅读 📚"])
        XCTAssertTrue(saved)
        let reopened = WorkspaceStore(url: url)
        await reopened.load()
        XCTAssertEqual(reopened.timers(at: .now).first?.title, "阅读 📚")
    }
    func testCorruptFileIsNeverOverwritten() async throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: url) }
        try Data("damaged".utf8).write(to: url)
        let store = WorkspaceStore(url: url)
        await store.load()
        XCTAssertFalse(store.ready)
        XCTAssertNotNil(store.error)
        let saved = await store.mutate(["action": "default"])
        XCTAssertFalse(saved)
        XCTAssertEqual(try String(contentsOf: url, encoding: .utf8), "damaged")
    }
    func testWriteFailureKeepsPreviousState() async throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: folder) }
        let url = folder.appendingPathComponent("workspace.json")
        let store = WorkspaceStore(url: url)
        await store.load()
        let previous = store.raw
        try FileManager.default.removeItem(at: folder)
        try Data("block directory creation".utf8).write(to: folder)
        let saved = await store.mutate(["action": "title", "slotId": 1, "text": "must not publish"])
        XCTAssertFalse(saved)
        XCTAssertEqual(store.raw, previous)
        XCTAssertNotNil(store.error)
    }
}
