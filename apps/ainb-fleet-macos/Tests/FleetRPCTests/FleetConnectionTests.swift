import Darwin
import Foundation
import XCTest
@testable import AINBFleet

final class FleetConnectionTests: XCTestCase {
    func testLocationHonorsNonEmptyAINBHangarHome() {
        let location = HangarLocation(
            environment: ["AINB_HANGAR_HOME": "/tmp/ainb-hangar"],
            homeDirectory: URL(fileURLWithPath: "/unused")
        )

        XCTAssertEqual(location.socketURL.path, "/tmp/ainb-hangar/hangar.sock")
        XCTAssertEqual(location.tokenURL.path, "/tmp/ainb-hangar/hangar/daemon.token")
    }

    func testLocationFallsBackToAgentsInABoxHome() {
        let location = HangarLocation(
            environment: ["AINB_HANGAR_HOME": ""],
            homeDirectory: URL(fileURLWithPath: "/tmp/test-home")
        )

        XCTAssertEqual(location.home.path, "/tmp/test-home/.agents-in-a-box")
    }

    func testTokenWhitespaceIsTrimmedOnce() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let tokenDirectory = directory.appendingPathComponent("hangar", isDirectory: true)
        try FileManager.default.createDirectory(at: tokenDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        try Data("  mdt_test\n".utf8).write(to: tokenDirectory.appendingPathComponent("daemon.token"))

        XCTAssertEqual(try HangarLocation(environment: ["AINB_HANGAR_HOME": directory.path]).readToken(), "mdt_test")
    }

