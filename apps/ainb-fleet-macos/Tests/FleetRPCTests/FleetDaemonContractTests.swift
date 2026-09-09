import Foundation
import XCTest
@testable import AINBFleet

final class FleetDaemonContractTests: XCTestCase {
    func testRealDaemonRejectsUnauthenticatedClient() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.connection()
        defer { Task { await connection.close() } }

        do {
            try await connection.authenticate(token: "mdt_invalid")
            XCTFail("expected daemon token rejection")
        } catch let FleetConnectionError.rpc(error) {
            XCTAssertEqual(error.code, -32_000)
        }
    }

    func testRealDaemonAuthenticatesTokenFile() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        let snapshot = try await connection.snapshot()
        XCTAssertEqual(snapshot.headRevision, 0)
    }

    func testRealDaemonNegotiatesProtocol() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedConnection()
        defer { Task { await connection.close() } }

        let result = try await connection.negotiate()
        XCTAssertEqual(result.protocolVersion, 2)
        XCTAssertTrue(result.readCompatible)
        XCTAssertTrue(result.writeCompatible)
        XCTAssertTrue(result.capabilityIDs.contains("fleet.subscription.live"))
    }

    /// I9 bump-and-refuse: a client whose declared range excludes v2 is refused
    /// by the real daemon on BOTH legs, and the connection surfaces the read
    /// refusal as a typed error.
    func testRealDaemonRefusesV1OnlyClientOnBothLegs() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedConnection()
        defer { Task { await connection.close() } }

        do {
            _ = try await connection.negotiate(
                readVersions: FleetProtocolRange(min: 1, max: 1),
                writeVersions: FleetProtocolRange(min: 1, max: 1)
            )
            XCTFail("expected read-incompatible refusal")
        } catch let FleetConnectionError.protocolReadIncompatible(result) {
            XCTAssertEqual(result.protocolVersion, 2)
            XCTAssertFalse(result.readCompatible)
            XCTAssertFalse(result.writeCompatible)
        }
    }

    /// The provider after `acp` must be capability-only: a token this build has
    /// never heard of decodes to `.unknown` instead of failing the snapshot.
    func testProviderDecodeIsTolerantAndKnowsAcp() throws {
        func decode(_ raw: String) throws -> FleetProvider {
            try JSONDecoder().decode(FleetProvider.self, from: Data("\"\(raw)\"".utf8))
        }
        XCTAssertEqual(try decode("acp"), .acp)
        XCTAssertEqual(try decode("some-future-provider"), .unknown)
    }

    func testProviderDecodeKnowsAntigravity() throws {
        func decode(_ raw: String) throws -> FleetProvider {
            try JSONDecoder().decode(FleetProvider.self, from: Data("\"\(raw)\"".utf8))
        }
        XCTAssertEqual(try decode("antigravity"), .antigravity)
    }

    /// The write leg is the trap: reading v2 while writing v1 connects fine and
    /// then fails every action at `requireWriteCapability`. Assert the ranges
    /// the SHIPPED store declares, not the `FleetConnection` defaults alone.
    @MainActor
    func testShippedStoreDeclaresV2OnReadAndWriteLegs() {
        let store = FleetStore()
        XCTAssertEqual(store.readVersions, FleetProtocolRange(min: 1, max: 2))
        XCTAssertEqual(store.writeVersions, FleetProtocolRange(min: 1, max: 2))
    }

    func testRealDaemonReturnsTypedSnapshot() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        _ = try fixture.seed("snapshot")
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        let snapshot = try await connection.snapshot()
        XCTAssertEqual(snapshot.headRevision, 1)
        XCTAssertEqual(snapshot.sessions.map(\.sessionKey), ["claude:fixture-session"])
    }

    func testRealDaemonReplaysEventsAfterCursor() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let first = try fixture.seed("replay-1")
        _ = try fixture.seed("replay-2", eventType: "Stop")
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        let subscription = try await connection.subscribe(afterRevision: first)
        XCTAssertEqual(subscription.replayState, .complete)
        XCTAssertEqual(subscription.replay.map(\.eventID), ["replay-2"])
    }

    func testRealDaemonStreamsLiveEventsOnOpenSocket() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let first = try fixture.seed("live-1")
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }
        let stream = await connection.incoming()
        let subscription = try await connection.subscribe(afterRevision: first)
        XCTAssertEqual(subscription.replayState, .complete)

        async let incoming = Self.nextFleetEvent(from: stream)
        _ = try fixture.seed("live-2", eventType: "Stop")
        let event = try await incoming
        XCTAssertEqual(event.eventID, "live-2")
        XCTAssertGreaterThan(event.revision, first)
    }

    func testRealDaemonGapForcesSnapshotReset() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        _ = try fixture.seed("reset")
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        let subscription = try await connection.subscribe(afterRevision: 99)
        XCTAssertEqual(subscription.replay, [])
        XCTAssertEqual(subscription.replayState, .snapshotReset(reason: .cursorAhead))
    }

    func testRealDaemonMalformedFrameClosesConnection() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.connection()
        defer { Task { await connection.close() } }

        try await connection.sendRawFrameForTesting(Data("X-Unsupported: 1\r\n\r\n".utf8))
        try await Task.sleep(for: .milliseconds(100))
        do {
            try await connection.authenticate(token: try fixture.location.readToken())
            XCTFail("expected malformed frame to close real daemon connection")
        } catch let error as FleetConnectionError {
            XCTAssertTrue(error == .notConnected || error == .closed || error == .disconnected)
        }
    }

    func testRealDaemonCancellationStopsSubscription() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        _ = try fixture.seed("cancel")
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        let stream = await connection.incoming()
        _ = try await connection.subscribe(afterRevision: 1)

        let waiter = Task { () -> FleetIncoming? in
            var iterator = stream.makeAsyncIterator()
            return await iterator.next()
        }
        waiter.cancel()
        _ = await waiter.value
        await connection.close()

        let replacement = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await replacement.close() } }
        let snapshot = try await replacement.snapshot()
        XCTAssertEqual(snapshot.headRevision, 1)
    }

    // MARK: - Shared chat fixtures (buzz-port part 2)
    //
    // The SAME files the Rust `chat_fixtures` test round-trips
    // (ainb-tui/crates/ainb-hangar-proto/fixtures/chat). One fixture set, two
    // suites: a field that drifts on one side goes red on the other instead of
    // being found by an operator. Read from the repo, never copied here — a
    // copy is how two suites agree with each other and disagree with the wire.

    /// Decode one shared fixture and assert it re-encodes to the same JSON.
    private func assertFixtureRoundTrips<T: Codable>(_ name: String, as type: T.Type, file: StaticString = #filePath, line: UInt = #line) throws -> T {
        let original = try Self.chatFixture(named: name)
        let decoded = try FleetWire.decoder().decode(T.self, from: original)
        let encoded = try FleetWire.encoder().encode(decoded)
        XCTAssertEqual(
            try JSONSerialization.data(withJSONObject: JSONSerialization.jsonObject(with: original), options: [.sortedKeys]),
            try JSONSerialization.data(withJSONObject: JSONSerialization.jsonObject(with: encoded), options: [.sortedKeys]),
            "\(name) does not round-trip",
            file: file,
            line: line
        )
        return decoded
    }

    private static func chatFixturesDirectory() -> URL {
        var repository = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repository.deleteLastPathComponent() }
        return repository.appendingPathComponent("ainb-tui/crates/ainb-hangar-proto/fixtures/chat")
    }

    private static func chatFixture(named name: String) throws -> Data {
        try Data(contentsOf: chatFixturesDirectory().appendingPathComponent(name))
    }

    /// Every fixture the tests below decode. Rust's
    /// `every_fixture_is_claimed_by_a_type` reads the same directory against its
    /// own bound-type list, so it only ever catches drift on the RUST side; this
    /// is the mirror that makes a fixture added (or deleted) without a Swift
    /// decoder go red here too.
    private static let decodedChatFixtures: Set<String> = [
        "channel_create_params.json",
        "channel_create_result.json",
        "channel_list_result.json",
        "copilot_configure_params.json",
        "copilot_configure_result.json",
        "confirm_list_result.json",
        "confirm_answer_approve_params.json",
        "confirm_answer_edit_params.json",
        "confirm_answer_result.json",
        "confirm_event.json",
        "activity_list_params.json",
        "activity_list_result.json",
        "activity_event.json",
    ]

    func testEveryChatFixtureIsDecodedBySwift() throws {
        let present = try FileManager.default
            .contentsOfDirectory(at: Self.chatFixturesDirectory(), includingPropertiesForKeys: nil)
            .map(\.lastPathComponent)
            .filter { $0.hasSuffix(".json") }

        XCTAssertEqual(
            Set(present),
            Self.decodedChatFixtures,
            "fixtures/chat and the Swift-decoded set disagree"
        )
        XCTAssertEqual(present.count, Self.decodedChatFixtures.count, "duplicate fixture name")

        // And the set is not aspirational: each name really loads.
        for name in Self.decodedChatFixtures {
            XCTAssertNoThrow(try Self.chatFixture(named: name), "\(name) is not readable")
        }
    }

    func testChatChannelFixturesDecodeAndMintAChannelScope() throws {
        let params = try assertFixtureRoundTrips("channel_create_params.json", as: FleetChannelCreateParams.self)
        XCTAssertEqual(params.kind, .broadcast)
        XCTAssertEqual(params.recipients?.count, 3)

        let created = try assertFixtureRoundTrips("channel_create_result.json", as: FleetChannelCreateResult.self)
        XCTAssertEqual(created.channel.scopeKey, "channel:\(created.channel.id)")

        let listed = try assertFixtureRoundTrips("channel_list_result.json", as: FleetChannelListResult.self)
        XCTAssertEqual(listed.channels.first?.kind, .copilot)
        XCTAssertEqual(listed.channels.first?.recipients, [])
        XCTAssertTrue(listed.channels.allSatisfy { $0.scopeKey.hasPrefix("channel:") })
    }

    func testCopilotConfigureFixturesCarryNoPermissionMode() throws {
        let params = try assertFixtureRoundTrips("copilot_configure_params.json", as: FleetCopilotConfigureParams.self)
        // The registry KEY, not an enum token: `claude` would not name an adapter.
        XCTAssertEqual(params.provider, "claude-agent-acp")
        XCTAssertEqual(params.copilotMode, .guarded)
        XCTAssertEqual(params.reasoningEffort, "medium")

        // The absence is the contract: a settable permission mode would be a
        // remote off-switch for the guardrails.
        let raw = try JSONSerialization.jsonObject(with: try Self.chatFixture(named: "copilot_configure_params.json")) as? [String: Any]
        for forbidden in ["permission_mode", "permissionMode", "mode"] {
            XCTAssertNil(raw?[forbidden], "\(forbidden) must never appear on copilot_configure")
        }

        let result = try assertFixtureRoundTrips("copilot_configure_result.json", as: FleetCopilotConfigureResult.self)
        XCTAssertEqual(result.provider, "claude-agent-acp")
        XCTAssertEqual(result.copilotMode, .guarded)
        XCTAssertFalse(result.sessionReplaced)
        XCTAssertTrue(result.personaSet)
    }

    func testConfirmFixturesDecodeThroughTheFullLifecycle() throws {
        let list = try assertFixtureRoundTrips("confirm_list_result.json", as: FleetConfirmListResult.self)
        XCTAssertEqual(list.confirms.count, 2)
        XCTAssertTrue(list.confirms[0].isAnswerable)
        XCTAssertGreaterThan(list.confirms[0].expiresAt, list.confirms[0].createdAt)
        XCTAssertNil(list.confirms[1].targetSessionKey)

        let approve = try assertFixtureRoundTrips("confirm_answer_approve_params.json", as: FleetConfirmAnswerParams.self)
        XCTAssertEqual(approve.answer, .approve)

        let edit = try assertFixtureRoundTrips("confirm_answer_edit_params.json", as: FleetConfirmAnswerParams.self)
        guard case let .edit(arguments) = edit.answer else {
            return XCTFail("expected an edit answer")
        }
        XCTAssertEqual(arguments.value("provider")?.stringValue, "codex")

        let answered = try assertFixtureRoundTrips("confirm_answer_result.json", as: FleetConfirmAnswerResult.self)
        XCTAssertEqual(answered.state, .approved)

        let event = try assertFixtureRoundTrips("confirm_event.json", as: FleetConfirmEventParams.self)
        XCTAssertEqual(event.confirm.state, .expired)
        XCTAssertFalse(event.confirm.isAnswerable)
    }

    func testActivityFixturesPageByCommitSeq() throws {
        let params = try assertFixtureRoundTrips("activity_list_params.json", as: FleetActivityListParams.self)
        XCTAssertEqual(params.afterSeq, 41)

        let result = try assertFixtureRoundTrips("activity_list_result.json", as: FleetActivityListResult.self)
        XCTAssertEqual(result.activities.map(\.activityClass), [.write, .destructive])
        XCTAssertEqual(result.activities.last?.outcome, .expired)
        XCTAssertEqual(result.nextAfterSeq, result.activities.last?.seq)
        XCTAssertEqual(result.activities.map(\.seq), result.activities.map(\.seq).sorted())

        let event = try assertFixtureRoundTrips("activity_event.json", as: FleetActivityEventParams.self)
        XCTAssertEqual(event.activity.activityClass, .read)
        XCTAssertNil(event.activity.detail)
    }

    /// The part-2 mirror of `testProviderDecodeIsTolerantAndKnowsAcp`: an event
    /// kind this build has never heard of degrades that ONE value instead of
    /// failing the page, and it never degrades into an actionable state.
    func testPartTwoEventKindDecodeIsTolerantAndFailsSafe() throws {
        func decode<T: Decodable>(_ raw: String, as type: T.Type) throws -> T {
            try FleetWire.decoder().decode(T.self, from: Data("\"\(raw)\"".utf8))
        }
        XCTAssertEqual(try decode("copilot", as: FleetChannelKind.self), .copilot)
        XCTAssertEqual(try decode("some-future-kind", as: FleetChannelKind.self), .unknown)
        // The provider is a registry STRING now, so there is no token to fall
        // back on; the dial is the enum that has to stay tolerant.
        XCTAssertEqual(try decode("some-future-mode", as: FleetCopilotMode.self), .unknown)
        XCTAssertEqual(try decode("some-future-state", as: FleetConfirmState.self), .unknown)
        XCTAssertEqual(try decode("some-future-class", as: FleetActivityClass.self), .unknown)
        XCTAssertEqual(try decode("some-future-outcome", as: FleetActivityOutcome.self), .unknown)

        // A whole frame carrying unknown tokens still decodes, and the card it
        // describes is NOT offered as answerable.
        let frame = Data(#"{"confirm":{"confirm_id":"01J0FUTURE","scope_key":"channel:copilot","tool":"future_tool","arguments":{},"state":"quarantined","created_at":1,"expires_at":2}}"#.utf8)
        let event = try FleetWire.decoder().decode(FleetConfirmEventParams.self, from: frame)
        XCTAssertEqual(event.confirm.state, .unknown)
        XCTAssertFalse(event.confirm.isAnswerable)

        let row = Data(#"{"activity":{"seq":1,"id":"01J0FUTURE","scope_key":"channel:copilot","tool":"future_tool","class":"quantum","outcome":"pending","created_at":1}}"#.utf8)
        let activity = try FleetWire.decoder().decode(FleetActivityEventParams.self, from: row)
        XCTAssertEqual(activity.activity.activityClass, .unknown)
        XCTAssertEqual(activity.activity.outcome, .unknown)
    }

    /// A part-2 capability is advertised exactly when its dispatch arm exists.
    /// This is the client-side half of the Rust advertisement test, and it is
    /// the assertion a UI depends on: gating a chat surface on the catalogue is
    /// only safe if the catalogue never names a method that answers -32601.
    ///
    /// It asserted the NEGATIVE while the arms were unlanded. Flipping it is
    /// the point rather than a chore: the day the arms land, this test is what
    /// says the client may now offer the surface.
    func testRealDaemonAdvertisesPartTwoCapabilitiesWithTheirArms() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedConnection()
        defer { Task { await connection.close() } }

        let result = try await connection.negotiate()
        for capability in ["fleet.chat.write", "fleet.chat.read", "fleet.copilot.configure", "fleet.confirm.answer"] {
            XCTAssertTrue(
                result.capabilityIDs.contains(capability),
                "\(capability) has a dispatch arm but is not advertised, so a UI gating on the catalogue stays dark"
            )
        }
    }

    /// The attach this client actually sends, against a real daemon: a create
    /// naming NEITHER `provider` nor `cwd` answers with the session the scope
    /// already holds.
    ///
    /// The Swift-side unit tests prove which frame each rung builds; only this
    /// proves the daemon reads that frame the way this client means it. It is
    /// the whole fix: the live copilot scope is held by a session opened from a
    /// worktree, and a menu-bar app naming `$HOME` was refused `ScopeHeld` on
    /// every one-second poll, leaving the composer with nobody to send to.
    func testRealDaemonAttachesToAHeldScopeWhenNeitherHalfIsNamed() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        let channel = try await connection.channelCreate(
            FleetChannelCreateParams(kind: .copilot, name: "copilot", recipients: nil)
        ).channel
        // Held at a root no client could guess, which is the live shape.
        let opened = try await connection.acpSessionCreate(FleetAcpSessionCreateParams(
            provider: copilotDefaultProvider,
            cwd: "/work/worktree",
            scopeKey: channel.scopeKey
        ))

        let attached = try await connection.acpSessionCreate(
            FleetAcpSessionCreateParams(provider: nil, cwd: nil, scopeKey: channel.scopeKey)
        )
        XCTAssertEqual(
            attached.sessionKey, opened.sessionKey,
            "an attach naming neither half must answer with the standing session"
        )
        XCTAssertEqual(attached.scopeKey, channel.scopeKey)
    }

    /// A scope with NO live session refuses the same frame, naming the field,
    /// because the daemon has no root to resolve and may not invent one.
    ///
    /// This is rung 2's trigger. The wording is load-bearing: the client's
    /// ladder matches on the field name plus "required", so a daemon that
    /// reworded this to something that names neither would leave a fresh
    /// copilot channel unopenable rather than retried with a directory.
    @MainActor
    func testRealDaemonRefusesAnOmittedCwdOnAScopeWithNoSession() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        let channel = try await connection.channelCreate(
            FleetChannelCreateParams(kind: .copilot, name: "copilot", recipients: nil)
        ).channel

        do {
            _ = try await connection.acpSessionCreate(
                FleetAcpSessionCreateParams(provider: nil, cwd: nil, scopeKey: channel.scopeKey)
            )
            XCTFail("the daemon must not root a session it was given no root for")
        } catch let FleetConnectionError.rpc(refusal) {
            XCTAssertEqual(refusal.code, -32602, "\(refusal)")
            // The EXACT phrase the ladder anchors on, not "cwd" and "required"
            // loose in the string: the daemon prefixes parse failures with a
            // shape hint that names every field, so a loose assertion here
            // would pass against a refusal about a different one.
            XCTAssertTrue(refusal.message.contains("cwd is required"), "\(refusal)")
        }

        // And the rung that answers it lands, which is what the ladder does
        // next: the same scope, now named.
        let rooted = try await FleetStore.mintCopilotSession(
            scopeKey: channel.scopeKey,
            home: "/work/fresh",
            create: { try await connection.acpSessionCreate($0) }
        )
        XCTAssertEqual(rooted.scopeKey, channel.scopeKey)
        XCTAssertFalse(rooted.sessionKey.isEmpty)
    }

    /// The whole point of PR B, against a real daemon: a message committed by
    /// SOMEBODY ELSE arrives on this connection as a notification, with nothing
    /// polled in between.
    ///
    /// Two connections, because one client sending to itself proves nothing
    /// about a push: the sender already holds the result. The second connection
    /// is the copilot, the TUI, the CLI, or another window, and the assertion
    /// is that this one hears about it.
    ///
    /// The ack alone is deliberately not the assertion. `head_id` says the
    /// daemon parsed the frame; only the event says it registered the forwarder
    /// that makes the pane live, and that registration happens in `serve_conn`
    /// AFTER the response is queued, i.e. in code the ack cannot reach.
    func testRealDaemonPushesACommittedMessageToASubscribedConnection() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let reader = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await reader.close() } }
        let writer = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await writer.close() } }

        // The copilot conversation, and the ACP session that IS its membership:
        // a `channel:` scope only accepts a send addressed to one of its
        // members, and a copilot channel's member is the session on its scope.
        let channel = try await writer.channelCreate(
            FleetChannelCreateParams(kind: .copilot, name: "copilot", recipients: nil)
        ).channel
        let session = try await writer.acpSessionCreate(FleetAcpSessionCreateParams(
            provider: copilotDefaultProvider,
            cwd: "/work/worktree",
            scopeKey: channel.scopeKey
        ))

        let stream = await reader.incoming()
        let acknowledgement = try await reader.messageSubscribe()
        XCTAssertNil(acknowledgement.headID, "an empty log has no head")

        async let incoming = Self.nextMessageEvent(from: stream)
        let sent = try await writer.messageSend(FleetMessageSendParams(
            scopeKey: channel.scopeKey,
            targets: [session.sessionKey],
            originMessageID: nil,
            text: "what is blocked?",
            requestID: UUID().uuidString
        ))
        let event = try await incoming

        XCTAssertEqual(event.message.id, sent.messageID)
        XCTAssertEqual(event.message.scopeKey, channel.scopeKey)
        XCTAssertEqual(event.message.body, "what is blocked?")
        // The daemon's own record of who wrote it, which is what the pane
        // attributes the row to. A push that lost this would render another
        // client's message as unattributed.
        XCTAssertEqual(event.message.sender, "operator")
    }

    /// The same connection carries the fleet subscription AND the chat one.
    ///
    /// They are separate forwarders over one writer daemon-side, and this is
    /// the reason the notch does not need a second socket for live chat. A
    /// daemon that started replacing one forwarder with the other would leave
    /// the roster frozen the moment the chat pane opened, which is the kind of
    /// regression nothing else here would catch.
    func testRealDaemonServesFleetAndChatSubscriptionsOnOneConnection() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let first = try fixture.seed("both-1")
        let reader = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await reader.close() } }
        let writer = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await writer.close() } }

        let channel = try await writer.channelCreate(
            FleetChannelCreateParams(kind: .copilot, name: "copilot", recipients: nil)
        ).channel
        let session = try await writer.acpSessionCreate(FleetAcpSessionCreateParams(
            provider: copilotDefaultProvider,
            cwd: "/work/worktree",
            scopeKey: channel.scopeKey
        ))

        let stream = await reader.incoming()
        _ = try await reader.subscribe(afterRevision: first)
        _ = try await reader.messageSubscribe()

        async let messageEvent = Self.nextMessageEvent(from: stream)
        _ = try await writer.messageSend(FleetMessageSendParams(
            scopeKey: channel.scopeKey,
            targets: [session.sessionKey],
            originMessageID: nil,
            text: "still here",
            requestID: UUID().uuidString
        ))
        let chat = try await messageEvent
        XCTAssertEqual(chat.message.body, "still here")

        // And the fleet half is still live on the same socket afterwards.
        async let fleetEvent = Self.nextFleetEvent(from: stream)
        _ = try fixture.seed("both-2", eventType: "Stop")
        let roster = try await fleetEvent
        XCTAssertEqual(roster.eventID, "both-2")
    }

    // MARK: - The ACP transcript stream (PR C)

    /// `fleet.transcript.read` is advertised, which is what a UI gating on the
    /// catalogue depends on.
    ///
    /// The client-side half of the daemon's own advertisement test: gating a
    /// surface on the catalogue is only safe if the catalogue never names a
    /// method that answers -32601, and never omits one that works.
    /// `fleet.transcript.prune` is asserted too, and separately, because the
    /// destructive verb having its own id is the reason this pane can read a
    /// transcript without being able to delete one.
    func testRealDaemonAdvertisesTheTranscriptCapabilities() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedConnection()
        defer { Task { await connection.close() } }

        let result = try await connection.negotiate()
        XCTAssertTrue(
            result.capabilityIDs.contains("fleet.transcript.read"),
            "the transcript arms exist but are not advertised, so a UI gating on the catalogue stays dark"
        )
        XCTAssertTrue(
            result.capabilityIDs.contains("fleet.transcript.prune"),
            "the destructive verb must be its own id, or a read surface cannot be granted without a delete"
        )
    }

    /// The whole point of PR C, against a real daemon: a transcript chunk
    /// committed by somebody else reaches this connection as a notification,
    /// with nothing polled in between.
    ///
    /// The ack alone is deliberately not the assertion. `head_order` says the
    /// daemon parsed the frame; only the event says it registered the forwarder
    /// that makes the pane live, and that registration happens in `serve_conn`
    /// AFTER the response is queued, i.e. in code the ack cannot reach.
    ///
    /// The chunk is committed by the FIXTURE rather than by this client because
    /// there is no client-facing write for a transcript row: the production
    /// writer is the ACP pool, driven by a real adapter subprocess. The fixture
    /// writes the same `source='acp'` row through the same repo and rings the
    /// same bell; the RPC, the head read and the forwarder are all the daemon's
    /// own code under test.
    func testRealDaemonPushesACommittedTranscriptChunkToASubscribedConnection() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let reader = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await reader.close() } }

        let stream = await reader.incoming()
        let acknowledgement = try await reader.transcriptSubscribe(
            FleetTranscriptSubscribeParams(sessionKey: "acp:contract", afterOrder: nil)
        )
        XCTAssertNil(acknowledgement.headOrder, "an empty transcript has no head")

        async let incoming = Self.nextTranscriptEvent(from: stream)
        let order = try fixture.seedTranscript(
            eventID: "chunk-1",
            sessionKey: "acp:contract",
            payload: ["text": "reading the code"]
        )
        let event = try await incoming

        XCTAssertEqual(event.chunk.ingestOrder, order)
        XCTAssertEqual(event.chunk.eventID, "chunk-1")
        XCTAssertEqual(event.chunk.sessionKey, "acp:contract")
        XCTAssertEqual(event.chunk.eventType, "acp.message")
        XCTAssertEqual(event.chunk.payload.value("text")?.stringValue, "reading the code")

        // And the SURFACE reaches the operator, not just the frame: the same
        // fold the store runs, through the same taxonomy, on a chunk that came
        // off a real socket rather than out of a literal.
        var surface = FleetChatSurface()
        surface.targetSessionKey = "acp:contract"
        surface.apply(.transcript(event.chunk))
        XCTAssertEqual(surface.transcriptState.rows.map(\.body), ["reading the code"])
        XCTAssertEqual(surface.transcriptState.rows.map(\.lane), [.agent])
    }

    /// The paged half, against the real daemon: an UNCURSORED read answers the
    /// tail of this session, a cursored one still walks forward, and both are
    /// ascending. This is the fact the store's bootstrap is built on.
    ///
    /// The second session is what makes it mean anything. `ingest_order` is one
    /// global AUTOINCREMENT sequence, so the newest orders in the table are not
    /// the newest rows of a session; a client approximating a tail by naming
    /// `head - limit` gets the neighbour's rows, or none. That approximation is
    /// what the uncursored arm replaces, and only a real daemon proves the arm
    /// is wired to the read it claims.
    func testRealDaemonAnswersAnUncursoredTranscriptReadWithTheSessionTail() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        var orders: [Int64] = []
        for index in 0..<5 {
            orders.append(try fixture.seedTranscript(
                eventID: "page-\(index)",
                sessionKey: "acp:paged",
                payload: ["text": "line \(index)"]
            ))
        }
        // A NOISIER neighbour committed afterwards, so it owns every one of the
        // newest global orders.
        for index in 0..<8 {
            _ = try fixture.seedTranscript(
                eventID: "other-\(index)",
                sessionKey: "acp:elsewhere",
                payload: ["text": "not mine"]
            )
        }

        let tail = try await connection.transcriptList(FleetTranscriptListParams(
            sessionKey: "acp:paged",
            afterOrder: nil,
            limit: 2
        ))
        XCTAssertEqual(
            tail.chunks.map(\.eventID), ["page-3", "page-4"],
            "an uncursored read must answer the NEWEST page of this session, oldest first"
        )
        XCTAssertEqual(tail.nextAfterOrder, orders.last)
        XCTAssertTrue(
            tail.chunks.allSatisfy { $0.sessionKey == "acp:paged" },
            "the neighbour's rows must never appear in this session's transcript"
        )

        let forward = try await connection.transcriptList(FleetTranscriptListParams(
            sessionKey: "acp:paged",
            afterOrder: orders[0],
            limit: fleetTranscriptListMax
        ))
        XCTAssertEqual(
            forward.chunks.map(\.eventID), ["page-1", "page-2", "page-3", "page-4"],
            "the cursor is exclusive and forward, so a client resuming from a row it holds does not re-read it"
        )

        // The head the subscribe publishes is this session's own last order,
        // not the table's, which is what makes the stream and the tail meet.
        let acknowledgement = try await connection.transcriptSubscribe(
            FleetTranscriptSubscribeParams(sessionKey: "acp:paged", afterOrder: nil)
        )
        XCTAssertEqual(acknowledgement.headOrder, orders.last)
    }

    // MARK: - Reconcile and the copilot dial (PRs D and E)

    /// The reconcile frame reaches the daemon's own dispatch, against a real
    /// daemon, and is refused for a reason that is about the SESSION.
    ///
    /// The Swift unit tests prove which frame the case builds; only this proves
    /// the daemon parses `reconcile_structured` at all. A tag the daemon does
    /// not know fails at `parse_params` with `-32602` naming the action, which
    /// is a different refusal from the version conflict a known tag gets
    /// against a session that does not exist, and telling the two apart is the
    /// whole assertion.
    func testRealDaemonParsesTheReconcileActionRatherThanRejectingItsTag() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        do {
            _ = try await connection.action(FleetActionParams(
                sessionKey: "claude:absent",
                expectedVersion: 1,
                requestID: UUID().uuidString,
                action: .reconcileStructured(requestFingerprint: "sha256:interview")
            ))
            XCTFail("a session that does not exist must not produce a receipt")
        } catch let FleetConnectionError.rpc(refusal) {
            XCTAssertFalse(
                refusal.message.lowercased().contains("unknown variant"),
                "the daemon could not parse the action tag, so this client is naming a variant it does not have: \(refusal)"
            )
            XCTAssertFalse(
                refusal.message.contains("reconcile_structured"),
                "a refusal quoting the tag back is a parse failure, not a session one: \(refusal)"
            )
        }
    }

    /// `fleet/adapter_list` answers a real daemon's live registry, and every
    /// row decodes through this client's model.
    ///
    /// The built-in floor is what makes this assertable without a config file:
    /// `chat_adapters` falls back to the config seed, which on a fixture home
    /// is the two adapters compiled in. The assertion is on the SHAPE rather
    /// than the exact names, because the registry is host config and a machine
    /// with `[acp.adapters]` entries legitimately answers with more.
    func testRealDaemonNamesItsAdapterRegistry() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        let adapters = try await connection.adapterList().adapters
        XCTAssertFalse(
            adapters.isEmpty,
            "a daemon with no spawnable adapter cannot open a copilot at all, so the registry must never be empty"
        )
        XCTAssertTrue(
            adapters.contains { $0.name == copilotDefaultProvider },
            "the adapter the copilot scope is minted with must be one the registry offers: \(adapters.map(\.name))"
        )
        for adapter in adapters {
            XCTAssertFalse(adapter.name.isEmpty)
            XCTAssertFalse(adapter.command.isEmpty, "\(adapter.name) names no program to spawn")
            XCTAssertFalse(adapter.permissionMode.isEmpty, "\(adapter.name) reports no pinned permission mode")
        }
    }

    /// `fleet/copilot_configure` refuses an adapter the registry does not know,
    /// and says so as an invalid parameter rather than attempting a spawn.
    ///
    /// This is why `provider` can be a validated STRING on the wire. The client
    /// offers only names `fleet/adapter_list` gave it, and the daemon is the
    /// backstop for everything else, so a name that reached this call by any
    /// other route is never a spawn request for an arbitrary program.
    func testRealDaemonRefusesAnAdapterItsRegistryDoesNotKnow() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        do {
            _ = try await connection.copilotConfigure(FleetCopilotConfigureParams(
                provider: "not-an-adapter",
                copilotMode: nil,
                model: nil,
                reasoningEffort: nil,
                persona: nil
            ))
            XCTFail("the daemon must not accept a provider its registry cannot spawn")
        } catch let FleetConnectionError.rpc(refusal) {
            XCTAssertEqual(refusal.code, -32602, "\(refusal)")
            XCTAssertTrue(
                refusal.message.contains("unknown adapter"),
                "the refusal must name the cause an operator can fix: \(refusal)"
            )
        }
    }

    /// The engine swap, end to end against a real daemon: a configure naming a
    /// DIFFERENT adapter answers `session_replaced`, with a new session key on
    /// the same channel scope, and a same-adapter one does not.
    ///
    /// This is the fact the client's invalidation hangs on. `session_replaced`
    /// is the only signal that the copilot session key this client is holding
    /// is dead, and everything the store does with it, dropping the mint,
    /// disowning the in-flight page, refusing to carry the transcript, follows
    /// from believing it. A scripted socket can only prove the store reacts;
    /// this proves the daemon sends it, and sends it for the right call.
    ///
    /// The attach that follows is the other half. The client learns the
    /// replacement's key by re-minting, not from the configure result, so the
    /// assertion that matters is that an `acp_session_create` naming neither
    /// provider nor cwd now answers with the NEW session.
    func testRealDaemonSwapsTheCopilotEngineAndReportsTheReplacedSession() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        let adapters = try await connection.adapterList().adapters
        let names = adapters.map(\.name)
        guard let other = names.first(where: { $0 != copilotDefaultProvider }) else {
            throw XCTSkip("this daemon's registry holds one adapter, so there is no swap to make: \(names)")
        }

        let channel = try await connection.channelCreate(
            FleetChannelCreateParams(kind: .copilot, name: "copilot", recipients: nil)
        ).channel
        let opened = try await connection.acpSessionCreate(FleetAcpSessionCreateParams(
            provider: copilotDefaultProvider,
            cwd: "/work",
            scopeKey: channel.scopeKey
        ))

        // The same adapter is a settings change, not a swap.
        let unchanged = try await connection.copilotConfigure(FleetCopilotConfigureParams(
            provider: copilotDefaultProvider,
            copilotMode: .help,
            model: nil,
            reasoningEffort: nil,
            persona: nil
        ))
        XCTAssertFalse(
            unchanged.sessionReplaced,
            "a configure that did not change the adapter must not retire the conversation"
        )
        XCTAssertEqual(unchanged.sessionKey, opened.sessionKey)
        XCTAssertEqual(unchanged.copilotMode, .help)

        // A different adapter is a different process, so the session goes.
        let swapped = try await connection.copilotConfigure(FleetCopilotConfigureParams(
            provider: other,
            copilotMode: nil,
            model: nil,
            reasoningEffort: nil,
            persona: nil
        ))
        XCTAssertTrue(swapped.sessionReplaced, "swapping the adapter must retire the session it was running")
        XCTAssertNotEqual(
            swapped.sessionKey, opened.sessionKey,
            "a replaced session must carry a new key, or a client cannot tell it apart from the dead one"
        )
        XCTAssertEqual(swapped.provider, other)
        XCTAssertEqual(
            swapped.copilotMode, .help,
            "an omitted copilot_mode leaves the guardrail where the last call put it"
        )

        let attached = try await connection.acpSessionCreate(
            FleetAcpSessionCreateParams(provider: nil, cwd: nil, scopeKey: channel.scopeKey)
        )
        XCTAssertEqual(
            attached.sessionKey, swapped.sessionKey,
            "the re-mint the store does after a swap must land on the replacement, not the retired session"
        )
        XCTAssertEqual(attached.scopeKey, channel.scopeKey, "the swap keeps the channel's scope")
    }

    /// The permission mode is NOT settable per session, and the daemon refuses
    /// the frame before it parses anything else.
    ///
    /// A settable mode here is a remote off-switch for the whole permission
    /// surface, so this client must never grow a field for it. The assertion is
    /// against a hand-built frame rather than `FleetCopilotConfigureParams`,
    /// because the type deliberately has no such property: the test that the
    /// door is shut has to knock on it.
    func testRealDaemonRefusesAPermissionModeOnTheCopilotWire() async throws {
        let fixture = try FixtureDaemon()
        defer { fixture.stop() }
        let connection = try await fixture.authenticatedAndNegotiatedConnection()
        defer { Task { await connection.close() } }

        for forbidden in [ForbiddenModeKey.permissionMode, .mode] {
            do {
                _ = try await connection.requestForTesting(
                    "fleet/copilot_configure",
                    params: ForbiddenModeParams(provider: copilotDefaultProvider, key: forbidden),
                    result: FleetCopilotConfigureResult.self
                )
                XCTFail("\(forbidden.rawValue) must be refused on the copilot wire")
            } catch let FleetConnectionError.rpc(refusal) {
                XCTAssertEqual(refusal.code, -32602, "\(forbidden.rawValue): \(refusal)")
                XCTAssertTrue(
                    refusal.message.contains("not settable per session"),
                    "\(forbidden.rawValue): \(refusal)"
                )
            }
        }
    }

    /// The keys the daemon refuses outright on `fleet/copilot_configure`.
    private enum ForbiddenModeKey: String {
        case permissionMode = "permission_mode"
        case mode
    }

    /// A configure frame carrying one of the refused keys.
    ///
    /// Hand-encoded because the shipped params type cannot carry them, which is
    /// the property under test.
    private struct ForbiddenModeParams: Encodable {
        let provider: String
        let key: ForbiddenModeKey

        private struct RawKey: CodingKey {
            let stringValue: String
            var intValue: Int? { nil }
            init(_ stringValue: String) { self.stringValue = stringValue }
            init?(stringValue: String) { self.stringValue = stringValue }
            init?(intValue: Int) { nil }
        }

        func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: RawKey.self)
            try container.encode(provider, forKey: RawKey("provider"))
            try container.encode("bypassPermissions", forKey: RawKey(key.rawValue))
        }
    }

    /// The next `fleet/transcript_event`, or a FAILURE within `timeout`.
    ///
    /// Bounded for the reason its message sibling is: the failure under test is
    /// one where the notification simply never comes, and an unbounded await
    /// turns that into a suite that hangs instead of a test that fails.
    private static func nextTranscriptEvent(
        from stream: AsyncStream<FleetIncoming>,
        timeout: Duration = .seconds(5)
    ) async throws -> FleetTranscriptEventParams {
        try await withThrowingTaskGroup(of: FleetTranscriptEventParams?.self) { group in
            group.addTask {
                var iterator = stream.makeAsyncIterator()
                while let incoming = await iterator.next() {
                    if case let .transcriptEvent(event) = incoming {
                        return event
                    }
                }
                return nil
            }
            group.addTask {
                try await Task.sleep(for: timeout)
                return nil
            }
            let first = try await group.next() ?? nil
            group.cancelAll()
            guard let event = first else {
                throw MissingChatNotification()
            }
            return event
        }
    }

    /// The next `fleet/message_event`, or a FAILURE within `timeout`.
    ///
    /// Bounded, unlike its `nextFleetEvent` sibling, because the thing it waits
    /// for is a push that has to be REGISTERED daemon-side: the failure mode
    /// under test is one where the notification simply never comes, and an
    /// unbounded await turns that into a suite that hangs instead of a test
    /// that fails.
    private static func nextMessageEvent(
        from stream: AsyncStream<FleetIncoming>,
        timeout: Duration = .seconds(5)
    ) async throws -> FleetMessageEventParams {
        try await withThrowingTaskGroup(of: FleetMessageEventParams?.self) { group in
            group.addTask {
                var iterator = stream.makeAsyncIterator()
                while let incoming = await iterator.next() {
                    if case let .messageEvent(event) = incoming {
                        return event
                    }
                }
                return nil
            }
            group.addTask {
                try await Task.sleep(for: timeout)
                return nil
            }
            let first = try await group.next() ?? nil
            group.cancelAll()
            guard let event = first else {
                throw MissingChatNotification()
            }
            return event
        }
    }

    /// Named rather than reusing `FleetConnectionError.closed`, so the failure
    /// says which subscription did not deliver instead of reading like a socket
    /// that hung up.
    private struct MissingChatNotification: Error, CustomStringConvertible {
        var description: String {
            "no fleet/message_event arrived on the subscribed connection"
        }
    }

    private static func nextFleetEvent(from stream: AsyncStream<FleetIncoming>) async throws -> FleetEvent {
        var iterator = stream.makeAsyncIterator()
        while let incoming = await iterator.next() {
            if case let .event(event) = incoming {
                return event
            }
        }
        throw FleetConnectionError.closed
    }
}

