import Combine
import Foundation

enum FleetOperatorAction: CaseIterable, Identifiable, Equatable {
    case sendPrompt, continueTurn, retry, interrupt, restart, stop, kill, archive

    var id: String { title }

    var title: String {
        switch self {
        case .sendPrompt: "Send prompt"
        case .continueTurn: "Continue"
        case .retry: "Retry"
        case .interrupt: "Interrupt"
        case .restart: "Restart"
        case .stop: "Stop"
        case .kill: "Kill"
        case .archive: "Archive"
        }
    }

    var isDestructive: Bool {
        switch self {
        case .interrupt, .stop, .kill, .archive: true
        default: false
        }
    }

    func isAvailable(in capabilities: FleetCapabilities) -> Bool {
        switch self {
        case .sendPrompt: capabilities.sendPrompt || capabilities.tmuxText
        case .continueTurn: capabilities.continueTurn
        case .retry: capabilities.retry
        case .interrupt: capabilities.interrupt
        case .restart: capabilities.restart
        case .stop: capabilities.stop
        case .kill: capabilities.kill
        case .archive: capabilities.archive
        }
    }

    func wireAction(prompt: String = "") -> ControlAction {
        switch self {
        case .sendPrompt: .sendPrompt(text: prompt)
        case .continueTurn: .continue
        case .retry: .retry
        case .interrupt: .interrupt
        case .restart: .restart
        case .stop: .stop
        case .kill: .kill
        case .archive: .archive
        }
    }
}

enum FleetStartPreflight {
    static func supports(_ provider: FleetProvider) -> Bool {
        provider == .codex
    }

    static func isExistingDirectory(_ path: String) -> Bool {
        var isDirectory: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory) && isDirectory.boolValue
    }
}

enum FleetApprovalDecision: Equatable {
    case allowOnce, deny, bypassSession

    var title: String {
        switch self {
        case .allowOnce: "Allow once"
        case .deny: "Deny"
        case .bypassSession: "Always allow this session"
        }
    }

    func action(for session: FleetSession) -> ControlAction {
        let fingerprint = session.currentRequestFingerprint ?? ""
        let identity = FleetRequestIdentity.from(request: session.currentRequest)
        return switch self {
        case .allowOnce: .approve(requestFingerprint: fingerprint, requestIdentity: identity)
        case .deny: .deny(requestFingerprint: fingerprint, requestIdentity: identity)
        case .bypassSession: .approveForSession(requestFingerprint: fingerprint, requestIdentity: identity)
        }
    }
}

@MainActor
final class FleetStore: ObservableObject {
    @Published private(set) var sessions: [FleetSession] = []
    @Published private(set) var connectionState: FleetConnectionState = .connecting
    @Published var selectedSessionKey: String?
    @Published private(set) var receipts: [FleetActionReceipt] = []
    @Published private(set) var atcInstances: [AtcInstance] = []
    @Published private(set) var atcSchedulerOwnership: AtcSchedulerOwnership?
    @Published private(set) var timeline: [FleetTimelineEntry] = []
    @Published private(set) var usageSummary: FleetUsageSummaryResult?
    @Published private(set) var usageDashboard: FleetUsageDashboardResult?
    @Published private(set) var quotaSummary: FleetQuotaSummaryResult?
    @Published private(set) var runtimeStatus: FleetRuntimeStatusResult?
    @Published private(set) var chat = FleetChatSurface()
    @Published private(set) var pendingIntentID: String?
    @Published private(set) var controlNotice: String?
    @Published private(set) var lastStart: FleetStartResult?

    private let location: HangarLocation
    // Internal (not private) so the contract tests can assert the ranges the
    // SHIPPED store actually declares, not the FleetConnection defaults alone.
    let readVersions: FleetProtocolRange
    let writeVersions: FleetProtocolRange
    private let makeConnection: (HangarLocation) -> FleetConnection
    private let reconnectDelayNanoseconds: (Int) -> UInt64
    private let notificationCenter = FleetNotificationCenter()
    private var connection: FleetConnection?
    private var connectionTask: Task<Void, Never>?
    private var reconnectTask: Task<Void, Never>?
    private var authoritativeRepairTask: Task<Void, Never>?
    private var connectionGeneration: UInt = 0
    private var projection = FleetProjection.empty
    private var negotiation: FleetNegotiateResult?
    private var lastAuthoritativeRefresh: Date?
    private var reconnectAttempts = 0
    private var hasEstablishedLiveConnection = false
    private var liveConnectionStartedAt: Date?
    private var hasAppliedAuthoritativeSnapshot = false
    /// The ACP session each chat scope was minted against, so the mint happens
    /// ONCE per scope instead of once per poll.
    ///
    /// `fleet/acp_session_create` is idempotent, but idempotent is not free: on
    /// the daemon side every call is an INSERT inside a write transaction that
    /// hits the live-scope unique index and then reads the incumbent back. At
    /// the one-second chat poll that was 86,400 write transactions a day
    /// against a database whose readers answer in 0.1s while this call was
    /// timing out at 5s under lock contention. Nothing about a standing session
    /// changes between polls, so nothing needs to be asked.
    ///
    /// Cleared whenever the answer could have gone stale: a new connection
    /// (`beginConnection`, the daemon may have restarted and torn the session
    /// down) and a send whose leg came back REJECTED (the session is gone,
    /// re-mint on the next page).
    ///
    /// ponytail: per connection, not per session lifetime. A session evicted by
    /// the pool while this app sits idle is only noticed at the next send, and
    /// that send is the one that reports REJECTED. The live chat stream does
    /// NOT close that gap, contrary to what this note said when it was written:
    /// `fleet/message_event` carries a committed message and nothing about its
    /// delivery legs, so a torn-down session is still invisible until something
    /// is addressed to it.
    private var copilotSessionKeyByScope: [String: String] = [:]
    /// Bumped by every invalidation, so a page that was already in flight when
    /// one happened cannot put the forgotten key back.
    ///
    /// A page is four round trips long, and the poll loop and the send path
    /// each run in their own Task on this actor. Without this, a poll that
    /// started before a send reported REJECTED would finish after the
    /// invalidation and write its now-dead key back over the empty slot, so the
    /// operator's next message went to the same dead session. That is the exact
    /// failure the invalidation exists to prevent, arriving one poll later.
    private var copilotCacheGeneration: UInt = 0
    /// Live chat events that landed while a page was in flight.
    ///
    /// A page is four round trips and it REPLACES the surface wholesale, which
    /// is the invariant that stops a half-applied refresh showing this page's
    /// cards next to the last one's timeline. That same wholesale replacement
    /// is what would drop a message committed after `fleet/message_list` read
    /// the log and before the page finished: folded into the live surface, then
    /// overwritten by a page that never saw it, and gone until the safety net
    /// pages again half a minute later.
    ///
    /// So the fold does both: it applies the event to what is on screen now,
    /// and, if a page is running, remembers it so that page can replay it onto
    /// its own result. Every fold is an upsert by id, so replaying an event the
    /// page already contains changes nothing.
    private var chatEventsDuringPage: [FleetChatEvent] = []
    /// How many pages are running. A count, not a flag: the poll loop and the
    /// send path both page, and they overlap. Buffering only while this is
    /// above zero is what keeps the buffer bounded by one page's duration
    /// rather than by how long the pane stays open.
    private var chatPagesInFlight = 0
    /// How many chat panes are on screen.
    ///
    /// The fold is gated on this, and the gate is not an optimisation. `chat`
    /// is `@Published` on the store the WHOLE notch observes, so every folded
    /// event re-evaluates the roster, the chips and the menu-bar summary. A
    /// scope filter alone does not stop that: once a pane has been opened once,
    /// `chat.scopeKey` stays set for the life of the connection, so a busy
    /// copilot would invalidate the roster several times a second while the
    /// operator is looking at Sessions and no chat surface exists at all.
    ///
    /// A count rather than a flag, because SwiftUI can have the outgoing and
    /// incoming instances of a view alive at once during a transition, and a
    /// flag cleared by the outgoing one would silence the incoming one.
    private var chatPanesOpen = 0
    private let maximumReconnectAttempts = 3
    private let reconnectResetInterval: TimeInterval = 30