    func testAuthMustCompleteBeforeNegotiate() async {
        let connection = FleetConnection(location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]))

        do {
            _ = try await connection.negotiate()
            XCTFail("expected authentication guard")
        } catch let error as FleetConnectionError {
            XCTAssertEqual(error, .notAuthenticated)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testConnectPreventsSIGPIPEOnOwnedSocket() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        defer { Darwin.close(descriptors[1]) }
        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: descriptors[0]
        )

        try await connection.connect()
        var enabled: Int32 = 0
        var length = socklen_t(MemoryLayout.size(ofValue: enabled))
        XCTAssertEqual(Darwin.getsockopt(descriptors[0], SOL_SOCKET, SO_NOSIGPIPE, &enabled, &length), 0)
        XCTAssertEqual(enabled, 1)
        await connection.close()
    }

    func testWriteMismatchIsBlocked() {
        XCTAssertThrowsError(try FleetConnection.validateWriteCompatibility(
            FleetNegotiateResult(
                daemonVersion: "test",
                protocolVersion: 1,
                readCompatible: true,
                writeCompatible: false,
                capabilityIDs: []
            )
        )) { XCTAssertEqual($0 as? FleetConnectionError, .protocolWriteIncompatible) }
    }

    func testReadMismatchCarriesDaemonAndProtocolVersions() {
        let result = FleetNegotiateResult(
            daemonVersion: "fixture-daemon",
            protocolVersion: 7,
            readCompatible: false,
            writeCompatible: false,
            capabilityIDs: []
        )
        XCTAssertEqual(
            FleetConnectionError.protocolReadIncompatible(result),
            .protocolReadIncompatible(result)
        )
    }

    func testOldCatalogueRefusesNewReadCapabilities() {
        let oldCatalogue = FleetNegotiateResult(
            daemonVersion: "fixture-daemon-0.9.0",
            protocolVersion: 1,
            readCompatible: true,
            writeCompatible: true,
            capabilityIDs: ["fleet.snapshot.read", "fleet.action.execute"]
        )

        XCTAssertThrowsError(try FleetConnection.validateCapability("fleet.runtime.read", in: oldCatalogue)) {
            XCTAssertEqual($0 as? FleetConnectionError, .missingNegotiatedCapability("fleet.runtime.read"))
        }
        XCTAssertThrowsError(try FleetConnection.validateCapability("fleet.usage.read", in: oldCatalogue)) {
            XCTAssertEqual($0 as? FleetConnectionError, .missingNegotiatedCapability("fleet.usage.read"))
        }
    }

    func testOldCatalogueRefusesActionBeforeWireIO() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let clientDescriptor = descriptors[0]
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }
        let serverDone = expectation(description: "old catalogue server finished")
        let serverResult = SocketServerResult()

        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                let authentication = try Self.readRequest(from: serverDescriptor)
                try Self.writeResponse(to: serverDescriptor, request: authentication, result: [:])
                let negotiation = try Self.readRequest(from: serverDescriptor)
                try Self.writeResponse(to: serverDescriptor, request: negotiation, result: [
                    "daemon_version": "old-daemon",
                    "protocol_version": 1,
                    "read_compatible": true,
                    "write_compatible": true,
                    "capability_ids": ["fleet.snapshot.read"],
                ])
                var pollDescriptor = pollfd(fd: serverDescriptor, events: Int16(POLLIN), revents: 0)
                if Darwin.poll(&pollDescriptor, 1, 200) > 0 {
                    throw StoreServerError.closed
                }
            } catch {
                serverResult.record(error)
            }
        }

        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: clientDescriptor
        )
        try await connection.connect()
        defer { Task { await connection.close() } }
        try await connection.authenticate(token: "mdt_test")
        _ = try await connection.negotiate()

        do {
            _ = try await connection.action(FleetActionParams(
                sessionKey: "claude:abc",
                expectedVersion: 1,
                requestID: "request",
                action: .interrupt
            ))
            XCTFail("old catalogue must refuse fleet/action")
        } catch let error as FleetConnectionError {
            XCTAssertEqual(error, .missingNegotiatedCapability("fleet.action.execute"))
        }
        await fulfillment(of: [serverDone], timeout: 1)
        try serverResult.throwIfRecorded()
    }

    func testUsageAndRuntimeReadOnlyRPCsRoundTripAfterNegotiation() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }
        let serverDone = expectation(description: "usage and runtime RPC server finished")
        let serverResult = SocketServerResult()

        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                let authentication = try Self.readRequest(from: serverDescriptor)
                try Self.writeResponse(to: serverDescriptor, request: authentication, result: [:])
                let negotiation = try Self.readRequest(from: serverDescriptor)
                try Self.writeResponse(to: serverDescriptor, request: negotiation, result: [
                    "daemon_version": "fixture-daemon",
                    "protocol_version": 2,
                    "read_compatible": true,
                    "write_compatible": false,
                    "capability_ids": ["fleet.runtime.read", "fleet.usage.read", "fleet.quota.read"],
                ])
                let runtime = try Self.readRequest(from: serverDescriptor)
                XCTAssertEqual(runtime["method"] as? String, "fleet/runtime_status")
                try Self.writeResponse(to: serverDescriptor, request: runtime, result: [
                    "daemon_version": "fixture-daemon",
                    "protocol_version": 2,
                    "hooks": [[
                        "provider": "codex", "installed": true, "hook_ready": true,
                        "delivery_ready": false, "last_event": NSNull(),
                    ]],
                ])
                let usage = try Self.readRequest(from: serverDescriptor)
                XCTAssertEqual(usage["method"] as? String, "fleet/usage_summary")
                XCTAssertEqual((usage["params"] as? [String: Any])?["period"] as? String, "trailing_7_days")
                try Self.writeResponse(to: serverDescriptor, request: usage, result: [
                    "state": "ready", "generated_at": 1, "start_at": 0, "end_at": 1,
                    "totals": [
                        "input_tokens": 100, "cache_creation_tokens": 0, "cache_read_tokens": 0,
                        "output_tokens": 23, "reasoning_tokens": 0, "call_count": 1,
                        "session_count": 1, "project_count": 1, "cost_usd": NSNull(),
                    ],
                    "daily": [], "providers": [], "models": [], "projects": [], "detail": NSNull(),
                ])
                let quota = try Self.readRequest(from: serverDescriptor)
                XCTAssertEqual(quota["method"] as? String, "fleet/quota_summary")
                try Self.writeResponse(to: serverDescriptor, request: quota, result: [
                    "state": "ready", "generated_at": 1,
                    "providers": [[
                        "provider": "codex",
                        "five_hour": ["used_percent": 42, "resets_at": 2_000, "estimated": false],
                        "seven_day": NSNull(), "plan_type": "pro", "updated_at": 1,
                    ]],
                    "detail": NSNull(),
                ])
            } catch {
                serverResult.record(error)
            }
        }

        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: descriptors[0]
        )
        try await connection.connect()
        defer { Task { await connection.close() } }
        try await connection.authenticate(token: "mdt_test")
        _ = try await connection.negotiate()
        let runtime = try await connection.runtimeStatus()
        let usage = try await connection.usageSummary(FleetUsageSummaryParams(period: .trailing7Days))
        let quota = try await connection.quotaSummary()

        XCTAssertEqual(runtime.hooks.map(\.provider), ["codex"])
        XCTAssertEqual(runtime.hooks.first?.deliveryReady, false)
        XCTAssertEqual(usage.state, .ready)
        XCTAssertEqual(usage.totals?.totalTokens, 123)
        XCTAssertNil(usage.totals?.costUSD)
        XCTAssertEqual(quota.providers.first?.fiveHour?.remainingPercent, 58)
        await fulfillment(of: [serverDone], timeout: 1)
        try serverResult.throwIfRecorded()
    }

    func testTmuxTextCapabilityAllowsPrompt() {
        let capabilities = FleetCapabilities(
            structuredAnswer: false, approvals: false, sendPrompt: false, continueTurn: false,
            retry: false, interrupt: false, start: false, stop: false, restart: false,
            kill: false, archive: false, tmuxAttach: false, tmuxText: true, verifiedPicker: false
        )

        XCTAssertTrue(FleetOperatorAction.sendPrompt.isAvailable(in: capabilities))
    }

    func testStructuredDismissWirePreservesExactIdentityAndOldCapabilitiesDefaultOff() throws {
        let action = ControlAction.dismissStructured(
            requestFingerprint: "sha256:request",
            requestIdentity: FleetRequestIdentity(
                requestID: .string("request-1"),
                threadID: "thread-1",
                turnID: "turn-2",
                itemID: "item-3"
            )
        )
        let data = try JSONEncoder().encode(action)
        let encoded = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        XCTAssertEqual(encoded?["action"] as? String, "dismiss_structured")
        XCTAssertEqual(encoded?["request_fingerprint"] as? String, "sha256:request")
        XCTAssertEqual(try JSONDecoder().decode(ControlAction.self, from: data), action)

        let legacy = try JSONDecoder().decode(FleetCapabilities.self, from: Data("{}".utf8))
        XCTAssertFalse(legacy.structuredDismiss)
        XCTAssertFalse(legacy.approvalSession)

        let bypass = ControlAction.approveForSession(
            requestFingerprint: "sha256:approval",
            requestIdentity: FleetRequestIdentity(
                requestID: .string("request-2"), threadID: "thread-1", turnID: "turn-2", itemID: "item-3"
            )
        )
        let bypassData = try JSONEncoder().encode(bypass)
        let bypassEncoded = try JSONSerialization.jsonObject(with: bypassData) as? [String: Any]
        XCTAssertEqual(bypassEncoded?["action"] as? String, "approve_for_session")
        XCTAssertEqual(try JSONDecoder().decode(ControlAction.self, from: bypassData), bypass)
    }

    /// The reconcile frame this client sends, key by key.
    ///
    /// The tag is what the daemon dispatches on and what a durable receipt
    /// records as `action_kind`, so a misspelling here is an action the daemon
    /// answers `-32602` for and a receipt nobody can search. The fingerprint is
    /// the staleness check: without it the daemon would reconcile whatever
    /// question the session happens to be on now.
    ///
    /// The frame carries NO `request_identity`, because the Rust variant has no
    /// such field: reconcile asks the broker about a fingerprint, it does not
    /// route an answer into a provider request.
    func testReconcileStructuredEncodesTheDaemonsTagAndOnlyItsFingerprint() throws {
        let action = ControlAction.reconcileStructured(requestFingerprint: "sha256:interview")
        let data = try JSONEncoder().encode(action)
        let encoded = try JSONSerialization.jsonObject(with: data) as? [String: Any]

        XCTAssertEqual(encoded?["action"] as? String, "reconcile_structured")
        XCTAssertEqual(encoded?["request_fingerprint"] as? String, "sha256:interview")
        XCTAssertNil(
            encoded?["request_identity"],
            "the Rust variant has no identity field, so sending one asks the daemon to parse a key it does not know"
        )
        XCTAssertEqual(
            Set((encoded ?? [:]).keys), ["action", "request_fingerprint"],
            "the frame must carry exactly the two fields the variant declares"
        )
        XCTAssertEqual(try JSONDecoder().decode(ControlAction.self, from: data), action)
    }

    /// A daemon-shaped frame decodes back into the case, so the enum stays
    /// exhaustive in BOTH directions.
    func testReconcileStructuredDecodesADaemonFramedAction() throws {
        let frame = Data(#"{"action":"reconcile_structured","request_fingerprint":"sha256:x"}"#.utf8)
        XCTAssertEqual(
            try JSONDecoder().decode(ControlAction.self, from: frame),
            .reconcileStructured(requestFingerprint: "sha256:x")
        )
    }

    func testUnavailablePromptNeverWritesActionWire() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }
        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let releasePoll = DispatchSemaphore(value: 0)
        let serverDone = expectation(description: "prompt refusal server finished")
        let serverResult = SocketServerResult()

        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                let authentication = try Self.readRequest(from: serverDescriptor)
                try Self.writeResponse(to: serverDescriptor, request: authentication, result: [:])
                let negotiation = try Self.readRequest(from: serverDescriptor)
                try Self.writeResponse(to: serverDescriptor, request: negotiation, result: [
                    "daemon_version": "fixture-daemon",
                    "protocol_version": 1,
                    "read_compatible": true,
                    "write_compatible": true,
                    "capability_ids": ["fleet.action.execute"],
                ])
                let subscription = try Self.readRequest(from: serverDescriptor)
                try Self.writeResponse(to: serverDescriptor, request: subscription, result: [
                    "snapshot": try Self.snapshotObject(head: 1, sessions: [try Self.sampleSessionObject(head: 1)]),
                    "replay": [],
                    "replay_state": ["state": "complete"],
                ])
                _ = releasePoll.wait(timeout: .now() + 1)
                var pollDescriptor = pollfd(fd: serverDescriptor, events: Int16(POLLIN), revents: 0)
                if Darwin.poll(&pollDescriptor, 1, 200) > 0 {
                    throw StoreServerError.closed
                }
            } catch {
                serverResult.record(error)
            }
        }

        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: { _ in FleetConnection(location: location, injectedDescriptor: descriptors[0]) },
                reconnectDelayNanoseconds: { _ in 10_000_000_000 }
            )
        }
        await MainActor.run { store.start() }
        let live = await Self.waitUntil {
            await MainActor.run {
                guard case .live = store.connectionState else { return false }
                return store.sessions.count == 1
            }
        }
        XCTAssertTrue(live)
        await MainActor.run {
            let session = store.sessions[0]
            store.selectedSessionKey = session.sessionKey
            XCTAssertFalse(store.canSendPrompt("hello", on: session))
            store.perform(.sendPrompt, on: session, prompt: "hello")
        }
        releasePoll.signal()
        await fulfillment(of: [serverDone], timeout: 1)
        try serverResult.throwIfRecorded()
        await MainActor.run { store.stop() }
    }

    func testFastSuccessfulReconnectsStillExhaustBoundedRetryBudget() async throws {
        var pairs = [[Int32]]()
        for _ in 0...4 {
            var pair = [Int32](repeating: 0, count: 2)
            XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &pair), 0)
            pairs.append(pair)
        }
        defer { Darwin.close(pairs[4][1]) }
        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let factory = TestConnectionFactory(descriptors: pairs.map { $0[0] }, location: location)
        let store = await MainActor.run {
            FleetStore(location: location, makeConnection: factory.make, reconnectDelayNanoseconds: { _ in 0 })
        }
        let serversDone = expectation(description: "fast-closing servers finished")
        serversDone.expectedFulfillmentCount = 4

        for serverDescriptor in pairs.prefix(4).map({ $0[1] }) {
            DispatchQueue.global().async {
                defer {
                    Darwin.shutdown(serverDescriptor, SHUT_RDWR)
                    Darwin.close(serverDescriptor)
                    serversDone.fulfill()
                }
                try? Self.serveStoreBootstrap(
                    descriptor: serverDescriptor,
                    subscriptionSnapshot: Self.snapshotObject(head: 0, sessions: []),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: Self.snapshotObject(head: 0, sessions: [])
                )
            }
        }

        await MainActor.run { store.start() }
        await fulfillment(of: [serversDone], timeout: 3)
        let stoppedReconnecting = await Self.waitUntil {
            await MainActor.run { factory.count == 4 && store.debugConnectionTaskCount == 0 }
        }
        if factory.count == 4 {
            Darwin.close(pairs[4][0])
        }
        await MainActor.run { store.stop() }

        XCTAssertTrue(stoppedReconnecting, "successful handshakes that close immediately must not reset the retry budget")
    }

    func testResyncNotificationStreamsThroughOwnedSocket() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let clientDescriptor = descriptors[0]
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }

        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: clientDescriptor
        )
        try await connection.connect()
        defer { Task { await connection.close() } }
        let stream = await connection.incoming()
        let waiter = Task { () -> FleetResyncRequired? in
            var iterator = stream.makeAsyncIterator()
            while let incoming = await iterator.next() {
                if case let .resyncRequired(resync) = incoming {
                    return resync
                }
            }
            return nil
        }

        let body = Data(#"{"jsonrpc":"2.0","method":"fleet/resync_required","params":{"after_revision":4,"missed":2}}"#.utf8)
        let frame = try ContentLengthEncoder.encode(body)
        let written = frame.withUnsafeBytes { Darwin.write(serverDescriptor, $0.baseAddress, frame.count) }
        XCTAssertEqual(written, frame.count)

        let resync = await waiter.value
        XCTAssertEqual(resync, FleetResyncRequired(afterRevision: 4, missed: 2))
    }

    func testCancelledRequestClosesOwnedDescriptor() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let clientDescriptor = descriptors[0]
        let serverDescriptor = descriptors[1]
        let requestReceived = expectation(description: "client request received")
        let peerClosed = expectation(description: "client descriptor closed")

        DispatchQueue.global().async {
            _ = Self.readFrame(from: serverDescriptor)
            requestReceived.fulfill()
            var byte: UInt8 = 0
            if Darwin.read(serverDescriptor, &byte, 1) == 0 {
                peerClosed.fulfill()
            }
            Darwin.close(serverDescriptor)
        }

        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: clientDescriptor
        )
        try await connection.connect()
        let authentication = Task {
            try await connection.authenticate(token: "mdt_test")
        }

        await fulfillment(of: [requestReceived], timeout: 1)
        authentication.cancel()
        do {
            try await authentication.value
            XCTFail("expected cancellation")
        } catch is CancellationError {
        } catch {
            XCTFail("unexpected error: \(error)")
        }
        await fulfillment(of: [peerClosed], timeout: 1)
    }

    func testStoreBuffersEventArrivingBeforeSubscribeResponse() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let clientDescriptor = descriptors[0]
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }

        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let connection = FleetConnection(location: location, injectedDescriptor: clientDescriptor)
        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: { _ in connection },
                reconnectDelayNanoseconds: { _ in 0 }
            )
        }
        let serverDone = expectation(description: "store bootstrap server finished")
        let serverResult = SocketServerResult()
        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: serverDescriptor,
                    subscriptionSnapshot: try Self.snapshotObject(head: 0, sessions: []),
                    eventBeforeSubscriptionResponse: true,
                    snapshotAfterEvent: try Self.snapshotObject(head: 1, sessions: [try Self.sampleSessionObject(head: 1)])
                )
            } catch {
                serverResult.record(error)
            }
        }

        await MainActor.run { store.start() }
        await fulfillment(of: [serverDone], timeout: 2)
        try serverResult.throwIfRecorded()
        let receivedSnapshot = await Self.waitUntil {
            await MainActor.run { store.sessions.map(\.sessionKey) == ["s1"] }
        }
        await MainActor.run { store.stop() }

        XCTAssertTrue(receivedSnapshot, "event delivered before subscribe response must trigger an authoritative snapshot")
    }

    func testStoreReconnectsAfterLiveSnapshotFailure() async throws {
        var first = [Int32](repeating: 0, count: 2)
        var second = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &first), 0)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &second), 0)
        let firstServer = first[1]
        let secondServer = second[1]
        defer {
            Darwin.close(firstServer)
            Darwin.close(secondServer)
        }

        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let factory = TestConnectionFactory(
            descriptors: [first[0], second[0]],
            location: location
        )
        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: factory.make,
                reconnectDelayNanoseconds: { _ in 0 }
            )
        }
        let firstServerDone = expectation(description: "first store server finished")
        let firstServerResult = SocketServerResult()
        DispatchQueue.global().async {
            defer { firstServerDone.fulfill() }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: firstServer,
                    subscriptionSnapshot: try Self.snapshotObject(head: 0, sessions: [try Self.sampleSessionObject(head: 0)]),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: nil
                )
                Darwin.shutdown(firstServer, SHUT_RDWR)
            } catch {
                firstServerResult.record(error)
            }
        }
        await MainActor.run { store.start() }
        await fulfillment(of: [firstServerDone], timeout: 2)
        try firstServerResult.throwIfRecorded()
        let reconnected = await Self.waitUntil {
            factory.count == 2
        }
        await MainActor.run { store.stop() }

        XCTAssertTrue(reconnected, "snapshot failure after a live subscription must start exactly one bounded reconnect attempt")
    }

    func testStoreReconnectsWhenBootstrapRequiresResubscribeBeforeLive() async throws {
        var first = [Int32](repeating: 0, count: 2)
        var second = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &first), 0)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &second), 0)
        let firstServer = first[1]
        let secondServer = second[1]
        defer {
            Darwin.close(firstServer)
            Darwin.close(secondServer)
        }

        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let factory = TestConnectionFactory(descriptors: [first[0], second[0]], location: location)
        let store = await MainActor.run {
            FleetStore(location: location, makeConnection: factory.make, reconnectDelayNanoseconds: { _ in 0 })
        }
        let firstServerDone = expectation(description: "invalid bootstrap server finished")
        let secondServerReady = expectation(description: "reconnected bootstrap server ready")
        let releaseSecondServer = DispatchSemaphore(value: 0)

        DispatchQueue.global().async {
            defer { firstServerDone.fulfill() }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: firstServer,
                    subscriptionSnapshot: try Self.snapshotObject(head: 0, sessions: []),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 0, sessions: []),
                    subscriptionReplay: [try Self.eventObject(revision: 1)],
                    replayState: ["state": "snapshot_reset", "reason": "bootstrap"]
                )
                Darwin.shutdown(firstServer, SHUT_RDWR)
            } catch {}
        }
        DispatchQueue.global().async {
            defer { Darwin.shutdown(secondServer, SHUT_RDWR) }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: secondServer,
                    subscriptionSnapshot: try Self.snapshotObject(head: 0, sessions: [try Self.sampleSessionObject(head: 0)]),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 0, sessions: [try Self.sampleSessionObject(head: 0)])
                )
                secondServerReady.fulfill()
                _ = releaseSecondServer.wait(timeout: .now() + 2)
            } catch {
                secondServerReady.fulfill()
            }
        }

        await MainActor.run { store.start() }
        await fulfillment(of: [firstServerDone, secondServerReady], timeout: 2)
        let reconnectedBeforeLive = await Self.waitUntil {
            await MainActor.run {
                guard case .live = store.connectionState else { return false }
                return factory.count == 2
            }
        }
        await MainActor.run { store.stop() }
        releaseSecondServer.signal()

        XCTAssertTrue(reconnectedBeforeLive, "invalid bootstrap replay must reconnect before Fleet becomes live")
    }

    func testStoreMarksProjectionForResubscribeBeforeResyncReconnect() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }

        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: { _ in FleetConnection(location: location, injectedDescriptor: descriptors[0]) },
                reconnectDelayNanoseconds: { _ in 10_000_000_000 }
            )
        }
        let serverDone = expectation(description: "resync notification sent")
        let serverResult = SocketServerResult()
        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: serverDescriptor,
                    subscriptionSnapshot: try Self.snapshotObject(head: 0, sessions: [try Self.sampleSessionObject(head: 0)]),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 0, sessions: []),
                    resyncAfterSubscription: true
                )
            } catch {
                serverResult.record(error)
            }
        }

        await MainActor.run { store.start() }
        await fulfillment(of: [serverDone], timeout: 2)
        try serverResult.throwIfRecorded()
        let reducerApplied = await Self.waitUntil {
            await MainActor.run { store.needsResubscribe }
        }
        await MainActor.run { store.stop() }

        XCTAssertTrue(reducerApplied, "resync notification must mark projection before reconnect scheduling")
    }

    private static func readFrame(from descriptor: Int32) -> Data? {
        var decoder = ContentLengthDecoder()
        var bytes = [UInt8](repeating: 0, count: 1024)
        while true {
            let count = Darwin.read(descriptor, &bytes, bytes.count)
            guard count > 0 else { return nil }
            guard let frames = try? decoder.append(Data(bytes.prefix(Int(count)))) else { return nil }
            if let frame = frames.first { return frame }
        }
    }

    /// The Pal session is minted ONCE per connection, not once per poll,
    /// and a leg refused because the SESSION IS GONE is what makes the next
    /// page mint again.
    ///
    /// This is the bug the cache exists for, counted on the wire rather than
    /// asserted about a dictionary. `fleet/acp_session_create` is idempotent
    /// but not free: daemon-side every call is an INSERT inside a write
    /// transaction that hits the live-scope unique index and reads the
    /// incumbent back, and the chat pane polls once a second, so this surface
    /// was opening 86,400 write transactions a day against a database whose
    /// reads answer in 0.1s while this call timed out at 5s.
    func testPalSessionIsMintedOncePerConnectionUntilTheSessionIsGone() async throws {
        try await runMintCacheScenario(rejectionDetail: "target_not_running", mintsAfterSend: 2)
    }

    /// A TRANSIENT refusal keeps the mint.
    ///
    /// `queue_full` is a pool that is busy, not a session that is gone, and the
    /// daemon still holds the session behind it. Re-minting here would pay the
    /// write transaction this cache exists to remove on a session that is
    /// about to answer the next prompt perfectly well.
    func testATransientRejectionDoesNotCostAReMint() async throws {
        try await runMintCacheScenario(rejectionDetail: "queue_full", mintsAfterSend: 1)
    }

    /// Two pages, then one send that comes back REJECTED with `rejectionDetail`,
    /// asserting how many mints the whole sequence cost.
    ///
    /// Shared so the two outcomes cannot drift apart: what differs between them
    /// is ONE daemon token, and everything else about the flow is identical.
    private func runMintCacheScenario(rejectionDetail: String, mintsAfterSend: Int) async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }
        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let serverDone = expectation(description: "chat server finished")
        let serverResult = SocketServerResult()
        let mints = ChatServerCounts(rejectionDetail: rejectionDetail)

        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: serverDescriptor,
                    subscriptionSnapshot: try Self.snapshotObject(head: 1, sessions: []),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 1, sessions: []),
                    capabilityIDs: [
                        "fleet.chat.read", "fleet.chat.write",
                        "fleet.message.read", "fleet.message.send", "fleet.acp.spawn",
                    ]
                )
                while true {
                    try Self.answerChatRequest(
                        try Self.readRequest(from: serverDescriptor),
                        to: serverDescriptor,
                        counts: mints
                    )
                }
            } catch StoreServerError.closed {
                // The client hung up at the end of the test. Not a failure.
            } catch {
                serverResult.record(error)
            }
        }

        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: { _ in FleetConnection(location: location, injectedDescriptor: descriptors[0]) },
                reconnectDelayNanoseconds: { _ in 10_000_000_000 }
            )
        }
        await MainActor.run { store.start() }
        let live = await Self.waitUntil {
            await MainActor.run {
                guard case .live = store.connectionState else { return false }
                return store.canWrite
            }
        }
        XCTAssertTrue(live, "the fixture never reached a live, writable connection")

        await store.refreshChatOnce()
        await MainActor.run { XCTAssertEqual(store.chat.targetSessionKey, "acp:1") }
        XCTAssertEqual(mints.acpCreates, 1, "the first page has to mint")

        await store.refreshChatOnce()
        await MainActor.run { XCTAssertEqual(store.chat.targetSessionKey, "acp:1") }
        XCTAssertEqual(
            mints.acpCreates, 1,
            "a second page must reuse the remembered session, not reopen a write transaction"
        )

        await MainActor.run { store.sendChatMessage("hello") }
        let reported = await Self.waitUntil {
            await MainActor.run { store.controlNotice?.contains("not delivered") == true }
        }
        XCTAssertTrue(reported, "the rejected leg must reach the operator")
        // The send re-pages itself, so waiting for the notice is not enough:
        // wait for the page that follows it to have run.
        let repaged = await Self.waitUntil {
            await MainActor.run { store.pendingIntentID == nil }
        }
        XCTAssertTrue(repaged, "the send never finished its own re-page")
        XCTAssertEqual(
            mints.acpCreates, mintsAfterSend,
            "\(rejectionDetail): wrong number of mints after the send"
        )

        await MainActor.run { store.stop() }
        await fulfillment(of: [serverDone], timeout: 5)
        try serverResult.throwIfRecorded()
    }

    /// A page already in flight when the cache is invalidated must NOT put the
    /// forgotten session back.
    ///
    /// A page is four round trips, and the poll loop and the send path each run
    /// in their own Task on the main actor, so they interleave. Without a
    /// generation check the sequence is: poll reads the cached key, send
    /// reports REJECTED and forgets it, poll finishes and writes the dead key
    /// back. The operator's next message then goes to the same dead session,
    /// which is the exact failure the invalidation exists to prevent, arriving
    /// one poll later and looking like the fix never worked.
    ///
    /// Driven by holding the daemon inside the page rather than by hoping the
    /// interleaving happens, so this fails deterministically without the guard.
    func testAPageInFlightDuringAnInvalidationDoesNotRestoreTheForgottenSession() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }
        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let serverDone = expectation(description: "chat server finished")
        let serverResult = SocketServerResult()
        let mints = ChatServerCounts()
        let gate = ChatServerGate()

        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: serverDescriptor,
                    subscriptionSnapshot: try Self.snapshotObject(head: 1, sessions: []),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 1, sessions: []),
                    capabilityIDs: [
                        "fleet.chat.read", "fleet.chat.write",
                        "fleet.message.read", "fleet.message.send", "fleet.acp.spawn",
                    ]
                )
                while true {
                    try Self.answerChatRequest(
                        try Self.readRequest(from: serverDescriptor),
                        to: serverDescriptor,
                        counts: mints,
                        gate: gate
                    )
                }
            } catch StoreServerError.closed {
                // The client hung up at the end of the test. Not a failure.
            } catch {
                serverResult.record(error)
            }
        }

        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: { _ in FleetConnection(location: location, injectedDescriptor: descriptors[0]) },
                reconnectDelayNanoseconds: { _ in 10_000_000_000 }
            )
        }
        await MainActor.run { store.start() }
        let live = await Self.waitUntil {
            await MainActor.run {
                guard case .live = store.connectionState else { return false }
                return store.canWrite
            }
        }
        XCTAssertTrue(live, "the fixture never reached a live, writable connection")

        await store.refreshChatOnce()
        XCTAssertEqual(mints.acpCreates, 1, "the first page fills the cache")

        // A second page, stopped in the middle: past the cache read, before the
        // write-back.
        gate.arm()
        let paging = Task { await store.refreshChatOnce() }
        let stopped = await Self.waitUntil { gate.hasReached }
        XCTAssertTrue(stopped, "the daemon never reached the point the page is held at")

        // What a REJECTED send does, on the actor, while that page is waiting.
        await MainActor.run { store.forgetPalSession(inScope: "channel:c1") }
        gate.letGo()
        await paging.value

        // The page finished after the invalidation, so the next one must ask
        // the daemon again rather than reusing what that page had read.
        await store.refreshChatOnce()
        XCTAssertEqual(
            mints.acpCreates, 2,
            "the in-flight page restored a session the operator had already been told is dead"
        )

        await MainActor.run { store.stop() }
        await fulfillment(of: [serverDone], timeout: 5)
        try serverResult.throwIfRecorded()
    }

    /// A message committed on the daemon reaches an open pane with no page
    /// behind it, and one committed in ANOTHER scope never appears.
    ///
    /// The negative half is ordered rather than timed: both frames go down one
    /// socket, in order, so when the second has landed the first has already
    /// been through the fold. Waiting a fixed interval to prove an absence is
    /// how a suite gets a flake that only fires on a loaded machine.
    func testALiveMessageEventReachesTheOpenPaneWithoutAPage() async throws {
        try await withChatStore { store, server in
            // The pane announces itself, exactly as its `.task` does. Without
            // this the fold is off, which is the whole point of the gate.
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertEqual(store.chat.scopeKey, "channel:c1")
                XCTAssertEqual(store.chat.messages, [], "the scripted page has no history")
            }

            try Self.writeNotification(
                to: server,
                method: "fleet/message_event",
                params: ["message": Self.messageObject(id: "01J0OTHER", scope: "channel:elsewhere")]
            )
            try Self.writeNotification(
                to: server,
                method: "fleet/message_event",
                params: ["message": Self.messageObject(id: "01J0LIVE", scope: "channel:c1")]
            )

            let landed = await Self.waitUntil {
                await MainActor.run { store.chat.messages.map(\.id) == ["01J0LIVE"] }
            }
            XCTAssertTrue(
                landed,
                "a committed message must reach the pane on the stream, and only this scope's"
            )
        }
    }

    /// A message that arrives while a page is READING must survive that page.
    ///
    /// The page replaces the surface wholesale, and its `fleet/message_list`
    /// answered before this message was committed. Folding it into the live
    /// surface alone is not enough: the page lands afterwards and rewinds the
    /// pane past it, and the operator does not see it again until the
    /// safety net pages half a minute later.
    ///
    /// Driven by holding the daemon inside the page rather than by hoping the
    /// interleaving happens, so it fails deterministically without the replay.
    func testALiveMessageIsNotLostByAPageAlreadyInFlight() async throws {
        let gate = ChatServerGate()
        try await withChatStore(gate: gate) { store, server in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()

            // A second page, stopped at `fleet/message_list`: the point where
            // the page has already read the timeline it is going to publish.
            gate.arm()
            let paging = Task { await store.refreshChatOnce() }
            let stopped = await Self.waitUntil { gate.hasReached }
            XCTAssertTrue(stopped, "the daemon never reached the point the page is held at")

            try Self.writeNotification(
                to: server,
                method: "fleet/message_event",
                params: ["message": Self.messageObject(id: "01J0RACE", scope: "channel:c1")]
            )
            let folded = await Self.waitUntil {
                await MainActor.run { store.chat.messages.map(\.id) == ["01J0RACE"] }
            }
            XCTAssertTrue(folded, "the event never reached the surface, so the page cannot be what dropped it")

            gate.letGo()
            await paging.value
            await MainActor.run {
                XCTAssertEqual(
                    store.chat.messages.map(\.id), ["01J0RACE"],
                    "the page overwrote a message that was committed while it was reading"
                )
            }
        }
    }

    /// A daemon that advertises the capability but REFUSES the subscription
    /// still gets a populated pane.
    ///
    /// The ordinary upgrade-the-app-keep-the-daemon window: the catalogue names
    /// `fleet.message.read` because `fleet/message_list` exists, and
    /// `fleet/message_subscribe` answers an error. Live push is a latency
    /// improvement, not a dependency, so the refusal must cost the pane nothing
    /// except the wait for `fleetChatSafetyNetInterval`.
    func testADaemonThatRefusesTheSubscriptionStillPages() async throws {
        try await withChatStore(refuseMessageSubscribe: true) { store, _ in
            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertTrue(store.connectionState.isLive, "a refused subscription must not cost the connection")
                XCTAssertEqual(store.chat.scopeKey, "channel:c1")
                XCTAssertEqual(store.chat.targetSessionKey, "acp:1")
            }
        }
    }

    /// With NO pane on screen, a chat event is not folded at all.
    ///
    /// `chat` is `@Published` on the store the whole notch observes, so a
    /// folded event redraws the roster, the chips and the menu-bar summary. A
    /// scope filter does not stop that once a pane has been opened once,
    /// because the scope stays resolved: a busy Pal would invalidate the
    /// roster several times a second while the operator is reading Sessions.
    ///
    /// A SETTLE, deliberately, and it is the honest shape for this one. The
    /// sibling tests order their assertions behind a second frame on the same
    /// socket, but nothing observable happens when an event is correctly
    /// dropped, so there is no barrier to wait behind. `waitUntil` polls for
    /// its full two seconds and reports that the timeline never filled, which
    /// is thousands of times longer than the microseconds an ungated fold takes
    /// once the frame is read. The positive control that follows is what stops
    /// this passing because the pipeline was dead rather than because the gate
    /// worked.
    func testAMessageEventWithNoPaneOpenIsNotFolded() async throws {
        try await withChatStore { store, server in
            await store.refreshChatOnce()
            await MainActor.run { XCTAssertEqual(store.chat.scopeKey, "channel:c1") }

            try Self.writeNotification(
                to: server,
                method: "fleet/message_event",
                params: ["message": Self.messageObject(id: "01J0CLOSED", scope: "channel:c1")]
            )
            let foldedWithNoPane = await Self.waitUntil {
                await MainActor.run { !store.chat.messages.isEmpty }
            }
            XCTAssertFalse(
                foldedWithNoPane,
                "an event with no pane open was rendered anyway, so every Pal message redraws the roster"
            )

            // The control: the same stream, the same scope, one open pane.
            await MainActor.run { store.chatPaneAppeared() }
            try Self.writeNotification(
                to: server,
                method: "fleet/message_event",
                params: ["message": Self.messageObject(id: "01J0OPEN", scope: "channel:c1")]
            )
            let landed = await Self.waitUntil {
                await MainActor.run { store.chat.messages.map(\.id) == ["01J0OPEN"] }
            }
            XCTAssertTrue(landed, "the gate is stuck shut, so nothing proved the drop above")
        }
    }

    /// A message committed during the VERY FIRST page of a connection survives
    /// that page.
    ///
    /// The opening page is the one window where the surface has no scope yet,
    /// so a buffer that filters incoming events against `chat.scopeKey` matches
    /// nothing at all, and `apply` drops the event live for the same reason.
    /// The message is then invisible until the next safety-net page, up to
    /// thirty seconds later, on the first conversation an operator opens.
    ///
    /// Distinct from its sibling, which pages once before arming the gate and
    /// therefore only ever exercises the scope-resolved path.
    func testALiveMessageDuringTheFirstPageOfAConnectionIsNotLost() async throws {
        let gate = ChatServerGate()
        try await withChatStore(gate: gate) { store, server in
            await MainActor.run { store.chatPaneAppeared() }

            // No page has run, so the surface has no scope. This is the window.
            gate.arm()
            let paging = Task { await store.refreshChatOnce() }
            let stopped = await Self.waitUntil { gate.hasReached }
            XCTAssertTrue(stopped, "the daemon never reached the point the page is held at")
            await MainActor.run {
                XCTAssertNil(store.chat.scopeKey, "the fixture published a scope before its first page")
            }

            try Self.writeNotification(
                to: server,
                method: "fleet/message_event",
                params: ["message": Self.messageObject(id: "01J0FIRST", scope: "channel:c1")]
            )
            gate.letGo()
            await paging.value

            await MainActor.run {
                XCTAssertEqual(
                    store.chat.messages.map(\.id), ["01J0FIRST"],
                    "the opening page dropped a message committed while it was reading"
                )
            }
        }
    }

    /// A page INVALIDATED while it was reading never reaches the screen.
    ///
    /// This is the PR A race arriving one layer out. The generation guarded the
    /// session cache but not the publish, so the sequence was: a send's leg
    /// reports `target_not_running`, the store forgets the dead session, and an
    /// older page that read the dead key lands afterwards and paints it back.
    /// At the old one-second poll that healed within a second. At
    /// `fleetChatSafetyNetInterval` the composer aims at a dead session for
    /// half a minute, which is the whole of an operator's next message.
    ///
    /// Each page answers with a row naming itself, so the assertion is about
    /// WHICH page's surface is on screen rather than about a value both would
    /// have produced.
    func testAPageInvalidatedWhileItWasReadingIsNeverPublished() async throws {
        let gate = ChatServerGate()
        try await withChatStore(gate: gate, pagesReturnMarkerRows: true) { store, _ in
            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertEqual(store.chat.messages.map(\.id), ["page-1"], "the first page is on screen")
            }

            // A second page, held after it has read the session cache.
            gate.arm()
            let paging = Task { await store.refreshChatOnce() }
            let stopped = await Self.waitUntil { gate.hasReached }
            XCTAssertTrue(stopped, "the daemon never reached the point the page is held at")

            // What a REJECTED send does, while that page is in flight.
            await MainActor.run { store.forgetPalSession(inScope: "channel:c1") }
            gate.letGo()
            await paging.value

            await MainActor.run {
                XCTAssertEqual(
                    store.chat.messages.map(\.id), ["page-1"],
                    "a page the store had already disowned was published over the current surface"
                )
            }
        }
    }

    // MARK: - The ACP transcript stream (PR C)

    /// The store opens the transcript stream for the session the page just
    /// resolved, then reads that session's tail with NO cursor.
    ///
    /// The absent cursor is the assertion. `ingest_order` is one global
    /// AUTOINCREMENT sequence over every provider's rows, so a client cannot
    /// name a window of its own that means "the newest rows of THIS session":
    /// the newest hundred orders on a busy machine can hold none of them. An
    /// uncursored read is the daemon's tail arm, and it is the only version of
    /// this read a client can be correct with.
    func testTheStoreSubscribesFirstThenReadsTheSessionTailWithNoCursor() async throws {
        let counts = ChatServerCounts(
            transcriptHeadOrder: 500,
            transcriptChunks: [Self.transcriptChunkObject(order: 500, text: "reading the code")]
        )
        try await withChatStore(counts: counts) { store, _ in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()

            let subscribes = counts.transcriptSubscribes
            XCTAssertEqual(subscribes.count, 1, "the stream must be opened exactly once per page")
            let subscribeParams = subscribes[0]["params"] as? [String: Any]
            XCTAssertEqual(
                subscribeParams?["session_key"] as? String, "acp:1",
                "the stream must follow the session the page minted, not the channel"
            )
            XCTAssertNil(
                subscribeParams?["after_order"],
                "an absent cursor is what asks the daemon to start at the head; a null is not the same frame"
            )

            let lists = counts.transcriptLists
            XCTAssertEqual(lists.count, 1)
            let listParams = lists[0]["params"] as? [String: Any]
            XCTAssertEqual(listParams?["session_key"] as? String, "acp:1")
            XCTAssertNil(
                listParams?["after_order"],
                "a client-computed window over GLOBAL orders is the bug; the tail read takes no cursor"
            )
            XCTAssertEqual((listParams?["limit"] as? NSNumber)?.uint32Value, fleetTranscriptListMax)

            await MainActor.run {
                XCTAssertEqual(store.chat.transcriptState.rows.map(\.body), ["reading the code"])
                XCTAssertEqual(store.chat.transcriptState.cursor, 500)
                XCTAssertNil(store.chat.transcriptDetail, "a page that worked must not explain an absence")
            }
        }
    }

    /// An EMPTY transcript opens the stream, reads its tail, and shows nothing
    /// without claiming the transcript is unreadable.
    ///
    /// The tail read is issued regardless: an empty answer and an unreadable
    /// one are different facts, and only the daemon can tell them apart. The
    /// stream is open, so the session's first chunk still arrives live.
    func testAnEmptyTranscriptOpensTheStreamAndShowsNothing() async throws {
        let counts = ChatServerCounts(transcriptHeadOrder: nil)
        try await withChatStore(counts: counts) { store, _ in
            await store.refreshChatOnce()

            XCTAssertEqual(counts.transcriptSubscribes.count, 1)
            XCTAssertEqual(counts.transcriptLists.count, 1, "an empty transcript is still read, not assumed")
            await MainActor.run {
                XCTAssertEqual(store.chat.transcriptState.rows, [])
                XCTAssertNil(store.chat.transcriptDetail, "empty is not the same fact as unreadable")
            }
        }
    }

    /// THE CRITICAL. A safety-net page must not throw away the transcript the
    /// operator is reading.
    ///
    /// The page rebuilds its surface from an empty one and the store publishes
    /// it wholesale, so without an explicit carry the rows accumulated live
    /// since the last page are discarded every thirty seconds. The concrete
    /// failure it caused: an agent runs a long turn, hundreds of rows arrive on
    /// the stream, and the pane drops to "Nothing yet" mid-turn.
    ///
    /// The scripted daemon answers with an empty transcript throughout, so this
    /// fails unless the rows are genuinely carried rather than re-read. That is
    /// also the live shape: a tail read can legitimately return nothing the
    /// client did not already have.
    func testASafetyNetPageKeepsTheTranscriptTheOperatorIsReading() async throws {
        let counts = ChatServerCounts(transcriptHeadOrder: nil)
        try await withChatStore(counts: counts) { store, server in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()

            // A turn's worth of live rows, none of which any page will return.
            for order in 1...3 {
                try Self.writeNotification(
                    to: server,
                    method: "fleet/transcript_event",
                    params: ["chunk": Self.transcriptChunkObject(order: Int64(order), text: "row \(order)")]
                )
            }
            let arrived = await Self.waitUntil {
                await MainActor.run { store.chat.transcriptState.rows.count == 3 }
            }
            XCTAssertTrue(arrived, "the live rows never landed, so the page cannot be what drops them")

            // The safety net fires.
            await store.refreshChatOnce()

            await MainActor.run {
                XCTAssertEqual(
                    store.chat.transcriptState.rows.map(\.body), ["row 1", "row 2", "row 3"],
                    "the page wiped a transcript the operator was reading mid-turn"
                )
                XCTAssertEqual(store.chat.transcriptState.cursor, 3, "and the cursor went with it")
            }
        }
    }

    /// The CLASSIFIER is carried too, which is the subtler half of the same
    /// fix.
    ///
    /// It holds the pending tool-title map, so a page that dropped it would
    /// leave the next tool result rendering under the unnamed `tool` form. That
    /// is the exact degradation the replay guard exists to prevent, arriving
    /// through the page instead of through a replay, and no row-count assertion
    /// would notice it.
    func testASafetyNetPageKeepsThePendingToolTitles() async throws {
        let counts = ChatServerCounts(transcriptHeadOrder: nil)
        try await withChatStore(counts: counts) { store, server in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()

            // A tool call, whose title only the classifier now remembers.
            try Self.writeNotification(
                to: server,
                method: "fleet/transcript_event",
                params: ["chunk": Self.transcriptChunkObject(
                    order: 1,
                    eventType: "acp.tool_call",
                    payload: ["sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Bash"]
                )]
            )
            let called = await Self.waitUntil {
                await MainActor.run { store.chat.transcriptState.rows.map(\.body) == ["Bash"] }
            }
            XCTAssertTrue(called, "the tool call never landed")

            // The safety net fires BETWEEN the call and its result.
            await store.refreshChatOnce()

            try Self.writeNotification(
                to: server,
                method: "fleet/transcript_event",
                params: ["chunk": Self.transcriptChunkObject(
                    order: 2,
                    eventType: "acp.tool_call",
                    payload: [
                        "sessionUpdate": "tool_call_update", "toolCallId": "t1",
                        "status": "completed", "rawOutput": "ok",
                    ]
                )]
            )
            let resolved = await Self.waitUntil {
                await MainActor.run { store.chat.transcriptState.rows.count == 2 }
            }
            XCTAssertTrue(resolved, "the tool result never landed")
            await MainActor.run {
                XCTAssertEqual(
                    store.chat.transcriptState.rows.last?.body, "Bash  ok",
                    "the page dropped the pending tool title, so the result lost the tool it belongs to"
                )
            }
        }
    }

    /// A page whose transcript was carried forward is never RE-READ.
    ///
    /// The subscribe repeats once per page, deliberately: naming the cursor
    /// makes a repeat neither gap nor duplicate, and repeating it is what
    /// re-arms a forwarder the daemon dropped on a transient error. The tail
    /// read is the half that must not repeat, because the rows are already on
    /// screen. This is also the guard against the carry-forward being
    /// implemented as a re-read that happens to produce the same rows.
    func testACarriedTranscriptIsNotReReadFromTheDaemon() async throws {
        let counts = ChatServerCounts(transcriptHeadOrder: nil)
        try await withChatStore(counts: counts) { store, server in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()
            XCTAssertEqual(counts.transcriptSubscribes.count, 1, "the first page opens the stream")
            XCTAssertEqual(counts.transcriptLists.count, 1, "and reads the tail once")

            try Self.writeNotification(
                to: server,
                method: "fleet/transcript_event",
                params: ["chunk": Self.transcriptChunkObject(order: 1, text: "live")]
            )
            let landed = await Self.waitUntil {
                await MainActor.run { store.chat.transcriptState.cursor == 1 }
            }
            XCTAssertTrue(landed, "the live row never landed")

            await store.refreshChatOnce()
            await store.refreshChatOnce()

            // The SUBSCRIBE repeats deliberately, once per page: it names a
            // cursor, so it can neither gap nor duplicate, and re-arming is
            // what notices a forwarder the daemon has silently dropped.
            XCTAssertEqual(
                counts.transcriptSubscribes.count, 3,
                "every page must re-arm the stream, or a dead forwarder is never noticed"
            )
            // The tail READ is the expensive half, and it must not repeat.
            XCTAssertEqual(
                counts.transcriptLists.count, 1,
                "a carried transcript must not be re-read from the daemon"
            )
            for subscribe in counts.transcriptSubscribes.dropFirst() {
                let params = subscribe["params"] as? [String: Any]
                XCTAssertEqual(
                    (params?["after_order"] as? NSNumber)?.int64Value, 1,
                    "a repeat subscribe must resume from the cursor, or it re-delivers the whole tail"
                )
            }
        }
    }

    /// A committed chunk reaches an open pane with no page behind it, and one
    /// for ANOTHER session never appears.
    ///
    /// Ordered rather than timed, like its message sibling: both frames go down
    /// one socket in order, so when the second has landed the first has already
    /// been through the fold. Waiting a fixed interval to prove an absence is
    /// how a suite gets a flake that only fires on a loaded machine.
    func testALiveTranscriptChunkReachesTheOpenPaneWithoutAPage() async throws {
        let counts = ChatServerCounts(transcriptHeadOrder: nil)
        try await withChatStore(counts: counts) { store, server in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()
            await MainActor.run { XCTAssertEqual(store.chat.targetSessionKey, "acp:1") }

            try Self.writeNotification(
                to: server,
                method: "fleet/transcript_event",
                params: ["chunk": Self.transcriptChunkObject(
                    order: 8, sessionKey: "acp:elsewhere", text: "another agent"
                )]
            )
            try Self.writeNotification(
                to: server,
                method: "fleet/transcript_event",
                params: ["chunk": Self.transcriptChunkObject(order: 9, text: "on it")]
            )

            let landed = await Self.waitUntil {
                await MainActor.run { store.chat.transcriptState.rows.map(\.body) == ["on it"] }
            }
            XCTAssertTrue(
                landed,
                "a committed chunk must reach the pane on the stream, and only this session's"
            )
        }
    }

    /// A chunk that arrives while a page is READING must survive that page.
    ///
    /// The page replaces the surface wholesale and its `fleet/transcript_list`
    /// answered before this chunk was committed, so folding it into the live
    /// surface alone is not enough: the page lands afterwards and rewinds the
    /// pane past it. Driven by holding the daemon inside the page rather than
    /// by hoping the interleaving happens, so it fails deterministically
    /// without the replay.
    func testALiveTranscriptChunkIsNotLostByAPageAlreadyInFlight() async throws {
        let gate = ChatServerGate()
        let counts = ChatServerCounts(transcriptHeadOrder: nil)
        try await withChatStore(gate: gate, counts: counts) { store, server in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()

            gate.arm()
            let paging = Task { await store.refreshChatOnce() }
            let stopped = await Self.waitUntil { gate.hasReached }
            XCTAssertTrue(stopped, "the daemon never reached the point the page is held at")

            try Self.writeNotification(
                to: server,
                method: "fleet/transcript_event",
                params: ["chunk": Self.transcriptChunkObject(order: 11, text: "mid-page")]
            )
            let folded = await Self.waitUntil {
                await MainActor.run { store.chat.transcriptState.rows.map(\.body) == ["mid-page"] }
            }
            XCTAssertTrue(folded, "the chunk never reached the surface, so the page cannot be what dropped it")

            gate.letGo()
            await paging.value
            await MainActor.run {
                XCTAssertEqual(
                    store.chat.transcriptState.rows.map(\.body), ["mid-page"],
                    "the page overwrote a chunk that was committed while it was reading"
                )
            }
        }
    }

    /// A transcript BURST during a page must not evict the chat message that
    /// page's buffer exists to protect.
    ///
    /// The buffers are per axis for exactly this. An agent mid-turn emits
    /// transcript chunks continuously while the conversation sits idle, so one
    /// shared hundred-entry FIFO is emptied of chat messages by transcript
    /// traffic inside a single page. That would silently undo the guarantee the
    /// previous PR added, and it would do it precisely when the pane is
    /// busiest.
    ///
    /// The message goes in FIRST and the burst after it, so a shared FIFO
    /// evicts the message and this fails.
    func testATranscriptBurstDoesNotEvictTheBufferedChatMessage() async throws {
        let gate = ChatServerGate()
        let counts = ChatServerCounts(transcriptHeadOrder: nil)
        try await withChatStore(gate: gate, counts: counts) { store, server in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()

            gate.arm()
            let paging = Task { await store.refreshChatOnce() }
            let stopped = await Self.waitUntil { gate.hasReached }
            XCTAssertTrue(stopped, "the daemon never reached the point the page is held at")

            try Self.writeNotification(
                to: server,
                method: "fleet/message_event",
                params: ["message": Self.messageObject(id: "01J0KEEP", scope: "channel:c1")]
            )
            // Well past the shared buffer's old hundred-entry ceiling.
            for order in 1...150 {
                try Self.writeNotification(
                    to: server,
                    method: "fleet/transcript_event",
                    params: ["chunk": Self.transcriptChunkObject(order: Int64(order), text: "burst \(order)")]
                )
            }
            let burstLanded = await Self.waitUntil {
                await MainActor.run { store.chat.transcriptState.cursor == 150 }
            }
            XCTAssertTrue(burstLanded, "the burst never landed, so nothing was under pressure")

            gate.letGo()
            await paging.value

            await MainActor.run {
                XCTAssertEqual(
                    store.chat.messages.map(\.id), ["01J0KEEP"],
                    "the transcript burst evicted the chat message the buffer was added to protect"
                )
            }
        }
    }

    /// The daemon's truncation flag reaches the surface, and survives the
    /// carry-forward.
    ///
    /// A short page is not the same fact as a short transcript: the tail is
    /// bounded by payload BYTES as well as rows, so a session of large chunks
    /// returns few of them. A pane that read that as completeness would draw a
    /// partial run as a whole one. The second page is the other half: the
    /// marker describes rows that are still on screen, so dropping it thirty
    /// seconds later would quietly turn a partial run into a complete-looking
    /// one.
    func testTheTruncationFlagReachesTheSurfaceAndSurvivesTheCarryForward() async throws {
        let counts = ChatServerCounts(
            transcriptHeadOrder: 9,
            transcriptChunks: [Self.transcriptChunkObject(order: 9, text: "the tail")],
            transcriptTruncated: true
        )
        try await withChatStore(counts: counts) { store, _ in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertTrue(
                    store.chat.transcriptState.truncated,
                    "the daemon said it left rows behind and the pane must say so too"
                )
                XCTAssertNil(store.chat.transcriptDetail, "a seam is not an error")
            }

            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertTrue(
                    store.chat.transcriptState.truncated,
                    "the seam marker was dropped by the page that kept the rows it describes"
                )
            }
        }
    }

    /// A daemon that cannot serve transcripts says SO, even when there is also
    /// no session to follow.
    ///
    /// Two absences, and the order of the guards decides which one the operator
    /// is told about. Reporting the missing session first sent the reader off
    /// to look at a session that would have been refused the read anyway.
    func testAMissingCapabilityIsReportedAheadOfAMissingSession() async throws {
        let counts = ChatServerCounts(transcriptHeadOrder: nil)
        try await withChatStore(serveTranscript: false, refuseAcpSessionCreate: true, counts: counts) { store, _ in
            await store.refreshChatOnce()

            await MainActor.run {
                XCTAssertNil(store.chat.targetSessionKey, "the fixture refused the mint, so there is no session")
                XCTAssertEqual(
                    store.chat.transcriptDetail, "This daemon does not serve ACP transcripts.",
                    "the capability is the real reason and must be the one reported"
                )
            }
        }
    }

    /// MAJOR 1. A MOMENTARY refusal must not pin the banner forever.
    ///
    /// The detail describes what happened on the page that set it. Carrying it
    /// forward made a single hiccup permanent: it rode onto every later
    /// surface, the steady-state page returned before anything could clear it,
    /// and the pane rendered "Transcript unavailable" above a live, updating
    /// transcript until the session was re-minted or the app restarted. Before
    /// the carry-forward existed this self-healed on the next page, so the
    /// regression arrived with the fix for something else.
    ///
    /// It takes a RECONNECT to reach, and the first version of this test did
    /// not: a refusal on the opening page leaves no cursor, so the carry never
    /// runs and the assertion passes against the bug. The failure needs an
    /// opening subscribe that SUCCEEDS, so a cursor exists to be carried, and a
    /// later one that does not. Five older assertions touch `transcriptDetail`
    /// and none watches it CLEAR, which is how this could land unnoticed.
    func testATransientTranscriptRefusalClearsOnceItStopsHolding() async throws {
        // The subscribe on the SECOND connection is refused; the opening one
        // and the retry after it are not.
        let counts = ChatServerCounts(
            transcriptHeadOrder: 5,
            transcriptChunks: [Self.transcriptChunkObject(order: 5, text: "still running")],
            refusedTranscriptSubscribes: [2]
        )
        try await withReconnectingChatStore(counts: counts) { store, firstServer, factory in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertEqual(store.chat.transcriptState.cursor, 5, "the opening page must leave a cursor to carry")
                XCTAssertNil(store.chat.transcriptDetail)
            }

            Darwin.shutdown(firstServer, SHUT_RDWR)
            let reconnected = await Self.waitUntil { factory.count == 2 }
            XCTAssertTrue(reconnected, "the store never reconnected")
            _ = await Self.waitUntil { await MainActor.run { store.connectionState.isLive } }

            // The page whose subscribe is refused, with a cursor carried.
            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertNotNil(
                    store.chat.transcriptDetail,
                    "a refused subscribe must be explained while it is still the truth"
                )
                XCTAssertEqual(
                    store.chat.transcriptState.cursor, 5,
                    "and the rows already read must survive the refusal"
                )
            }

            // The daemon is well again. Nothing else changed.
            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertNil(
                    store.chat.transcriptDetail,
                    "the banner outlived the refusal and now sits above a working transcript"
                )
            }
        }
    }

    /// MAJOR 2. A reconnect resumes the stream from the rows already on screen,
    /// not from the daemon's head.
    ///
    /// A bare subscribe starts the forwarder at the head, so everything
    /// committed while this client was disconnected is never pushed. The tail
    /// page used to be the backstop and no longer is, because a carried cursor
    /// skips it: a five-second outage over a busy turn silently lost every row
    /// in the gap, with no seam to show it happened. `openChatStream` records
    /// this lesson for the chat half; the transcript half repeated it.
    ///
    /// Two connections, because the failure only exists across a reconnect: the
    /// assertion is on the SECOND subscribe's cursor.
    func testAReconnectResumesTheTranscriptStreamFromTheRowsAlreadyShown() async throws {
        let counts = ChatServerCounts(
            transcriptHeadOrder: 5,
            transcriptChunks: [Self.transcriptChunkObject(order: 5, text: "before the drop")]
        )
        try await withReconnectingChatStore(counts: counts) { store, firstServer, factory in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()
            // One row past the page, so the cursor is somewhere only the live
            // stream could have put it.
            try Self.writeNotification(
                to: firstServer,
                method: "fleet/transcript_event",
                params: ["chunk": Self.transcriptChunkObject(order: 6, text: "live before the drop")]
            )
            let atSix = await Self.waitUntil {
                await MainActor.run { store.chat.transcriptState.cursor == 6 }
            }
            XCTAssertTrue(atSix, "the live row never landed, so there is no cursor to resume from")

            // The outage.
            Darwin.shutdown(firstServer, SHUT_RDWR)
            let reconnected = await Self.waitUntil { factory.count == 2 }
            XCTAssertTrue(reconnected, "the store never reconnected")
            let relive = await Self.waitUntil {
                await MainActor.run { store.connectionState.isLive }
            }
            XCTAssertTrue(relive, "the second connection never came up")

            // The first page on the new socket, which is where the stream reopens.
            await store.refreshChatOnce()

            let subscribes = counts.transcriptSubscribes
            XCTAssertEqual(subscribes.count, 2, "the new connection must re-open the stream")
            let resumed = subscribes[1]["params"] as? [String: Any]
            XCTAssertEqual(
                (resumed?["after_order"] as? NSNumber)?.int64Value, 6,
                "the reconnect subscribed at the daemon's head, so every row committed during the outage is lost"
            )
        }
    }

    // MARK: - The Pal engine dial (PR E)

    /// The registry read that fills the engine picker, and the ONE thing this
    /// client refuses to guess.
    ///
    /// `fleet/adapter_list` names what the daemon COULD spawn. Nothing on the
    /// wire names what it IS running: the Pal channel carries no provider
    /// and the roster files an ACP session under the token `acp`. So a list
    /// that has answered must leave `engine` nil, and the header must say "not
    /// reported" rather than the first adapter's name, which is what the
    /// terminal client's dial shows and is a guess.
    func testTheAdapterListFillsThePickerWithoutClaimingWhichEngineIsRunning() async throws {
        let counts = ChatServerCounts(adapters: [
            Self.adapterObject(name: "claude-agent-acp", models: ["opus", "sonnet"]),
            Self.adapterObject(name: "house-adapter", builtIn: false),
        ])
        try await withChatStore(servePalDial: true, counts: counts) { store, _ in
            await MainActor.run { store.refreshAdaptersIfNeeded() }
            let listed = await Self.waitUntil {
                await MainActor.run { store.palDial.adaptersListed }
            }
            XCTAssertTrue(listed, "the registry read never answered")

            await MainActor.run {
                XCTAssertEqual(
                    store.palDial.adapters.map(\.name),
                    ["claude-agent-acp", "house-adapter"],
                    "the picker must offer the daemon's live registry, in its order"
                )
                XCTAssertNil(
                    store.palDial.engine,
                    "the registry says what CAN be spawned; defaulting to its first entry states a fact nobody sent"
                )
                XCTAssertEqual(FleetChatLabels.palEngine(store.palDial), "not reported")
                XCTAssertEqual(
                    store.palDial.models, [],
                    "with no engine known there is no adapter whose models these would be"
                )
            }
        }
    }

    /// A refresh with the socket down keeps the registry on screen and blames
    /// no daemon for it.
    ///
    /// Two absences that had been collapsed into one guard. "Not connected yet"
    /// says nothing about what the daemon serves, so this must say nothing
    /// either; only a LIVE negotiation missing `fleet.chat.read` is grounds for
    /// the "does not serve" sentence. Collapsed, the offline path emptied the
    /// picker and told the operator the daemon serves no registry, about a
    /// daemon that had answered with one seconds earlier.
    ///
    /// It is reached in the app because the chat pane's bootstrap sits on the
    /// outer stack, above its own `canReadChat` branch, so it runs while the
    /// socket is down; a reconnect marks the registry unread, and switching
    /// back to Chat before the socket comes up calls straight into this.
    func testAnOfflineRefreshKeepsTheRegistryAndBlamesNoDaemon() async throws {
        let counts = ChatServerCounts(adapters: [Self.adapterObject(name: "claude-agent-acp")])
        try await withChatStore(servePalDial: true, counts: counts) { store, server in
            await MainActor.run { store.refreshAdaptersIfNeeded() }
            let listed = await Self.waitUntil {
                await MainActor.run { store.palDial.adaptersListed }
            }
            XCTAssertTrue(listed, "the registry read never answered, so there is nothing to preserve")

            Darwin.shutdown(server, SHUT_RDWR)
            let offline = await Self.waitUntil {
                await MainActor.run { !store.connectionState.isLive }
            }
            XCTAssertTrue(offline, "the connection never dropped")

            await MainActor.run {
                store.refreshAdapters()
                XCTAssertEqual(
                    store.palDial.adapters.map(\.name), ["claude-agent-acp"],
                    "an offline refresh emptied the engine picker the last live one had filled"
                )
                XCTAssertNotEqual(
                    store.palDial.detail,
                    "This daemon does not serve the adapter registry.",
                    "a dropped socket is not a daemon that serves no registry, and it served one a moment ago"
                )
            }
        }
    }

    /// A LIVE daemon that does not advertise `fleet.chat.read` is the one case
    /// that MAY clear the registry and say so.
    ///
    /// The other half of the split above. Without this, an over-corrected guard
    /// that never cleared would leave a stale picker up against a daemon that
    /// genuinely cannot serve it.
    func testALiveDaemonWithoutTheChatCapabilityClearsTheRegistryAndSaysSo() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }
        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let serverDone = expectation(description: "capability-free server finished")
        let serverResult = SocketServerResult()

        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                // A live, writable connection whose catalogue names only the
                // action capability: no `fleet.chat.read` anywhere.
                try Self.serveStoreBootstrap(
                    descriptor: serverDescriptor,
                    subscriptionSnapshot: try Self.snapshotObject(head: 1, sessions: []),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 1, sessions: []),
                    capabilityIDs: ["fleet.action.execute"]
                )
                while true {
                    _ = try Self.readRequest(from: serverDescriptor)
                }
            } catch StoreServerError.closed {
                // The client hung up at the end of the test. Not a failure.
            } catch {
                serverResult.record(error)
            }
        }

        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: { _ in FleetConnection(location: location, injectedDescriptor: descriptors[0]) },
                reconnectDelayNanoseconds: { _ in 10_000_000_000 }
            )
        }
        await MainActor.run { store.start() }
        let live = await Self.waitUntil {
            await MainActor.run { store.connectionState.isLive }
        }
        XCTAssertTrue(live, "the fixture never reached a live connection")

        await MainActor.run {
            XCTAssertFalse(store.canReadAdapters, "the catalogue has no fleet.chat.read, so the registry is unreadable")
            store.refreshAdapters()
            XCTAssertEqual(
                store.palDial.adapters, [],
                "a daemon that cannot serve the registry must not leave one on screen"
            )
            XCTAssertFalse(store.palDial.adaptersListed)
            XCTAssertEqual(store.palDial.detail, "This daemon does not serve the adapter registry.")
            store.stop()
        }
        await fulfillment(of: [serverDone], timeout: 5)
        try serverResult.throwIfRecorded()
    }

    /// The swap, and the two pieces of client state it must take with it.
    ///
    /// `session_replaced` says the daemon retired the Pal session and
    /// minted a new one on the same channel scope, so this client is holding a
    /// dead key in TWO places. The mint cache would send the operator's next
    /// message to a session nobody is listening on. The carried transcript
    /// would paint the RETIRED adapter's execution under the new one's name.
    ///
    /// The transcript half is falsified rather than assumed: the fixture's
    /// chunks are emptied before the swap, so a row still on screen afterwards
    /// can only have been carried across the boundary.
    func testASwapThatReplacesTheSessionRetargetsThePaneAndDropsTheOldTranscript() async throws {
        let counts = ChatServerCounts(
            transcriptHeadOrder: 5,
            transcriptChunks: [Self.transcriptChunkObject(order: 5, text: "the retired adapter's work")],
            adapters: [
                Self.adapterObject(name: "claude-agent-acp"),
                Self.adapterObject(name: "house-adapter", builtIn: false),
            ]
        )
        try await withChatStore(servePalDial: true, counts: counts) { store, _ in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()
            await MainActor.run {
                XCTAssertEqual(store.chat.targetSessionKey, "acp:1")
                XCTAssertEqual(
                    store.chat.transcriptState.rows.map(\.body),
                    ["the retired adapter's work"],
                    "the swap needs a transcript on screen to be able to drop one"
                )
            }
            XCTAssertEqual(counts.acpCreates, 1)

            // What the daemon does on a provider swap: a new session key on the
            // same scope, and a transcript that no longer holds the old rows.
            counts.acpSessionKey = "acp:2"
            counts.transcriptChunks = []
            counts.palConfigureResult = [
                "session_key": "acp:2",
                "provider": "house-adapter",
                "copilot_mode": "guarded",
                "session_replaced": true,
                "persona_set": false,
            ]

            await MainActor.run { store.configurePal(provider: "house-adapter") }
            let retargeted = await Self.waitUntil {
                await MainActor.run { store.chat.targetSessionKey == "acp:2" }
            }
            XCTAssertTrue(
                retargeted,
                "the pane kept aiming at the retired session, so the next message goes nowhere"
            )

            await MainActor.run {
                XCTAssertEqual(store.palDial.engine, "house-adapter")
                XCTAssertEqual(store.palDial.mode, .guarded)
                // The replacement is asserted through what the OPERATOR is
                // shown, which is the only place this client reports it. There
                // is no separate flag to check, deliberately: nothing on screen
                // read one, so a field carrying it would have been state kept
                // alive by its own test.
                XCTAssertEqual(
                    store.palDial.detail,
                    "Engine set to house-adapter. The Pal session was replaced.",
                    "a swap that retired the conversation's session must say so on screen"
                )
                XCTAssertEqual(
                    store.chat.transcriptState.rows, [],
                    "the retired adapter's transcript was carried onto the new session's pane"
                )
            }
            XCTAssertEqual(
                counts.acpCreates, 2,
                "the mint cache still held the retired key, so no page asked the daemon for the live one"
            )
            let configures = counts.palConfigures
            XCTAssertEqual(configures.count, 1)
            let params = configures[0]["params"] as? [String: Any]
            XCTAssertEqual(params?["provider"] as? String, "house-adapter")
            XCTAssertNil(
                params?["copilot_mode"],
                "an engine pick must not also move the guardrail dial"
            )
            XCTAssertNil(params?["persona"], "this surface has no persona editor and must send none")
        }
    }

    /// A refused configure changes NOTHING this client displays.
    ///
    /// The daemon rolls its own dial back on a failure for a stated reason: the
    /// client adopts a mode only from a successful configure, so a `yolo` that
    /// survived a failure would be armed underneath a header still reading
    /// `guarded`. That contract only holds if this side keeps its half.
    func testARefusedConfigureLeavesTheDialOnTheLastSettingsThatLanded() async throws {
        let counts = ChatServerCounts(
            adapters: [
                Self.adapterObject(name: "claude-agent-acp"),
                Self.adapterObject(name: "house-adapter", builtIn: false),
            ],
            palConfigureResult: [
                "session_key": "acp:1",
                "provider": "claude-agent-acp",
                "copilot_mode": "help",
                "session_replaced": false,
                "model": "opus",
                "persona_set": false,
            ]
        )
        try await withChatStore(servePalDial: true, counts: counts) { store, _ in
            await MainActor.run { store.chatPaneAppeared() }
            await store.refreshChatOnce()

            // A configure that LANDS, so the dial holds something a failure
            // could plausibly move. Without this the assertions below cannot
            // tell "does not adopt on failure" from "resets on failure", and
            // both would pass against an untouched dial.
            await MainActor.run { store.configurePal(provider: "claude-agent-acp", mode: .help) }
            let landed = await Self.waitUntil {
                await MainActor.run { store.palDial.engine == "claude-agent-acp" }
            }
            XCTAssertTrue(landed, "the first configure never landed, so there is no state to preserve")
            await MainActor.run {
                XCTAssertEqual(store.palDial.mode, .help)
                XCTAssertEqual(store.palDial.model, "opus")
            }

            // Now one that fails, asking for a different engine and the
            // loosest guardrail: the two values a rollback bug would leak.
            counts.palConfigureResult = nil
            await MainActor.run { store.configurePal(provider: "house-adapter", mode: .yolo) }
            let refused = await Self.waitUntil {
                await MainActor.run { store.palDial.detail?.hasPrefix("Pal configure refused") == true }
            }
            XCTAssertTrue(refused, "a refused configure must say so")

            await MainActor.run {
                XCTAssertEqual(
                    store.palDial.engine, "claude-agent-acp",
                    "a failed swap must neither name the engine it asked for nor forget the one in force"
                )
                XCTAssertEqual(
                    store.palDial.mode, .help,
                    "adopting the requested mode from a failure arms a guardrail the daemon rolled back"
                )
                XCTAssertEqual(store.palDial.model, "opus", "a failed configure must not clear the model in force")
                XCTAssertEqual(store.chat.targetSessionKey, "acp:1", "nothing was replaced, so nothing is retargeted")
            }
            XCTAssertEqual(counts.acpCreates, 1, "a failed configure must not spend a re-mint")
        }
    }

    /// A daemon that serves the chat but not `fleet.copilot.configure` gets a
    /// dark dial rather than a picker whose every choice answers -32601.
    ///
    /// The two ids are checked by different daemon arms, so the registry can be
    /// readable while the write is not, and this asserts the client splits them
    /// the same way.
    func testTheDialIsGatedOnTheCapabilityItsOwnDaemonArmChecks() async throws {
        let counts = ChatServerCounts(adapters: [Self.adapterObject(name: "claude-agent-acp")])
        try await withChatStore(servePalDial: false, counts: counts) { store, _ in
            await MainActor.run {
                XCTAssertTrue(store.canReadAdapters, "fleet.chat.read is served, so the registry is readable")
                XCTAssertFalse(
                    store.canConfigurePal,
                    "fleet.copilot.configure is absent, so the picker must not be offered"
                )
                store.configurePal(provider: "claude-agent-acp")
                XCTAssertEqual(
                    store.palDial.detail,
                    "Configuring Pal is unavailable for this daemon."
                )
            }
            XCTAssertTrue(counts.palConfigures.isEmpty, "a gated-out dial must not reach the wire")
        }
    }

    /// One `FleetAdapter` in the shape the daemon frames it.
    ///
    /// `models` is omitted when empty, which is the ordinary shape: the Rust
    /// field is `#[serde(default)]` and ACP has no model-discovery call, so an
    /// adapter that declares none simply has no key here.
    private static func adapterObject(
        name: String,
        builtIn: Bool = true,
        models: [String] = []
    ) -> [String: Any] {
        var object: [String: Any] = [
            "name": name,
            "command": "/usr/local/bin/\(name)",
            "permission_mode": "default",
            "built_in": builtIn,
        ]
        if !models.isEmpty {
            object["models"] = models
        }
        return object
    }

    /// Bring a store up against TWO scripted chat connections, so a test can
    /// drop the first and watch what the second asks for.
    ///
    /// Both legs are served by the same `serveChatConnection`, and both share
    /// one `counts`, which is what lets a test compare the frames the two
    /// connections sent. A second copy of the bootstrap is how the two legs of
    /// a reconnect test start disagreeing about the wire.
    private func withReconnectingChatStore(
        counts: ChatServerCounts,
        _ body: (FleetStore, Int32, TestConnectionFactory) async throws -> Void
    ) async throws {
        var first = [Int32](repeating: 0, count: 2)
        var second = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &first), 0)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &second), 0)
        let firstServer = first[1]
        let secondServer = second[1]
        defer {
            Darwin.close(firstServer)
            Darwin.close(secondServer)
        }
        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let factory = TestConnectionFactory(descriptors: [first[0], second[0]], location: location)
        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: factory.make,
                reconnectDelayNanoseconds: { _ in 0 }
            )
        }

        let serversDone = expectation(description: "both chat servers finished")
        serversDone.expectedFulfillmentCount = 2
        let serverResult = SocketServerResult()
        for descriptor in [firstServer, secondServer] {
            DispatchQueue.global().async {
                defer { serversDone.fulfill() }
                do {
                    try Self.serveChatConnection(descriptor: descriptor, counts: counts)
                } catch StoreServerError.closed {
                    // The client hung up. Expected on both legs.
                } catch {
                    serverResult.record(error)
                }
            }
        }

        await MainActor.run { store.start() }
        let live = await Self.waitUntil {
            await MainActor.run { store.connectionState.isLive && store.canWrite }
        }
        XCTAssertTrue(live, "the fixture never reached a live connection")

        try await body(store, firstServer, factory)

        await MainActor.run { store.stop() }
        await fulfillment(of: [serversDone], timeout: 5)
        try serverResult.throwIfRecorded()
    }

    /// Bootstrap one scripted chat connection and serve it until it closes.
    ///
    /// Extracted so a test with TWO connections drives both the same way
    /// `withChatStore` drives one; a second copy of the bootstrap is how the
    /// two legs of a reconnect test start disagreeing about the wire.
    private static func serveChatConnection(descriptor: Int32, counts: ChatServerCounts) throws {
        try serveStoreBootstrap(
            descriptor: descriptor,
            subscriptionSnapshot: try snapshotObject(head: 1, sessions: []),
            eventBeforeSubscriptionResponse: false,
            snapshotAfterEvent: try snapshotObject(head: 1, sessions: []),
            capabilityIDs: [
                "fleet.chat.read", "fleet.chat.write",
                "fleet.message.read", "fleet.message.send", "fleet.acp.spawn",
                "fleet.transcript.read",
            ]
        )
        while true {
            try answerChatRequest(try readRequest(from: descriptor), to: descriptor, counts: counts)
        }
    }

    /// A daemon that does not advertise `fleet.transcript.read` costs the pane
    /// its transcript and NOTHING else, with the absence explained.
    ///
    /// Gated separately from the conversation for exactly this: a daemon built
    /// between phases serves the chat while answering -32601 here, and a
    /// combined gate would either hide a working conversation or open a stream
    /// that errors on every page.
    func testADaemonWithoutTheTranscriptCapabilityExplainsTheAbsence() async throws {
        let counts = ChatServerCounts(transcriptHeadOrder: 5)
        try await withChatStore(serveTranscript: false, counts: counts) { store, _ in
            await store.refreshChatOnce()

            await MainActor.run {
                XCTAssertFalse(store.canReadTranscript)
                XCTAssertTrue(store.canReadChat, "the conversation must survive a missing transcript capability")
                XCTAssertEqual(store.chat.scopeKey, "channel:c1", "and the page still published")
                XCTAssertNotNil(store.chat.transcriptDetail, "an unreadable transcript must say so")
                XCTAssertEqual(store.chat.transcriptState.rows, [])
            }
            XCTAssertEqual(counts.transcriptSubscribes.count, 0, "a capability this daemon lacks must not be called")
            XCTAssertEqual(counts.transcriptLists.count, 0)
        }
    }

    /// One transcript chunk in the shape the daemon frames it.
    private static func transcriptChunkObject(
        order: Int64,
        sessionKey: String = "acp:1",
        eventType: String = "acp.message",
        text: String = "hello",
        payload: [String: Any]? = nil
    ) -> [String: Any] {
        [
            "ingest_order": order,
            "event_id": "evt-\(order)",
            "session_key": sessionKey,
            "event_type": eventType,
            "payload": payload ?? ["text": text],
            "observed_at": 1_700_000_000_000,
        ]
    }

    /// Bring a store up against the scripted chat daemon and hand both it and
    /// the SERVER end of the socket to `body`.
    ///
    /// Handing over the raw descriptor is what lets a test push a notification.
    /// It is safe because the server thread is blocked reading (or held at the
    /// gate) whenever `body` runs, so there is never a second writer.
    /// `counts` is passed IN rather than handed back, so a test that needs to
    /// assert on what the wire was asked for holds the same object the
    /// scripted daemon is writing to.
    private func withChatStore(
        refuseMessageSubscribe: Bool = false,
        gate: ChatServerGate? = nil,
        pagesReturnMarkerRows: Bool = false,
        serveTranscript: Bool = true,
        refuseAcpSessionCreate: Bool = false,
        servePalDial: Bool = false,
        counts providedCounts: ChatServerCounts? = nil,
        _ body: (FleetStore, Int32) async throws -> Void
    ) async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }
        let location = try Self.testLocation()
        defer { try? FileManager.default.removeItem(at: location.home) }
        let serverDone = expectation(description: "chat server finished")
        let serverResult = SocketServerResult()
        let counts = providedCounts ?? ChatServerCounts(pagesReturnMarkerRows: pagesReturnMarkerRows)

        DispatchQueue.global().async {
            defer { serverDone.fulfill() }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: serverDescriptor,
                    subscriptionSnapshot: try Self.snapshotObject(head: 1, sessions: []),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 1, sessions: []),
                    capabilityIDs: [
                        "fleet.chat.read", "fleet.chat.write",
                        "fleet.message.read", "fleet.message.send", "fleet.acp.spawn",
                    ]
                        + (serveTranscript ? ["fleet.transcript.read"] : [])
                        // Opt-in, so a daemon that serves the chat but not the
                        // dial stays the DEFAULT shape every other test here
                        // runs against. `fleet.copilot.configure` is its own id
                        // on the daemon side and this fixture keeps it separate
                        // for the same reason.
                        + (servePalDial ? ["fleet.copilot.configure"] : []),
                    refuseMessageSubscribe: refuseMessageSubscribe
                )
                while true {
                    try Self.answerChatRequest(
                        try Self.readRequest(from: serverDescriptor),
                        to: serverDescriptor,
                        counts: counts,
                        gate: gate,
                        refuseAcpSessionCreate: refuseAcpSessionCreate
                    )
                }
            } catch StoreServerError.closed {
                // The client hung up at the end of the test. Not a failure.
            } catch {
                serverResult.record(error)
            }
        }

        let store = await MainActor.run {
            FleetStore(
                location: location,
                makeConnection: { _ in FleetConnection(location: location, injectedDescriptor: descriptors[0]) },
                reconnectDelayNanoseconds: { _ in 10_000_000_000 }
            )
        }
        await MainActor.run { store.start() }
        let live = await Self.waitUntil {
            await MainActor.run {
                guard case .live = store.connectionState else { return false }
                return store.canWrite
            }
        }
        XCTAssertTrue(live, "the fixture never reached a live, writable connection")

        try await body(store, serverDescriptor)

        await MainActor.run { store.stop() }
        await fulfillment(of: [serverDone], timeout: 5)
        try serverResult.throwIfRecorded()
    }

    /// One `FleetMessage` in the shape the daemon frames it.
    private static func messageObject(id: String, scope: String) -> [String: Any] {
        [
            "id": id, "scope_key": scope, "sender": "copilot",
            "kind": "agent", "body": "from Pal", "created_at": 1_700_000_000_000,
        ]
    }

    /// Answer one chat-page RPC with a canned result, counting the mints.
    ///
    /// Dispatched by METHOD rather than by position, so the test asserts how
    /// many times the store asked rather than pinning an exact call order that
    /// a later page change would have to rewrite.
    private static func answerChatRequest(
        _ request: [String: Any],
        to descriptor: Int32,
        counts: ChatServerCounts,
        gate: ChatServerGate? = nil,
        refuseAcpSessionCreate: Bool = false
    ) throws {
        switch request["method"] as? String {
        case "fleet/channel_list":
            try writeResponse(to: descriptor, request: request, result: ["channels": [[
                "id": "c1",
                "kind": "copilot",
                "name": "copilot",
                "scope_key": "channel:c1",
                "recipients": [],
                "created_at": 1,
            ]]])
        case "fleet/acp_session_create":
            counts.recordAcpCreate()
            // A refused mint leaves the surface with no target at all, because
            // a Pal channel carries no recipient list to fall back on.
            if refuseAcpSessionCreate {
                try writeError(to: descriptor, request: request)
                return
            }
            try writeResponse(to: descriptor, request: request, result: [
                "session_key": counts.acpSessionKey,
                "scope_key": "channel:c1",
                "turn_deadline_ms": 1_800_000,
            ])
        case "fleet/adapter_list":
            try writeResponse(to: descriptor, request: request, result: ["adapters": counts.adapters])
        case "fleet/copilot_configure":
            guard let result = counts.recordPalConfigure(request) else {
                try writeError(to: descriptor, request: request)
                return
            }
            try writeResponse(to: descriptor, request: request, result: result)
        case "fleet/message_list":
            // The page's first call AFTER the mint decision, which makes it the
            // window a test needs to invalidate the cache in.
            let page = counts.recordMessageList()
            gate?.pauseIfArmed()
            // Marker rows are opt-in, because most tests here assert on the
            // mint count and want an empty timeline. A test that needs to tell
            // WHICH page published turns them on: the row names its page.
            let rows: [Any] = counts.pagesReturnMarkerRows
                ? [messageObject(id: "page-\(page)", scope: "channel:c1")]
                : []
            try writeResponse(to: descriptor, request: request, result: ["messages": rows])
        case "fleet/confirm_list":
            try writeResponse(to: descriptor, request: request, result: ["confirms": []])
        case "fleet/activity_list":
            try writeResponse(to: descriptor, request: request, result: ["activities": []])
        case "fleet/transcript_subscribe":
            // The head the page's window is measured back from, and the
            // session it was asked for, both recorded: the ack is the only
            // place the wire says WHICH session's stream the store opened.
            if counts.recordTranscriptSubscribe(request) {
                try writeError(to: descriptor, request: request)
                return
            }
            try writeResponse(
                to: descriptor,
                request: request,
                result: ["head_order": counts.transcriptHeadOrder as Any]
            )
        case "fleet/transcript_list":
            counts.recordTranscriptList(request)
            try writeResponse(to: descriptor, request: request, result: [
                "chunks": counts.transcriptChunks,
                "next_after_order": counts.transcriptChunks.isEmpty
                    ? NSNull()
                    : counts.transcriptHeadOrder as Any,
                "truncated": counts.transcriptTruncated,
            ])
        case "fleet/message_send":
            try writeResponse(to: descriptor, request: request, result: [
                "message_id": "01J0MSG",
                "deliveries": [[
                    "session_key": "acp:1",
                    "state": "REJECTED",
                    "detail": counts.rejectionDetail,
                ]],
            ])
        default:
            try writeError(to: descriptor, request: request)
        }
    }

    /// The three chat notifications reach `incoming()` as their own cases, on
    /// the same socket that already carries `fleet/event`.
    ///
    /// The frames are written inline rather than from the shared fixtures
    /// because what is under test here is the DISPATCH: which method name lands
    /// in which case. The payload shapes are pinned against the shared fixtures
    /// by `FleetChatPresentationTests` and the daemon contract suite, which is
    /// where a renamed wire key has to fail.
    func testChatNotificationsStreamThroughOwnedSocket() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }

        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: descriptors[0]
        )
        try await connection.connect()
        defer { Task { await connection.close() } }
        let stream = await connection.incoming()
        let waiter = Task { () -> [FleetIncoming] in
            var received: [FleetIncoming] = []
            var iterator = stream.makeAsyncIterator()
            while received.count < 3, let incoming = await iterator.next() {
                received.append(incoming)
            }
            return received
        }

        try Self.writeNotification(
            to: serverDescriptor,
            method: "fleet/message_event",
            params: ["message": [
                "id": "01J0MSG", "scope_key": "channel:copilot", "sender": "copilot",
                "kind": "agent", "body": "restarted sess-a", "created_at": 1_700_000_000_000,
            ]]
        )
        try Self.writeNotification(
            to: serverDescriptor,
            method: "fleet/confirm_event",
            params: ["confirm": [
                "confirm_id": "01J0CONFIRM", "scope_key": "channel:copilot", "tool": "kill",
                "arguments": ["session": "tmux:sess-c"], "state": "open",
                "created_at": 1, "expires_at": 2,
            ]]
        )
        try Self.writeNotification(
            to: serverDescriptor,
            method: "fleet/activity_event",
            params: ["activity": [
                "seq": 44, "id": "01J0ACTIVITY3", "scope_key": "channel:copilot",
                "tool": "fleet_status", "class": "read", "outcome": "ok", "created_at": 3,
            ]]
        )

        let received = await waiter.value
        XCTAssertEqual(received.count, 3)
        guard case let .messageEvent(message) = received[0],
              case let .confirmEvent(confirm) = received[1],
              case let .activityEvent(activity) = received[2] else {
            return XCTFail("chat notifications landed in the wrong cases: \(received)")
        }
        XCTAssertEqual(message.message.id, "01J0MSG")
        XCTAssertEqual(message.message.scopeKey, "channel:copilot")
        // The card arrives RAW, so the store can put it through the same
        // tolerant decode the paged list uses.
        XCTAssertEqual(confirm.confirm.value("confirm_id")?.stringValue, "01J0CONFIRM")
        XCTAssertEqual(FleetChatConfirmCard.decode(confirm.confirm).id, "01J0CONFIRM")
        XCTAssertTrue(FleetChatConfirmCard.decode(confirm.confirm).isAnswerable)
        XCTAssertEqual(activity.activity.seq, 44)
        XCTAssertEqual(activity.activity.activityClass, .read)
    }

    /// A method this build has never heard of degrades to
    /// `.unknownNotification` instead of tearing the connection down.
    ///
    /// The arm that does it is the one `default` this file keeps, and it is
    /// load-bearing: the daemon adds notifications, and one aimed at some other
    /// client's subscription must not cost this one its socket.
    func testAnUnknownNotificationDoesNotTearDownTheConnection() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }

        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: descriptors[0]
        )
        try await connection.connect()
        defer { Task { await connection.close() } }
        let stream = await connection.incoming()
        let waiter = Task { () -> FleetIncoming? in
            var iterator = stream.makeAsyncIterator()
            return await iterator.next()
        }

        try Self.writeNotification(to: serverDescriptor, method: "fleet/some_future_event", params: ["what": 1])

        let received = await waiter.value
        XCTAssertEqual(received, .unknownNotification("fleet/some_future_event"))
    }

    /// A chat notification whose PARAMS do not decode costs the FRAME, never
    /// the connection.
    ///
    /// The opposite answer from `fleet/event`, deliberately. That one is this
    /// client's own roster stream, where a frame it cannot read means the two
    /// ends disagree about the wire. The three chat frames are fleet-wide
    /// broadcasts carrying every conversation on the daemon, so one row from a
    /// scope this pane never renders would otherwise take the roster and the
    /// menu bar down with it. The frame is named and dropped; the safety-net
    /// page repairs anything it carried.
    func testAMalformedChatNotificationDoesNotCostTheConnection() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }

        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: descriptors[0]
        )
        try await connection.connect()
        defer { Task { await connection.close() } }
        let stream = await connection.incoming()
        let waiter = Task { () -> [FleetIncoming] in
            var received: [FleetIncoming] = []
            var iterator = stream.makeAsyncIterator()
            while received.count < 2, let incoming = await iterator.next() {
                received.append(incoming)
            }
            return received
        }

        // A `fleet/message_event` with no `message` key at all, followed by a
        // perfectly good one. The second is the proof: it can only arrive if
        // the first did not take the socket with it.
        try Self.writeNotification(to: serverDescriptor, method: "fleet/message_event", params: ["nope": 1])
        try Self.writeNotification(
            to: serverDescriptor,
            method: "fleet/message_event",
            params: ["message": Self.messageObject(id: "01J0AFTER", scope: "channel:c1")]
        )

        let received = await waiter.value
        XCTAssertEqual(received.count, 2)
        XCTAssertEqual(
            received.first, .unknownNotification("fleet/message_event"),
            "an undecodable chat frame must be named and dropped, not silently swallowed"
        )
        guard case let .messageEvent(params) = received[1] else {
            return XCTFail("the connection did not survive the undecodable frame: \(received)")
        }
        XCTAssertEqual(params.message.id, "01J0AFTER")
    }

    /// The roster stream keeps the OPPOSITE rule: a `fleet/event` this build
    /// cannot read still closes the connection.
    ///
    /// Pinned next to its neighbour on purpose. The two arms answer a malformed
    /// frame differently, and the difference is a decision about blast radius,
    /// so a later change that made them agree should have to delete one of
    /// these tests and say why.
    func testAMalformedRosterEventStillClosesTheConnection() async throws {
        var descriptors = [Int32](repeating: 0, count: 2)
        XCTAssertEqual(Darwin.socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors), 0)
        let serverDescriptor = descriptors[1]
        defer { Darwin.close(serverDescriptor) }

        let connection = FleetConnection(
            location: HangarLocation(environment: ["AINB_HANGAR_HOME": "/unused"]),
            injectedDescriptor: descriptors[0]
        )
        try await connection.connect()
        defer { Task { await connection.close() } }
        let stream = await connection.incoming()
        let finished = Task { () -> [FleetIncoming] in
            var received: [FleetIncoming] = []
            for await incoming in stream { received.append(incoming) }
            return received
        }

        try Self.writeNotification(to: serverDescriptor, method: "fleet/event", params: ["nope": 1])

        let yielded = await finished.value
        XCTAssertEqual(yielded, [], "a roster frame that does not decode must not be yielded as anything")
    }

    private static func testLocation() throws -> HangarLocation {
        let home = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let tokenDirectory = home.appendingPathComponent("hangar", isDirectory: true)
        try FileManager.default.createDirectory(at: tokenDirectory, withIntermediateDirectories: true)
        try Data("mdt_test".utf8).write(to: tokenDirectory.appendingPathComponent("daemon.token"))
        return HangarLocation(environment: ["AINB_HANGAR_HOME": home.path])
    }

    private static func serveStoreBootstrap(
        descriptor: Int32,
        subscriptionSnapshot: Any,
        eventBeforeSubscriptionResponse: Bool,
        snapshotAfterEvent: Any?,
        subscriptionReplay: [Any] = [],
        replayState: [String: Any] = ["state": "complete"],
        resyncAfterSubscription: Bool = false,
        capabilityIDs: [String] = [],
        refuseMessageSubscribe: Bool = false
    ) throws {
        let authentication = try readRequest(from: descriptor)
        try writeResponse(to: descriptor, request: authentication, result: [:])

        let negotiation = try readRequest(from: descriptor)
        try writeResponse(to: descriptor, request: negotiation, result: [
            "daemon_version": "fixture-daemon",
            "protocol_version": 1,
            "read_compatible": true,
            "write_compatible": true,
            "capability_ids": capabilityIDs,
        ])

        let subscription = try readRequest(from: descriptor)
        let event = try eventObject(revision: 1)
        if eventBeforeSubscriptionResponse {
            try writeNotification(to: descriptor, method: "fleet/event", params: event)
        }
        try writeResponse(to: descriptor, request: subscription, result: [
            "snapshot": subscriptionSnapshot,
            "replay": subscriptionReplay,
            "replay_state": replayState,
        ])
        // The live chat stream, opened with the connection. Asserting the METHOD
        // here is what proves the store opened it: a store that skipped the call
        // would leave this read waiting for its next request instead, and the
        // method it eventually got would not be this one.
        if capabilityIDs.contains("fleet.message.read") {
            let messageSubscribe = try readRequest(from: descriptor)
            XCTAssertEqual(messageSubscribe["method"] as? String, "fleet/message_subscribe")
            if refuseMessageSubscribe {
                try writeError(to: descriptor, request: messageSubscribe)
            } else {
                try writeResponse(to: descriptor, request: messageSubscribe, result: ["head_id": NSNull()])
            }
        }
        if resyncAfterSubscription {
            try writeNotification(
                to: descriptor,
                method: "fleet/resync_required",
                params: ["after_revision": 0, "missed": 1]
            )
        }
        if eventBeforeSubscriptionResponse {
            let snapshot = try readRequest(from: descriptor)
            XCTAssertEqual(snapshot["method"] as? String, "fleet/snapshot")
            try writeResponse(to: descriptor, request: snapshot, result: snapshotAfterEvent ?? subscriptionSnapshot)
        } else if snapshotAfterEvent == nil {
            try writeNotification(to: descriptor, method: "fleet/event", params: event)
            _ = try readRequest(from: descriptor)
        }
    }

    private static func readRequest(from descriptor: Int32) throws -> [String: Any] {
        guard let frame = readFrame(from: descriptor),
              let request = try JSONSerialization.jsonObject(with: frame) as? [String: Any] else {
            throw StoreServerError.closed
        }
        return request
    }

    private static func writeResponse(to descriptor: Int32, request: [String: Any], result: Any) throws {
        try writeJSONObject(["jsonrpc": "2.0", "id": request["id"] as Any, "result": result], to: descriptor)
    }

    private static func writeError(to descriptor: Int32, request: [String: Any]) throws {
        try writeJSONObject([
            "jsonrpc": "2.0",
            "id": request["id"] as Any,
            "error": ["code": -32603, "message": "fixture failure"],
        ], to: descriptor)
    }

    private static func writeNotification(to descriptor: Int32, method: String, params: Any) throws {
        try writeJSONObject(["jsonrpc": "2.0", "method": method, "params": params], to: descriptor)
    }

    private static func writeJSONObject(_ object: [String: Any], to descriptor: Int32) throws {
        let frame = try ContentLengthEncoder.encode(JSONSerialization.data(withJSONObject: object))
        let written = frame.withUnsafeBytes { Darwin.write(descriptor, $0.baseAddress, frame.count) }
        guard written == frame.count else { throw StoreServerError.closed }
    }

    private static func snapshotObject(head: Int64, sessions: [Any]) throws -> Any {
        ["head_revision": head, "sessions": sessions]
    }

    private static func sampleSessionObject(head: Int64) throws -> Any {
        let object = try JSONSerialization.jsonObject(with: FleetWire.encoder().encode(sampleSnapshot(head: head)))
        guard let snapshot = object as? [String: Any], let session = (snapshot["sessions"] as? [Any])?.first else {
            throw StoreServerError.closed
        }
        return session
    }

    private static func eventObject(revision: Int64) throws -> Any {
        try JSONSerialization.jsonObject(with: FleetWire.encoder().encode(sampleEvent(revision: revision, eventID: "event-\(revision)")))
    }

    private static func waitUntil(_ condition: @escaping () async -> Bool) async -> Bool {
        let deadline = Date().addingTimeInterval(2)
        while Date() < deadline {
            if await condition() { return true }
            try? await Task.sleep(for: .milliseconds(20))
        }
        return false
    }
}