private final class FixtureDaemon {
    let home: URL
    let location: HangarLocation
    private let process: Process
    private let input = Pipe()
    private let output = Pipe()
    private let error = Pipe()
    private let responseState = FixtureResponseState()
    private let errorState = FixtureResponseState()
    private var pipesTornDown = false

    init() throws {
        let binary = try Self.fixtureBinary()
        home = URL(fileURLWithPath: "/tmp", isDirectory: true)
            .appendingPathComponent("ainb-fleet-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        location = HangarLocation(environment: ["AINB_HANGAR_HOME": home.path])
        process = Process()
        process.executableURL = URL(fileURLWithPath: binary)
        process.standardInput = input
        process.standardOutput = output
        process.standardError = error
        process.terminationHandler = { [responseState] terminatedProcess in
            responseState.recordExit(status: terminatedProcess.terminationStatus)
        }
        output.fileHandleForReading.readabilityHandler = { [responseState] handle in
            let chunk = handle.availableData
            if chunk.isEmpty {
                handle.readabilityHandler = nil
            } else {
                responseState.appendOutput(chunk)
            }
        }
        error.fileHandleForReading.readabilityHandler = { [errorState] handle in
            let chunk = handle.availableData
            if chunk.isEmpty {
                handle.readabilityHandler = nil
            } else {
                errorState.appendOutput(chunk)
            }
        }
        var environment = ProcessInfo.processInfo.environment
        environment["AINB_HANGAR_HOME"] = home.path
        process.environment = environment
        try process.run()
        try waitForSocket()
    }

    private static func fixtureBinary() throws -> String {
        if let configured = ProcessInfo.processInfo.environment["AINB_FLEET_FIXTURE_DAEMON"], !configured.isEmpty {
            return configured
        }
        var repository = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repository.deleteLastPathComponent() }
        let built = repository.appendingPathComponent("ainb-tui/target/debug/examples/fleet_fixture_daemon")
        guard FileManager.default.isExecutableFile(atPath: built.path) else {
            throw XCTSkip("build fleet_fixture_daemon before real daemon contract tests")
        }
        return built.path
    }

