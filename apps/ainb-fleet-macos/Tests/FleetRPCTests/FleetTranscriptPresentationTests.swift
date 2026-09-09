import Foundation
import XCTest
@testable import AINBFleet

/// The Swift half of the ACP transcript taxonomy, held against the Rust it was
/// ported from (`ainb-tui/crates/ainb-hangar-proto/src/transcript.rs`).
///
/// These assert on the EXACT strings, double spaces included, because that is
/// the whole contract: the notch and the terminal client render one transcript,
/// and an operator watching both must not have to learn two vocabularies for
/// one tool call. A test that only checked "the row mentions the tool" would
/// pass while the two surfaces drifted apart.
final class FleetTranscriptPresentationTests: XCTestCase {
    // MARK: - The taxonomy, arm by arm

    /// `acp.message` and `acp.thought` fold their text, one row per line, into
    /// their own lanes.
    func testProseAndThinkingFoldIntoTheirOwnLanes() {
        var classifier = AcpTranscriptClassifier()

        let prose = classifier.classify(
            eventType: "acp.message",
            payload: Self.json(["text": "first line\nsecond line"])
        )
        XCTAssertEqual(prose.map(\.lane), [.agent, .agent])
        XCTAssertEqual(prose.map(\.body), ["first line", "second line"])

        let thought = classifier.classify(
            eventType: "acp.thought",
            payload: Self.json(["text": "weighing it up"])
        )
        XCTAssertEqual(thought.map(\.lane), [.thinking])
        XCTAssertEqual(thought.map(\.body), ["weighing it up"])
    }

