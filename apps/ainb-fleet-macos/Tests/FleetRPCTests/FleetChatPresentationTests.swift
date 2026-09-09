import Foundation
import XCTest
@testable import AINBFleet

/// The macOS chat pane's display contract.
///
/// Reads the SAME fixtures the Rust round-trips and the Swift daemon-contract
/// suite read (`ainb-tui/crates/ainb-hangar-proto/fixtures/chat`), never copies
/// of them: a copy is how two suites agree with each other and disagree with
/// the wire.
///
/// The label tests are the Swift mirror of
/// `every_wire_provider_renders_a_label_operators_can_read`: each one iterates
/// `allCases` and classifies every variant through a wildcard-free `switch`, so
/// adding a variant fails to COMPILE both in `FleetChatLabels` and here rather
/// than silently rendering as something an operator misreads.
final class FleetChatPresentationTests: XCTestCase {
    // MARK: - Attribution

    /// The guarantee the wire's `actor` exists to provide, at the last inch: a
    /// copilot row and an operator row must be TELLABLE APART on screen.
    func testCopilotAndOperatorRowsAreDistinguishable() throws {
        let human = FleetChatMessageRow(message: try Self.message(sender: "operator", body: "restart sess-a"))
        let copilot = FleetChatMessageRow(message: try Self.message(sender: "copilot", body: "restart sess-a"))

        XCTAssertNotEqual(human.actor, copilot.actor)
        XCTAssertNotEqual(human.actor.label, copilot.actor.label)
        XCTAssertNotEqual(human.actor.accessibilityLabel, copilot.actor.accessibilityLabel)
        XCTAssertNotEqual(human.actor.identifier, copilot.actor.identifier)
        // Side is the glanceable half. Only a human's own writing is mine.
        XCTAssertTrue(human.actor.isOperator)
        XCTAssertFalse(copilot.actor.isOperator)
        // Identical BODIES: attribution cannot come from the text, because the
        // text is the one thing a prompt-injected copilot fully controls.
        XCTAssertEqual(human.body, copilot.body)
    }

    /// A blank sender must never read as the operator. The daemon refuses one,
    /// so this is the belt for a wire that changes its mind.
    func testBlankSenderIsUnattributedRatherThanTheOperator() {
        for blank in ["", "   ", "\n"] {
            XCTAssertEqual(FleetChatActor.from(wire: blank), .unattributed, "\(blank.debugDescription) read as somebody")
        }
        XCTAssertFalse(FleetChatActor.unattributed.isOperator)
        XCTAssertEqual(FleetChatActor.from(wire: "  operator "), .operatorHuman)
        XCTAssertEqual(FleetChatActor.from(wire: "tmux:sess-a"), .session("tmux:sess-a"))
    }

    /// Every actor renders a distinct, non-empty label. Exhaustive on purpose.
    func testEveryActorRendersADistinctLabel() {
        let actors: [FleetChatActor] = [.operatorHuman, .copilot, .session("tmux:sess-a"), .unattributed]
        // Wildcard-free: a new actor kind is a compile error here.
        func isNamedHuman(_ actor: FleetChatActor) -> Bool {
            switch actor {
            case .operatorHuman: true
            case .copilot, .session, .unattributed: false
            }
        }
        XCTAssertEqual(actors.filter(isNamedHuman).count, 1, "exactly one actor may read as the human")
        XCTAssertEqual(Set(actors.map(\.label)).count, actors.count, "two actors share a label")
        XCTAssertEqual(Set(actors.map(\.accessibilityLabel)).count, actors.count, "two actors sound alike")
        XCTAssertTrue(actors.allSatisfy { !$0.label.isEmpty })
    }

    // MARK: - Display mappings

    func testEveryMessageKindRendersALabel() {
        assertLabelsAreDistinctAndNamed(FleetMessageKind.allCases, FleetChatLabels.messageKind) { kind in
            switch kind {
            case .user, .agent, .marker: true
            case .unknown: false
            }
        }
    }

    func testEveryConfirmStateRendersALabel() {
        assertLabelsAreDistinctAndNamed(FleetConfirmState.allCases, FleetChatLabels.confirmState) { state in
            switch state {
            case .open, .approved, .denied, .expired: true
            case .unknown: false
            }
        }
    }

    func testEveryActivityClassRendersALabel() {
        assertLabelsAreDistinctAndNamed(FleetActivityClass.allCases, FleetChatLabels.activityClass) { activityClass in
            switch activityClass {
            case .read, .write, .destructive: true
            case .unknown: false
            }
        }
    }

    /// A class this build cannot name is styled LOUD, never quiet. Over-warning
    /// about a future class is recoverable; painting it as a harmless read is
    /// not.
    func testUnknownActivityClassWarnsRatherThanReadsAsHarmless() {
        XCTAssertTrue(FleetChatLabels.activityClassIsLoud(.unknown))
        XCTAssertTrue(FleetChatLabels.activityClassIsLoud(.destructive))
        XCTAssertFalse(FleetChatLabels.activityClassIsLoud(.read))
    }

    func testEveryActivityOutcomeRendersALabel() {
        assertLabelsAreDistinctAndNamed(FleetActivityOutcome.allCases, FleetChatLabels.activityOutcome) { outcome in
            switch outcome {
            case .ok, .denied, .expired, .error: true
            case .unknown: false
            }
        }
    }

    func testEveryChannelKindRendersALabel() {
        assertLabelsAreDistinctAndNamed(FleetChannelKind.allCases, FleetChatLabels.channelKind) { kind in
            switch kind {
            case .copilot, .broadcast: true
            case .unknown: false
            }
        }
    }

    /// The provider is a registry name, so it renders verbatim — an operator
    /// reading it here and in `ainb fleet adapter list` reads one vocabulary.
    /// Only the empty string, which names no adapter, is called out.
    func testACopilotProviderRendersItsRegistryName() {
        XCTAssertEqual(FleetChatLabels.copilotProvider("claude-agent-acp"), "claude-agent-acp")
        XCTAssertEqual(FleetChatLabels.copilotProvider("some-vendor-acp"), "some-vendor-acp")
        XCTAssertEqual(FleetChatLabels.copilotProvider(""), "Unrecognised provider")
    }

    func testEveryCopilotModeRendersALabel() {
        assertLabelsAreDistinctAndNamed(FleetCopilotMode.allCases, FleetChatLabels.copilotMode) { mode in
            switch mode {
            case .help, .guarded, .yolo: true
            case .unknown: false
            }
        }
    }

    // MARK: - Confirm cards

