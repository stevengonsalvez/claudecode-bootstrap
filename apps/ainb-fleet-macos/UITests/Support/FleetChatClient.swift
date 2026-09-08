import Darwin
import Foundation

/// A second client on the fixture daemon's socket, for the journey that has to
/// prove the app hears about somebody ELSE's message.
///
/// This exists because the fixture daemon's stdin protocol only injects hook
/// observations (`seed`) and shutdown; there is no command that files a chat
/// message. The push journey needs a writer that is not the app, so it opens
/// its own connection, exactly as the copilot, the terminal UI or a second
/// window would.
///
/// Deliberately NOT the app's `FleetConnection`. The UI test bundle runs out of
/// process and links none of the app's sources, and reaching for them would
/// couple a journey to the implementation it is meant to observe from outside.
/// It is a blocking client on the calling thread, which is what a test wants:
/// every call is request, wait, response, in order, on one socket.
final class FleetChatClient {
    private let descriptor: Int32
    private var buffer = Data()
    private var nextRequestID = 1

    /// Connect, authenticate with the daemon's own token file, and negotiate.
    ///
    /// The token is read from disk rather than passed in, so the client cannot
    /// disagree with the daemon about which home it is talking to.
    init(home: URL) throws {
        let socketPath = home.appendingPathComponent("hangar.sock").path
        descriptor = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw ClientError.socket(errno) }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let path = Array(socketPath.utf8)
        guard path.count + 1 <= MemoryLayout.size(ofValue: address.sun_path) else {
            Darwin.close(descriptor)
            throw ClientError.socket(ENAMETOOLONG)
        }
        withUnsafeMutableBytes(of: &address.sun_path) { destination in
            destination.initializeMemory(as: UInt8.self, repeating: 0)
            destination.copyBytes(from: path)
        }
        let length = socklen_t(MemoryLayout<sa_family_t>.size + path.count + 1)
        let connected = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(descriptor, $0, length)
            }
        }
        guard connected == 0 else {
            Darwin.close(descriptor)
            throw ClientError.socket(errno)
        }

        let token = try String(contentsOf: home.appendingPathComponent("hangar/daemon.token"), encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        _ = try call("auth/hello", ["token": token])
        _ = try call("fleet/negotiate", [
            "client_name": "fleet-ui-journey",
            "client_version": "0.1.0",
            "read_versions": ["min": 1, "max": 2],
            "write_versions": ["min": 1, "max": 2],
        ])
    }

    deinit { Darwin.close(descriptor) }

    /// File one message into the copilot conversation the APP already opened.
    ///
    /// It resolves rather than creates: the channel is the newest copilot one
    /// on the daemon, which is the one the app's own page created or found, and
    /// the recipient is resolved by an idempotent create naming neither
    /// `provider` nor `cwd`, which the daemon answers with the session already
    /// held on that scope. Creating a second channel here would file the
    /// message in a conversation the app is not showing, and the journey would
    /// fail for a reason that has nothing to do with the push.
    @discardableResult
    func sendToCopilotChannel(text: String) throws -> String {
        let channels = try call("fleet/channel_list", [:])["channels"] as? [[String: Any]] ?? []
        guard let channel = channels.last(where: { $0["kind"] as? String == "copilot" }),
              let scope = channel["scope_key"] as? String else {
            throw ClientError.noCopilotChannel
        }
        let session = try call("fleet/acp_session_create", ["scope_key": scope])
        guard let target = session["session_key"] as? String else {
            throw ClientError.noCopilotSession
        }
        let sent = try call("fleet/message_send", [
            "scope_key": scope,
            "targets": [target],
            "text": text,
            "request_id": UUID().uuidString,
        ])
        guard let id = sent["message_id"] as? String else {
            throw ClientError.noMessageID
        }
        return id
    }

    private func call(_ method: String, _ params: [String: Any]) throws -> [String: Any] {
        let id = nextRequestID
        nextRequestID += 1
        let body = try JSONSerialization.data(withJSONObject: [
            "jsonrpc": "2.0", "id": id, "method": method, "params": params,
        ])
        var frame = Data("Content-Length: \(body.count)\r\n\r\n".utf8)
        frame.append(body)
        try write(frame)

        // This client holds no subscription, so every frame it reads is the
        // answer to the one request outstanding.
        let response = try readFrame()
        if let failure = response["error"] as? [String: Any] {
            throw ClientError.rpc(method, String(describing: failure))
        }
        return response["result"] as? [String: Any] ?? [:]
    }

    private func write(_ data: Data) throws {
        try data.withUnsafeBytes { bytes in
            guard let base = bytes.baseAddress else { return }
            var written = 0
            while written < bytes.count {
                let result = Darwin.write(descriptor, base.advanced(by: written), bytes.count - written)
                if result > 0 {
                    written += result
                } else if result < 0, errno == EINTR {
                    continue
                } else {
                    throw ClientError.socket(errno)
                }
            }
        }
    }

    private func readFrame() throws -> [String: Any] {
        var chunk = [UInt8](repeating: 0, count: 16 * 1024)
        let deadline = Date().addingTimeInterval(5)
        while Date() < deadline {
            if let frame = try takeFrame() {
                return try JSONSerialization.jsonObject(with: frame) as? [String: Any] ?? [:]
            }
            let count = Darwin.read(descriptor, &chunk, chunk.count)
            if count > 0 {
                buffer.append(contentsOf: chunk[0..<count])
                continue
            }
            if count == 0 { throw ClientError.disconnected }
            if errno != EINTR { throw ClientError.socket(errno) }
        }
        throw ClientError.timedOut
    }

    private func takeFrame() throws -> Data? {
        guard let header = buffer.range(of: Data("\r\n\r\n".utf8)),
              let text = String(data: buffer[..<header.lowerBound], encoding: .ascii),
              let lengthText = text.split(separator: ":").last,
              let length = Int(lengthText.trimmingCharacters(in: .whitespaces)) else { return nil }
        let end = header.upperBound + length
        guard buffer.count >= end else { return nil }
        let body = buffer.subdata(in: header.upperBound..<end)
        buffer.removeSubrange(..<end)
        return body
    }

    enum ClientError: LocalizedError {
        case socket(Int32)
        case disconnected
        case timedOut
        case rpc(String, String)
        case noCopilotChannel
        case noCopilotSession
        case noMessageID

        var errorDescription: String? {
            switch self {
            case let .socket(code): "fleet chat client socket failed: \(code)"
            case .disconnected: "the daemon closed the chat client's connection"
            case .timedOut: "the daemon did not answer the chat client"
            case let .rpc(method, failure): "\(method) refused: \(failure)"
            case .noCopilotChannel: "the daemon has no copilot channel; the app never opened one"
            case .noCopilotSession: "the copilot scope holds no session; the app never minted one"
            case .noMessageID: "the daemon accepted the send without naming a message"
            }
        }
    }
}