    deinit {
        stop()
    }

    func stop() {
        guard process.isRunning else {
            tearDownPipes()
            removeHome()
            return
        }
        _ = try? send(["command": "shutdown"])
        if process.isRunning {
            process.terminate()
        }
        process.waitUntilExit()
        tearDownPipes()
        removeHome()
    }

    func connection() async throws -> FleetConnection {
        let connection = FleetConnection(location: location)
        try await connection.connect()
        return connection
    }

    func authenticatedConnection() async throws -> FleetConnection {
        let connection = try await connection()
        try await connection.authenticate(token: try location.readToken())
        return connection
    }

    func authenticatedAndNegotiatedConnection() async throws -> FleetConnection {
        let connection = try await authenticatedConnection()
        _ = try await connection.negotiate()
        return connection
    }

    func seed(_ eventID: String, eventType: String = "SessionStart") throws -> Int64 {
        let response = try send([
            "command": "seed",
            "event_id": eventID,
            "event_type": eventType,
        ])
        guard response["ok"] as? Bool == true, let revision = response["revision"] as? NSNumber else {
            throw FixtureError.invalidResponse
        }
        return revision.int64Value
    }

    /// Commit one ACP transcript chunk, returning its `ingest_order`.
    ///
    /// The fixture writes the row the ACP pool writes (`source='acp'`, through
    /// the real repo) and then rings the daemon's own transcript bell. It does
    /// NOT emulate the RPC, the head read, or the forwarder, which are exactly
    /// what the tests above are about.
    func seedTranscript(
        eventID: String,
        sessionKey: String,
        eventType: String = "acp.message",
        payload: [String: Any]
    ) throws -> Int64 {
        let response = try send([
            "command": "seed_transcript",
            "event_id": eventID,
            "session_key": sessionKey,
            "event_type": eventType,
            "payload": payload,
        ])
        guard response["ok"] as? Bool == true, let order = response["ingest_order"] as? NSNumber else {
            throw FixtureError.invalidResponse
        }
        return order.int64Value
    }

