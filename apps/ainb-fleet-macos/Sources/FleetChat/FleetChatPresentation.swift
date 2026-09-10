import Foundation

// The macOS half of the Fleet chat surface, in pure display terms so the tests
// can hold it without a window. The TUI solved the same problems first
// (`ainb-plugin-hangar/src/screen/fleet_chat.rs`) and this MATCHES ITS
// SEMANTICS even though the idiom differs: same attribution vocabulary, same
// answerability gate, same "a card this build cannot name is never offered an
// approve button".
//
// Every `switch` below is exhaustive with NO `default`. That is deliberate and
// it is the whole point of the file: a new wire variant must fail to COMPILE
// here rather than fall through to a label that reads like something else. The
// bug that rule exists for is real -- an ACP session rendered as UNKNOWN in the
// Fleet panel because the provider was mapped twice and only one half knew the
// token. `default` is how that happens quietly.

/// Who wrote one chat row.
///
/// Derived from `FleetMessage.sender`, which the daemon records from the send's
/// `actor` and never from the body. The wire carries it precisely so a Pal
/// write cannot masquerade as a human's, and that guarantee dies at the last
/// inch if the pane renders both the same, so the label is part of the
/// contract, not decoration.
enum FleetChatActor: Equatable {
    /// A human at a client: `sender == "operator"`.
    case operatorHuman
    /// Pal writing through its MCP tools: `sender == "copilot"` on the wire,
    /// which keeps the pre-rename spelling deliberately.
    case pal
    /// An agent session replying in its own name: `sender` is a session key.
    case session(String)
    /// A row whose sender is blank.
    ///
    /// The daemon refuses a blank actor, so this should be unreachable over a
    /// current wire. It exists because the alternative default is "operator",
    /// and a row that cannot say who wrote it must NEVER claim a human did.
    case unattributed

    /// Map a wire `sender` token onto the actor the pane paints.
    static func from(wire sender: String) -> Self {
        switch sender.trimmingCharacters(in: .whitespacesAndNewlines) {
        case "": .unattributed
        case "operator": .operatorHuman
        case "copilot": .pal
        case let other: .session(other)
        }
    }

    /// The on-screen attribution label. Matches the TUI's vocabulary exactly:
    /// an operator reading both surfaces must not have to learn two words for
    /// the same fact.
    var label: String {
        switch self {
        case .operatorHuman: "YOU"
        case .pal: "PAL"
        case let .session(key): key
        case .unattributed: "UNATTRIBUTED"
        }
    }

    /// Spoken attribution. VoiceOver users get the SAME distinction sighted
    /// ones do: a pane that only separates the two by colour or alignment has
    /// not actually attributed anything.
    var accessibilityLabel: String {
        switch self {
        case .operatorHuman: "From you"
        case .pal: "From Pal"
        case let .session(key): "From session \(key)"
        case .unattributed: "Sender unknown"
        }
    }

    /// Whether the row is drawn on the operator's side of the timeline.
    ///
    /// Only a human's own writing is; a Pal row is never mine. Side alone
    /// is not the attribution (the label is), it is the second, glanceable
    /// signal that carries when a row scrolls past at speed.
    var isOperator: Bool {
        switch self {
        case .operatorHuman: true
        case .pal, .session, .unattributed: false
        }
    }

    /// A stable identifier for the accessibility tree and the UI tests, so a
    /// journey can assert "this row is attributed to Pal" rather than
    /// matching a substring that could appear anywhere in the pane.
    var identifier: String {
        switch self {
        case .operatorHuman: "fleet.chat.actor.operator"
        case .pal: "fleet.chat.actor.pal"
        case .session: "fleet.chat.actor.session"
        case .unattributed: "fleet.chat.actor.unattributed"
        }
    }
}

/// One timeline row, already attributed.
struct FleetChatMessageRow: Equatable, Identifiable {
    let id: String
    let actor: FleetChatActor
    let kind: FleetMessageKind
    let body: String
    /// Whether this row answers another one. `origin_message_id` is the only
    /// thing on the wire that says so.
    let isReply: Bool
    let createdAt: Int64

