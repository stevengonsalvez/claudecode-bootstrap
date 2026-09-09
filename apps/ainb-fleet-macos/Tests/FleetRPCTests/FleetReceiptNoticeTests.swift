import XCTest
@testable import AINBFleet

/// What the operator is told about a `fleet/action`, before it is sent and
/// after it comes back.
///
/// REGRESSION, on the shared notice. A `fleet/action` that round-trips is not
/// an action that landed. The daemon answers a refused action with a successful
/// RPC carrying a non-delivered status plus a `detail`; the surface used to
/// report "Delivered. Confirming Fleet state." for every non-throwing call, so
/// a mirrored picker that refused the answer, sent ZERO keys and left the
/// target session sitting on its question still read as success here.
///
/// The reconcile cases cover the ONE action that inverts that shared reading,
/// where `UNKNOWN` is the daemon deliberately keeping a card rather than a
/// delivery it could not confirm, and the gate cases cover the refusals the
/// notch must state in the same order and words the terminal client does.
final class FleetReceiptNoticeTests: XCTestCase {
    func testFailedReceiptIsNotReportedAsDelivered() {
        let notice = FleetStore.controlNotice(
            for: receipt(.failed, detail: "visible picker option order does not match current question"),
            failurePrefix: "Interview"
        )
        XCTAssertFalse(notice.contains("Delivered"), "a FAILED receipt must never read as delivered: \(notice)")
        XCTAssertTrue(notice.contains("failed"), notice)
        XCTAssertTrue(
            notice.contains("visible picker option order does not match current question"),
            "the daemon's reason is the only place the cause exists, so it must be surfaced: \(notice)"
        )
    }

    func testRejectedAndUnknownAlsoSurfaceAsNotDelivered() {
        for status in [ActionReceiptStatus.rejected, .unknown] {
            let notice = FleetStore.controlNotice(for: receipt(status, detail: "nope"), failurePrefix: "Approval")
            XCTAssertFalse(notice.contains("Delivered"), "\(status) must not read as delivered: \(notice)")
            XCTAssertTrue(notice.hasPrefix("Approval "), notice)
        }
    }

    func testDeliveredKeepsTheConfirmingWording() {
        let notice = FleetStore.controlNotice(for: receipt(.delivered, detail: nil), failurePrefix: "Interview")
        XCTAssertEqual(notice, "Delivered. Confirming Fleet state.")
    }

    func testPendingIsNeitherDeliveredNorAFailure() {
        let notice = FleetStore.controlNotice(for: receipt(.pending, detail: nil), failurePrefix: "Interview")
        XCTAssertFalse(notice.contains("Delivered"), notice)
        XCTAssertTrue(notice.contains("awaiting delivery"), notice)
    }

    func testMissingDetailStillProducesAReadableNotice() {
        let notice = FleetStore.controlNotice(for: receipt(.failed, detail: "   "), failurePrefix: "Interview")
        XCTAssertEqual(notice, "Interview failed.")
    }

    /// The one action whose `UNKNOWN` is a WORKING outcome, not a failure.
    ///
    /// `reconcile_claude_structured` answers `UNKNOWN` for the case it has
    /// decided in the operator's favour: it cannot prove the interview is
    /// finished, so it KEEPS the card. Rendering that through the shared notice
    /// would put "could not be confirmed" beside a real delivery failure and
    /// send the reader looking for a fault that is not there.
    func testAnUnknownReconcileReadsAsStillCheckingRatherThanAFailure() {
        let retained = receipt(.unknown, detail: "Claude interview liveness is unresolved; Fleet card retained")
        let notice = FleetStore.reconcileNotice(for: retained)

        XCTAssertTrue(notice.hasPrefix("Still checking"), notice)
        XCTAssertTrue(
            notice.contains("Fleet card retained"),
            "the daemon's own account of what it did with the card is the actionable half: \(notice)"
        )
        XCTAssertFalse(
            notice.contains("could not be confirmed"),
            "the shared wording groups UNKNOWN with the failures, which inverts this arm's meaning: \(notice)"
        )
        XCTAssertNotEqual(
            notice,
            FleetStore.controlNotice(for: retained, failurePrefix: "Interview check"),
            "a retained card must not read the way a refused delivery does"
        )
    }

    /// The two settled outcomes and the two broken ones stay apart.
    ///
    /// `DELIVERED` covers both halves of a settled question, the interview
    /// being live and the picker having closed, and the daemon's detail is the
    /// only thing that says which, so it is carried verbatim. `FAILED` and
    /// `REJECTED` mean the CHECK did not run and read as the failures they are.
    func testReconcileTellsASettledCheckApartFromABrokenOne() {
        let live = FleetStore.reconcileNotice(for: receipt(.delivered, detail: "Claude interview is live"))
        XCTAssertEqual(live, "Interview checked: Claude interview is live")

        let cleared = FleetStore.reconcileNotice(
            for: receipt(.delivered, detail: "Claude native picker completed, cleared Fleet card")
        )
        XCTAssertTrue(cleared.contains("cleared Fleet card"), cleared)

        for status in [ActionReceiptStatus.failed, .rejected] {
            let broken = FleetStore.reconcileNotice(for: receipt(status, detail: "broker socket missing"))
            XCTAssertTrue(broken.hasPrefix("Interview check \(status.operatorToken)"), broken)
            XCTAssertTrue(broken.contains("broker socket missing"), broken)
            XCTAssertFalse(broken.contains("Still checking"), "a broken check must not read as an open one: \(broken)")
        }
    }