private final class TestConnectionFactory: @unchecked Sendable {
    private let lock = NSLock()
    private var descriptors: [Int32]
    private let initialCount: Int
    private let location: HangarLocation

    init(descriptors: [Int32], location: HangarLocation) {
        self.descriptors = descriptors
        initialCount = descriptors.count
        self.location = location
    }

    var count: Int {
        lock.lock()
        defer { lock.unlock() }
        return initialCount - descriptors.count
    }

    func make(_: HangarLocation) -> FleetConnection {
        lock.lock()
        defer { lock.unlock() }
        return FleetConnection(location: location, injectedDescriptor: descriptors.removeFirst())
    }
}

private enum StoreServerError: Error {
    case closed
}

private final class SocketServerResult: @unchecked Sendable {
    private let lock = NSLock()
    private var error: Error?

    func record(_ error: Error) {
        lock.lock()
        self.error = error
        lock.unlock()
    }

    func throwIfRecorded() throws {
        lock.lock()
        defer { lock.unlock() }
        if let error { throw error }
    }
}

/// How many times the scripted chat daemon was asked for each thing.
///
/// The mint count is the assertion Pal cache exists to make: it is a
/// WRITE transaction daemon-side, so "how often was it asked" is the whole
/// question, and only the wire can answer it.
private final class ChatServerCounts: @unchecked Sendable {
    private let lock = NSLock()
    private var acpCreateCount = 0
    private var messageListCount = 0
    /// The daemon token the scripted `message_send` refuses with. Settable
    /// because whether a refusal forgets the mint depends entirely on WHICH
    /// token it carries, not on the REJECTED state.
    let rejectionDetail: String
    /// Whether each page answers with a row naming itself, so a test can tell
    /// which page's surface reached the screen.
    let pagesReturnMarkerRows: Bool