    init(message: FleetMessage) {
        id = message.id
        actor = .from(wire: message.sender)
        kind = message.kind
        body = message.body
        isReply = message.originMessageID != nil
        createdAt = message.createdAt
    }
}

/// One confirm card as this build understands it.
///
/// The list is decoded ROW BY ROW from raw JSON. A typed array would fail the
/// whole frame on one undecodable row and the operator would lose the cards
/// this build does understand along with the one it does not.
enum FleetChatConfirmCard: Equatable, Identifiable {
    /// A card this build decoded completely.
    case known(FleetConfirm)
    /// A card carrying something this build cannot decode. Rendered, never
    /// answerable.
    case unrecognised(confirmID: String, tool: String, detail: String)

    /// Decode one confirm frame, tolerantly.
    static func decode(_ value: JSONValue) -> Self {
        do {
            let data = try FleetWire.encoder().encode(value)
            return .known(try FleetWire.decoder().decode(FleetConfirm.self, from: data))
        } catch {
            return .unrecognised(
                confirmID: value.value("confirm_id")?.stringValue ?? "unknown",
                tool: value.value("tool")?.stringValue ?? "unknown",
                detail: String(describing: error)
            )
        }
    }

    var id: String {
        switch self {
        case let .known(confirm): confirm.confirmID
        case let .unrecognised(confirmID, _, _): confirmID
        }
    }

    /// The tool Pal asked to run.
    var tool: String {
        switch self {
        case let .known(confirm): confirm.tool
        case let .unrecognised(_, tool, _): tool
        }
    }

    /// The state label the row shows.
    var stateLabel: String {
        switch self {
        case let .known(confirm): FleetChatLabels.confirmState(confirm.state)
        case .unrecognised: "UNRECOGNISED"
        }
    }

    /// Whether this row may be approved, denied, or edited.
    ///
    /// Delegates to `FleetConfirm.isAnswerable` rather than re-deriving the
    /// rule. That gate existing in ONE place is the reason an unknown state
    /// cannot be approved blind: a second copy here would be a second thing to
    /// remember to update, and the state that got missed would be the one this
    /// build could not name.
    var isAnswerable: Bool {
        switch self {
        case let .known(confirm): confirm.isAnswerable
        case .unrecognised: false
        }
    }

    /// Why the row is not answerable, for the operator and the bug report.
    var refusal: String {
        switch self {
        case let .known(confirm):
            "\(confirm.confirmID) is \(FleetChatLabels.confirmState(confirm.state)), not answerable"
        case let .unrecognised(_, _, detail):
            "card not understood by this build, not answerable: \(detail)"
        }
    }

    /// The tool arguments, already projected by the daemon.
    ///
    /// The daemon projects a card's arguments down to its tool's declared
    /// schema keys before persisting, so model-authored prose never reaches
    /// this line. The pane renders what it is given and adds nothing: it does
    /// not read the arguments, it re-encodes them.
    var argumentsLine: String {
        switch self {
        case let .known(confirm):
            (try? FleetWire.encoder().encode(confirm.arguments)).map { String(decoding: $0, as: UTF8.self) } ?? ""
        case .unrecognised:
            ""
        }
    }
}

/// Every display mapping for a part-2 wire enum, in one place.
///
/// One place on purpose: the ACP-provider bug happened because a provider was
/// mapped TWICE and only one half learned the new token. A single mapping
/// cannot disagree with itself.
enum FleetChatLabels {
    static func messageKind(_ kind: FleetMessageKind) -> String {
        switch kind {
        case .user: "prompt"
        case .agent: "reply"
        case .marker: "marker"
        case .unknown: "unrecognised kind"
        }
    }

    /// The engine the header shows, or the honest absence.
    ///
    /// "not reported" rather than a name, because the daemon serves no read for
    /// the running adapter and a plausible-looking guess in this slot is worse
    /// than a gap: an operator reading a wrong engine has no reason to look
    /// again, while one reading "not reported" knows to pick.
    static func palEngine(_ dial: FleetPalDial) -> String {
        dial.engine ?? "not reported"
    }