    /// A receipt with no detail still reads as a sentence rather than trailing
    /// a bare colon.
    func testReconcileWithoutADetailStillReadsAsASentence() {
        XCTAssertEqual(FleetStore.reconcileNotice(for: receipt(.delivered, detail: "  ")), "Interview checked.")
        XCTAssertEqual(
            FleetStore.reconcileNotice(for: receipt(.unknown, detail: nil)),
            "Still checking. The Fleet card is kept."
        )
        XCTAssertEqual(
            FleetStore.reconcileNotice(for: receipt(.pending, detail: nil)),
            "Interview check accepted, awaiting delivery."
        )
    }

    /// The WORDING of each of the five row refusals, one failing condition at
    /// a time.
    ///
    /// Mirrored from `reconcile_blocked_reason` in
    /// `ainb-plugin-hangar/src/screen/fleet.rs`. Every case here fails exactly
    /// one predicate, so this pins the five sentences and says nothing about
    /// which one wins when a row fails several. That is a separate property and
    /// it has its own test below, because a helper that defaults every
    /// dimension to a passing value cannot express it.
    @MainActor
    func testTheReconcileGateWordsEachRefusalTheWayTheTerminalClientDoes() throws {
        let store = FleetStore()

        XCTAssertEqual(
            store.reconcileBlockedReason(on: try session(provider: .codex, attention: .ask, structuredAnswer: true)),
            "reconcile is a Claude-only action"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(on: try session(management: .degraded, attention: .ask, structuredAnswer: true)),
            "degraded session has no reconcile channel"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(on: try session(attention: .approval, structuredAnswer: true)),
            "session is not waiting on a structured question"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(on: try session(attention: .ask, structuredAnswer: false)),
            "session lacks structured_answer capability"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(on: try session(attention: .ask, structuredAnswer: true, fingerprint: nil)),
            "no live structured request to reconcile"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(on: try session(attention: .ask, structuredAnswer: true, fingerprint: "  ")),
            "no live structured request to reconcile",
            "a blank fingerprint is no fingerprint; sending it would ask the daemon about the empty string"
        )
    }

    /// A row that fails SEVERAL predicates is refused by the earliest one, in
    /// the terminal client's order.
    ///
    /// This is the case the mirror exists for and the one the wording test
    /// cannot reach. `reconcile_blocked_reason` returns on its first match, so
    /// a row failing two names whichever predicate the author put first, and
    /// the two surfaces have to put them in the same place: an operator told
    /// "not waiting on a structured question" in the pane and "Claude-only" in
    /// the notch has no way to know which one to believe.
    ///
    /// Each case below fails at least two conditions and asserts the earlier
    /// wins, so reordering the predicates in `reconcileBlockedReason` fails
    /// here rather than passing quietly.
    @MainActor
    func testTheReconcileGateNamesTheEarliestRefusalWhenARowFailsSeveral() throws {
        let store = FleetStore()

        XCTAssertEqual(
            store.reconcileBlockedReason(
                on: try session(provider: .codex, management: .degraded, attention: .ask, structuredAnswer: true)
            ),
            "reconcile is a Claude-only action",
            "the provider is checked before the management state, as it is in the terminal client"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(
                on: try session(management: .degraded, attention: .approval, structuredAnswer: true)
            ),
            "degraded session has no reconcile channel",
            "the management state is checked before the attention state"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(
                on: try session(attention: .approval, structuredAnswer: false)
            ),
            "session is not waiting on a structured question",
            "the attention state is checked before the capability"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(
                on: try session(attention: .ask, structuredAnswer: false, fingerprint: nil)
            ),
            "session lacks structured_answer capability",
            "the capability is checked before the fingerprint"
        )
        XCTAssertEqual(
            store.reconcileBlockedReason(
                on: try session(provider: .codex, management: .degraded, attention: .approval, structuredAnswer: false, fingerprint: nil)
            ),
            "reconcile is a Claude-only action",
            "a row that fails every predicate must still name the first one"
        )
    }

    /// A row that clears all five is still refused by this client's own state,
    /// and that refusal comes LAST.
    ///
    /// The store under test has never connected, so it can send nothing. The
    /// assertion is that the row reasons are still reported first for a row
    /// that fails one: a transport complaint in front of "Claude-only" sends
    /// the reader to check the daemon for a card that was never reconcilable.
    @MainActor
    func testTheClientsOwnRefusalComesAfterEveryRowReason() throws {
        let store = FleetStore()
        let reconcilable = try session(attention: .ask, structuredAnswer: true)

        XCTAssertEqual(
            store.reconcileBlockedReason(on: reconcilable),
            "Fleet cannot send an action on this session right now"
        )
        XCTAssertFalse(store.canReconcileStructuredInterview(on: reconcilable))
        XCTAssertEqual(
            store.reconcileBlockedReason(on: try session(provider: .codex, attention: .ask, structuredAnswer: true)),
            "reconcile is a Claude-only action",
            "the row's own fact must outrank this client's transport state"
        )
    }