    init(
        location: HangarLocation = HangarLocation(),
        readVersions: FleetProtocolRange = FleetProtocolRange(min: 1, max: 2),
        writeVersions: FleetProtocolRange = FleetProtocolRange(min: 1, max: 2),
        makeConnection: @escaping (HangarLocation) -> FleetConnection = { FleetConnection(location: $0) },
        reconnectDelayNanoseconds: @escaping (Int) -> UInt64 = { UInt64($0) * 500_000_000 }
    ) {
        self.location = location
        self.readVersions = readVersions
        self.writeVersions = writeVersions
        self.makeConnection = makeConnection
        self.reconnectDelayNanoseconds = reconnectDelayNanoseconds
    }

    deinit {
        connectionTask?.cancel()
        reconnectTask?.cancel()
        authoritativeRepairTask?.cancel()
    }

    var activeCount: Int {
        sessions.filter { $0.lifecycle == .starting || $0.lifecycle == .running }.count
    }

    var needsYouCount: Int { sessions.filter { $0.attention != .none }.count }

    var canWrite: Bool {
        guard case let .live(_, writeCompatible) = connectionState else { return false }
        return writeCompatible && negotiation?.readCompatible == true
    }

    var canReadReceipts: Bool {
        connectionState.isLive && negotiation?.capabilityIDs.contains("fleet.receipt.read") == true
    }

    var canReadATC: Bool {
        connectionState.isLive && negotiation?.capabilityIDs.contains("fleet.atc.read") == true
    }

    var canReadTimeline: Bool {
        connectionState.isLive && negotiation?.capabilityIDs.contains("fleet.timeline.read") == true
    }

    var canReadUsage: Bool {
        connectionState.isLive && negotiation?.capabilityIDs.contains("fleet.usage.read") == true
    }

    var canReadDashboard: Bool {
        connectionState.isLive && negotiation?.capabilityIDs.contains("fleet.dashboard.read") == true
    }

    var canReadQuota: Bool {
        connectionState.isLive && negotiation?.capabilityIDs.contains("fleet.quota.read") == true
    }

    var canReadRuntime: Bool {
        connectionState.isLive && negotiation?.capabilityIDs.contains("fleet.runtime.read") == true
    }

    // MARK: - Fleet chat (buzz-port part 2, Phase A3)
    //
    // Each surface is gated on the capability its own daemon arm checks, and
    // gated SEPARATELY: a daemon built between phases serves the timeline while
    // answering -32601 for confirm cards, and a single combined gate would
    // either hide a working conversation or offer an approve button that
    // errors on the click.

    var canReadChat: Bool {
        guard connectionState.isLive, let capabilities = negotiation?.capabilityIDs else { return false }
        return capabilities.contains("fleet.chat.read") && capabilities.contains("fleet.message.read")
    }

    var canSendChat: Bool {
        canWrite && negotiation?.capabilityIDs.contains("fleet.message.send") == true && pendingIntentID == nil
    }

    var canAnswerConfirms: Bool {
        canWrite && negotiation?.capabilityIDs.contains("fleet.confirm.answer") == true && pendingIntentID == nil
    }

    var canStart: Bool {
        canWrite && negotiation?.capabilityIDs.contains("fleet.start.execute") == true && pendingIntentID == nil
    }

    var canBroadcast: Bool {
        canWrite && negotiation?.capabilityIDs.contains("fleet.broadcast.execute") == true && pendingIntentID == nil
    }

    #if DEBUG
    var debugConnectionTaskCount: Int {
        [connectionTask, reconnectTask].compactMap { $0 }.count
    }
    #endif

    var needsResubscribe: Bool { projection.needsResubscribe }

    func start() {
        guard connectionTask == nil, reconnectTask == nil else { return }
        connectionState = .connecting
        startAuthoritativeRepair()
        beginConnection()
    }

    func retry() {
        reconnectAttempts = 0
        hasEstablishedLiveConnection = false
        liveConnectionStartedAt = nil
        reconnectTask?.cancel()
        reconnectTask = nil
        connectionState = .connecting
        beginConnection()
    }

    func refresh() {
        guard let connection else { return }
        Task { [weak self] in
            guard let self else { return }
            await self.refreshAuthoritativeState(using: connection)
        }
    }

    func stop() {
        connectionGeneration &+= 1
        hasEstablishedLiveConnection = false
        liveConnectionStartedAt = nil
        connectionTask?.cancel()
        connectionTask = nil
        reconnectTask?.cancel()
        reconnectTask = nil
        authoritativeRepairTask?.cancel()
        authoritativeRepairTask = nil
        let currentConnection = connection
        connection = nil
        negotiation = nil
        Task { await currentConnection?.close() }
    }