    /// A blank line inside a block yields no row, so a reply padded with
    /// newlines does not paint empty rows.
    func testBlankLinesInsideABlockYieldNoRows() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.message",
            payload: Self.json(["text": "one\n\n   \ntwo\n"])
        )

        XCTAssertEqual(rows.map(\.body), ["one", "two"])
    }

    /// A chunk with no text but a NON-TEXT block is named rather than dropped.
    ///
    /// The Rust's reason, kept: a transcript that silently omits a row reads as
    /// a complete one, so an image the agent produced has to leave a mark.
    func testANonTextBlockIsNamedRatherThanDropped() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.message",
            payload: Self.json(["text": "", "block": ["content": ["type": "image"]]])
        )

        XCTAssertEqual(rows.map(\.body), ["· image content"])
        XCTAssertEqual(rows.map(\.lane), [.agent])
    }

    /// A block whose content type this build cannot read still leaves a row,
    /// under the taxonomy's own placeholder rather than the adapter's.
    func testANonTextBlockWithNoTypeStillLeavesARow() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.message",
            payload: Self.json(["text": "  ", "block": ["content": ["nothing": 1]]])
        )

        XCTAssertEqual(rows.map(\.body), ["· non-text content"])
    }

    /// A chunk with neither text nor a block yields nothing at all.
    func testAnEmptyTextChunkYieldsNothing() {
        var classifier = AcpTranscriptClassifier()

        XCTAssertTrue(classifier.classify(eventType: "acp.message", payload: Self.json(["text": ""])).isEmpty)
    }

    /// A `tool_call` renders its title and a compact rawInput summary, picking
    /// the most telling field rather than dumping the JSON.
    func testAToolCallRendersItsTitleAndACompactInput() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call",
                "toolCallId": "t1",
                "title": "Bash",
                "status": "pending",
                "rawInput": ["command": "cargo test", "description": "run the suite"],
            ])
        )

        XCTAssertEqual(rows.map(\.lane), [.toolCall])
        XCTAssertEqual(rows.map(\.body), ["Bash  cargo test"], "two spaces, and the command not the description")
    }

    /// With no telling field the summary is a flat key=value rendering.
    func testAToolCallWithNoTellingFieldRendersFlatPairs() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call",
                "toolCallId": "t1",
                "title": "Weird",
                "rawInput": ["beta": 2, "alpha": "one"],
            ])
        )

        XCTAssertEqual(rows.map(\.body), ["Weird  alpha=one beta=2"])
    }

    /// A call with no rawInput at all is just its name, with no trailing gap.
    func testAToolCallWithNoInputIsJustItsName() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash"])
        )

        XCTAssertEqual(rows.map(\.body), ["Bash"])
    }

    /// A LATER update names its tool only because the call's title was
    /// remembered. This is the one reason the classifier is stateful at all.
    func testAnUpdateResolvesItsToolNameFromTheEarlierCall() {
        var classifier = AcpTranscriptClassifier()
        _ = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash", "status": "pending",
            ])
        )
        let update = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call_update",
                "toolCallId": "t1",
                "status": "completed",
                "content": [["type": "content", "content": ["type": "text", "text": "6 passed"]]],
            ])
        )

        XCTAssertEqual(update.map(\.lane), [.toolResult])
        XCTAssertEqual(update.map(\.body), ["Bash  6 passed"])
    }

    /// An update whose call fell outside the page degrades to the UNNAMED
    /// form, never to a guess and never to nothing.
    func testAnUpdateWithNoRememberedCallDegradesToTheUnnamedForm() {
        var classifier = AcpTranscriptClassifier()
        let update = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call_update", "toolCallId": "gone", "status": "completed",
            ])
        )

        XCTAssertEqual(update.map(\.body), ["tool  (completed)"])
    }

    /// A terminal update CONSUMES the remembered title, so a re-delivered
    /// update for the same id degrades exactly as an unknown one does.
    func testATerminalUpdateConsumesTheRememberedTitle() {
        var classifier = AcpTranscriptClassifier()
        _ = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Read"])
        )
        let first = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed"])
        )
        let repeated = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed"])
        )

        XCTAssertEqual(first.map(\.body), ["Read  (completed)"])
        XCTAssertEqual(repeated.map(\.body), ["tool  (completed)"], "the resolved id must be evicted")
    }

    /// A FAILED update is the error lane, carries `[error]`, and FORGETS the
    /// title. The Rust arm does all three and so must this one.
    func testAFailedUpdateIsAnErrorRowAndForgetsTheTitle() {
        var classifier = AcpTranscriptClassifier()
        _ = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash"])
        )
        let failed = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call_update",
                "toolCallId": "t1",
                "status": "failed",
                "rawOutput": "exit 1",
            ])
        )
        let afterwards = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed"])
        )

        XCTAssertEqual(failed.map(\.lane), [.error])
        XCTAssertEqual(failed.map(\.body), ["Bash  exit 1  [error]"])
        XCTAssertEqual(afterwards.map(\.body), ["tool  (completed)"], "a failed call must not keep its title")
    }

    /// A failure with no output still says which tool failed.
    func testAFailedUpdateWithNoOutputStillNamesItsTool() {
        var classifier = AcpTranscriptClassifier()
        _ = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash"])
        )
        let failed = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "failed"])
        )

        XCTAssertEqual(failed.map(\.body), ["Bash  [error]"])
    }

    /// `pending → in_progress` with no output is NOT a transcript row.
    ///
    /// Every adapter emits one per call, so rendering it would double the row
    /// count of every transcript for no information at all.
    func testAnInProgressUpdateWithNoOutputIsNotARow() {
        var classifier = AcpTranscriptClassifier()
        _ = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash"])
        )
        let moved = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "in_progress",
            ])
        )

        XCTAssertEqual(moved.count, 0)
    }

    /// An initial call that ALREADY carries its output emits both rows and
    /// names itself on the second without consulting the map.
    func testACallCarryingItsOwnOutputEmitsBothRows() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call",
                "toolCallId": "t1",
                "title": "Read",
                "status": "completed",
                "rawInput": ["file_path": "/tmp/x"],
                "content": [["type": "content", "content": ["type": "text", "text": "hello"]]],
            ])
        )

        XCTAssertEqual(rows.map(\.lane), [.toolCall, .toolResult])
        XCTAssertEqual(rows.map(\.body), ["Read  /tmp/x", "Read  hello"])
    }

    /// A diff or an embedded terminal contributes NOTHING to the summary, so a
    /// JSON blob is never rendered as if it were the tool's output.
    func testOnlyTextBlocksReachTheResultSummary() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call_update",
                "toolCallId": "t1",
                "status": "completed",
                "content": [
                    ["type": "diff", "oldText": "a", "newText": "b"],
                    ["type": "content", "content": ["type": "text", "text": "one line"]],
                ],
            ])
        )

        XCTAssertEqual(rows.map(\.body), ["tool  one line"])
    }

    /// With no readable content the summary falls back to `rawOutput`.
    func testTheResultSummaryFallsBackToRawOutput() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call_update",
                "toolCallId": "t1",
                "status": "completed",
                "content": [["type": "diff", "oldText": "a"]],
                "rawOutput": "fell back",
            ])
        )

        XCTAssertEqual(rows.map(\.body), ["tool  fell back"])
    }

    /// A multi-line tool output is FLATTENED to one row and clipped to the
    /// summary width, so a result never takes over the pane.
    func testAToolResultSummaryIsOneClippedLine() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call_update",
                "toolCallId": "t1",
                "status": "completed",
                "rawOutput": String(repeating: "abc\n", count: 200),
            ])
        )

        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(
            rows[0].body.unicodeScalars.count, "tool  ".unicodeScalars.count + 84,
            "the snippet is clipped to the summary width"
        )
        XCTAssertTrue(rows[0].body.hasSuffix("…"))
        XCTAssertFalse(rows[0].body.contains("\n"), "a result summary is one row")
    }

    /// Past the pending-call backstop the map is cleared wholesale rather than
    /// growing forever behind a cancelled turn.
    func testPendingToolTitlesAreCappedRatherThanLeaked() {
        var classifier = AcpTranscriptClassifier()
        for index in 0..<600 {
            _ = classifier.classify(
                eventType: "acp.tool_call",
                payload: Self.json([
                    "sessionUpdate": "tool_call", "toolCallId": "t\(index)", "title": "Read",
                ])
            )
        }
        // The clear is wholesale, so an id from before the last flush is gone
        // and its update degrades rather than naming a tool at random.
        let stale = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call_update", "toolCallId": "t0", "status": "completed"])
        )

        XCTAssertEqual(stale.map(\.body), ["tool  (completed)"])
    }

    /// A plan folds one row per entry, so it reads as the checklist it is.
    func testAPlanFoldsOneRowPerEntry() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.plan",
            payload: Self.json(["entries": [
                ["status": "completed", "content": "read the code"],
                ["status": "in_progress", "content": "write the port"],
                ["content": "run the suite"],
            ]])
        )

        XCTAssertEqual(rows.map(\.lane), [.toolCall, .toolCall, .toolCall])
        XCTAssertEqual(rows.map(\.body), [
            "plan · completed · read the code",
            "plan · in_progress · write the port",
            "plan · pending · run the suite",
        ])
    }

    /// A permission ask is named for the tool it gates, falling back to the
    /// call id when the envelope carries no title.
    func testAPermissionAskIsNamedForTheToolItGates() {
        var classifier = AcpTranscriptClassifier()

        let titled = classifier.classify(
            eventType: "acp.permission",
            payload: Self.json(["toolCall": ["title": "Bash", "toolCallId": "t1"]])
        )
        XCTAssertEqual(titled.map(\.lane), [.toolCall])
        XCTAssertEqual(titled.map(\.body), ["[approval] Bash"])

        let untitled = classifier.classify(
            eventType: "acp.permission",
            payload: Self.json(["toolCall": ["toolCallId": "t2"]])
        )
        XCTAssertEqual(untitled.map(\.body), ["[approval] t2"])

        let bare = classifier.classify(eventType: "acp.permission", payload: Self.json([:]))
        XCTAssertEqual(bare.map(\.body), ["[approval] tool"])
    }

    /// The truncation marker is the one row the transcript MUST show, in the
    /// error lane, because a silently short transcript reads as a complete one.
    func testTheTruncationMarkerIsLoudRatherThanSilent() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.transcript_truncated",
            payload: Self.json(["droppedRows": 1200])
        )

        XCTAssertEqual(rows.map(\.lane), [.error])
        XCTAssertEqual(rows.map(\.body), ["· transcript truncated · 1200 rows dropped"])
    }

    /// The three turn-end kinds close the run the way the process executor's
    /// own result line does, and only a completed turn is the calm lane.
    func testEveryTurnEndKindClosesTheRun() {
        var classifier = AcpTranscriptClassifier()

        let completed = classifier.classify(
            eventType: "acp.turn_completed",
            payload: Self.json(["durationMs": 1500])
        )
        XCTAssertEqual(completed.map(\.lane), [.toolResult])
        XCTAssertEqual(completed.map(\.body), ["· turn_completed · 1.5s"])

        let failed = classifier.classify(
            eventType: "acp.turn_failed",
            payload: Self.json(["durationMs": 61_000, "cause": "adapter exited"])
        )
        XCTAssertEqual(failed.map(\.lane), [.error])
        XCTAssertEqual(failed.map(\.body), ["· turn_failed · 1m1s · adapter exited"])

        let interrupted = classifier.classify(eventType: "acp.turn_interrupted", payload: Self.json([:]))
        XCTAssertEqual(interrupted.map(\.lane), [.error])
        XCTAssertEqual(interrupted.map(\.body), ["· turn_interrupted"])
    }

    /// A duration under a second reads in milliseconds, and a NEGATIVE one
    /// (clock skew between two chunks) reads as zero rather than as nonsense.
    func testDurationsRenderCompactlyAndNeverNegative() {
        var classifier = AcpTranscriptClassifier()

        XCTAssertEqual(
            classifier.classify(eventType: "acp.turn_completed", payload: Self.json(["durationMs": 250])).map(\.body),
            ["· turn_completed · 250ms"]
        )
        XCTAssertEqual(
            classifier.classify(eventType: "acp.turn_completed", payload: Self.json(["durationMs": -5])).map(\.body),
            ["· turn_completed · 0ms"]
        )
    }

    /// A FRACTIONAL duration is not a whole number of milliseconds and is
    /// omitted rather than rounded, so an adapter bug stays visible.
    func testAFractionalDurationIsOmittedRatherThanRounded() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.turn_completed",
            payload: try! FleetWire.decoder().decode(JSONValue.self, from: Data(#"{"durationMs":1.5}"#.utf8))
        )

        XCTAssertEqual(rows.map(\.body), ["· turn_completed"])
    }

    /// The chunk types the Rust is deliberately silent on stay silent here, and
    /// so does a type this build has never heard of.
    func testTheSilentAndUnknownChunkTypesYieldNothing() {
        var classifier = AcpTranscriptClassifier()
        for eventType in [
            "acp.usage", "acp.user_message", "acp.turn_started", "acp.context_rebuilt",
            "acp.some_future_kind", "", "message",
        ] {
            XCTAssertEqual(
                classifier.classify(eventType: eventType, payload: Self.json(["text": "ignored"])).count, 0,
                "\(eventType) must not render"
            )
        }
    }

    // MARK: - The caps, which are the reason a transcript cannot cost the pane

    /// The body cap counts CHARACTERS, not bytes.
    ///
    /// The Rust pins exactly this (`the_body_cap_never_splits_a_multibyte_char`)
    /// because adapter prose carries non-ASCII routinely and a byte-index cut
    /// would land mid-scalar. Three bytes per character here, so a byte cap
    /// would be caught by the count.
    func testTheBodyCapCountsCharactersNotBytes() {
        var classifier = AcpTranscriptClassifier()
        let huge = String(repeating: "日", count: 8192 * 2)
        let rows = classifier.classify(eventType: "acp.message", payload: Self.json(["text": huge]))

        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows[0].body.unicodeScalars.count, 8192, "capped by scalars")
        XCTAssertGreaterThan(rows[0].body.utf8.count, 8192, "and the byte length is larger, which is the point")
        XCTAssertTrue(rows[0].body.hasSuffix("…"), "the elision marker survives intact")
    }

    /// The cap counts SCALARS, not Swift `Character`s, and this is the test
    /// that tells the two apart.
    ///
    /// A grapheme cluster has no length limit: one base letter followed by a
    /// hundred thousand combining marks is a SINGLE `Character`. A cap counted
    /// in characters would see a length of one and let the whole thing through,
    /// which is not a loose bound but no bound at all, and it is reachable by
    /// any adapter that emits combining marks. Counting scalars, as the Rust
    /// does, is what makes the backstop a backstop.
    func testTheBodyCapIsNotDefeatedByOneEnormousGraphemeCluster() {
        var classifier = AcpTranscriptClassifier()
        // One base scalar plus 40,000 combining acute accents: 40,001 scalars
        // and, to Swift, exactly one Character.
        let oneHugeCluster = "e" + String(repeating: "\u{0301}", count: 40_000)
        XCTAssertEqual(oneHugeCluster.count, 1, "Swift really does read this as a single Character")

        let rows = classifier.classify(
            eventType: "acp.message",
            payload: Self.json(["text": oneHugeCluster])
        )

        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(
            rows[0].body.unicodeScalars.count, 8192,
            "a Character-counted cap would have passed all 40,001 scalars through untouched"
        )
    }

    /// An arbitrarily large block is capped rather than carried whole: every
    /// classified body sits in a `@Published` surface the whole notch observes.
    func testAHugeTextBlockIsCapped() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.message",
            payload: Self.json(["text": String(repeating: "x", count: 100_000)])
        )

        XCTAssertEqual(rows.count, 1, "one block, one body")
        XCTAssertEqual(rows[0].body.unicodeScalars.count, 8192)
    }

    /// A tool NAME is an adapter string too, and it does not pass through the
    /// line splitter. Capping only the prose left exactly this uncapped, which
    /// is the bug the Rust's `a_huge_tool_name_is_capped_too` records.
    func testAHugeToolTitleIsCappedToo() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.classify(
            eventType: "acp.tool_call",
            payload: Self.json([
                "sessionUpdate": "tool_call",
                "toolCallId": "t1",
                "title": String(repeating: "N", count: 50_000),
            ])
        )

        XCTAssertEqual(rows.map(\.lane), [.toolCall])
        XCTAssertEqual(rows[0].body.unicodeScalars.count, 8192)
    }

    /// One chunk cannot classify into unbounded rows, and the count of what was
    /// dropped is REPORTED rather than lost.
    ///
    /// The Rust asserts the exact dropped count for the reason it gives: an
    /// off-by-one here ships silently in both halves.
    func testOneChunkCannotClassifyIntoUnboundedRows() {
        var classifier = AcpTranscriptClassifier()
        let block = String(repeating: "line\n", count: 512 * 3)
        let rows = classifier.classify(eventType: "acp.message", payload: Self.json(["text": block]))

        XCTAssertEqual(rows.count, 512)
        XCTAssertEqual(rows.last?.body, "… \(512 * 3 - 511) more lines")
        XCTAssertEqual(rows.last?.lane, .toolResult)
    }

    // MARK: - Rows, ids and the lane labels

    /// A chunk's rows are addressed by its COMMIT CURSOR plus their index, so
    /// the same chunk classified twice produces the same ids.
    func testRowIDsAreAnchoredToTheCommitCursor() {
        var classifier = AcpTranscriptClassifier()
        let rows = classifier.rows(for: Self.chunk(
            order: 42,
            eventType: "acp.message",
            payload: ["text": "one\ntwo"]
        ))

        XCTAssertEqual(rows.map(\.id), ["42.0", "42.1"])
        XCTAssertEqual(rows.map(\.ingestOrder), [42, 42])
    }

    /// A chunk that classifies into nothing produces no rows at all, rather
    /// than an empty placeholder row.
    func testASilentChunkProducesNoRows() {
        var classifier = AcpTranscriptClassifier()

        XCTAssertEqual(classifier.rows(for: Self.chunk(order: 1, eventType: "acp.usage", payload: [:])), [])
    }

    /// Every lane renders a label and a spoken label, and only the error lane
    /// is loud. Exhaustive over the enum, so a sixth lane fails here.
    func testEveryLaneRendersADistinctLabel() {
        let labels = FleetTranscriptLane.allCases.map(FleetTranscriptLabels.lane)
        let spoken = FleetTranscriptLane.allCases.map(FleetTranscriptLabels.accessibilityLane)

        XCTAssertEqual(Set(labels).count, FleetTranscriptLane.allCases.count, "two lanes read the same")
        XCTAssertEqual(Set(spoken).count, FleetTranscriptLane.allCases.count, "two lanes SOUND the same")
        XCTAssertTrue(labels.allSatisfy { !$0.isEmpty })
        XCTAssertEqual(
            FleetTranscriptLane.allCases.filter(FleetTranscriptLabels.laneIsLoud), [.error],
            "a transcript that shouted at every tool call would not mark the line to act on"
        )
    }

    /// The classifier's EQUALITY includes its pending calls, because two
    /// surfaces with identical rows and different pending titles will classify
    /// the next chunk differently.
    func testClassifierEqualityIncludesThePendingCalls() {
        var withPending = AcpTranscriptClassifier()
        _ = withPending.classify(
            eventType: "acp.tool_call",
            payload: Self.json(["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash"])
        )

        XCTAssertNotEqual(withPending, AcpTranscriptClassifier())
    }

    // MARK: - Fixtures

    /// One transcript chunk in the shape the daemon frames it.
    static func chunk(
        order: Int64,
        sessionKey: String = "acp:1",
        eventType: String,
        payload: [String: Any]
    ) -> FleetTranscriptChunk {
        FleetTranscriptChunk(
            ingestOrder: order,
            eventID: "evt-\(order)",
            sessionKey: sessionKey,
            eventType: eventType,
            payload: json(payload),
            observedAt: 1_700_000_000_000
        )
    }

    /// A `JSONValue` built through the REAL decoder, so a payload here is one
    /// the wire could actually deliver.
    static func json(_ object: [String: Any]) -> JSONValue {
        let data = try! JSONSerialization.data(withJSONObject: object)
        return try! FleetWire.decoder().decode(JSONValue.self, from: data)
    }
}