    /// The head order this fixture's transcript reports, or nil for an empty
    /// one. The store measures its page window back from this.
    let transcriptHeadOrder: Int64?
    /// The chunks `fleet/transcript_list` answers with.
    ///
    /// SETTABLE mid-test, so a test can prove the difference between rows the
    /// store CARRIED and rows a page re-read: empty the fixture, page again,
    /// and whatever is still on screen was carried.
    var transcriptChunks: [[String: Any]] {
        get {
            lock.lock()
            defer { lock.unlock() }
            return scriptedTranscriptChunks
        }
        set {
            lock.lock()
            scriptedTranscriptChunks = newValue
            lock.unlock()
        }
    }
    private var scriptedTranscriptChunks: [[String: Any]]
    /// The session key `fleet/acp_session_create` mints, settable so a test can
    /// script the REPLACEMENT a Pal engine swap mints on the same scope.
    var acpSessionKey: String {
        get {
            lock.lock()
            defer { lock.unlock() }
            return scriptedAcpSessionKey
        }
        set {
            lock.lock()
            scriptedAcpSessionKey = newValue
            lock.unlock()
        }
    }
    private var scriptedAcpSessionKey = "acp:1"
    /// Every `fleet/copilot_configure` frame the store sent.
    private var palConfigureRequests: [[String: Any]] = []
    /// What the scripted `fleet/copilot_configure` answers with, or nil to
    /// refuse. Settable because the two outcomes a client must tell apart, a
    /// same-adapter configure and a swap, differ only in this result.
    var palConfigureResult: [String: Any]? {
        get {
            lock.lock()
            defer { lock.unlock() }
            return scriptedPalConfigureResult
        }
        set {
            lock.lock()
            scriptedPalConfigureResult = newValue
            lock.unlock()
        }
    }
    private var scriptedPalConfigureResult: [String: Any]?
    /// The adapters `fleet/adapter_list` answers with. Fixed at construction,
    /// which is before the server thread exists, so it needs no lock.
    let adapters: [[String: Any]]