    /// The guardrail dial the header shows.
    ///
    /// `.unknown` is the tolerant decode's fallback, so it means the daemon
    /// named a mode this build cannot, which is a DIFFERENT fact from never
    /// having been told and reads differently.
    static func palMode(_ dial: FleetPalDial) -> String {
        guard let mode = dial.mode else { return "not reported" }
        switch mode {
        case .help: return "help"
        case .guarded: return "guarded"
        case .yolo: return "yolo"
        case .unknown: return "unrecognised mode"
        }
    }

    /// The model the header shows.
    ///
    /// Three states, not two. No engine means nothing can be said about a
    /// model; a known engine with no override is running the adapter's own
    /// default, which is a fact rather than an absence.
    static func palModel(_ dial: FleetPalDial) -> String {
        if let model = dial.model, !model.isEmpty { return model }
        return dial.engine == nil ? "not reported" : "adapter default"
    }

    static func confirmState(_ state: FleetConfirmState) -> String {
        switch state {
        case .open: "OPEN"
        case .approved: "APPROVED"
        case .denied: "DENIED"
        case .expired: "EXPIRED"
        case .unknown: "UNRECOGNISED"
        }
    }

    static func activityClass(_ activityClass: FleetActivityClass) -> String {
        switch activityClass {
        case .read: "READ"
        case .write: "WRITE"
        case .destructive: "DESTRUCTIVE"
        case .unknown: "UNRECOGNISED"
        }
    }

    /// Whether a class is drawn with the loud styling.
    ///
    /// `.unknown` is loud on purpose: over-warning about a class from a newer
    /// daemon is recoverable, silently painting it as a harmless read is not.
    static func activityClassIsLoud(_ activityClass: FleetActivityClass) -> Bool {
        switch activityClass {
        case .destructive, .unknown: true
        case .read, .write: false
        }
    }

    static func activityOutcome(_ outcome: FleetActivityOutcome) -> String {
        switch outcome {
        case .ok: "ok"
        case .denied: "denied"
        case .expired: "expired"
        case .error: "error"
        case .unknown: "unrecognised"
        }
    }

    static func channelKind(_ kind: FleetChannelKind) -> String {
        switch kind {
        case .pal: "Pal"
        case .broadcast: "Broadcast"
        case .unknown: "Unrecognised channel"
        }
    }

    /// The adapter name, as the daemon's registry spells it.
    ///
    /// Verbatim rather than prettified: the registry is config-driven, so there
    /// is no fixed set to map, and an operator reading `claude-agent-acp` here
    /// and in `ainb fleet adapter list` is reading one vocabulary.
    static func palProvider(_ provider: String) -> String {
        provider.isEmpty ? "Unrecognised provider" : provider
    }

    /// The guardrail dial, in words. Wildcard-free, so a new mode is a compile
    /// error here rather than a dial rendering as whichever arm was last.
    static func palMode(_ mode: FleetPalMode) -> String {
        switch mode {
        case .help: "Help (reads only)"
        case .guarded: "Guarded (writes ask)"
        case .yolo: "Yolo (writes fire)"
        case .unknown: "Unrecognised mode"
        }
    }

    /// The state word for one delivery leg.
    ///
    /// The daemon's own token, verbatim, because the TUI pane
    /// (`fleet_chat::delivery_state_label`) and the CLI (`msg::render_delivery`)
    /// print exactly these words: an operator watching two surfaces must read
    /// ONE vocabulary. Wildcard-free, so a new status is a compile error here.
    static func deliveryState(_ state: ActionReceiptStatus) -> String {
        switch state {
        case .pending: "PENDING"
        case .delivered: "DELIVERED"
        case .failed: "FAILED"
        case .unknown: "UNKNOWN"
        case .rejected: "REJECTED"
        }
    }

    /// One leg as one line: state, recipient, and the daemon's REASON when it
    /// has one. Mirrors the TUI's `receipt_line`.
    static func receiptLine(_ delivery: FleetMessageDelivery) -> String {
        let state = deliveryState(delivery.state)
        let detail = delivery.detail?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return detail.isEmpty
            ? "\(state) \(delivery.sessionKey)"
            : "\(state) \(delivery.sessionKey) · \(detail)"
    }