    private func startAuthoritativeRepair() {
        guard authoritativeRepairTask == nil else { return }
        authoritativeRepairTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(10))
                guard let self, let connection = self.connection else { continue }
                await self.refreshAuthoritativeState(using: connection)
            }
        }
    }

    func canPerform(_ action: FleetOperatorAction, on session: FleetSession) -> Bool {
        canWrite
            && pendingIntentID == nil
            && negotiation?.capabilityIDs.contains("fleet.action.execute") == true
            && selectedSessionKey == session.sessionKey
            && !session.sessionKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && session.version > 0
            && action.isAvailable(in: session.capabilities)
    }

    func canSendPrompt(_ prompt: String, on session: FleetSession) -> Bool {
        canPerform(.sendPrompt, on: session)
            && !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func perform(_ action: FleetOperatorAction, on session: FleetSession, prompt: String = "") {
        let trimmedPrompt = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        let allowed = action == .sendPrompt
            ? canSendPrompt(trimmedPrompt, on: session)
            : canPerform(action, on: session)
        guard allowed else {
            controlNotice = "Action is unavailable or incomplete."
            return
        }
        guard selectedSessionKey == session.sessionKey,
              let current = sessions.first(where: { $0.sessionKey == session.sessionKey }),
              current.version == session.version else {
            controlNotice = "Fleet state changed. Review the current session before sending."
            return
        }
        guard let connection else {
            controlNotice = "Fleet connection is unavailable."
            return
        }
        let requestID = UUID().uuidString
        pendingIntentID = requestID
        controlNotice = nil
        Task { [weak self] in
            guard let self else { return }
            defer { self.pendingIntentID = nil }
            do {
                let result = try await connection.action(FleetActionParams(
                    sessionKey: current.sessionKey,
                    expectedVersion: current.version,
                    requestID: requestID,
                    action: action.wireAction(prompt: trimmedPrompt)
                ))
                self.record(result.receipt)
                self.controlNotice = Self.controlNotice(for: result.receipt, failurePrefix: "Action")
                await self.refreshAuthoritativeState(using: connection)
            } catch {
                self.controlNotice = "Action refused: \(String(describing: error))"
            }
        }
    }

    func submitStructuredAnswers(_ answers: [FleetQuestionAnswer], on session: FleetSession) {
        guard !answers.isEmpty else {
            controlNotice = "Complete every interview question before submit."
            return
        }
        performStructured(
            .structuredAnswer(
                requestFingerprint: session.currentRequestFingerprint ?? "",
                requestIdentity: FleetRequestIdentity.from(request: session.currentRequest),
                answers: answers
            ),
            on: session,
            allowed: session.capabilities.structuredAnswer,
            failurePrefix: "Interview"
        )
    }

    func dismissStructuredInterview(on session: FleetSession) {
        performStructured(
            .dismissStructured(
                requestFingerprint: session.currentRequestFingerprint ?? "",
                requestIdentity: FleetRequestIdentity.from(request: session.currentRequest)
            ),
            on: session,
            allowed: session.provider == .claude && session.capabilities.structuredDismiss,
            failurePrefix: "Interview rejection"
        )
    }

    func openStructuredInterviewInClaude(on session: FleetSession) {
        performStructured(
            .releaseStructured(requestFingerprint: session.currentRequestFingerprint ?? ""),
            on: session,
            allowed: session.provider == .claude && session.capabilities.structuredAnswer,
            failurePrefix: "Open Claude picker"
        )
    }

    func canDecideApproval(_ decision: FleetApprovalDecision, on session: FleetSession) -> Bool {
        guard session.attention == .approval,
              session.capabilities.approvals,
              !session.currentRequestFingerprint.orEmpty.isEmpty else { return false }
        if decision == .bypassSession {
            return session.provider == .codex && session.capabilities.approvalSession && canPerformRequest(on: session)
        }
        return canPerformRequest(on: session)
    }

    func decideApproval(_ decision: FleetApprovalDecision, on session: FleetSession) {
        guard canDecideApproval(decision, on: session) else {
            controlNotice = "Approval action is unavailable or stale."
            return
        }
        performStructured(
            decision.action(for: session),
            on: session,
            allowed: true,
            failurePrefix: "Approval"
        )
    }

    private func canPerformRequest(on session: FleetSession) -> Bool {
        canWrite
            && pendingIntentID == nil
            && negotiation?.capabilityIDs.contains("fleet.action.execute") == true
            && selectedSessionKey == session.sessionKey
            && !session.sessionKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && session.version > 0
    }

    private func performStructured(
        _ action: ControlAction,
        on session: FleetSession,
        allowed: Bool,
        failurePrefix: String
    ) {
        guard canPerformRequest(on: session),
              allowed,
              let fingerprint = session.currentRequestFingerprint,
              !fingerprint.isEmpty,
              let connection else {
            controlNotice = "Interview action is unavailable or stale."
            return
        }
        guard let current = sessions.first(where: { $0.sessionKey == session.sessionKey }),
              current.version == session.version,
              current.currentRequestFingerprint == fingerprint else {
            controlNotice = "Fleet interview changed. Review current question before sending."
            return
        }
        let requestID = UUID().uuidString
        pendingIntentID = requestID
        controlNotice = nil
        Task { [weak self] in
            guard let self else { return }
            defer { self.pendingIntentID = nil }
            do {
                let result = try await connection.action(FleetActionParams(
                    sessionKey: current.sessionKey,
                    expectedVersion: current.version,
                    requestID: requestID,
                    action: action
                ))
                self.record(result.receipt)
                self.controlNotice = Self.controlNotice(for: result.receipt, failurePrefix: failurePrefix)
                await self.refreshAuthoritativeState(using: connection)
            } catch {
                self.controlNotice = "\(failurePrefix) refused: \(String(describing: error))"
            }
        }
    }

    func start(provider: FleetProvider, cwd: String, prompt: String?) {
        let trimmedCWD = cwd.trimmingCharacters(in: .whitespacesAndNewlines)
        guard canStart,
              FleetStartPreflight.supports(provider),
              !trimmedCWD.isEmpty,
              FleetStartPreflight.isExistingDirectory(trimmedCWD),
              let connection else {
            controlNotice = "Start is unavailable or incomplete."
            return
        }
        let requestID = UUID().uuidString
        pendingIntentID = requestID
        controlNotice = nil
        Task { [weak self] in
            guard let self else { return }
            defer { self.pendingIntentID = nil }
            do {
                let result = try await connection.start(FleetStartParams(
                    requestID: requestID,
                    provider: provider,
                    cwd: trimmedCWD,
                    prompt: prompt?.trimmingCharacters(in: .whitespacesAndNewlines)
                ))
                self.lastStart = result
                self.record(result.receipt)
                self.controlNotice = Self.controlNotice(for: result.receipt, failurePrefix: "Start")
                await self.refreshAuthoritativeState(using: connection)
            } catch {
                self.controlNotice = "Start refused: \(String(describing: error))"
            }
        }
    }

    func broadcast(targetKeys: [String], text: String) {
        var seen = Set<String>()
        let orderedTargets = targetKeys.filter { seen.insert($0).inserted }
        let trimmedText = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard canBroadcast,
              !orderedTargets.isEmpty,
              !trimmedText.isEmpty,
              let connection,
              orderedTargets.allSatisfy({ key in
                  sessions.contains { $0.sessionKey == key && $0.version > 0 && ($0.capabilities.sendPrompt || $0.capabilities.tmuxText) }
              }) else {
            controlNotice = "Broadcast is unavailable or incomplete."
            return
        }
        let intentID = UUID().uuidString
        pendingIntentID = intentID
        controlNotice = nil
        Task { [weak self] in
            guard let self else { return }
            defer { self.pendingIntentID = nil }
            do {
                let result = try await connection.broadcast(FleetBroadcastParams(
                    targetKeys: orderedTargets,
                    text: trimmedText,
                    idempotencyKey: intentID
                ))
                self.record(result.receipts)
                self.controlNotice = Self.broadcastNotice(for: result.receipts)
                await self.refreshAuthoritativeState(using: connection)
            } catch {
                self.controlNotice = "Broadcast refused: \(String(describing: error))"
            }
        }
    }

    func refreshReceipts() {
        guard canReadReceipts, let connection else {
            controlNotice = "Receipt reads are unavailable for this daemon."
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                self.receipts = try await connection.receiptList(FleetReceiptListParams(limit: 50)).receipts
            } catch {
                self.controlNotice = "Receipt refresh refused: \(String(describing: error))"
            }
        }
    }

    func refreshATC() {
        guard canReadATC, let connection else {
            controlNotice = "ATC reads are unavailable for this daemon."
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                let result = try await connection.atcList()
                atcInstances = result.instances
                atcSchedulerOwnership = result.schedulerOwnership
            } catch {
                controlNotice = "ATC refresh refused: \(String(describing: error))"
            }
        }
    }

    func refreshTimeline() {
        guard canReadTimeline, let connection else {
            controlNotice = "Timeline reads are unavailable for this daemon."
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                timeline = try await connection.timeline(FleetTimelineParams(afterRevision: nil, sessionKey: nil, limit: 100)).entries
            } catch {
                controlNotice = "Timeline refresh refused: \(String(describing: error))"
            }
        }
    }

    /// Usage is requested only from the Usage route or its explicit refresh
    /// control. It can scan local provider histories, so connection bootstrap
    /// must remain fast and side-effect free.
    func refreshUsage(period: FleetUsagePeriod = .trailing7Days) {
        guard canReadUsage, let connection else {
            usageSummary = nil
            controlNotice = "Usage is unavailable for this daemon."
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                usageSummary = try await connection.usageSummary(FleetUsageSummaryParams(period: period))
            } catch {
                usageSummary = nil
                controlNotice = "Usage refresh refused: \(String(describing: error))"
            }
        }
    }

    func refreshDashboard() {
        guard canReadDashboard, let connection else {
            usageDashboard = nil
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                usageDashboard = try await connection.usageDashboard()
            } catch {
                usageDashboard = nil
                controlNotice = "Dashboard refresh refused: \(String(describing: error))"
            }
        }
    }

    func refreshQuota() {
        guard canReadQuota, let connection else {
            quotaSummary = nil
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                quotaSummary = try await connection.quotaSummary()
            } catch {
                quotaSummary = nil
                controlNotice = "Quota refresh refused: \(String(describing: error))"
            }
        }
    }

    func refreshRuntime() {
        guard canReadRuntime, let connection else {
            runtimeStatus = nil
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                runtimeStatus = try await connection.runtimeStatus()
            } catch {
                runtimeStatus = nil
                controlNotice = "Runtime status refused: \(String(describing: error))"
            }
        }
    }

    // MARK: - Fleet chat

    /// Page the copilot conversation: timeline, confirm cards, activity.
    ///
    /// Mirrors the TUI's resolution (`ainb-core/src/fleet/control.rs`) step for
    /// step so the two clients cannot disagree about which channel `#copilot`
    /// is. Anything else and an operator watching both surfaces sees two
    /// conversations and has no way to tell which one the copilot is in.
    func refreshChat() {
        Task { [weak self] in await self?.refreshChatOnce() }
    }

    /// One page, awaited.
    ///
    /// The safety-net loop awaits THIS rather than firing `refreshChat()` on a
    /// timer: a page that takes longer than the interval would otherwise stack,
    /// and five overlapping RPC sets racing each other means whichever finishes
    /// last wins and the pane can go backwards in time.
    ///
    /// This is the BOOTSTRAP for a pane that has just opened and the repair for
    /// one whose stream dropped a frame. The conversation itself arrives on
    /// `fleet/message_subscribe` between these calls.
    func refreshChatOnce() async {
        guard canReadChat, let connection else {
            // Assigned only on a real change: a @Published write redraws the
            // pane whether or not the value moved, and this shares `chat` with
            // a live event stream that can write it several times a second.
            let unavailable = FleetChatSurface(sessionDetail: "Chat is unavailable for this daemon.")
            if chat != unavailable { chat = unavailable }
            return
        }
        do {
            // Same guard as the unavailable branch above, for the same reason:
            // an unconditional @Published write re-evaluates the whole window
            // subtree even when nothing moved, and `FleetChatSurface` is
            // Equatable, so the comparison is free.
            // A nil page is a page whose read was invalidated while it ran.
            // Publishing it would put back state the store already knows is
            // dead, so it is dropped and the next page asks again.
            guard let paged = try await pagedChat(using: connection) else { return }
            if chat != paged { chat = paged }
        } catch {
            controlNotice = "Chat refresh refused: \(String(describing: error))"
        }
    }

    /// Post one operator message into the copilot channel.
    ///
    /// No `actor` rides this and none can: `FleetMessageSendParams` has no such
    /// field, so this surface cannot file a row under anybody but the operator
    /// the daemon already authenticated.
    func sendChatMessage(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard canSendChat,
              !trimmed.isEmpty,
              let scopeKey = chat.scopeKey,
              let target = chat.targetSessionKey,
              let connection else {
            controlNotice = "Chat send is unavailable or incomplete."
            return
        }
        let requestID = UUID().uuidString
        pendingIntentID = requestID
        controlNotice = nil
        Task { [weak self] in
            guard let self else { return }
            defer { self.pendingIntentID = nil }
            do {
                // The RESULT is the honest half. A 200 says the daemon accepted
                // the send, not that anybody received it: a lone leg coming back
                // REJECTED (`target_not_running`) or UNKNOWN
                // (`tmux_identity_unknown`) answers 200 too, and discarding it
                // would clear the composer, re-page, and leave the operator
                // reading their own message in the timeline as proof it landed.
                let result = try await connection.messageSend(FleetMessageSendParams(
                    scopeKey: scopeKey,
                    targets: [target],
                    originMessageID: nil,
                    text: trimmed,
                    requestID: requestID
                ))
                self.controlNotice = FleetChatLabels.deliverySummary(result.deliveries)
                // A leg refused because the session is GONE is the one signal
                // this surface gets that its remembered key is dead. Forgetting
                // it here is what makes the NEXT page mint a live one instead
                // of sending into the same dead key forever. Transient
                // refusals deliberately keep the mint.
                if Self.reportsSessionGone(target, in: result.deliveries) {
                    self.forgetCopilotSession(inScope: scopeKey)
                }
            } catch {
                self.controlNotice = "Chat send refused: \(String(describing: error))"
                return
            }
            self.publish(try? await self.pagedChat(using: connection))
        }
    }

    /// Answer one guardrail confirm card.
    ///
    /// The card's own `isAnswerable` is the gate, not a state comparison
    /// rewritten here. A card whose state this build cannot name reports false
    /// and is refused, which is the same answer the button already gave by
    /// being disabled: this is the second lock, for the paths a disabled
    /// control does not cover (a keyboard shortcut, a card that expired between
    /// the render and the click).
    func answerConfirm(_ card: FleetChatConfirmCard, answer: FleetConfirmAnswer) {
        guard canAnswerConfirms, card.isAnswerable, let connection else {
            controlNotice = card.isAnswerable
                ? "Confirm answers are unavailable for this daemon."
                : card.refusal
            return
        }
        let intentID = UUID().uuidString
        pendingIntentID = intentID
        controlNotice = nil
        Task { [weak self] in
            guard let self else { return }
            defer { self.pendingIntentID = nil }
            do {
                let result = try await connection.confirmAnswer(
                    FleetConfirmAnswerParams(confirmID: card.id, answer: answer)
                )
                self.controlNotice = "Card \(result.confirmID) is \(FleetChatLabels.confirmState(result.state))."
            } catch {
                self.controlNotice = "Confirm answer refused: \(String(describing: error))"
            }
            self.publish(try? await self.pagedChat(using: connection))
        }
    }

    /// Resolve the copilot channel and page everything filed under its scope.
    ///
    /// Only the timeline is fatal. The confirm and activity feeds degrade to an
    /// explained absence: a daemon built between phases answers -32601 for
    /// them, and a chat that refuses to render its conversation over that is a
    /// worse surface than one that says which half is missing.
    ///
    /// `mintedSessionKeyByScope` is what keeps the mint OFF the poll: a scope
    /// already minted against this connection skips `fleet/acp_session_create`
    /// entirely. See `copilotSessionKeyByScope` for why that matters.
    ///
    /// `minted` says whether the target IS that session, so only a real mint is
    /// cached. The fallbacks are not: a channel's first recipient is a guess
    /// this page makes when the mint was REFUSED, and caching it would make the
    /// refusal permanent for the life of the connection.
    private static func pageChat(
        using connection: FleetConnection,
        canWrite: Bool,
        mintedSessionKeyByScope: [String: String]
    ) async throws -> (surface: FleetChatSurface, minted: Bool) {
        var surface = FleetChatSurface()
        var minted = false
        let channels = try await connection.channelList().channels
        // Newest-wins, matching the TUI: a race that created two copilot
        // channels must not leave the two clients reading different ones.
        let existing = channels.last { $0.kind == .copilot }
        let channel: FleetChannel
        if let existing {
            channel = existing
        } else if canWrite {
            // Create-if-absent on a read path, deliberately: the copilot
            // channel is a singleton an operator expects to simply exist and
            // there is no other door to it here.
            channel = try await connection.channelCreate(
                FleetChannelCreateParams(kind: .copilot, name: "copilot", recipients: nil)
            ).channel
        } else {
            surface.sessionDetail = "No copilot channel yet, and this connection may not create one."
            return (surface, minted)
        }
        surface.scopeKey = channel.scopeKey

        // A COPILOT channel carries no recipient list: its membership is the
        // ACP session that ANSWERS on the scope, so the recipient is resolved
        // the way it is minted, by creating that session against this scope.
        // The call is idempotent per live scope. The daemon's refusal is KEPT
        // rather than swallowed: its wording is the only actionable thing an
        // operator gets when a client in another directory already claimed it.
        if let cached = mintedSessionKeyByScope[channel.scopeKey] {
            surface.targetSessionKey = cached
            minted = true
        } else if canWrite {
            do {
                surface.targetSessionKey = try await Self.mintCopilotSession(
                    scopeKey: channel.scopeKey,
                    home: FileManager.default.homeDirectoryForCurrentUser.path,
                    create: { try await connection.acpSessionCreate($0) }
                ).sessionKey
                minted = true
            } catch {
                surface.targetSessionKey = channel.recipients.first
                surface.sessionDetail = String(describing: error)
            }
        } else {
            surface.targetSessionKey = channel.recipients.first
            surface.sessionDetail = "This connection may not open a copilot session."
        }

        surface.messages = try await connection.messageList(FleetMessageListParams(
            scopeKey: channel.scopeKey,
            originID: nil,
            afterID: nil,
            limit: fleetMessageListMax
        )).messages.map(FleetChatMessageRow.init(message:))

        do {
            surface.confirms = try await connection
                .confirmList(FleetConfirmListParams(scopeKey: channel.scopeKey))
                .confirms
                .map(FleetChatConfirmCard.decode)
        } catch {
            surface.confirms = []
            surface.confirmsDetail = String(describing: error)
        }
        surface.activity = (try? await connection.activityList(FleetActivityListParams(
            scopeKey: channel.scopeKey,
            afterSeq: nil,
            limit: fleetActivityListMax
        )).activities) ?? []
        return (surface, minted)
    }

    /// The delivery details that mean the remembered session is GONE.
    ///
    /// The daemon's own tokens: `session_gone` from the ACP pool, plus
    /// `target_unknown` (no such row) and `target_not_running` (the row exists
    /// but its session exited) from the delivery leg. Every other REJECTED
    /// token describes a session that is still there. `breaker_open` and
    /// `queue_full` are transient back-pressure, and `task_scope_refused` is a
    /// scope this surface never addresses, so forgetting the mint on any of
    /// them would spend the write transaction this whole cache exists to
    /// remove, on a session that would have answered the next prompt.
    private static let sessionGoneDetails: Set<String> = [
        "session_gone", "target_unknown", "target_not_running",
    ]

    /// Whether the daemon says the leg addressed to `target` failed because
    /// that session no longer exists.
    ///
    /// REJECTED only, not every non-DELIVERED state: a PENDING leg is an ACP
    /// turn that has not finished yet, which is the normal answer to a copilot
    /// prompt.
    ///
    /// An absent or unrecognised detail is NOT a reason to forget. A daemon
    /// that grows a new rejection token this build has never heard of would
    /// otherwise re-mint on every send, which is the failure mode the cache was
    /// added to remove, arriving silently through a wire change.
    static func reportsSessionGone(_ target: String, in deliveries: [FleetMessageDelivery]) -> Bool {
        deliveries.contains { delivery in
            guard delivery.sessionKey == target, delivery.state == .rejected,
                  let detail = delivery.detail else { return false }
            return sessionGoneDetails.contains(
                detail.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            )
        }
    }

    /// Forget the session remembered for `scope`, so the next page mints a live
    /// one, and invalidate any page already in flight.
    ///
    /// Both halves matter. Removing the key alone loses the race against a page
    /// that read the cache before this call and writes back after it.
    func forgetCopilotSession(inScope scope: String) {
        copilotCacheGeneration &+= 1
        copilotSessionKeyByScope.removeValue(forKey: scope)
    }

    /// One page, with the copilot mint remembered for the next one.
    ///
    /// Every caller of `pageChat` goes through here so the cache cannot be
    /// updated by one path and not another.
    ///
    /// The generation is captured BEFORE the page and checked after: four round
    /// trips is long enough for a send to report a dead session and forget it,
    /// and a page that finished afterwards must not restore what it read at the
    /// start. It still renders its own result; only the remembering is dropped,
    /// so the next page asks the daemon again.
    /// Returns nil when the page is STALE, meaning the cache was invalidated
    /// while it was reading.
    ///
    /// Dropping the write-back is not enough, and that was the gap: the page
    /// still returned a surface, and every caller published it. A send whose
    /// leg came back `target_not_running` forgets the dead session, re-pages,
    /// mints a live one and publishes it; an older page that read the dead key
    /// then lands and puts it back on screen, and the composer aims at a
    /// session nobody is listening on. At a one-second poll that healed itself
    /// in a second. At `fleetChatSafetyNetInterval` it lasts half a minute,
    /// which is the whole of an operator's next message.
    ///
    /// So the generation is a TICKET, not just a guard on the cache: a page
    /// that cannot prove it read current state does not get to render.
    private func pagedChat(using connection: FleetConnection) async throws -> FleetChatSurface? {
        let generation = copilotCacheGeneration
        beginChatPage()
        defer { endChatPage() }
        let (paged, minted) = try await Self.pageChat(
            using: connection,
            canWrite: canWrite,
            mintedSessionKeyByScope: copilotSessionKeyByScope
        )
        guard copilotCacheGeneration == generation else { return nil }
        if minted,
           let scope = paged.scopeKey,
           let sessionKey = paged.targetSessionKey {
            copilotSessionKeyByScope[scope] = sessionKey
        }
        // Everything that arrived live while this page was reading, replayed
        // onto it. Without this the page silently rewinds the pane past any
        // message committed after `fleet/message_list` answered.
        var surface = paged
        for event in chatEventsDuringPage {
            surface.apply(event)
        }
        return surface
    }

    /// Open the live chat stream, RESUMING from the newest row already shown.
    ///
    /// A bare subscribe starts at the daemon's head, so everything committed
    /// while this client was disconnected is never pushed. The page behind it
    /// used to be the backstop, and no longer is: a page reads one bounded
    /// window, so an outage longer than that window loses the middle for good.
    /// Naming the last row this surface holds asks the daemon to replay the gap.
    ///
    /// The retry is the honest half. A remembered id the daemon has never heard
    /// of, which is what a restarted or pruned daemon answers, is refused as
    /// invalid params, and a swallowed refusal there would leave the connection
    /// with NO stream at all: silently poll-only, looking identical to working.
    /// So the refusal costs the resume, not the subscription.
    private func openChatStream(on connection: FleetConnection) async {
        guard let resume = chat.messages.last?.id else {
            _ = try? await connection.messageSubscribe()
            return
        }
        do {
            _ = try await connection.messageSubscribe(afterID: resume)
        } catch FleetConnectionError.rpc(let refusal) where refusal.code == -32602 {
            _ = try? await connection.messageSubscribe()
        } catch {
            // Anything else is the connection itself failing, and the bootstrap
            // that follows will report it.
        }
    }

    /// The chat pane appeared. Live events are folded from here.
    func chatPaneAppeared() {
        chatPanesOpen += 1
    }

    /// The chat pane went away. Floored at zero so a stray unbalanced call
    /// cannot drive the count negative and silence the fold permanently.
    func chatPaneDisappeared() {
        chatPanesOpen = max(0, chatPanesOpen - 1)
    }

    /// Show a page, unless it has nothing to show.
    ///
    /// Two different nils arrive here and both mean "keep what is on screen":
    /// a page that threw, and a page the store disowned because its read went
    /// stale mid-flight. Neither is a reason to blank a pane the operator is
    /// reading.
    private func publish(_ surface: FleetChatSurface??) {
        guard let surface = surface ?? nil else { return }
        if chat != surface { chat = surface }
    }

    /// Start counting live events against a page that is about to run.
    ///
    /// The buffer is cleared by the FIRST page only: a second page starting
    /// while one is still running must not throw away what the first one still
    /// has to replay.
    private func beginChatPage() {
        if chatPagesInFlight == 0 {
            chatEventsDuringPage.removeAll()
        }
        chatPagesInFlight += 1
    }

    private func endChatPage() {
        chatPagesInFlight -= 1
    }

    /// Get-or-create the copilot session for `scopeKey`, naming as little as
    /// the daemon in front of us will accept.
    ///
    /// Three rungs, cheapest first, each one adding back a field only because
    /// the daemon said it needs it:
    ///
    /// 1. neither `provider` nor `cwd`. What this client actually wants: THE
    ///    session on this scope, whatever engine it runs and wherever it was
    ///    opened. A menu-bar app cannot know either, and guessing was fatal:
    ///    naming `$HOME` against a scope held by a session opened from a
    ///    worktree is refused `ScopeHeld` on every poll, which left the
    ///    composer with nobody to send to.
    /// 2. `cwd` named as `home`. Either a daemon built before the field became
    ///    optional, or a current one saying the scope has no live session to
    ///    take a root from. Both are answered by naming one, and for a fresh
    ///    copilot channel the operator's home is the honest root.
    /// 3. `provider` named too, the legacy adapter. A daemon older still, in
    ///    the ordinary upgrade-the-app-keep-the-daemon window. Exactly what
    ///    this call sent before either field became optional, so it is no worse
    ///    than it was against a daemon that cannot do better.
    ///
    /// Any other refusal propagates untouched, including a held scope: its
    /// wording names the directory that holds it, and retrying would replace
    /// the only actionable thing the operator gets with the same refusal twice.
    static func mintCopilotSession(
        scopeKey: String,
        home: String,
        create: (FleetAcpSessionCreateParams) async throws -> FleetAcpSessionCreateResult
    ) async throws -> FleetAcpSessionCreateResult {
        var refusal: Error
        do {
            return try await create(
                FleetAcpSessionCreateParams(provider: nil, cwd: nil, scopeKey: scopeKey)
            )
        } catch {
            refusal = error
        }
        if daemonRefusalNames(refusal, field: "cwd") {
            do {
                return try await create(
                    FleetAcpSessionCreateParams(provider: nil, cwd: home, scopeKey: scopeKey)
                )
            } catch {
                refusal = error
            }
        }
        if daemonRefusalNames(refusal, field: "provider") {
            return try await create(FleetAcpSessionCreateParams(
                provider: copilotDefaultProvider,
                cwd: home,
                scopeKey: scopeKey
            ))
        }
        throw refusal
    }

    /// Whether a daemon refusal is that daemon asking for `field` to be NAMED.
    ///
    /// The Swift half of the TUI's `names_missing_field`, anchored to the field
    /// name rather than looking for two loose substrings, and for the same
    /// reason: the daemon formats every parse refusal as
    /// `expected {shape}: {serde error}`, and the SHAPE HINT names every field.
    /// So a legacy daemon refusing an absent provider says
    /// `expected { provider, cwd, scope_key? }: missing field \`provider\``,
    /// which contains both "cwd" and "missing". A matcher looking for those
    /// separately fires the cwd rung on a provider refusal and spends it on a
    /// request the daemon never made.
    ///
    /// Exactly two shapes count:
    ///
    /// - ``missing field `<field>` ``, serde's own, backticks included. Without
    ///   them the shape hint alone would satisfy the match.
    /// - `<field> is required`, this daemon's own refusal.
    ///
    /// Only an RPC refusal counts at all, so a dead socket never spends a rung,
    /// and a refusal that names the field for a REAL reason (an unknown
    /// adapter, a scope held at another root) matches neither shape and
    /// propagates with its wording intact.
    private static func daemonRefusalNames(_ error: Error, field: String) -> Bool {
        guard case let FleetConnectionError.rpc(refusal) = error else { return false }
        let message = refusal.message.lowercased()
        let field = field.lowercased()
        return message.contains("missing field `\(field)`") || message.contains("\(field) is required")
    }

    private func beginConnection() {
        connectionGeneration &+= 1
        // A new connection may be a new DAEMON: a restart tears every ACP
        // session down, so a key remembered from the last one addresses
        // nothing. Re-minting costs one call per scope; sending to a dead key
        // costs the operator their message. The generation bump also disowns a
        // page still in flight against the connection being replaced.
        copilotCacheGeneration &+= 1
        copilotSessionKeyByScope.removeAll()
        let generation = connectionGeneration
        let currentConnection = connection
        connection = nil
        connectionTask?.cancel()
        connectionTask = Task { [weak self] in
            await currentConnection?.close()
            guard !Task.isCancelled else { return }
            await self?.connectAndConsume(generation: generation)
        }
    }

    private func connectAndConsume(generation: UInt) async {
        defer {
            if connectionGeneration == generation {
                connectionTask = nil
            }
        }
        let newConnection = makeConnection(location)
        connection = newConnection
        var established = false
        do {
            try await newConnection.connect()
            try await newConnection.authenticate(token: location.readToken())
            // Declaring the write range is load-bearing: omitting it left the
            // connection's default in charge, and a stale default silently
            // fails every action at requireWriteCapability after a bump.
            let result = try await newConnection.negotiate(readVersions: readVersions, writeVersions: writeVersions)
            negotiation = result
            let stream = await newConnection.incoming()
            let subscription = try await newConnection.subscribe(afterRevision: projection.committedRevision)
            let bootstrapped = FleetProjectionReducer.bootstrap(subscription)
            guard !bootstrapped.needsResubscribe else {
                projection = bootstrapped
                await reconnectOrBecomeUnavailable(
                    reason: "Fleet subscription requires resubscription",
                    connection: newConnection,
                    generation: generation
                )
                return
            }
            apply(bootstrapped)
            // The live chat stream, on this same socket.
            //
            // Opened with the CONNECTION, not with the pane: the daemon runs
            // one message forwarder per socket, so a subscription opened per
            // appearance of the sheet would be a second stream writing the same
            // surface, and closing the sheet would have to decide which one to
            // keep. It costs nothing while no pane is open, because the fold
            // is gated on one being on screen.
            //
            // Opened BEFORE any page, so the window between the two is covered
            // from the page's side: the daemon starts the forwarder at the head
            // it just acked, and the page that follows reads everything up to
            // it. The other order would leave messages committed in between
            // visible to neither.
            //
            // A refusal is not fatal and is not reported. A daemon built before
            // this method answers -32601, and the honest consequence is that
            // the pane is carried by `fleetChatSafetyNetInterval` alone: later
            // than live, but never wrong, and the same surface either way.
            if result.capabilityIDs.contains("fleet.message.read") {
                await openChatStream(on: newConnection)
            }
            if result.capabilityIDs.contains("fleet.receipt.read") {
                do {
                    receipts = try await newConnection.receiptList(FleetReceiptListParams(limit: 50)).receipts
                } catch {}
            } else {
                receipts = []
            }
            if result.capabilityIDs.contains("fleet.atc.read") {
                do {
                    let atc = try await newConnection.atcList()
                    atcInstances = atc.instances
                    atcSchedulerOwnership = atc.schedulerOwnership
                } catch {}
            } else {
                atcInstances = []
                atcSchedulerOwnership = nil
            }
            if result.capabilityIDs.contains("fleet.timeline.read") {
                do {
                    timeline = try await newConnection.timeline(FleetTimelineParams(afterRevision: nil, sessionKey: nil, limit: 100)).entries
                } catch {}
            } else {
                timeline = []
            }
            if result.capabilityIDs.contains("fleet.runtime.read") {
                runtimeStatus = try? await newConnection.runtimeStatus()
            } else {
                runtimeStatus = nil
            }
            connectionState = .live(daemonVersion: result.daemonVersion, writeCompatible: result.writeCompatible)
            established = true
            hasEstablishedLiveConnection = true
            liveConnectionStartedAt = Date()

            for await incoming in stream {
                if Task.isCancelled || connectionGeneration != generation { return }
                if try await handle(incoming, on: newConnection, generation: generation) == false { return }
            }
            if !Task.isCancelled, connectionGeneration == generation {
                await reconnectOrBecomeUnavailable(
                    reason: "Fleet daemon connection closed",
                    connection: newConnection,
                    generation: generation
                )
            }
        } catch let error as FleetConnectionError {
            if case .protocolReadIncompatible = error {
                handle(error)
                await close(newConnection, ifCurrentGeneration: generation)
            } else if established || hasEstablishedLiveConnection {
                await reconnectOrBecomeUnavailable(
                    reason: error.localizedDescription,
                    connection: newConnection,
                    generation: generation
                )
            } else {
                handle(error)
                await close(newConnection, ifCurrentGeneration: generation)
            }
        } catch is CancellationError {
        } catch {
            if established || hasEstablishedLiveConnection {
                await reconnectOrBecomeUnavailable(
                    reason: error.localizedDescription,
                    connection: newConnection,
                    generation: generation
                )
            } else {
                handle(error)
                await close(newConnection, ifCurrentGeneration: generation)
            }
        }
    }

    private func handle(_ incoming: FleetIncoming, on connection: FleetConnection, generation: UInt) async throws -> Bool {
        switch incoming {
        case let .event(event):
            let next = FleetProjectionReducer.live(event, from: projection)
            if next.needsResubscribe {
                await reconnectOrBecomeUnavailable(
                    reason: "Fleet stream needs resubscription",
                    connection: connection,
                    generation: generation
                )
                return false
            }
            apply(next)
            if next.needsSnapshot {
                apply(FleetProjectionReducer.snapshot(try await connection.snapshot(), from: next))
            }
        case .resyncRequired:
            projection = FleetProjectionReducer.resyncRequired(from: projection)
            await reconnectOrBecomeUnavailable(
                reason: "Fleet daemon requested resync",
                connection: connection,
                generation: generation
            )
            return false
        case let .messageEvent(params):
            fold(.message(params.message))
        case let .confirmEvent(params):
            // Decoded the way the PAGE decodes a card, tolerantly, so a row
            // this build cannot fully read renders as unanswerable instead of
            // vanishing. Its scope is taken from the raw frame, and a card that
            // cannot even say which conversation it belongs to is dropped:
            // rendering it would put another scope's approval in front of this
            // operator.
            if let scope = params.confirm.value("scope_key")?.stringValue {
                fold(.confirm(card: FleetChatConfirmCard.decode(params.confirm), scopeKey: scope))
            }
        case let .activityEvent(params):
            fold(.activity(params.activity))
        case .unknownNotification:
            break
        }
        return true
    }

    /// Fold one live chat notification into the surface on screen.
    ///
    /// Two things, and both are needed. The surface is updated so the pane
    /// moves NOW, and the event is remembered if a page is running so that
    /// page cannot overwrite it with a read that predates it.
    ///
    /// The scope filter lives in `FleetChatSurface.apply`, where the scope key
    /// is: the daemon's chat stream is fleet-wide, and every one of the three
    /// event types carries the scope it was filed under.
    ///
    /// The write is guarded on a real change for the same reason the poll's is:
    /// an assignment to a `@Published` value redraws the pane whether or not
    /// anything moved, and a busy copilot emits activity rows faster than a
    /// human reads.
    private func fold(_ event: FleetChatEvent) {
        // Nothing is folded with no pane on screen. The page that runs when one
        // opens is what makes the surface current again, so the only thing lost
        // is liveness for a view nobody is looking at.
        guard chatPanesOpen > 0 else { return }
        if chatPagesInFlight > 0, chat.scopeKey == nil || event.scopeKey == chat.scopeKey {
            // Buffered by SCOPE at the door, not at replay. The stream is
            // fleet-wide, so an unfiltered buffer collects every conversation's
            // traffic, and a page whose RPC never answers holds the in-flight
            // count above zero for as long as that call hangs.
            //
            // The nil case is not a hole in that filter, it is the FIRST page.
            // Until one publishes there is no scope to compare against, and a
            // filter that answered "no match" there would drop exactly what the
            // buffer exists for: a message committed while the opening page was
            // still reading its confirm and activity feeds. Replay applies the
            // paged surface's own scope, so nothing foreign gets rendered, and
            // the window lasts one page with the cap still in force.
            chatEventsDuringPage.append(event)
            if chatEventsDuringPage.count > Int(fleetMessageListMax) {
                chatEventsDuringPage.removeFirst()
            }
        }
        var next = chat
        next.apply(event)
        if next != chat { chat = next }
    }

    private func apply(_ next: FleetProjection) {
        let previousSessions = sessions
        projection = next
        sessions = next.snapshot?.sessions ?? sessions
        if next.snapshot != nil {
            lastAuthoritativeRefresh = Date()
            if hasAppliedAuthoritativeSnapshot {
                let events = FleetNotificationPolicy.events(previous: previousSessions, current: sessions)
                Task { await notificationCenter.deliver(events) }
            }
            hasAppliedAuthoritativeSnapshot = true
        }
        if let selectedSessionKey, !sessions.contains(where: { $0.sessionKey == selectedSessionKey }) {
            self.selectedSessionKey = nil
        }
    }

    private func becomeStale(reason: String) {
        if sessions.isEmpty {
            connectionState = .unavailable(message: reason)
        } else {
            connectionState = .stale(lastUpdated: lastAuthoritativeRefresh ?? Date(), reason: reason)
        }
        negotiation = nil
    }

    private func reconnectOrBecomeUnavailable(
        reason: String,
        connection: FleetConnection,
        generation: UInt
    ) async {
        guard connectionGeneration == generation else { return }
        if let liveConnectionStartedAt,
           Date().timeIntervalSince(liveConnectionStartedAt) >= reconnectResetInterval {
            reconnectAttempts = 0
        }
        liveConnectionStartedAt = nil
        becomeStale(reason: reason)
        await close(connection, ifCurrentGeneration: generation)
        guard connectionGeneration == generation,
              reconnectAttempts < maximumReconnectAttempts,
              !Task.isCancelled else { return }
        reconnectAttempts += 1
        let delay = reconnectDelayNanoseconds(reconnectAttempts)
        reconnectTask?.cancel()
        reconnectTask = Task { @MainActor [weak self] in
            do {
                try await Task.sleep(nanoseconds: delay)
            } catch {
                return
            }
            guard let self,
                  !Task.isCancelled,
                  self.connectionGeneration == generation else { return }
            self.reconnectTask = nil
            self.connectionState = .connecting
            self.beginConnection()
        }
    }

    private func close(_ connection: FleetConnection, ifCurrentGeneration generation: UInt) async {
        await connection.close()
        guard connectionGeneration == generation, self.connection === connection else { return }
        self.connection = nil
    }

    private func handle(_ error: FleetConnectionError) {
        switch error {
        case let .protocolReadIncompatible(result):
            negotiation = result
            connectionState = .readIncompatible(daemonVersion: result.daemonVersion, protocolVersion: result.protocolVersion)
        default:
            becomeStale(reason: error.localizedDescription)
        }
    }

    private func handle(_ error: Error) {
        becomeStale(reason: error.localizedDescription)
    }

    /// Operator-facing notice for a receipt the daemon actually returned.
    ///
    /// A `FLEET_ACTION` that round-trips is NOT the same as one that landed: the
    /// daemon answers a rejected or undeliverable action with a perfectly
    /// successful RPC carrying a non-`delivered` status and a `detail` saying
    /// why. Reporting "Delivered." for every non-throwing call is how a picker
    /// that refused the answer, sent zero keys and left the target session
    /// sitting on its question still read as success on this surface — the
    /// failure was real, loud in the receipt, and invisible here.
    ///
    /// `detail` is surfaced verbatim because it is the only place the reason
    /// exists; without it the operator sees "failed" and has to go read the
    /// database to learn what happened.
    /// The one success wording, named so the refresh path can recognise it as
    /// safe to overwrite.
    nonisolated static let deliveredNotice = "Delivered. Confirming Fleet state."

    /// Operator-facing notice for a fan-out, where "some landed" is the case
    /// that matters and a bare success string would hide it.
    nonisolated static func broadcastNotice(for receipts: [FleetActionReceipt]) -> String {
        guard !receipts.isEmpty else { return "Broadcast reached no sessions." }
        let failed = receipts.filter { $0.status != .delivered }
        if failed.isEmpty {
            return "Delivered to \(receipts.count) session\(receipts.count == 1 ? "" : "s")."
        }
        let reason = failed.compactMap { $0.detail?.trimmingCharacters(in: .whitespacesAndNewlines) }
            .first { !$0.isEmpty }
        let head = "Broadcast: \(receipts.count - failed.count)/\(receipts.count) delivered, \(failed.count) failed"
        return reason.map { "\(head): \($0)" } ?? "\(head)."
    }

    nonisolated static func controlNotice(for receipt: FleetActionReceipt, failurePrefix: String) -> String {
        switch receipt.status {
        case .delivered:
            return deliveredNotice
        case .pending:
            return "\(failurePrefix) accepted, awaiting delivery."
        case .failed, .rejected, .unknown:
            let reason = receipt.detail?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            let outcome = receipt.status.operatorToken
            return reason.isEmpty
                ? "\(failurePrefix) \(outcome)."
                : "\(failurePrefix) \(outcome): \(reason)"
        }
    }

    /// Surface a message from a caller that is not itself a store action.
    ///
    /// `controlNotice` stays `private(set)` so the store keeps ownership of what
    /// the operator is told; this is the one narrow door, used by side actions
    /// like "Open session" whose failure must not be silent.
    func reportControlNotice(_ message: String) {
        controlNotice = message
    }

    private func record(_ receipt: FleetActionReceipt) {
        record([receipt])
    }

    private func record(_ incoming: [FleetActionReceipt]) {
        receipts = Self.mergedReceipts(incoming, existing: receipts)
    }

    static func mergedReceipts(_ incoming: [FleetActionReceipt], existing: [FleetActionReceipt]) -> [FleetActionReceipt] {
        let incomingIDs = Set(incoming.map(\.requestID))
        return incoming + existing.filter { !incomingIDs.contains($0.requestID) }
    }

    private func refreshAuthoritativeState(using connection: FleetConnection) async {
        do {
            let snapshot = try await connection.snapshot()
            apply(FleetProjectionReducer.snapshot(snapshot, from: projection))
        } catch {
            // A refresh complaint must NOT overwrite a delivery failure we just
            // set: the receipt's reason is the only record of WHY the action was
            // refused, and replacing it with a read error tells the operator the
            // action landed and only the follow-up snapshot failed — the exact
            // inversion this file was changed to stop.
            if controlNotice == nil || controlNotice == Self.deliveredNotice {
                controlNotice = "Fleet refresh refused: \(String(describing: error))"
            }
        }
        guard canReadReceipts else { return }
        do {
            receipts = try await connection.receiptList(FleetReceiptListParams(limit: 50)).receipts
        } catch {
            controlNotice = "Receipt refresh refused: \(String(describing: error))"
        }
    }
}

private extension Optional where Wrapped == String {
    var orEmpty: String { self ?? "" }
}
