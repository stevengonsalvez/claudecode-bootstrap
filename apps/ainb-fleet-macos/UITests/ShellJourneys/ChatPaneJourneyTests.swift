import XCTest

/// The copilot conversation, driven through the real app against the real
/// daemon fixture.
///
/// Nothing had ever opened this pane. Every other proof of it is a unit test
/// against the store or a contract test against the wire, and both of those
/// pass perfectly well while the surface is unreachable or dark. These two
/// journeys are the only place the pane itself is asserted on.
final class ChatPaneJourneyTests: FleetUITestCase {
    /// Opening Chat resolves a scope AND a recipient, so the composer can send.
    ///
    /// This is the assertion that would have caught the bug PR 875 fixed at the
    /// pane instead of at the wire: a scope resolved but no session minted
    /// leaves the pane looking populated while Send is dead and the daemon's
    /// refusal sits in a caption nobody reads.
    @MainActor
    func testChatRouteResolvesAScopeAndALiveComposer() throws {
        launchExpandedNotch()
        openChatRoute()

        waitFor(app.staticTexts["fleet.chat.scope"])
        XCTAssertFalse(
            app.otherElements["fleet.chat.unavailable"].exists,
            "the fixture daemon serves chat, so the pane must not be in its unavailable state. \(app.debugDescription)"
        )
        // The daemon's refusal caption. Present only when the copilot session
        // could NOT be resolved, which is exactly the dead-Send state.
        XCTAssertFalse(
            app.staticTexts["fleet.chat.session-detail"].exists,
            "the copilot session was refused: \(app.staticTexts["fleet.chat.session-detail"].label)"
        )

        let composer = chatComposer()
        waitFor(composer)
        composer.click()
        composer.typeText("what is blocked?")

        let send = app.buttons["fleet.chat.send"]
        waitFor(send)
        XCTAssertTrue(
            send.isEnabled,
            "Send stayed dead, so the pane resolved no recipient. \(app.debugDescription)"
        )
    }

    /// A message filed by SOMEBODY ELSE appears without the pane being
    /// refreshed by hand.
    ///
    /// The whole of PR B, observed from outside the app. The writer is a second
    /// connection on the same socket, standing in for the copilot, the terminal
    /// UI or another window, and Refresh is never clicked.
    ///
    /// The clock is the load-bearing part. The pane's own safety-net page runs
    /// thirty seconds apart (`fleetChatSafetyNetInterval`), so an assertion that
    /// took longer than that could be satisfied by a poll and would prove
    /// nothing about the push. The elapsed time is asserted, not assumed.
    @MainActor
    func testACommittedMessageArrivesWithoutTouchingRefresh() throws {
        launchExpandedNotch()
        openChatRoute()

        // The bootstrap page has run: the scope is resolved and the timeline is
        // empty. Everything after this point is the stream's work.
        waitFor(app.staticTexts["fleet.chat.scope"])
        waitFor(app.staticTexts["fleet.chat.timeline.empty"])
        let pagedAt = Date()

        let client = try FleetChatClient(home: fixture.home)
        let messageID = try client.sendToCopilotChannel(text: "pushed without a poll")

        let row = app.descendants(matching: .any)["fleet.chat.message.\(messageID)"]
        XCTAssertTrue(
            row.waitForExistence(timeout: 8),
            "a message committed by another client never reached the open pane. \(app.debugDescription)"
        )
        // Twenty, not thirty. Thirty is the safety-net interval itself, so a
        // run that landed at 29.9 seconds would pass while proving nothing: the
        // page could have been the thing that delivered the row. The margin is
        // what makes this assertion mean "the stream did it". A healthy run
        // takes a couple of seconds, so the margin costs nothing.
        XCTAssertLessThan(
            Date().timeIntervalSince(pagedAt), 20,
            "the row arrived too close to a safety-net page to prove the stream delivered it"
        )
        XCTAssertFalse(
            app.staticTexts["fleet.chat.timeline.empty"].exists,
            "the empty placeholder outlived the message that filled the timeline"
        )

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = "chat-pane-live-push"
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }

    // MARK: - Helpers

    /// Move the notch to its Chat route.
    ///
    /// A RADIO GROUP, not a segmented control. A SwiftUI `Picker` with
    /// `.pickerStyle(.segmented)` is published to the accessibility tree as a
    /// `RadioGroup` of `RadioButton`s on this platform; `app.segmentedControls`
    /// matches nothing at all. That mismatch is what
    /// `MenuBarRosterJourneyTests` was failing on, and it is a stale query
    /// rather than anything wrong with the control, which carries its
    /// identifier and its labels correctly.
    @MainActor
    private func openChatRoute(file: StaticString = #filePath, line: UInt = #line) {
        let chat = app.radioGroups["fleet.notch.route"].radioButtons["Chat"]
        guard chat.waitForExistence(timeout: 8) else {
            return XCTFail("no Chat route control in the notch. \(app.debugDescription)", file: file, line: line)
        }
        chat.click()
    }

    /// The composer, which is a vertical `TextField` and so can present as
    /// either a text field or a text view depending on the run.
    @MainActor
    private func chatComposer() -> XCUIElement {
        let field = app.textFields["fleet.chat.composer"]
        return field.exists ? field : app.textViews["fleet.chat.composer"]
    }
}

