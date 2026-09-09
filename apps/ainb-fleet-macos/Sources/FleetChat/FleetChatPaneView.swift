import SwiftUI

/// The Fleet copilot conversation: timeline, composer, confirm cards, activity.
///
/// Attribution is the load-bearing part of this view. A copilot row and an
/// operator row are separated FOUR ways -- a named label, a colour, a side, and
/// an accessibility label -- because the wire carries `sender` precisely so a
/// copilot write cannot masquerade as a human's, and that guarantee dies at the
/// last inch if the pane renders both the same. Colour alone would not survive
/// VoiceOver; side alone would not survive a screenshot; the LABEL is the one
/// that always reads, and the rest make it glanceable.
struct FleetChatPaneView: View {
    @ObservedObject var store: FleetStore
    /// How this host dismisses the pane.
    ///
    /// Injected rather than taken from `@Environment(\.dismiss)` because the
    /// pane has two hosts with nothing in common: a sheet, where dismiss is the
    /// right verb, and a ROUTE inside the notch panel, where there is no
    /// presentation to dismiss and the close has to move the navigation back.
    /// An environment dismiss in the second host is not an error, it is a
    /// button that silently does nothing, which is the worse failure.
    let close: () -> Void
    @State private var composer = ""
    @State private var editingCard: FleetChatConfirmCard?
    @State private var editedArguments = ""

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(FleetChatPalette.separator)
            if store.canReadChat {
                HSplitView {
                    conversation.frame(minWidth: 380)
                    sidebar.frame(minWidth: 280, idealWidth: 320)
                }
            } else {
                unavailable
            }
        }
        .frame(minWidth: 760, minHeight: 520)
        .background(FleetChatPalette.canvas)
        // The bootstrap page, then a slow safety net.
        //
        // The conversation itself arrives on the store's live subscription,
        // which belongs to the CONNECTION and is opened once when it comes up.
        // This view opens nothing: a subscription per appearance would mean a
        // second stream every time the sheet is reopened, all of them writing
        // the same surface.
        //
        // Bound to the pane's lifetime: the loop is cancelled when the sheet
        // closes, so a closed chat costs the daemon nothing. The sleep is where
        // that cancellation is observed, and it RETURNS rather than falling
        // through, so a sheet closed during the wait cannot spend one more page
        // on a pane nobody is looking at.
        .task {
            // The pane announces itself, and the store folds live events only
            // while at least one is on screen. `chat` is observed by the whole
            // notch, so an event folded with no pane open redraws the roster
            // for nothing.
            store.chatPaneAppeared()
            defer { store.chatPaneDisappeared() }
            // The registry, once. NOT in the loop below: the adapter list is
            // the daemon's own config and does not move under a running
            // daemon, so re-reading it every safety-net page would spend a
            // round trip a minute to learn the same answer.
            store.refreshAdaptersIfNeeded()
            while !Task.isCancelled {
                await store.refreshChatOnce()
                do {
                    try await Task.sleep(for: fleetChatSafetyNetInterval)
                } catch {
                    return
                }
            }
        }
        .sheet(item: $editingCard) { card in
            confirmEditor(card)
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                Text("Copilot chat")
                    .font(.title3.weight(.bold))
                if let scope = store.chat.scopeKey {
                    Text(scope)
                        .font(.caption.monospaced())
                        .foregroundStyle(FleetChatPalette.muted)
                        .accessibilityIdentifier("fleet.chat.scope")
                }
                Spacer(minLength: 0)
                Button("Refresh") { store.refreshChat() }
                    .disabled(!store.canReadChat)
                    .accessibilityIdentifier("fleet.chat.refresh")
                // Explicit, not Esc-only: the composer holds focus, and a surface
                // whose only exit is a key the focused control might eat is a trap.
                Button("Close", action: close)
                    .accessibilityIdentifier("fleet.chat.close")
            }
            engineDial
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 12)
    }

    /// The engine, guardrail and model the copilot is running under.
    ///
    /// Its own row under the title rather than more controls beside Refresh and
    /// Close: these three describe the AGENT, while those two act on the pane,
    /// and a swap here can replace the session the conversation is talking to.
    ///
    /// Every label reads what the store was TOLD, which is why the untouched
    /// state of this row is three "not reported" values rather than a plausible
    /// default. The daemon serves no read for the running adapter, so anything
    /// else on this row before an operator has set one would be a guess.
    @ViewBuilder private var engineDial: some View {
        HStack(spacing: 8) {
            Menu("Engine: \(FleetChatLabels.copilotEngine(store.copilotDial))") {
                if store.copilotDial.adapters.isEmpty {
                    Text(store.copilotDial.adaptersListed
                         ? "No adapters in this daemon's registry"
                         : "Reading the adapter registry")
                }
                ForEach(store.copilotDial.adapters) { adapter in
                    Button(adapterLabel(adapter)) {
                        store.configureCopilot(provider: adapter.name)
                    }
                }
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .disabled(!store.canConfigureCopilot || store.copilotDial.adapters.isEmpty)
            .accessibilityIdentifier("fleet.chat.dial.engine")

            // Both of these need an engine, and that is not a UI convenience.
            // `provider` is required on every configure, so moving the mode
            // with no engine known would mean naming one, and naming the wrong
            // one does not fail, it SWAPS to it and retires the session.
            Menu("Mode: \(FleetChatLabels.copilotMode(store.copilotDial))") {
                ForEach([FleetCopilotMode.help, .guarded, .yolo], id: \.self) { mode in
                    Button(mode.rawValue) {
                        guard let engine = store.copilotDial.engine else { return }
                        store.configureCopilot(provider: engine, mode: mode)
                    }
                }
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .disabled(!store.canConfigureCopilot || store.copilotDial.engine == nil)
            .help(store.copilotDial.engine == nil
                  ? "Pick an engine first: this daemon does not report the one in force"
                  : "Move the copilot's guardrail dial")
            .accessibilityIdentifier("fleet.chat.dial.mode")

            Menu("Model: \(FleetChatLabels.copilotModel(store.copilotDial))") {
                if store.copilotDial.models.isEmpty {
                    Text("This adapter declares no models, so it runs its own default")
                }
                // Identified by POSITION, not by the string. These are operator
                // -authored ids from `[acp.adapters.*].models`, so nothing stops
                // the same one appearing twice, and two rows sharing an identity
                // is undefined behaviour in SwiftUI rather than a cosmetic
                // repeat. The engine picker above can key on the adapter name
                // because that name is a registry KEY daemon-side and unique by
                // construction; this list is a plain sequence and is not.
                ForEach(Array(store.copilotDial.models.enumerated()), id: \.offset) { _, model in
                    Button(model) {
                        guard let engine = store.copilotDial.engine else { return }
                        store.configureCopilot(provider: engine, model: model)
                    }
                }
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .disabled(!store.canConfigureCopilot
                      || store.copilotDial.engine == nil
                      || store.copilotDial.models.isEmpty)
            .accessibilityIdentifier("fleet.chat.dial.model")

            if let effort = store.copilotDial.reasoningEffort, !effort.isEmpty {
                Text("Effort: \(effort)")
                    .font(.caption)
                    .foregroundStyle(FleetChatPalette.muted)
                    .accessibilityIdentifier("fleet.chat.dial.effort")
            }

            Button("Retry") { store.refreshAdapters() }
                .font(.caption)
                .disabled(!store.canReadAdapters)
                .accessibilityIdentifier("fleet.chat.dial.retry")

            Spacer(minLength: 0)
        }
        .font(.caption)
        // The last thing this row did, said out loud. A swap that replaced the
        // session is the one an operator most needs to see, because the
        // conversation below is now answered by a different process.
        if let detail = store.copilotDial.detail {
            Text(detail)
                .font(.caption2)
                .foregroundStyle(FleetChatPalette.muted)
                .accessibilityIdentifier("fleet.chat.dial.detail")
        }
    }

    /// An adapter's picker row: its name, and where it came from.
    ///
    /// The origin is shown because a name in `[acp.adapters]` and one from the
    /// built-in floor behave identically here but are fixed in different
    /// places when they are wrong.
    private func adapterLabel(_ adapter: FleetAdapter) -> String {
        adapter.builtIn ? adapter.name : "\(adapter.name) (config)"
    }

    private var unavailable: some View {
        VStack(spacing: 8) {
            Text("This daemon does not serve Fleet chat.")
                .font(.headline)
            Text("The copilot conversation needs fleet.chat.read and fleet.message.read.")
                .font(.callout)
                .foregroundStyle(FleetChatPalette.muted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityIdentifier("fleet.chat.unavailable")
    }

    private var conversation: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 8) {
                        if store.chat.messages.isEmpty {
                            Text("No messages yet. Ask the copilot below.")
                                .font(.callout)
                                .foregroundStyle(FleetChatPalette.muted)
                                .padding(.top, 24)
                                .accessibilityIdentifier("fleet.chat.timeline.empty")
                        }
                        ForEach(store.chat.messages) { row in
                            FleetChatMessageView(row: row).id(row.id)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(16)
                }
                .onChange(of: store.chat.messages.last?.id) { _, newest in
                    guard let newest else { return }
                    proxy.scrollTo(newest, anchor: .bottom)
                }
            }
            Divider().overlay(FleetChatPalette.separator)
            composerBar
        }
    }

    private var composerBar: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let detail = store.chat.sessionDetail {
                // The daemon's own refusal wording, verbatim. Paraphrasing it
                // costs the operator the only sentence that names the fix.
                Text(detail)
                    .font(.caption)
                    .foregroundStyle(FleetChatPalette.amber)
                    .accessibilityIdentifier("fleet.chat.session-detail")
            }
            if let notice = store.controlNotice {
                Text(notice)
                    .font(.caption)
                    .foregroundStyle(FleetChatPalette.muted)
                    .accessibilityIdentifier("fleet.chat.notice")
            }
            HStack(spacing: 8) {
                TextField("Message the copilot", text: $composer, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...4)
                    .accessibilityIdentifier("fleet.chat.composer")
                    .onSubmit(send)
                Button("Send", action: send)
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(!canSend)
                    .accessibilityIdentifier("fleet.chat.send")
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var sidebar: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                confirmSection
                activitySection
                transcriptSection
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
        }
        .background(FleetChatPalette.sidebar)
    }

    private var confirmSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Confirm cards")
                .font(.caption.weight(.bold))
                .foregroundStyle(FleetChatPalette.muted)
            // An empty feed and an UNREADABLE feed are different facts. Saying
            // "none open" when the daemon refused the call tells the operator
            // there is nothing to approve while cards pile up unseen.
            if let detail = store.chat.confirmsDetail {
                Text("Cards unavailable: \(detail)")
                    .font(.caption)
                    .foregroundStyle(FleetChatPalette.amber)
                    .accessibilityIdentifier("fleet.chat.confirms.detail")
            } else if store.chat.confirms.isEmpty {
                Text("None open")
                    .font(.callout)
                    .foregroundStyle(FleetChatPalette.muted)
                    .accessibilityIdentifier("fleet.chat.confirms.empty")
            }
            ForEach(store.chat.confirms) { card in
                confirmRow(card)
            }
        }
    }

    private func confirmRow(_ card: FleetChatConfirmCard) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Text(card.stateLabel)
                    .font(.caption2.weight(.bold))
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(card.isAnswerable ? FleetChatPalette.amber.opacity(0.22) : FleetChatPalette.control, in: Capsule())
                Text(card.tool)
                    .font(.callout.weight(.semibold))
                Spacer(minLength: 0)
            }
            if !card.argumentsLine.isEmpty {
                Text(card.argumentsLine)
                    .font(.caption.monospaced())
                    .foregroundStyle(FleetChatPalette.muted)
                    .lineLimit(3)
            }
            if card.isAnswerable {
                HStack(spacing: 6) {
                    Button("Approve") { store.answerConfirm(card, answer: .approve) }
                        .accessibilityIdentifier("fleet.chat.confirm.approve")
                    Button("Deny") { store.answerConfirm(card, answer: .deny) }
                        .accessibilityIdentifier("fleet.chat.confirm.deny")
                    Button("Edit") {
                        editedArguments = card.argumentsLine
                        editingCard = card
                    }
                    .accessibilityIdentifier("fleet.chat.confirm.edit")
                }
                .controlSize(.small)
                .disabled(!store.canAnswerConfirms)
            } else {
                // No control at all, not a disabled one: a greyed-out Approve
                // next to a state this build cannot name still reads as "there
                // is an approve for this", and `isAnswerable` said there is not.
                Text(card.refusal)
                    .font(.caption)
                    .foregroundStyle(FleetChatPalette.muted)
                    .accessibilityIdentifier("fleet.chat.confirm.not-answerable")
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(FleetChatPalette.control, in: RoundedRectangle(cornerRadius: 8))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("fleet.chat.confirm.\(card.id)")
    }

    @ViewBuilder private var activitySection: some View {
        if !store.chat.activity.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text("Copilot activity")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(FleetChatPalette.muted)
                ForEach(store.chat.activity, id: \.seq) { row in
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(FleetChatLabels.activityClass(row.activityClass))
                            .font(.caption2.weight(.bold))
                            .foregroundStyle(
                                FleetChatLabels.activityClassIsLoud(row.activityClass)
                                    ? FleetChatPalette.coral
                                    : FleetChatPalette.muted
                            )
                        Text(row.tool).font(.caption)
                        Text(FleetChatLabels.activityOutcome(row.outcome))
                            .font(.caption)
                            .foregroundStyle(FleetChatPalette.muted)
                        Spacer(minLength: 0)
                    }
                    .accessibilityElement(children: .combine)
                    .accessibilityIdentifier("fleet.chat.activity.\(row.seq)")
                }
            }
        }
    }

    /// The target session's ACP execution stream.
    ///
    /// LAST in the sidebar on purpose: the confirm cards above it are the only
    /// thing here an operator has to act on, and a transcript that pushed them
    /// below the fold would bury an approval under an agent's thinking.
    ///
    /// Gated on `canReadTranscript`, which is its own capability rather than
    /// the pane's, so a daemon that serves the conversation but not the
    /// transcript renders one and explains the other. Every empty state names
    /// WHY it is empty: an idle session and an unreadable one are different
    /// facts, and a section that showed silence for both would tell the
    /// operator the agent has done nothing when the truth is that this build
    /// cannot see what it did.
    @ViewBuilder private var transcriptSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Execution transcript")
                .font(.caption.weight(.bold))
                .foregroundStyle(FleetChatPalette.muted)
            if let detail = store.chat.transcriptDetail {
                Text("Transcript unavailable: \(detail)")
                    .font(.caption)
                    .foregroundStyle(FleetChatPalette.amber)
                    .accessibilityIdentifier("fleet.chat.transcript.detail")
            } else if store.chat.transcriptState.rows.isEmpty {
                Text("Nothing yet. The session has not produced a turn.")
                    .font(.callout)
                    .foregroundStyle(FleetChatPalette.muted)
                    .accessibilityIdentifier("fleet.chat.transcript.empty")
            }
            // The seam the daemon reported, above the rows it describes. A
            // transcript that silently starts mid-run reads as a whole one,
            // which is the failure the flag exists to prevent.
            if store.chat.transcriptState.truncated, !store.chat.transcriptState.rows.isEmpty {
                transcriptRow(fleetTranscriptTruncationRow)
            }
            ForEach(store.chat.transcriptState.rows) { row in
                transcriptRow(row)
            }
        }
    }

    private func transcriptRow(_ row: FleetTranscriptRow) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(FleetTranscriptLabels.lane(row.lane))
                .font(.caption2.weight(.bold))
                .foregroundStyle(laneColor(row.lane))
            Text(row.body)
                .font(.caption.monospaced())
                // Bounded here as well as in the classifier: the cap upstream
                // is a leak backstop at eight thousand characters, which is
                // still far more than a sidebar row can show.
                .lineLimit(4)
                .textSelection(.enabled)
            Spacer(minLength: 0)
        }
        .accessibilityElement(children: .combine)
        // The lane is SPOKEN, not left to the colour. A VoiceOver user must be
        // able to tell an error line from a tool result, and colour is the one
        // signal that does not survive.
        .accessibilityLabel("\(FleetTranscriptLabels.accessibilityLane(row.lane)): \(row.body)")
        .accessibilityIdentifier("fleet.chat.transcript.\(row.id)")
    }

    /// Wildcard-free, so a sixth lane is a compile error here rather than a row
    /// painted in whichever colour the last arm happened to name.
    private func laneColor(_ lane: FleetTranscriptLane) -> Color {
        switch lane {
        case .agent: FleetChatPalette.mint
        case .thinking: FleetChatPalette.violet
        case .toolCall: FleetChatPalette.amber
        case .toolResult: FleetChatPalette.muted
        case .error: FleetChatPalette.coral
        }
    }

    private func confirmEditor(_ card: FleetChatConfirmCard) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Edit arguments for \(card.tool)")
                .font(.headline)
            TextEditor(text: $editedArguments)
                .font(.callout.monospaced())
                .frame(minWidth: 420, minHeight: 160)
                .accessibilityIdentifier("fleet.chat.confirm.edit.arguments")
            HStack {
                Spacer()
                Button("Cancel") { editingCard = nil }
                Button("Answer") {
                    submitEdit(card)
                }
                .keyboardShortcut(.defaultAction)
                .disabled(editedJSON == nil)
                .accessibilityIdentifier("fleet.chat.confirm.edit.submit")
            }
            if editedJSON == nil {
                // Refused HERE rather than on the wire: the daemon would answer
                // with a parse error the operator has to translate back into
                // "your JSON was wrong", and the edited text would be gone.
                Text("Arguments must be valid JSON.")
                    .font(.caption)
                    .foregroundStyle(FleetChatPalette.coral)
            }
        }
        .padding(18)
    }

    private var editedJSON: JSONValue? {
        try? FleetWire.decoder().decode(JSONValue.self, from: Data(editedArguments.utf8))
    }

    private func submitEdit(_ card: FleetChatConfirmCard) {
        guard let arguments = editedJSON else { return }
        store.answerConfirm(card, answer: .edit(arguments: arguments))
        editingCard = nil
    }

    private var canSend: Bool {
        store.canSendChat
            && store.chat.targetSessionKey != nil
            && !composer.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func send() {
        guard canSend else { return }
        store.sendChatMessage(composer)
        composer = ""
    }
}