    /// What a send actually did, as one operator-facing line.
    ///
    /// A send whose only leg came back REJECTED still answers 200 on the RPC:
    /// the daemon reports per-leg honesty in the RESULT, not in the transport.
    /// A client that discarded the result would clear the composer and show the
    /// operator's own message in the timeline with no notice at all, i.e. a
    /// 1-of-1 fan-out where 0 delivered reading as complete success. The
    /// counts and the words are the TUI's (`ChatState::apply_receipts` plus
    /// `receipt_line`), so the two surfaces cannot disagree about what happened.
    static func deliverySummary(_ deliveries: [FleetMessageDelivery]) -> String {
        let total = deliveries.count
        let undelivered = deliveries.filter { $0.state != .delivered }
        let delivered = total - undelivered.count
        var line = "delivered to \(delivered)/\(total)"
        if !undelivered.isEmpty {
            line += " · \(undelivered.count) not delivered"
            // The reason, per refused leg: "REJECTED" alone does not say
            // whether to retry or to go and look at the session.
            line += " · " + undelivered.map(receiptLine).joined(separator: " · ")
        }
        return line
    }
}

/// Pal's engine, guardrail dial and model: what is in force, what it
/// can be moved to, and what the last move did.
///
/// DELIBERATELY NOT A FIELD ON `FleetChatSurface`, and that placement is the
/// whole design rather than a filing choice. A page REBUILDS the surface from
/// an empty one, and nothing a page reads can rebuild this: the registry comes
/// from `fleet/adapter_list`, which no page calls, and the settings come from a
/// `fleet/copilot_configure` result, which only an operator's own action
/// produces. On the surface it would be wiped by the next safety-net page
/// thirty seconds later, which is exactly the defect the transcript's carried
/// state was introduced to fix. It lives beside the surface on the store, so it
/// outlives every page and is untouched by one.
///
/// It is also a different LIFETIME. The surface belongs to one conversation;
/// this belongs to the channel behind it, and survives the session swap that
/// replaces the conversation's target outright.
struct FleetPalDial: Equatable {
    /// Every adapter the daemon named, in the order it named them.
    var adapters: [FleetAdapter] = []
    /// Whether `fleet/adapter_list` has answered on THIS connection.
    ///
    /// Distinguishes "the registry is empty" from "nobody has asked yet". They
    /// render differently, because the first is a fact about the daemon's
    /// config and the second is a pending read that would otherwise look like
    /// that fact.
    ///
    /// Per connection, not per app run: a reconnect can be to a different
    /// daemon home with a different `[acp.adapters]`. `beginConnection` clears
    /// this while LEAVING `adapters` in place, so the next pane bootstrap
    /// re-reads without the picker going blank while it does.
    var adaptersListed: Bool = false
    /// The adapter in force, or `nil` when this client HAS NOT BEEN TOLD.
    ///
    /// `nil` is the state a freshly opened pane is in and it stays that way
    /// until an operator applies a configure, because the daemon serves no read
    /// for it: `fleet/adapter_list` names what COULD be spawned, the Pal
    /// channel's wire form carries no provider, and the roster files an ACP
    /// session under the token `acp` rather than its concrete adapter.
    ///
    /// The terminal client's dial defaults this to the registry's first entry.
    /// That is a GUESS, and on a machine whose Pal is running the second
    /// adapter it is a wrong one displayed as a fact. This surface reports the
    /// gap instead, because a header that says "not reported" sends an operator
    /// to the right question and a header naming the wrong engine does not.
    var engine: String? = nil
    /// The guardrail dial in force, `nil` until told, for the same reason.
    ///
    /// NEVER `.guarded` as a stand-in. `guarded` is the daemon's default, so
    /// showing it unasked would read as a fact about the running channel, and
    /// the one value it would be wrong about is `yolo`.
    var mode: FleetPalMode? = nil
    /// The model override in force. `nil` means either untold or no override,
    /// which is why the header words it against `engine` rather than alone.
    var model: String? = nil
    /// The reasoning-effort override in force, when the daemon reported one.
    var reasoningEffort: String? = nil
    /// Why the dial is empty or what the last call refused with. Kept verbatim:
    /// the daemon's own wording names the adapter it did not know, or the
    /// channel that has no live session, and nothing else says which.
    var detail: String? = nil

