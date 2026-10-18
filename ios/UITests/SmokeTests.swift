import XCTest

final class SmokeTests: XCTestCase {
    func testTimerSheetAndNoteOnIPhone() {
        let app = XCUIApplication()
        app.launch()
        let open = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "格子 1")).firstMatch
        XCTAssertTrue(open.waitForExistence(timeout: 20))
        open.tap()
        let name = app.textFields["任务名称"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("Reading")
        app.buttons["保存名称"].tap()
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = "iPhone-slot-sheet"
        shot.lifetime = .keepAlways
        add(shot)
        app.buttons["关闭快捷操作"].tap()
        app.tabBars.buttons["便签"].tap()
        app.buttons["新建便签"].tap()
        let editor = app.textViews["便签正文"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        editor.tap()
        editor.typeText("First iPhone note")
        app.buttons["保存"].tap()
        XCTAssertTrue(app.staticTexts["First iPhone note"].waitForExistence(timeout: 5))
        app.terminate()
        app.launch()
        app.tabBars.buttons["便签"].tap()
        XCTAssertTrue(app.staticTexts["First iPhone note"].waitForExistence(timeout: 5))
    }
}