    /// Record the configure and answer with whatever is scripted, or refuse.
    func recordPalConfigure(_ request: [String: Any]) -> [String: Any]? {
        lock.lock()
        defer { lock.unlock() }
        palConfigureRequests.append(request)
        return scriptedPalConfigureResult
    }

    var palConfigures: [[String: Any]] {
        lock.lock()
        defer { lock.unlock() }
        return palConfigureRequests
    }
    /// Whether the scripted tail read says it left older rows behind.
    let transcriptTruncated: Bool
    /// Which `fleet/transcript_subscribe` calls are refused, counted from one.
    ///
    /// By INDEX rather than "the first N", because the failure that pinned the
    /// banner needs an opening subscribe that SUCCEEDS (so a cursor exists to
    /// be carried) and a later one that does not.
    private var transcriptSubscribeRequests: [[String: Any]] = []
    private var transcriptListRequests: [[String: Any]] = []

    init(
        rejectionDetail: String = "target_not_running",
        pagesReturnMarkerRows: Bool = false,
        transcriptHeadOrder: Int64? = nil,
        transcriptChunks: [[String: Any]] = [],
        transcriptTruncated: Bool = false,
        refusedTranscriptSubscribes: Set<Int> = [],
        adapters: [[String: Any]] = [],
        palConfigureResult: [String: Any]? = nil
    ) {
        self.refusedTranscriptSubscribes = refusedTranscriptSubscribes
        self.rejectionDetail = rejectionDetail
        self.pagesReturnMarkerRows = pagesReturnMarkerRows
        self.transcriptHeadOrder = transcriptHeadOrder
        self.scriptedTranscriptChunks = transcriptChunks
        self.transcriptTruncated = transcriptTruncated
        self.adapters = adapters
        self.scriptedPalConfigureResult = palConfigureResult
    }