/// One attributed timeline row.
private struct FleetChatMessageView: View {
    let row: FleetChatMessageRow

    var body: some View {
        HStack {
            if row.actor.isOperator { Spacer(minLength: 60) }
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(row.actor.label)
                        .font(.caption2.weight(.bold))
                        .foregroundStyle(attributionColor)
                        .accessibilityIdentifier(row.actor.identifier)
                    Text(FleetChatLabels.messageKind(row.kind))
                        .font(.caption2)
                        .foregroundStyle(FleetChatPalette.muted)
                    if row.isReply {
                        Text("reply")
                            .font(.caption2)
                            .foregroundStyle(FleetChatPalette.muted)
                    }
                }
                Text(row.body)
                    .font(.callout)
                    .textSelection(.enabled)
            }
            .padding(10)
            .background(bubble, in: RoundedRectangle(cornerRadius: 10))
            .overlay(
                RoundedRectangle(cornerRadius: 10)
                    .strokeBorder(attributionColor.opacity(0.5), lineWidth: 1)
            )
            if !row.actor.isOperator { Spacer(minLength: 60) }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(row.actor.accessibilityLabel): \(row.body)")
        // Addressable by the id the daemon minted, the way a confirm card is.
        // A journey that has just filed a message knows that id and can assert
        // on THE row rather than on a body string that could appear anywhere.
        .accessibilityIdentifier("fleet.chat.message.\(row.id)")
    }

    private var attributionColor: Color {
        switch row.actor {
        case .operatorHuman: FleetChatPalette.amber
        case .copilot: FleetChatPalette.violet
        case .session: FleetChatPalette.mint
        case .unattributed: FleetChatPalette.coral
        }
    }

    private var bubble: Color {
        row.actor.isOperator ? FleetChatPalette.selected : FleetChatPalette.control
    }
}