    /// The shared fixture's two open cards are answerable, and the pane's gate
    /// agrees with the wire type's rather than re-deriving it.
    func testSharedConfirmFixtureRendersAnswerableCards() throws {
        let cards = try Self.confirmCards(fromFixture: "confirm_list_result.json")
        XCTAssertEqual(cards.count, 2)
        XCTAssertTrue(cards.allSatisfy(\.isAnswerable))
        XCTAssertEqual(cards.map(\.stateLabel), ["OPEN", "OPEN"])
        XCTAssertEqual(cards.first?.tool, "kill")
        XCTAssertEqual(cards.first?.id, "01J0CONFIRM")
        // The arguments line is re-encoded, never read: the daemon already
        // projected them onto the tool's declared schema keys.
        XCTAssertEqual(cards.first?.argumentsLine, #"{"session":"tmux:sess-c"}"#)
    }

    /// An expired card from the shared event fixture renders and is refused.
    func testExpiredCardRendersButIsNotAnswerable() throws {
        let event = try FleetWire.decoder().decode(
            FleetConfirmEventParams.self,
            from: try Self.chatFixture(named: "confirm_event.json")
        )
        let card = FleetChatConfirmCard.known(event.confirm)
        XCTAssertEqual(card.stateLabel, "EXPIRED")
        XCTAssertFalse(card.isAnswerable)
        XCTAssertTrue(card.refusal.contains("EXPIRED"), card.refusal)
    }

    /// A state token this build has never heard of decodes to `.unknown` and is
    /// NOT answerable, so the pane cannot offer an approve button for a
    /// lifecycle it does not understand.
    func testStateFromANewerDaemonIsNeverAnswerable() throws {
        let card = FleetChatConfirmCard.decode(try Self.json(#"""
        {"confirm_id":"01J0FUTURE","scope_key":"channel:copilot","tool":"future_tool",
         "arguments":{},"state":"quarantined","created_at":1,"expires_at":2}
        """#))
        guard case let .known(confirm) = card else { return XCTFail("expected a decoded card") }
        XCTAssertEqual(confirm.state, .unknown)
        XCTAssertFalse(card.isAnswerable)
        XCTAssertEqual(card.stateLabel, "UNRECOGNISED")
    }

    /// One undecodable row must not cost the operator the cards this build DOES
    /// understand. Row-by-row decoding is the whole reason `confirmList`
    /// returns raw rows.
    func testOneUndecodableRowDoesNotBlankThePage() throws {
        let raw = try FleetWire.decoder().decode(FleetConfirmListRawResult.self, from: Data(#"""
        {"confirms":[
          {"confirm_id":"01J0GOOD","scope_key":"channel:copilot","tool":"kill",
           "arguments":{},"state":"open","created_at":1,"expires_at":2},
          {"confirm_id":"01J0BAD","tool":"spawn_session","created_at":"not-a-number"}
        ]}
        """#.utf8))
        let cards = raw.confirms.map(FleetChatConfirmCard.decode)

        XCTAssertEqual(cards.count, 2)
        XCTAssertTrue(cards[0].isAnswerable, "a good card was lost to a bad neighbour")
        XCTAssertFalse(cards[1].isAnswerable)
        XCTAssertEqual(cards[1].id, "01J0BAD", "identity is salvaged so the row is reportable")
        XCTAssertEqual(cards[1].tool, "spawn_session")
        XCTAssertEqual(cards[1].stateLabel, "UNRECOGNISED")
        XCTAssertTrue(cards[1].refusal.contains("not answerable"), cards[1].refusal)
        XCTAssertEqual(cards[1].argumentsLine, "", "an undecoded card must not claim arguments it never read")
    }

    /// A row with no readable identity at all is still rendered rather than
    /// dropped: an absence is unreportable, an "unknown" row is not.
    func testARowWithNoReadableIdentityStillRenders() throws {
        let card = FleetChatConfirmCard.decode(try Self.json(#"{"nothing":"useful"}"#))
        XCTAssertEqual(card.id, "unknown")
        XCTAssertEqual(card.tool, "unknown")
        XCTAssertFalse(card.isAnswerable)
    }

    // MARK: - Activity

    func testSharedActivityFixtureRendersEveryRow() throws {
        let result = try FleetWire.decoder().decode(
            FleetActivityListResult.self,
            from: try Self.chatFixture(named: "activity_list_result.json")
        )
        XCTAssertEqual(result.activities.map { FleetChatLabels.activityClass($0.activityClass) }, ["WRITE", "DESTRUCTIVE"])
        XCTAssertEqual(result.activities.map { FleetChatLabels.activityOutcome($0.outcome) }, ["ok", "expired"])
        XCTAssertTrue(result.activities.map(\.activityClass).contains { FleetChatLabels.activityClassIsLoud($0) })
    }

    // MARK: - Send params

    /// The client cannot file a message under anybody else's name, because
    /// `FleetMessageSendParams` has no `actor` to set. Asserted on the ENCODED
    /// frame: a field that exists but is never populated is one refactor away
    /// from being populated.
    func testOperatorSendCarriesNoActorKey() throws {
        let encoded = try FleetWire.encoder().encode(FleetMessageSendParams(
            scopeKey: "channel:copilot",
            targets: ["acp:01J0COPILOT"],
            originMessageID: nil,
            text: "status?",
            requestID: "req-1"
        ))
        let object = try XCTUnwrap(try JSONSerialization.jsonObject(with: encoded) as? [String: Any])

        XCTAssertNil(object["actor"], "an operator surface must never name the sender")
        XCTAssertNil(object["origin_message_id"], "an absent optional is omitted, not sent as null")
        XCTAssertEqual(object["scope_key"] as? String, "channel:copilot")
        XCTAssertEqual(object["targets"] as? [String], ["acp:01J0COPILOT"])
        XCTAssertEqual(object["request_id"] as? String, "req-1")
    }

    // MARK: - Send receipts

    /// The daemon's REASON survives the wire onto this client.
    ///
    /// Decoded from the `message_send` result shape rather than built in
    /// Swift: a `detail` the type declares but the CodingKeys never read would
    /// pass an initialiser-based test and still print REJECTED with no reason,
    /// which is the one thing the operator needs to decide whether to retry.
    func testDeliveryDetailSurvivesTheWire() throws {
        let result = try FleetWire.decoder().decode(FleetMessageSendResult.self, from: Data("""
        {"message_id":"01J0MSG","deliveries":[
          {"session_key":"claude:gone","state":"REJECTED","detail":"target_not_running"}]}
        """.utf8))

        let leg = try XCTUnwrap(result.deliveries.first)
        XCTAssertEqual(leg.detail, "target_not_running", "the reason must cross the socket")
        XCTAssertTrue(
            FleetChatLabels.receiptLine(leg).contains("target_not_running"),
            "a receipt that cannot say WHY is one an operator can only stare at"
        )
    }

    /// A leg with no reason decodes too: the daemon omits the key rather than
    /// sending null, and an older daemon never sends it at all.
    func testADeliveryWithNoReasonStillDecodes() throws {
        let result = try FleetWire.decoder().decode(FleetMessageSendResult.self, from: Data("""
        {"message_id":"01J0MSG","deliveries":[{"session_key":"claude:one","state":"DELIVERED"}]}
        """.utf8))

        XCTAssertNil(try XCTUnwrap(result.deliveries.first).detail)
    }

    /// The hunt-2 failure in its worst form: a 1-of-1 fan-out where 0
    /// delivered must NOT read as success. Asserted on the decoded result, not
    /// on a view, so it fails whatever the pane happens to look like.
    func testALoneRejectedLegNeverReadsAsSent() throws {
        let result = try FleetWire.decoder().decode(FleetMessageSendResult.self, from: Data("""
        {"message_id":"01J0MSG","deliveries":[
          {"session_key":"claude:gone","state":"REJECTED","detail":"target_not_running"}]}
        """.utf8))

        let summary = FleetChatLabels.deliverySummary(result.deliveries)
        XCTAssertTrue(summary.contains("delivered to 0/1"), summary)
        XCTAssertTrue(summary.contains("1 not delivered"), summary)
        XCTAssertTrue(summary.contains("claude:gone"), summary)
        XCTAssertTrue(summary.contains("target_not_running"), summary)
    }

    /// The vocabulary is the TUI's, word for word
    /// (`ChatState::apply_receipts`), so an operator watching both surfaces
    /// reads one sentence rather than two dialects of it.
    func testAFullyDeliveredSendReadsLikeTheTUI() {
        XCTAssertEqual(
            FleetChatLabels.deliverySummary([
                FleetMessageDelivery(sessionKey: "claude:one", state: .delivered, detail: nil),
                FleetMessageDelivery(sessionKey: "codex:two", state: .delivered, detail: nil),
            ]),
            "delivered to 2/2"
        )
    }

    /// Every receipt state renders a distinct word, and only `DELIVERED` reads
    /// as success. `UNKNOWN` counts as NOT delivered on purpose: at-most-once
    /// delivery means an unknown leg may never have arrived, and counting it as
    /// sent is the single lie this summary exists to prevent. Adding a status
    /// fails to COMPILE in `deliveryState` and in the classifier below.
    func testEveryReceiptStateRendersALabelAndOnlyDeliveredIsSuccess() {
        let states = ActionReceiptStatus.allCases
        XCTAssertEqual(Set(states.map(FleetChatLabels.deliveryState)).count, states.count)
        for state in states {
            let word = FleetChatLabels.deliveryState(state)
            XCTAssertEqual(word, state.rawValue, "the label must be the daemon's own token")
            let readsAsSuccess: Bool = switch state {
            case .delivered: true
            case .pending, .failed, .unknown, .rejected: false
            }
            let summary = FleetChatLabels.deliverySummary([
                FleetMessageDelivery(sessionKey: "claude:one", state: state, detail: nil)
            ])
            XCTAssertEqual(
                summary == "delivered to 1/1",
                readsAsSuccess,
                "\(word) is counted wrongly: \(summary)"
            )
        }
    }

    /// The page size is the daemon's own maximum. A client that pages
    /// differently from the TUI shows a different conversation for the same
    /// scope, which reads as message loss.
    func testChatPagesAtTheDaemonMaximums() {
        XCTAssertEqual(fleetMessageListMax, 100, "FLEET_MESSAGE_LIST_MAX")
        XCTAssertEqual(fleetActivityListMax, 200, "FLEET_ACTIVITY_LIST_MAX")
    }

    /// The adapter token the copilot scope is bound to. The daemon binds a
    /// scope to whatever the FIRST `fleet/acp_session_create` names, so a
    /// mismatch with the TUI means whoever opened the chat first wins and the
    /// other client is refused forever.
    func testCopilotProviderMatchesTheTUI() {
        XCTAssertEqual(copilotDefaultProvider, "claude-agent-acp")
    }

    // MARK: - Copilot mint ladder

    /// The refusals a REAL daemon sends, verbatim.
    ///
    /// `parse_params` wraps every serde error as `expected {shape}: {error}`,
    /// and the shape hint lists the OTHER fields, so a refusal about the
    /// provider carries the word "cwd" as well. Feeding invented one-line
    /// messages here is how a matcher that answered a provider refusal by
    /// naming a directory kept a green suite.
    private static let legacyMissingProvider =
        "expected { provider, cwd, scope_key? }: missing field `provider`"
    private static let cwdEraDaemonMissingProvider =
        "expected { provider?, cwd, scope_key? }: missing field `provider`"
    private static let legacyMissingCwd =
        "expected { provider?, cwd, scope_key? }: missing field `cwd`"
    /// This daemon's own refusal when a create has no live session to take a
    /// root from. Not a skew, but answered by the same retry.
    private static let cwdRequired =
        "cwd is required unless the named scope already has a live session to take its root from"

    /// Rung 1 is what a modern daemon gets, and it names NOTHING.
    ///
    /// Naming either half is how this client was refused on every poll: the
    /// live copilot scope is held by a session opened from a worktree, and an
    /// app that names the operator's home directory is told the scope is held
    /// with a different cwd, forever.
    @MainActor
    func testTheFirstRungNamesNeitherProviderNorCwd() async throws {
        let recorder = MintRecorder()
        let created = try await FleetStore.mintCopilotSession(
            scopeKey: "channel:c1",
            home: "/Users/operator",
            create: recorder.answer
        )

        XCTAssertEqual(created.sessionKey, "acp:01J0KEY")
        XCTAssertEqual(recorder.sent.count, 1, "a daemon that answers must not be asked twice")
        let attach = try XCTUnwrap(recorder.sent.first)
        XCTAssertNil(attach.provider, "the engine is the operator's choice, not this client's")
        XCTAssertNil(attach.cwd, "the root is the session's, and this client cannot know it")
        XCTAssertEqual(attach.scopeKey, "channel:c1")
    }

    /// Rung 2: a daemon that asks for a cwd gets one, and only then.
    ///
    /// Both refusals are fed VERBATIM as the daemon sends them, wrapped in the
    /// `expected {shape}: {error}` envelope every parse failure carries. That
    /// envelope is the whole reason the matcher is anchored: its shape hint
    /// names every field, so a legacy refusal about the provider mentions "cwd"
    /// too.
    @MainActor
    func testACwdIsNamedOnlyWhenTheDaemonAsksForOne() async throws {
        for refusal in [Self.legacyMissingCwd, Self.cwdRequired] {
            let recorder = MintRecorder(refuseUntilAttempt: 2, message: refusal)
            _ = try await FleetStore.mintCopilotSession(
                scopeKey: "channel:c1",
                home: "/Users/operator",
                create: recorder.answer
            )

            XCTAssertEqual(recorder.sent.count, 2, "\(refusal): one retry, not a loop")
            let rooted = try XCTUnwrap(recorder.sent.last)
            XCTAssertEqual(rooted.cwd, "/Users/operator", "\(refusal)")
            XCTAssertNil(rooted.provider, "\(refusal): the engine is still not this client's to name")
        }
    }

    /// Rung 3: the legacy daemon, which requires both fields, gets the frame
    /// this client sent before either became optional.
    ///
    /// Both legacy generations are covered: the one that requires provider and
    /// cwd, and the one that made provider optional but still requires cwd, and
    /// therefore answers an attach by naming provider first.
    @MainActor
    func testTheLegacyRungNamesBothFieldsForADaemonThatRequiresThem() async throws {
        for refusal in [Self.legacyMissingProvider, Self.cwdEraDaemonMissingProvider] {
            let recorder = MintRecorder(refuseUntilAttempt: 2, message: refusal)
            _ = try await FleetStore.mintCopilotSession(
                scopeKey: "channel:c1",
                home: "/Users/operator",
                create: recorder.answer
            )

            XCTAssertEqual(
                recorder.sent.count, 2,
                "\(refusal): a refusal naming provider must skip the cwd rung, not spend it"
            )
            let named = try XCTUnwrap(recorder.sent.last)
            XCTAssertEqual(named.provider, copilotDefaultProvider, "\(refusal)")
            XCTAssertEqual(named.cwd, "/Users/operator", "\(refusal)")
        }
    }

    /// A refusal for one field never satisfies the other rung.
    ///
    /// This is the assertion a loose two-substring matcher failed while its
    /// tests passed, because those tests fed messages no daemon emits. Here the
    /// SECOND frame is the assertion: a provider refusal must be answered by
    /// naming the provider, never by naming a directory the daemon did not ask
    /// for.
    @MainActor
    func testAProviderRefusalIsNeverAnsweredByNamingADirectoryAlone() async throws {
        for refusal in [Self.legacyMissingProvider, Self.cwdEraDaemonMissingProvider] {
            let recorder = MintRecorder(refuseUntilAttempt: 2, message: refusal)
            _ = try await FleetStore.mintCopilotSession(
                scopeKey: "channel:c1",
                home: "/Users/operator",
                create: recorder.answer
            )

            let retry = try XCTUnwrap(recorder.sent.last)
            XCTAssertNotNil(
                retry.provider,
                "the shape hint names cwd, but the daemon asked for a provider: \(refusal)"
            )
        }
    }

    /// And the reverse: a daemon asking for a root must not be answered with an
    /// adapter it never asked about, which would revert an engine the operator
    /// swapped.
    @MainActor
    func testACwdRefusalIsNeverAnsweredByNamingAnAdapter() async throws {
        for refusal in [Self.legacyMissingCwd, Self.cwdRequired] {
            let recorder = MintRecorder(refuseUntilAttempt: 2, message: refusal)
            _ = try await FleetStore.mintCopilotSession(
                scopeKey: "channel:c1",
                home: "/Users/operator",
                create: recorder.answer
            )

            let retry = try XCTUnwrap(recorder.sent.last)
            XCTAssertNil(retry.provider, "\(refusal)")
            XCTAssertEqual(retry.cwd, "/Users/operator", "\(refusal)")
        }
    }

    /// A held scope is a REAL refusal and must propagate untouched.
    ///
    /// Its wording names the directory that holds the scope, which is the only
    /// actionable thing the operator gets. Retrying would spend a rung to
    /// receive the same refusal and would replace that wording with itself.
    @MainActor
    func testAHeldScopeIsReportedRatherThanRetried() async {
        let recorder = MintRecorder(
            refuseUntilAttempt: .max,
            message: "scope_key \"channel:c1\" is already held by a session whose cwd is "
                + "\"/work/api\", not \"/Users/operator\"; stop it before creating a different one"
        )

        do {
            _ = try await FleetStore.mintCopilotSession(
                scopeKey: "channel:c1",
                home: "/Users/operator",
                create: recorder.answer
            )
            XCTFail("a held scope is not something this client can retry its way out of")
        } catch {
            XCTAssertEqual(recorder.sent.count, 1, "no rung may be spent on a real refusal")
            XCTAssertTrue(
                String(describing: error).contains("/work/api"),
                "the directory that holds the scope must reach the operator: \(error)"
            )
        }
    }

    /// The predicate that forgets a remembered session fires only for the
    /// daemon's DEAD-SESSION tokens, on the target's own leg.
    ///
    /// The tokens are the daemon's, not invented here: `session_gone` comes
    /// from the ACP pool, `target_unknown` and `target_not_running` from the
    /// delivery leg.
    @MainActor
    func testOnlyADeadSessionRejectionForgetsTheRememberedSession() throws {
        for detail in ["session_gone", "target_unknown", "target_not_running"] {
            let legs = try Self.deliveries(
                #"{"session_key":"acp:1","state":"REJECTED","detail":"\#(detail)"}"#
            )
            XCTAssertTrue(FleetStore.reportsSessionGone("acp:1", in: legs), detail)
            XCTAssertFalse(
                FleetStore.reportsSessionGone("acp:2", in: legs),
                "\(detail): another target's refusal says nothing about ours"
            )
        }

        XCTAssertFalse(
            FleetStore.reportsSessionGone(
                "acp:1",
                in: try Self.deliveries(#"{"session_key":"acp:1","state":"PENDING"}"#)
            ),
            "a turn still running is the normal case, not a dead session"
        )
        XCTAssertFalse(
            FleetStore.reportsSessionGone(
                "acp:1",
                in: try Self.deliveries(#"{"session_key":"acp:1","state":"DELIVERED"}"#)
            )
        )
    }

    /// A session that is still ALIVE keeps its mint.
    ///
    /// `queue_full` and `breaker_open` are transient back-pressure from a pool
    /// that still holds the session, and `task_scope_refused` names a scope
    /// this surface never addresses. Forgetting on any of them would spend the
    /// write transaction the cache exists to remove, on a session that would
    /// have answered the next prompt.
    @MainActor
    func testATransientRejectionKeepsTheRememberedSession() throws {
        for detail in ["queue_full", "breaker_open", "task_scope_refused", "provider_at_capacity"] {
            XCTAssertFalse(
                FleetStore.reportsSessionGone(
                    "acp:1",
                    in: try Self.deliveries(
                        #"{"session_key":"acp:1","state":"REJECTED","detail":"\#(detail)"}"#
                    )
                ),
                "\(detail) is not a dead session and must not cost a re-mint"
            )
        }
    }

    /// A refusal this build cannot name is not a reason to forget.
    ///
    /// Fail-closed on the cheap side: a daemon that grows a token this build
    /// has never heard of would otherwise re-mint on every send, reintroducing
    /// the per-send write transaction silently, through a wire change nobody
    /// here would see. An absent detail is the same case.
    @MainActor
    func testAnUnrecognisedOrAbsentDetailKeepsTheRememberedSession() throws {
        XCTAssertFalse(
            FleetStore.reportsSessionGone(
                "acp:1",
                in: try Self.deliveries(
                    #"{"session_key":"acp:1","state":"REJECTED","detail":"some_future_token"}"#
                )
            ),
            "an unknown token must not be read as a dead session"
        )
        XCTAssertFalse(
            FleetStore.reportsSessionGone(
                "acp:1",
                in: try Self.deliveries(#"{"session_key":"acp:1","state":"REJECTED"}"#)
            ),
            "a refusal with no reason says nothing about the session"
        )
    }

    // MARK: - Live chat events folded into the surface

    /// A message committed in the scope on screen lands in the timeline, in
    /// commit order, without a page.
    func testALiveMessageForTheShownScopeAppendsToTheTimeline() throws {
        var surface = try Self.shownSurface()
        surface.apply(.message(try Self.liveMessage(id: "01J0B", body: "second")))

        XCTAssertEqual(surface.messages.map(\.id), ["01J0A", "01J0B"])
        XCTAssertEqual(surface.messages.last?.body, "second")
    }

    /// A message for a DIFFERENT scope is dropped, never rendered.
    ///
    /// This is the whole reason the fold reads the scope: the daemon's message
    /// forwarder is fleet-wide, so a broadcast channel's traffic and every
    /// other copilot conversation arrive on the same socket. A pane that
    /// rendered them would attribute another conversation's message to this one
    /// and offer the operator a reply that goes somewhere else.
    func testALiveMessageForAnotherScopeIsDropped() throws {
        var surface = try Self.shownSurface()
        surface.apply(.message(try Self.liveMessage(id: "01J0B", body: "elsewhere", scope: "channel:other")))

        XCTAssertEqual(surface.messages.map(\.id), ["01J0A"])
    }

    /// The same id twice REPLACES.
    ///
    /// The daemon's forwarder replays from a cursor, so a message can arrive
    /// live and again in the page that follows. Appending both would show the
    /// operator their own message twice with no way to tell which one the
    /// copilot answered.
    func testALiveMessageThatAlreadyExistsReplacesRatherThanDuplicates() throws {
        var surface = try Self.shownSurface()
        surface.apply(.message(try Self.liveMessage(id: "01J0A", body: "edited")))

        XCTAssertEqual(surface.messages.map(\.id), ["01J0A"])
        XCTAssertEqual(surface.messages.first?.body, "edited")
    }

    /// Nothing is folded before a page has resolved a scope.
    ///
    /// An event that arrives between `fleet/message_subscribe` and the first
    /// page has nothing to be compared against, and a surface with no scope
    /// cannot prove the event belongs to it. The page that follows carries it.
    func testAnEventArrivingBeforeAScopeIsResolvedIsDropped() throws {
        var surface = FleetChatSurface()
        surface.apply(.message(try Self.liveMessage(id: "01J0B", body: "early")))

        XCTAssertEqual(surface.messages, [])
    }

    /// The timeline stays inside the page's own ceiling, dropping the oldest.
    ///
    /// A pane left open all day would otherwise grow without bound, and the
    /// bound that matters is the one a fresh page would show.
    func testTheTimelineStaysBoundedByThePageCeiling() throws {
        var surface = try Self.shownSurface()
        for index in 0..<Int(fleetMessageListMax) {
            surface.apply(.message(try Self.liveMessage(id: "live-\(index)", body: "row \(index)")))
        }

        XCTAssertEqual(surface.messages.count, Int(fleetMessageListMax))
        XCTAssertEqual(
            surface.messages.first?.id, "live-0",
            "the ceiling drops the OLDEST row, which is the page's first one"
        )
        XCTAssertEqual(surface.messages.last?.id, "live-\(Int(fleetMessageListMax) - 1)")
    }

    /// A confirm card is upserted by its id, and an ANSWERED card is kept in
    /// its new state rather than dropped.
    ///
    /// `fleet/confirm_list` only returns OPEN cards, so dropping an answered
    /// one here would make the card vanish the instant it was approved, with
    /// nothing to say the approval was what removed it.
    func testALiveConfirmUpsertsByIDAndKeepsAnAnsweredCard() throws {
        var surface = try Self.shownSurface()
        surface.apply(Self.liveConfirm(id: "01J0CARD", state: "open"))
        XCTAssertEqual(surface.confirms.map(\.id), ["01J0CARD"])
        XCTAssertTrue(surface.confirms[0].isAnswerable)

        surface.apply(Self.liveConfirm(id: "01J0CARD", state: "approved"))
        XCTAssertEqual(surface.confirms.map(\.id), ["01J0CARD"], "the answer must not add a second card")
        XCTAssertEqual(surface.confirms[0].stateLabel, "APPROVED")
        XCTAssertFalse(surface.confirms[0].isAnswerable)
    }

    /// A live card this build cannot decode still reaches the pane, as an
    /// UNANSWERABLE row.
    ///
    /// The tolerance the paged list has always had, now on the live path too.
    /// The alternative is the one shape of card the operator never sees: the
    /// page renders it as unrecognised, so a stream that dropped it would make
    /// a card appear only on the safety net, half a minute late, having been
    /// silently withheld in between.
    func testALiveConfirmThisBuildCannotDecodeStillRendersUnanswerable() throws {
        var surface = try Self.shownSurface()
        let broken = Self.brokenConfirmValue(id: "01J0BAD", scope: "channel:copilot")

        surface.apply(.confirm(card: FleetChatConfirmCard.decode(broken), scopeKey: "channel:copilot"))

        XCTAssertEqual(surface.confirms.map(\.id), ["01J0BAD"], "the card was withheld entirely")
        XCTAssertFalse(surface.confirms[0].isAnswerable, "a card this build cannot read must never be approvable")
        XCTAssertEqual(surface.confirms[0].stateLabel, "UNRECOGNISED")
    }

    /// A confirm card for another scope is dropped, like every other event.
    func testALiveConfirmForAnotherScopeIsDropped() throws {
        var surface = try Self.shownSurface()
        surface.apply(Self.liveConfirm(id: "01J0CARD", state: "open", scope: "channel:other"))

        XCTAssertEqual(surface.confirms, [])
    }

    /// Activity APPENDS, oldest first, because that is the order its page
    /// returns (`ORDER BY seq ASC`). A feed with one half ascending and the
    /// other descending cannot be read at all.
    func testALiveActivityAppendsInPageOrderAndStaysBounded() throws {
        var surface = try Self.shownSurface()
        surface.apply(.activity(try Self.liveActivity(seq: 2)))
        surface.apply(.activity(try Self.liveActivity(seq: 3)))

        XCTAssertEqual(surface.activity.map(\.seq), [1, 2, 3])

        for seq in 4...(Int64(fleetActivityListMax) + 1) {
            surface.apply(.activity(try Self.liveActivity(seq: seq)))
        }
        XCTAssertEqual(surface.activity.count, Int(fleetActivityListMax))
        XCTAssertEqual(surface.activity.first?.seq, 2, "the ceiling drops the oldest row")
        XCTAssertEqual(surface.activity.last?.seq, Int64(fleetActivityListMax) + 1)
    }

    /// The same `seq` twice replaces rather than duplicating, so a replayed
    /// row cannot show one tool call as two.
    func testALiveActivityRowIsUpsertedBySeq() throws {
        var surface = try Self.shownSurface()
        surface.apply(.activity(try Self.liveActivity(seq: 1, tool: "kill_session")))

        XCTAssertEqual(surface.activity.map(\.seq), [1])
        XCTAssertEqual(surface.activity.first?.tool, "kill_session")
    }

    /// Every event type names EXACTLY ONE addressing axis, and which one it
    /// names is which daemon stream it came off.
    ///
    /// This is what makes broad streams safe to render in a narrow pane: the
    /// three chat frames are filed under a scope and the transcript chunk
    /// belongs to a session, so each is filtered on the axis it carries. A
    /// shape naming NEITHER could not be filtered at all, and one naming BOTH
    /// would have two answers to which pane it belongs in. Exhaustive over the
    /// enum on purpose: a fifth event shape must fail here.
    func testEveryLiveEventNamesExactlyOneAddressingAxis() throws {
        let scoped: [FleetChatEvent] = [
            .message(try Self.liveMessage(id: "01J0B", body: "b")),
            Self.liveConfirm(id: "01J0CARD", state: "open"),
            .activity(try Self.liveActivity(seq: 9)),
        ]
        let sessioned: [FleetChatEvent] = [.transcript(try Self.liveChunk(order: 9))]

        XCTAssertEqual(scoped.map(\.scopeKey), Array(repeating: "channel:copilot", count: scoped.count))
        XCTAssertEqual(scoped.compactMap(\.sessionKey), [], "a chat frame must not claim a session")
        XCTAssertEqual(sessioned.map(\.sessionKey), ["acp:1"])
        XCTAssertEqual(sessioned.compactMap(\.scopeKey), [], "a transcript chunk must not claim a scope")
    }

    // MARK: - The transcript section of the surface

    /// A chunk for the session on screen is classified and appended, and the
    /// surface's cursor moves with it.
    func testALiveTranscriptChunkForTheShownSessionAppends() throws {
        var surface = try Self.shownSurface()
        surface.apply(.transcript(try Self.liveChunk(order: 7, text: "on it")))

        XCTAssertEqual(surface.transcriptState.rows.map(\.body), ["on it"])
        XCTAssertEqual(surface.transcriptState.rows.map(\.lane), [.agent])
        XCTAssertEqual(surface.transcriptState.cursor, 7)
    }

    /// A chunk from ANOTHER session is dropped, never rendered.
    ///
    /// The transcript's version of the scope filter, and it matters more: a
    /// foreign chat message is at least visibly somebody else's conversation,
    /// while a foreign transcript row reads as this agent's own execution.
    func testATranscriptChunkForAnotherSessionIsDropped() throws {
        var surface = try Self.shownSurface()
        surface.apply(.transcript(try Self.liveChunk(order: 7, sessionKey: "acp:elsewhere")))

        XCTAssertEqual(surface.transcriptState.rows, [])
        XCTAssertNil(surface.transcriptState.cursor, "a foreign chunk must not move this session's cursor")
    }

    /// Nothing is folded before a page has resolved a session, exactly as
    /// nothing is folded before one has resolved a scope.
    func testATranscriptChunkArrivingBeforeASessionIsResolvedIsDropped() throws {
        var surface = FleetChatSurface()
        surface.apply(.transcript(try Self.liveChunk(order: 1)))

        XCTAssertEqual(surface.transcriptState.rows, [])
    }

    /// A chunk at or below the cursor is dropped BEFORE the taxonomy sees it.
    ///
    /// Not merely a de-duplication: the classifier is stateful, so a second
    /// pass over a tool call's update would find the pending title already
    /// consumed and re-render the result under the unnamed `tool` form. The
    /// guard is what stops a replayed chunk REWRITING a row that was correct.
    func testAReplayedChunkIsDroppedBeforeItCanDegradeTheRowItAlreadyProduced() throws {
        var surface = try Self.shownSurface()
        surface.apply(.transcript(try Self.liveChunk(
            order: 5,
            eventType: "acp.tool_call",
            payload: ["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash"]
        )))
        surface.apply(.transcript(try Self.liveChunk(
            order: 6,
            eventType: "acp.tool_call",
            payload: ["sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed"]
        )))
        XCTAssertEqual(surface.transcriptState.rows.map(\.body), ["Bash", "Bash  (completed)"])

        // The same chunk again, as a page window overlapping the stream.
        surface.apply(.transcript(try Self.liveChunk(
            order: 6,
            eventType: "acp.tool_call",
            payload: ["sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed"]
        )))
        XCTAssertEqual(
            surface.transcriptState.rows.map(\.body), ["Bash", "Bash  (completed)"],
            "a replayed chunk must neither duplicate its rows nor degrade them to the unnamed form"
        )
        XCTAssertEqual(surface.transcriptState.cursor, 6)
    }

    /// The classifier CARRIES across chunks, which is the only reason a tool
    /// result can name the tool it belongs to.
    func testTheSurfaceCarriesTheClassifierAcrossChunks() throws {
        var surface = try Self.shownSurface()
        surface.apply(.transcript(try Self.liveChunk(
            order: 1,
            eventType: "acp.tool_call",
            payload: ["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Read"]
        )))
        surface.apply(.transcript(try Self.liveChunk(
            order: 2,
            eventType: "acp.tool_call",
            payload: [
                "sessionUpdate": "tool_call_update", "toolCallId": "t1",
                "status": "completed", "rawOutput": "hello",
            ]
        )))

        XCTAssertEqual(
            surface.transcriptState.rows.last?.body, "Read  hello",
            "the result lost the title the call left behind, so the fold is not carrying the classifier"
        )
    }

    /// One chunk classifying into many rows still yields one row per line, each
    /// addressed by the chunk's cursor.
    func testOneChunkClassifiesIntoManyAddressableRows() throws {
        var surface = try Self.shownSurface()
        surface.apply(.transcript(try Self.liveChunk(order: 3, text: "alpha\nbeta")))

        XCTAssertEqual(surface.transcriptState.rows.map(\.id), ["3.0", "3.1"])
        XCTAssertEqual(surface.transcriptState.rows.map(\.body), ["alpha", "beta"])
    }

    /// The transcript stays inside its display ceiling, dropping the OLDEST
    /// rows, because it is a tail view of a running session.
    func testTheTranscriptStaysBoundedByItsDisplayCeiling() throws {
        var surface = try Self.shownSurface()
        for order in 1...(fleetTranscriptRowMax + 10) {
            surface.apply(.transcript(try Self.liveChunk(order: Int64(order), text: "row \(order)")))
        }

        XCTAssertEqual(surface.transcriptState.rows.count, fleetTranscriptRowMax)
        XCTAssertEqual(surface.transcriptState.rows.first?.body, "row 11", "the ceiling drops the oldest row")
        XCTAssertEqual(surface.transcriptState.rows.last?.body, "row \(fleetTranscriptRowMax + 10)")
    }

    /// A chunk this taxonomy does not carry moves the cursor and adds no row.
    ///
    /// Both halves matter. Skipping the row is the taxonomy's contract, and
    /// moving the cursor is what stops a session that only emits bookkeeping
    /// from re-reading the same chunk on every page.
    func testASilentChunkMovesTheCursorWithoutAddingARow() throws {
        var surface = try Self.shownSurface()
        surface.apply(.transcript(try Self.liveChunk(order: 4, eventType: "acp.usage", payload: [:])))

        XCTAssertEqual(surface.transcriptState.rows, [])
        XCTAssertEqual(surface.transcriptState.cursor, 4)
    }

    /// A page's chunks and a live chunk go through ONE door, so a surface built
    /// by replaying a page is identical to one built live.
    func testAPagedFoldAndALiveFoldProduceTheSameSurface() throws {
        let chunks = [
            try Self.liveChunk(order: 1, eventType: "acp.tool_call", payload: [
                "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash",
            ]),
            try Self.liveChunk(order: 2, text: "done"),
        ]
        var paged = try Self.shownSurface()
        var live = try Self.shownSurface()
        for chunk in chunks { paged.apply(.transcript(chunk)) }
        for chunk in chunks { live.apply(.transcript(chunk)) }

        XCTAssertEqual(paged, live)
        XCTAssertEqual(paged.transcriptState.rows.map(\.body), ["Bash", "done"])
    }

    /// The buffer door matches on the axis the event carries, and the FIRST
    /// page's unresolved axis is admitted rather than filtered out.
    ///
    /// The store's in-flight buffer reads this. Getting it wrong on the
    /// transcript axis would drop exactly what the buffer exists for: a chunk
    /// committed while the opening page was still reading.
    func testTheBufferDoorMatchesOnTheAxisTheEventCarries() throws {
        let resolved = try Self.shownSurface()
        let opening = FleetChatSurface()

        XCTAssertTrue(FleetChatEvent.transcript(try Self.liveChunk(order: 1)).belongsToPage(of: resolved))
        XCTAssertFalse(
            FleetChatEvent.transcript(try Self.liveChunk(order: 1, sessionKey: "acp:other")).belongsToPage(of: resolved),
            "another session's chunk must not be buffered against this page"
        )
        XCTAssertTrue(
            FleetChatEvent.transcript(try Self.liveChunk(order: 1)).belongsToPage(of: opening),
            "the first page has no session yet, and dropping here loses the chunk entirely"
        )
        // And the chat axis is unchanged by any of it.
        XCTAssertTrue(FleetChatEvent.activity(try Self.liveActivity(seq: 1)).belongsToPage(of: resolved))
        XCTAssertFalse(
            FleetChatEvent.activity(try Self.liveActivity(seq: 1, scope: "channel:other"))
                .belongsToPage(of: resolved)
        )
    }

    // MARK: - The chat route's way back

    /// Closing the chat pane returns the operator to the route they came from,
    /// not to Sessions.
    ///
    /// The pane is a route rather than a presentation, so Close has to name a
    /// destination rather than dismissing one. Always answering Sessions throws
    /// away whatever the operator had set up before they went to read the
    /// conversation, which for anybody who lives in Needs you is every time.
    @MainActor
    func testClosingChatReturnsToTheRouteItWasOpenedFrom() {
        let navigation = FleetNotchNavigation()

        navigation.route = .needsYou
        navigation.route = .chat
        XCTAssertEqual(navigation.routeBeforeChat, .needsYou)

        navigation.route = navigation.routeBeforeChat
        XCTAssertEqual(navigation.route, .needsYou)
    }

    /// Leaving chat does not make chat the way back.
    ///
    /// Without the guard, the route being left is recorded unconditionally, so
    /// the first Close records chat and the next one returns to the pane it
    /// just closed.
    @MainActor
    func testLeavingChatDoesNotMakeChatTheWayBack() {
        let navigation = FleetNotchNavigation()

        navigation.route = .usage
        navigation.route = .chat
        navigation.route = navigation.routeBeforeChat
        // Second visit, from where the first one put them.
        navigation.route = .chat

        XCTAssertEqual(navigation.routeBeforeChat, .usage, "Close would have reopened the pane it just closed")
    }

    /// The default is the roster, for an operator whose first act is Chat.
    @MainActor
    func testChatOpenedFirstReturnsToTheRoster() {
        let navigation = FleetNotchNavigation()

        navigation.route = .chat

        XCTAssertEqual(navigation.routeBeforeChat, .sessions)
    }

    // MARK: - Helpers

    /// A surface in the state a paged pane is in: one scope, one message, one
    /// activity row.
    private static func shownSurface() throws -> FleetChatSurface {
        var surface = FleetChatSurface()
        surface.scopeKey = "channel:copilot"
        surface.targetSessionKey = "acp:1"
        surface.messages = [FleetChatMessageRow(message: try liveMessage(id: "01J0A", body: "first"))]
        surface.activity = [try liveActivity(seq: 1)]
        return surface
    }

    /// Built from JSON in the shape the Rust struct defines, like every other
    /// wire value in this suite: a renamed key fails here rather than being
    /// renamed on both sides.
    private static func liveMessage(id: String, body: String, scope: String = "channel:copilot") throws -> FleetMessage {
        try FleetWire.decoder().decode(FleetMessage.self, from: Data("""
        {"id":"\(id)","scope_key":"\(scope)","sender":"copilot",
         "kind":"agent","body":"\(body)","created_at":1700000000000}
        """.utf8))
    }

    /// One live confirm event, built the way the STORE builds one: raw JSON in,
    /// through the page's own tolerant decode, with the scope read off the
    /// frame. Anything less would test a path the app does not take.
    private static func liveConfirm(id: String, state: String, scope: String = "channel:copilot") -> FleetChatEvent {
        let raw = confirmValue(id: id, state: state, scope: scope)
        return .confirm(card: FleetChatConfirmCard.decode(raw), scopeKey: scope)
    }

    /// A card whose `created_at` is the wrong TYPE, which is the failure a
    /// tolerant enum does not cover and the row-by-row decode exists for.
    private static func brokenConfirmValue(id: String, scope: String) -> JSONValue {
        (try? FleetWire.decoder().decode(JSONValue.self, from: Data("""
        {"confirm_id":"\(id)","scope_key":"\(scope)","tool":"spawn_session","arguments":{},
         "state":"open","created_at":"not-a-number","expires_at":2}
        """.utf8))) ?? .null
    }

    private static func confirmValue(id: String, state: String, scope: String, tool: String = "kill_session") -> JSONValue {
        (try? FleetWire.decoder().decode(JSONValue.self, from: Data("""
        {"confirm_id":"\(id)","scope_key":"\(scope)","tool":"\(tool)","arguments":{},
         "state":"\(state)","created_at":1,"expires_at":2}
        """.utf8))) ?? .null
    }

    /// One transcript chunk, built through the REAL decoder so a renamed wire
    /// key fails here rather than being renamed on both sides.
    ///
    /// THROWING, not `try?` with a hand-built fallback. A fallback defeats the
    /// only guarantee this helper offers: a renamed key would decode to nothing,
    /// the fallback would supply a chunk anyway, and the test would pass while
    /// the wire and this build disagreed. Every caller is already `throws`.
    private static func liveChunk(
        order: Int64,
        sessionKey: String = "acp:1",
        eventType: String = "acp.message",
        text: String = "hello",
        payload: [String: Any]? = nil
    ) throws -> FleetTranscriptChunk {
        let frame: [String: Any] = [
            "ingest_order": order,
            "event_id": "evt-\(order)",
            "session_key": sessionKey,
            "event_type": eventType,
            "payload": payload ?? ["text": text],
            "observed_at": 1_700_000_000_000,
        ]
        let data = try JSONSerialization.data(withJSONObject: frame)
        return try FleetWire.decoder().decode(FleetTranscriptChunk.self, from: data)
    }

    private static func liveActivity(seq: Int64, tool: String = "list_sessions", scope: String = "channel:copilot") throws -> FleetActivityRow {
        try FleetWire.decoder().decode(FleetActivityRow.self, from: Data("""
        {"seq":\(seq),"id":"act-\(seq)","scope_key":"\(scope)","tool":"\(tool)",
         "class":"read","outcome":"ok","created_at":1}
        """.utf8))
    }

    private static func deliveries(_ legs: String) throws -> [FleetMessageDelivery] {
        try FleetWire.decoder().decode(
            FleetMessageSendResult.self,
            from: Data(#"{"message_id":"01J0MSG","deliveries":[\#(legs)]}"#.utf8)
        ).deliveries
    }

    private func assertLabelsAreDistinctAndNamed<Value: Hashable>(
        _ values: [Value],
        _ label: (Value) -> String,
        _ operatorsShouldRecognise: (Value) -> Bool,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertFalse(values.isEmpty, "no variants to check", file: file, line: line)
        XCTAssertEqual(Set(values.map(label)).count, values.count, "two variants share a label", file: file, line: line)
        for value in values {
            XCTAssertFalse(label(value).trimmingCharacters(in: .whitespaces).isEmpty, "\(value) renders blank", file: file, line: line)
            if !operatorsShouldRecognise(value) {
                XCTAssertTrue(
                    label(value).lowercased().contains("unrecognised"),
                    "\(value) is a fallback but does not say so, so it reads like a real value",
                    file: file,
                    line: line
                )
            }
        }
        XCTAssertEqual(values.filter { !operatorsShouldRecognise($0) }.count, 1, "expected exactly one fallback variant", file: file, line: line)
    }

    /// A `FleetMessage` built from JSON in the shape the Rust struct defines
    /// (`ainb-hangar-proto::fleet::FleetMessage`). There is no shared fixture
    /// for a chat MESSAGE: part 2 ships fixtures for the channel, confirm and
    /// activity frames only, and this suite may not add files under
    /// `ainb-tui/`. Built from JSON rather than a Swift initialiser anyway, so
    /// a renamed wire key fails here instead of being renamed on both sides.
    private static func message(sender: String, body: String) throws -> FleetMessage {
        try FleetWire.decoder().decode(FleetMessage.self, from: Data("""
        {"id":"01J0MSG","scope_key":"channel:copilot","sender":"\(sender)",
         "kind":"user","body":"\(body)","created_at":1700000000000}
        """.utf8))
    }

    private static func json(_ raw: String) throws -> JSONValue {
        try FleetWire.decoder().decode(JSONValue.self, from: Data(raw.utf8))
    }

    private static func confirmCards(fromFixture name: String) throws -> [FleetChatConfirmCard] {
        try FleetWire.decoder()
            .decode(FleetConfirmListRawResult.self, from: try chatFixture(named: name))
            .confirms
            .map(FleetChatConfirmCard.decode)
    }

    private static func chatFixture(named name: String) throws -> Data {
        var repository = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repository.deleteLastPathComponent() }
        return try Data(contentsOf: repository
            .appendingPathComponent("ainb-tui/crates/ainb-hangar-proto/fixtures/chat")
            .appendingPathComponent(name))
    }
}

/// A stand-in for `fleet/acp_session_create` that records what each rung sent.
///
/// The ladder is tested through this rather than through a socket because what
/// is under test is WHICH frame goes out for a given refusal, and a socket adds
/// a daemon's opinion to an assertion about this client's decision.
@MainActor
private final class MintRecorder {
    private(set) var sent: [FleetAcpSessionCreateParams] = []
    private let refuseUntilAttempt: Int
    private let message: String

    /// `refuseUntilAttempt` is the first attempt that SUCCEEDS; every earlier
    /// one is refused with `message`.
    init(refuseUntilAttempt: Int = 1, message: String = "") {
        self.refuseUntilAttempt = refuseUntilAttempt
        self.message = message
    }

    func answer(_ params: FleetAcpSessionCreateParams) async throws -> FleetAcpSessionCreateResult {
        sent.append(params)
        guard sent.count >= refuseUntilAttempt else {
            throw FleetConnectionError.rpc(RPCError(code: -32602, message: message, data: nil))
        }
        return FleetAcpSessionCreateResult(
            sessionKey: "acp:01J0KEY",
            scopeKey: params.scopeKey ?? "session:acp:01J0KEY",
            turnDeadlineMs: 1_800_000
        )
    }
}