    let refusedTranscriptSubscribes: Set<Int>

    /// Record the call and say whether this one is refused.
    func recordTranscriptSubscribe(_ request: [String: Any]) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        transcriptSubscribeRequests.append(request)
        return refusedTranscriptSubscribes.contains(transcriptSubscribeRequests.count)
    }

    func recordTranscriptList(_ request: [String: Any]) {
        lock.lock()
        transcriptListRequests.append(request)
        lock.unlock()
    }

    var transcriptSubscribes: [[String: Any]] {
        lock.lock()
        defer { lock.unlock() }
        return transcriptSubscribeRequests
    }

    var transcriptLists: [[String: Any]] {
        lock.lock()
        defer { lock.unlock() }
        return transcriptListRequests
    }

    /// Count this page's timeline read and answer with its ordinal.
    func recordMessageList() -> Int {
        lock.lock()
        defer { lock.unlock() }
        messageListCount += 1
        return messageListCount
    }

    var acpCreates: Int {
        lock.lock()
        defer { lock.unlock() }
        return acpCreateCount
    }

    func recordAcpCreate() {
        lock.lock()
        acpCreateCount += 1
        lock.unlock()
    }
}

/// Lets a test stop the scripted chat daemon in the middle of one page.
///
/// One-shot, and armed per page rather than always on: a page that could not
/// finish would hang the poll loop for every other assertion in the file. The
/// reached flag is polled rather than waited on so the test never blocks a
/// cooperative thread; only the server's own dispatch queue blocks, which is
/// what "the daemon is slow" means here.
private final class ChatServerGate: @unchecked Sendable {
    private let lock = NSLock()
    private var armed = false
    private var reached = false
    private let release = DispatchSemaphore(value: 0)

    var hasReached: Bool {
        lock.lock()
        defer { lock.unlock() }
        return reached
    }

    func arm() {
        lock.lock()
        armed = true
        reached = false
        lock.unlock()
    }

    func letGo() {
        release.signal()
    }

    /// Called on the server thread. Blocks that thread, once, if armed.
    func pauseIfArmed() {
        lock.lock()
        let isArmed = armed
        armed = false
        if isArmed { reached = true }
        lock.unlock()
        guard isArmed else { return }
        // Bounded: a test that never releases fails on its own assertions
        // rather than hanging the suite.
        _ = release.wait(timeout: .now() + 5)
    }
}