    private func send(_ command: [String: Any]) throws -> [String: Any] {
        let data = try JSONSerialization.data(withJSONObject: command)
        input.fileHandleForWriting.write(data)
        input.fileHandleForWriting.write(Data("\n".utf8))
        return try readResponse()
    }

    private func readResponse() throws -> [String: Any] {
        let timeout = DispatchTime.now() + .seconds(5)
        while true {
            if let line = responseState.takeOutputLine() {
                guard let object = try JSONSerialization.jsonObject(with: line) as? [String: Any] else {
                    throw FixtureError.invalidJSON
                }
                return object
            }
            if let exitStatus = responseState.recordedExitStatus() {
                throw FixtureError.exited(exitStatus, errorState.outputText())
            }
            guard responseState.waitForResponse(timeout: timeout) else {
                if let exitStatus = responseState.recordedExitStatus() {
                    throw FixtureError.exited(exitStatus, errorState.outputText())
                }
                throw FixtureError.responseTimeout
            }
        }
    }

    private func waitForSocket() throws {
        let deadline = Date().addingTimeInterval(5)
        while !FileManager.default.fileExists(atPath: location.socketURL.path) {
            guard process.isRunning else {
                throw FixtureError.exited(responseState.recordedExitStatus() ?? -1, errorState.outputText())
            }
            guard Date() < deadline else { throw FixtureError.socketTimeout }
            Thread.sleep(forTimeInterval: 0.02)
        }
    }