    /// The store's own second lock: a reconcile that reaches it while blocked
    /// is refused with the reason, never silently dropped.
    ///
    /// NOT reachable from the Verify button, and the doc used to imply it was.
    /// That button is hidden for a non-Claude card and disabled for every other
    /// refusal, so the UI never delivers a blocked click here. This is the lock
    /// behind the control, for the paths a disabled button does not cover: a
    /// row whose state moved between the render and the click, and any future
    /// caller of the store API that has not consulted the gate. Both leave the
    /// operator with a stated reason rather than a button that did nothing.
    @MainActor
    func testABlockedReconcileReachingTheStoreIsRefusedWithItsReason() throws {
        let store = FleetStore()
        store.reconcileStructuredInterview(on: try session(provider: .codex, attention: .ask, structuredAnswer: true))
        XCTAssertEqual(store.controlNotice, "reconcile is a Claude-only action")
    }

    /// A Codex interview card renders, and carries no Verify control at all.
    ///
    /// The terminal client hides `r Reconcile` rather than advertising a key
    /// that refuses, and this is the notch's version of that decision.
    /// `FleetInterviewDeck` gates on attention, capability, fingerprint and a
    /// parseable payload but NOT on provider, so a Codex session genuinely
    /// reaches this card. Asserting the deck exists first is what makes the
    /// second assertion mean "the card is here and the button is not" rather
    /// than "there is no card".
    ///
    /// `offersReconcileControl` is the predicate the view itself branches on,
    /// for both the button and the sentence beneath it, so this is a claim
    /// about what is rendered rather than about a parallel rule.
    @MainActor
    func testACodexInterviewCardRendersWithNoVerifyControl() throws {
        let store = FleetStore()
        let codex = try session(provider: .codex, request: Self.interviewPayload)
        let claude = try session(request: Self.interviewPayload)

        XCTAssertNotNil(
            FleetInterviewDeck(session: codex),
            "the Codex card must still form a deck, or this test proves nothing about hiding a control on one"
        )
        XCTAssertFalse(
            store.offersReconcileControl(on: codex),
            "a Codex card cannot ever be reconciled, so it must carry no button and no sentence about one"
        )

        XCTAssertNotNil(FleetInterviewDeck(session: claude))
        XCTAssertTrue(
            store.offersReconcileControl(on: claude),
            "hiding the control for the provider must not hide it for the one provider it works on"
        )
    }

    /// One structured interview in the shape the daemon sends it.
    private static let interviewPayload = """
    {"questions":[{"id":"q1","header":"Scope","question":"Which files?","options":["all","some"]}]}
    """

    private func session(
        provider: FleetProvider = .claude,
        management: ManagementState = .managed,
        attention: AttentionState = .ask,
        structuredAnswer: Bool = true,
        fingerprint: String? = "sha256:interview",
        request: String? = nil
    ) throws -> FleetSession {
        FleetSession(
            sessionKey: "claude:abc",
            provider: provider,
            providerSessionID: "sess-1",
            tmuxTarget: nil,
            processStartFingerprint: nil,
            cwd: "/work",
            displayName: "work",
            lifecycle: .running,
            activeWorkCount: nil,
            attention: attention,
            currentRequestFingerprint: fingerprint,
            currentRequest: try request.map { try JSONDecoder().decode(JSONValue.self, from: Data($0.utf8)) },
            management: management,
            transportHealth: .healthy,
            capabilities: FleetCapabilities(
                structuredAnswer: structuredAnswer, approvals: false, sendPrompt: false,
                continueTurn: false, retry: false, interrupt: false, start: false, stop: false,
                restart: false, kill: false, archive: false, tmuxAttach: false, tmuxText: false,
                verifiedPicker: false
            ),
            provenance: .authoritative,
            confidence: .high,
            discoveredAt: 0,
            lastObservedAt: 0,
            lifecycleUpdatedAt: 0,
            attentionUpdatedAt: 0,
            version: 1,
            updatedRevision: 1,
            model: nil,
            reasoningEffort: nil,
            modelUpdatedAt: nil
        )
    }

    private func receipt(_ status: ActionReceiptStatus, detail: String?) -> FleetActionReceipt {
        FleetActionReceipt(
            requestID: "req-1",
            sessionKey: "claude:abc",
            actionKind: "structured_answer",
            actionFingerprint: "fnv1a64:0",
            expectedVersion: 1,
            idempotencyKey: nil,
            status: status,
            detail: detail,
            sessionVersion: 1,
            createdAt: 0,
            updatedAt: 0
        )
    }
}
