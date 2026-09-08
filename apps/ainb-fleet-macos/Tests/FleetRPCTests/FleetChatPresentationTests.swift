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

    // MARK: - Helpers

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