    /// The models the CURRENT engine declares, in picker order.
    ///
    /// Empty whenever the engine is unknown, which is not the same shape as an
    /// engine that declares none, and the picker words the two differently.
    var models: [String] {
        guard let engine else { return [] }
        return adapters.first(where: { $0.name == engine })?.models ?? []
    }
}

/// Everything one page of the Pal conversation put on screen.
///
/// A value type so the store can replace it atomically: a half-applied refresh
/// that shows this poll's cards next to the last poll's timeline is a pane that
/// lies about what Pal just did.
struct FleetChatSurface: Equatable {
    /// The channel scope being paged, once resolved.
    var scopeKey: String? = nil
    /// The Pal session messages are delivered to. Without it there is
    /// nobody to send to, and the composer says so rather than posting into a
    /// conversation no agent answers on.
    var targetSessionKey: String? = nil
    var messages: [FleetChatMessageRow] = []
    var confirms: [FleetChatConfirmCard] = []
    var activity: [FleetActivityRow] = []
    /// Everything about the transcript that SURVIVES a page.
    ///
    /// One nested value, not four loose fields, and that is the whole point.
    /// A page rebuilds its surface from an empty one, so every field here is
    /// either carried across that boundary or rebuilt by it, and nothing about
    /// a bare `var` says which. `carryTranscriptForward` used to answer with a
    /// hand-written assignment per field, so a new field landed on whichever
    /// side its author remembered and the compiler was happy either way.
    ///
    /// Two bugs came out of that one gap, and they were the same bug twice.
    /// `transcriptDetail` was put on the carry side, so a momentary refusal
    /// pinned its banner above a live transcript until the app restarted. The
    /// cursor was carried but not USED on the resubscribe, so a reconnect
    /// silently lost every row committed during the outage.
    ///
    /// Now the question is answered once, at the declaration: a field inside
    /// this type is carried, a field beside it is rebuilt.
    var transcriptState = FleetTranscriptState()
    /// Why the transcript is empty, when it is empty for a REASON.
    ///
    /// The same distinction `confirmsDetail` draws: a daemon that does not
    /// advertise `fleet.transcript.read`, or one that refused the page, is not
    /// the same fact as a session that has not run a turn yet, and a pane that
    /// rendered both as silence would tell the operator the agent is idle when
    /// it simply cannot see.
    ///
    /// A PEER of `transcriptState`, never a member, and that placement is the
    /// fix rather than a detail of it: this describes the page that set it, so
    /// it must be rebuilt by the next page and cannot be carried by accident.
    var transcriptDetail: String? = nil
    /// Why the confirm feed is empty, when it is empty for a REASON. A daemon
    /// built between phases answers -32601 here, and a pane that renders that
    /// as "no cards open" is telling the operator there is nothing to approve
    /// when it simply cannot see.
    var confirmsDetail: String? = nil
    /// The daemon's own refusal wording when the Pal session could not be
    /// resolved. Kept verbatim: it is the only actionable thing an operator
    /// gets, and it is usually "this scope is already held by a session whose
    /// cwd is X, not Y".
    var sessionDetail: String? = nil

    /// Fold one live notification into this page.
    ///
    /// The filter is HERE rather than at the call site because the daemon's
    /// streams are broader than this pane: `fleet/message_event` carries every
    /// committed message on the socket, not this scope's. The surface is the
    /// only thing that knows which conversation is on screen, so it is the only
    /// thing that can say an event is not this one's. An event that does not
    /// belong is dropped, never rendered.
    ///
    /// The three chat frames are filtered by SCOPE and the transcript frame by
    /// SESSION, because that is what each one is addressed by: a chat message
    /// is filed under a scope key, while a transcript chunk belongs to the ACP
    /// session that produced it. Filtering the transcript on scope would match
    /// nothing at all, and filtering it on nothing would paint another
    /// session's execution into this pane.
    ///
    /// Nothing is folded before a page has resolved the axis it is filtered on:
    /// an event that cannot be proved to belong here does not get the benefit
    /// of the doubt.
    mutating func apply(_ event: FleetChatEvent) {
        switch event {
        case let .message(message):
            guard scopeKey != nil, scopeKey == message.scopeKey else { return }
            upsert(FleetChatMessageRow(message: message))
        case let .confirm(card, cardScope):
            guard scopeKey != nil, scopeKey == cardScope else { return }
            upsert(card)
        case let .activity(row):
            guard scopeKey != nil, scopeKey == row.scopeKey else { return }
            upsert(row)
        case let .transcript(chunk):
            guard targetSessionKey != nil, targetSessionKey == chunk.sessionKey else { return }
            append(chunk)
        }
    }

