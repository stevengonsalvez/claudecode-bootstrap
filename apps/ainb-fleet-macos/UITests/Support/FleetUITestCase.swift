import XCTest

/// Base for the shell journeys, which drive the REAL app against a fixture
/// daemon rather than a mock.
///
/// These do not run in CI, and the attempt is recorded here so nobody repeats
/// it. A GitHub-hosted macOS runner has no usable GUI session for a real app:
/// the launch hangs for around two minutes and then fails with "Application
/// 'dev.ainb.fleet' does not have a process ID", and it does so for every
/// journey, including ones that pass locally in seconds. Signing is not the
/// problem, and a UI runner does need signing, so the step cannot simply borrow
/// the CODE_SIGNING_ALLOWED=NO the unit step above it uses.
///
/// Run them by hand, from the repository root:
///
///     xcodebuild test \
///       -project apps/ainb-fleet-macos/AINBFleet.xcodeproj \
///       -scheme FleetUITests \
///       -destination 'platform=macOS'
///
/// Two things the machine needs first. The terminal invoking xcodebuild must be
/// allowed under Privacy and Security, Accessibility, or every journey dies at
/// "Timed out while enabling automation mode" before a single test is selected.
/// And when that timeout appears anyway on a machine that has the permission,
/// `testmanagerd` is wedged: kill it by its exact process id, let launchd
/// restart it, and re-run. That happened twice in one session while these
/// journeys were being written.
///
/// The fixture daemon binary is resolved as
/// `<repo>/ainb-tui/target/debug/examples/fleet_fixture_daemon`, so build it
/// first. `AINB_FLEET_FIXTURE_DAEMON` is read by the runner process but does
/// NOT survive being set in the shell that invokes xcodebuild; symlink the
/// binary into place instead.

class FleetUITestCase: XCTestCase {
    var fixture: FleetFixtureDaemon!
    var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        fixture = try FleetFixtureDaemon()
        app = XCUIApplication()
    }

    override func tearDownWithError() throws {
        app?.terminate()
        fixture?.stop(removeHome: true)
    }

    @MainActor
    func launchApp(arguments: [String] = []) {
        if app.state != .notRunning {
            app.terminate()
        }
        app.launchEnvironment["AINB_HANGAR_HOME"] = fixture.home.path
        app.launchEnvironment["AINB_FLEET_TEST_ISOLATE_DEFAULTS"] = "1"
        app.launchEnvironment["AINB_FLEET_UI_TEST_MODE"] = "1"
        if arguments.contains("--fleet-test-read-range=3...3") {
            app.launchEnvironment["AINB_FLEET_TEST_READ_RANGE"] = "3...3"
        }
        app.launchArguments = arguments
        app.launch()
    }

    @MainActor
    func launchExpandedNotch(
        arguments: [String] = [],
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        launchApp(arguments: arguments)
        let notch = app.buttons["fleet.notch"]
        XCTAssertTrue(notch.waitForExistence(timeout: 8), "Fleet notch missing. \(app.debugDescription)", file: file, line: line)
        notch.click()
        XCTAssertTrue(app.textFields["fleet.notch.search"].waitForExistence(timeout: 8), "Fleet notch did not expand. \(app.debugDescription)", file: file, line: line)
    }

    @MainActor
    func waitFor(_ element: XCUIElement, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertTrue(
            element.waitForExistence(timeout: 8),
            "Missing \(element). \(app.debugDescription)",
            file: file,
            line: line
        )
    }

    @MainActor
    func fleetRow(_ sessionKey: String) -> XCUIElement {
        app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", "fleet.notch.row.\(sessionKey)"))
            .firstMatch
    }

    @MainActor
    func fleetDetail(_ sessionKey: String) -> XCUIElement {
        app.staticTexts["fleet.notch.detail.\(sessionKey)"]
    }

    @MainActor
    func textValue(beginningWith value: String) -> XCUIElement {
        app.staticTexts
            .matching(NSPredicate(format: "value BEGINSWITH %@", value))
            .firstMatch
    }

}
