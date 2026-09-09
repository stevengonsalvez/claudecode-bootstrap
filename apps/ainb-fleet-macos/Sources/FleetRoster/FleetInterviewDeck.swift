import Foundation

/// The structured interview a session is currently asking, parsed out of its
/// live request.
///
/// This is domain logic, not a view: it owns the payload shape the daemon
/// sends and the rule about which decks may be answered here. Every surface
/// that draws an interview reads it through this type.
struct FleetInterviewDeck {
    let session: FleetSession
    let questions: [FleetInterviewQuestion]
    let nativePicker: Bool
    let mirroredPicker: Bool

    /// Claude drew its own picker, so this deck can be READ here but not
    /// answered here.
    ///
    /// Both stamps mean the same thing operationally, the interview was never
    /// held, Claude owns that pane's stdin, and the daemon refuses answers for
    /// either. They are kept as separate fields because they still describe
    /// different origins, but every UI decision keys off this.
    var readOnlyPicker: Bool { nativePicker || mirroredPicker }

    init?(session: FleetSession) {
        guard session.attention == .ask,
              session.capabilities.structuredAnswer,
              session.currentRequestFingerprint != nil,
              let request = session.currentRequest else { return nil }
        let payload = request.value("payload") ?? request
        let input = payload.value("tool_input", "input") ?? payload
        let questions: [FleetInterviewQuestion] = input.value("questions")?.arrayValue?.enumerated().compactMap { index, value in
            guard let text = value.value("question", "text")?.stringValue else { return nil }
            let options = value.value("options")?.arrayValue?.compactMap { $0.stringValue ?? $0.value("label")?.stringValue } ?? []
            return FleetInterviewQuestion(
                id: value.value("id")?.stringValue ?? String(index),
                header: value.value("header")?.stringValue ?? "Question \(index + 1)",
                text: text,
                options: options,
                multiSelect: value.value("multiSelect", "multi_select")?.boolValue ?? false
            )
        } ?? []
        guard !questions.isEmpty else { return nil }
        self.session = session
        self.questions = questions
        self.nativePicker = request.value("fleet_delivery")?.stringValue == "native_claude"
        self.mirroredPicker = request.value("fleet_delivery")?.stringValue == "mirrored"
    }

    static func priority(_ session: FleetSession) -> (Int, Int64) {
        let attention = session.attention == .ask ? 0 : 1
        return (attention, -session.attentionUpdatedAt)
    }
}

struct FleetInterviewQuestion: Identifiable {
    let id: String
    let header: String
    let text: String
    let options: [String]
    let multiSelect: Bool
}