    /// Replace the row with this id, or append it.
    ///
    /// Append, because `fleet/message_list` returns ascending commit order and
    /// a committed message is newer than everything already paged. Replace in
    /// place on a repeat, because the daemon replays from a cursor and a
    /// boundary row can arrive live AND in the page: appending it twice would
    /// show the operator their own message twice with no way to tell which is
    /// real.
    private mutating func upsert(_ row: FleetChatMessageRow) {
        if let index = messages.firstIndex(where: { $0.id == row.id }) {
            messages[index] = row
            return
        }
        messages.append(row)
        // The same ceiling the page asks for, so a pane left open for a day
        // does not grow without bound. The oldest goes, matching what a fresh
        // page of a longer conversation would show.
        if messages.count > Int(fleetMessageListMax) {
            messages.removeFirst(messages.count - Int(fleetMessageListMax))
        }
    }

    /// Upsert a confirm card by its confirm id.
    ///
    /// An ANSWERED card is kept and re-rendered in its new state rather than
    /// dropped: `fleet/confirm_list` only returns open cards, so dropping it
    /// here would make the card vanish the instant the operator approved it,
    /// with no confirmation that the approval was what removed it. The next
    /// page retires it.
    private mutating func upsert(_ card: FleetChatConfirmCard) {
        if let index = confirms.firstIndex(where: { $0.id == card.id }) {
            confirms[index] = card
            return
        }
        confirms.append(card)
    }

    /// Append one activity row, oldest-first, bounded like its page.
    ///
    /// APPEND, not prepend: `fleet/activity_list` pages `ORDER BY seq ASC`, so
    /// the feed reads oldest at the top, and a live row prepended to that is a
    /// feed that puts the newest event above rows it happened after. One
    /// ordering rule for both halves or the pane cannot be read at all.
    private mutating func upsert(_ row: FleetActivityRow) {
        if let index = activity.firstIndex(where: { $0.seq == row.seq }) {
            activity[index] = row
            return
        }
        activity.append(row)
        if activity.count > Int(fleetActivityListMax) {
            activity.removeFirst(activity.count - Int(fleetActivityListMax))
        }
    }

    /// Classify one transcript chunk and append its rows, oldest first.
    ///
    /// APPEND, never upsert, and the guard in front of it is the cursor rather
    /// than a row id. One chunk classifies into MANY rows, and the classifier
    /// is stateful, so re-running a chunk is not idempotent: the second pass
    /// finds the pending tool title already consumed and renders the result
    /// under the unnamed `tool` form. The cursor stops the second pass ever
    /// happening, which is both cheaper and the only version that is correct.
    ///
    /// Bounded like its neighbours, dropping the OLDEST rows, because this is
    /// a tail view of a running session and the newest rows are the ones an
    /// operator is reading.
    private mutating func append(_ chunk: FleetTranscriptChunk) {
        if let cursor = transcriptState.cursor, chunk.ingestOrder <= cursor { return }
        transcriptState.cursor = chunk.ingestOrder
        transcriptState.rows.append(contentsOf: transcriptState.classifier.rows(for: chunk))
        if transcriptState.rows.count > fleetTranscriptRowMax {
            transcriptState.rows.removeFirst(transcriptState.rows.count - fleetTranscriptRowMax)
        }
    }
}