/// The chat module's own palette. Owned here, borrowed from nowhere.
///
/// These are the Fleet surface colours, declared rather than imported, plus the
/// chat-only accent (`violet`, the copilot's colour in the terminal client
/// too). Copies of a palette are usually a smell, and this one is deliberate:
/// the values also appear in the roster's file-private palette, and a shared
/// one would have to live somewhere both can see, which is a module this app
/// does not have. Owning them keeps this pane compiling no matter what happens
/// to any other view.
private enum FleetChatPalette {
    static let canvas = Color(red: 0.055, green: 0.071, blue: 0.086)
    static let sidebar = Color(red: 0.043, green: 0.057, blue: 0.071)
    static let control = Color(red: 0.091, green: 0.118, blue: 0.141)
    static let selected = Color(red: 0.075, green: 0.172, blue: 0.255)
    static let separator = Color.white.opacity(0.08)
    static let muted = Color(red: 0.57, green: 0.64, blue: 0.71)
    static let mint = Color(red: 0.37, green: 0.88, blue: 0.76)
    static let amber = Color(red: 0.96, green: 0.72, blue: 0.23)
    static let coral = Color(red: 0.95, green: 0.43, blue: 0.49)
    static let violet = Color(red: 0.71, green: 0.60, blue: 0.98)
}
