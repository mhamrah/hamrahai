//
//  hamrah_ios_uitests.swift
//  hamrah-ios-uitests
//
//  Created by Mike Hamrah on 10/12/25.
//

import XCTest

final class hamrah_ios_uitests: XCTestCase {

    override func setUpWithError() throws {
        // Put setup code here. This method is called before the invocation of each test method in the class.

        // In UI tests it is usually best to stop immediately when a failure occurs.
        continueAfterFailure = false

        // In UI tests it’s important to set the initial state - such as interface orientation - required for your tests before they run. The setUp method is a good place to do this.
    }

    override func tearDownWithError() throws {
        // Put teardown code here. This method is called after the invocation of each test method in the class.
    }

    @MainActor
    func testExample() throws {
        // UI tests must launch the application that they test.
        let app = XCUIApplication()
        app.launch()

        // Use XCTAssert and related functions to verify your tests produce the correct results.
    }

    @MainActor
    func testMusicTabOpensMusicManagementWithoutALikedSongsCheckbox() throws {
        let app = XCUIApplication()
        app.launchArguments.append("--ui-testing-authenticated")
        app.launch()

        let musicTab = app.tabBars.buttons["Music"]
        XCTAssertTrue(musicTab.waitForExistence(timeout: 5))
        musicTab.tap()

        XCTAssertTrue(app.navigationBars["Music"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.switches["Sync Liked Songs"].exists)
    }

    @MainActor
    func testLaunchPerformance() throws {
        // This measures how long it takes to launch your application.
        measure(metrics: [XCTApplicationLaunchMetric()]) {
            XCUIApplication().launch()
        }
    }
}