/// One live chat notification, in the three shapes the chat surface folds.
///
/// Every case carries what the PAGE carries, built by the page's own
/// constructor, so the live half and the paged half of one surface cannot
/// disagree about a row. `FleetChatMessageRow.init(message:)` and
/// `FleetChatConfirmCard.decode` are the same two the page calls.
///
/// The confirm case carries a decoded CARD rather than a `FleetConfirm`, and
/// that is the same reason rather than an inconsistency: `confirm_list` is
/// decoded row by row precisely so one card this build cannot read does not
/// cost the operator the ones it can, and a live card that skipped that
/// tolerance would be the one shape of card the pane could not show. Its scope
/// rides alongside because an unrecognised card has no readable fields to take
/// it from.
/// The transcript case carries the chunk VERBATIM rather than pre-classified
/// rows, and that is the same reasoning once more: the taxonomy is stateful, so
/// the rows a chunk produces depend on which classifier reads it, and the
/// classifier that must read it is the one belonging to the surface the chunk
/// is folded into. Classifying at the door would have bound every chunk to
/// whichever classifier happened to be current when the frame arrived,
/// including one owned by a page the store went on to disown.
enum FleetChatEvent: Equatable, Sendable {
    case message(FleetMessage)
    case confirm(card: FleetChatConfirmCard, scopeKey: String)
    case activity(FleetActivityRow)
    case transcript(FleetTranscriptChunk)

    /// The scope this event was filed under, for the three chat frames that
    /// have one. Nil for the transcript, which is addressed by session.
    var scopeKey: String? {
        switch self {
        case let .message(message): message.scopeKey
        case let .confirm(_, scopeKey): scopeKey
        case let .activity(row): row.scopeKey
        case .transcript: nil
        }
    }

    /// The ACP session this event belongs to, for the transcript frame that has
    /// one. Nil for the three chat frames, which are addressed by scope.
    ///
    /// Every event names exactly ONE of the two axes, and which one it names is
    /// which stream it came off. A shape that named neither could not be
    /// filtered at all.
    var sessionKey: String? {
        switch self {
        case .message, .confirm, .activity: nil
        case let .transcript(chunk): chunk.sessionKey
        }
    }

    /// Whether a page that is still RUNNING should buffer this event to replay
    /// onto its own result.
    ///
    /// Filtered at the door, per axis, for the reason the store's buffer
    /// documents: the daemon's streams are broader than this pane, so an
    /// unfiltered buffer collects every conversation's traffic while a page
    /// whose RPC hangs holds the in-flight count open.
    ///
    /// The nil case on either axis is NOT a hole in that filter, it is the
    /// FIRST page. Until one publishes there is nothing to compare against, and
    /// a filter that answered "no match" there would drop exactly what the
    /// buffer exists for. Replay applies the paged surface's own scope and
    /// session, so nothing foreign gets rendered.
    func belongsToPage(of surface: FleetChatSurface) -> Bool {
        switch self {
        case .message, .confirm, .activity:
            return surface.scopeKey == nil || surface.scopeKey == scopeKey
        case .transcript:
            return surface.targetSessionKey == nil || surface.targetSessionKey == sessionKey
        }
    }
}

/// Seconds between SAFETY-NET pages while the chat surface is open.
///
/// The pane is carried by `fleet/message_subscribe` and the three notifications
/// that follow it, so this timer is not how the conversation arrives any more.
/// It stays, at thirty seconds rather than one, because a push stream has one
/// failure mode a poll does not: silence and health look identical. The
/// daemon's chat notification forwarder drops frames when a slow client lags
/// its broadcast channel (`spawn_notification_forwarder` logs the miss and says
/// in as many words that the client re-reads via `fleet/confirm_list` and
/// `fleet/activity_list`), and nothing on this side is told. Without a page
/// behind it, one dropped frame strands the pane until the operator notices and
/// hits Refresh.
///
/// Thirty seconds is the number because it is far enough out to make the RPC
/// cost of an open pane a rounding error (four calls per half minute against
/// five every second before this), and near enough that a stranded pane repairs
/// itself inside the time an operator spends reading the message they are
/// answering. It deliberately does NOT match the TUI's one-second poll: the TUI
/// has no subscription, so its timer IS its transport.
let fleetChatSafetyNetInterval = Duration.seconds(30)