    private func tearDownPipes() {
        guard !pipesTornDown else { return }
        pipesTornDown = true
        output.fileHandleForReading.readabilityHandler = nil
        error.fileHandleForReading.readabilityHandler = nil
        input.fileHandleForWriting.closeFile()
        output.fileHandleForReading.closeFile()
        error.fileHandleForReading.closeFile()
    }

    private func removeHome() {
        guard FileManager.default.fileExists(atPath: home.path) else { return }
        do {
            try FileManager.default.removeItem(at: home)
        } catch let error as CocoaError where error.code == .fileNoSuchFile {
        } catch {
            XCTFail("failed to remove fixture home: \(error)")
        }
    }
}

private final class FixtureResponseState: @unchecked Sendable {
    private let lock = NSLock()
    private let signal = DispatchSemaphore(value: 0)
    private var outputBuffer = Data()
    private var exitStatus: Int32?

    func appendOutput(_ chunk: Data) {
        lock.lock()
        outputBuffer.append(chunk)
        lock.unlock()
        signal.signal()
    }

    func takeOutputLine() -> Data? {
        lock.lock()
        defer { lock.unlock() }
        guard let newline = outputBuffer.firstRange(of: Data("\n".utf8)) else { return nil }
        let line = outputBuffer.subdata(in: 0..<newline.lowerBound)
        outputBuffer.removeSubrange(0..<newline.upperBound)
        return line
    }

    func recordExit(status: Int32) {
        lock.lock()
        exitStatus = status
        lock.unlock()
        signal.signal()
    }

    func recordedExitStatus() -> Int32? {
        lock.lock()
        defer { lock.unlock() }
        return exitStatus
    }

    func outputText() -> String {
        lock.lock()
        defer { lock.unlock() }
        return String(decoding: outputBuffer, as: UTF8.self)
    }

    func waitForResponse(timeout: DispatchTime) -> Bool {
        signal.wait(timeout: timeout) != .timedOut
    }
}

private enum FixtureError: Error {
    case invalidJSON
    case invalidResponse
    case exited(Int32, String)
    case socketTimeout
    case responseTimeout
}
