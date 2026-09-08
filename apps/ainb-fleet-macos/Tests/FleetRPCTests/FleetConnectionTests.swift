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

    func testOldCatalogueRefusesNewReadCapabilitiesAndStart() {
        let oldCatalogue = FleetNegotiateResult(
            daemonVersion: "fixture-daemon-0.9.0",
            protocolVersion: 1,
            readCompatible: true,
            writeCompatible: true,
            capabilityIDs: ["fleet.snapshot.read", "fleet.action.execute"]
        )

        XCTAssertThrowsError(try FleetConnection.validateCapability("fleet.receipt.read", in: oldCatalogue)) {
            XCTAssertEqual($0 as? FleetConnectionError, .missingNegotiatedCapability("fleet.receipt.read"))
        }
        XCTAssertThrowsError(try FleetConnection.validateCapability("fleet.start.execute", in: oldCatalogue)) {
            XCTAssertEqual($0 as? FleetConnectionError, .missingNegotiatedCapability("fleet.start.execute"))
        }
        XCTAssertThrowsError(try FleetConnection.validateCapability("fleet.runtime.read", in: oldCatalogue)) {
            XCTAssertEqual($0 as? FleetConnectionError, .missingNegotiatedCapability("fleet.runtime.read"))
        }
        XCTAssertThrowsError(try FleetConnection.validateCapability("fleet.usage.read", in: oldCatalogue)) {
            XCTAssertEqual($0 as? FleetConnectionError, .missingNegotiatedCapability("fleet.usage.read"))
        }
    }

    func testATCReadProjectionDecodesOwnershipAndScheduleFacts() throws {
        let result = try FleetWire.decoder().decode(AtcListResult.self, from: Data(#"""
        {"instances":[{"name":"main","cwd":"/tmp","tmux_session":"atc-main","heartbeat_cron":"*/2 * * * *","err_retry_cap":3,"idle_pause_min":60,"next_tick_at":2000,"enabled":true,"last_heartbeat_at":1000,"config_generation":4}],"scheduler_ownership":"legacy_timer_reconciliation_required"}
        """#.utf8))

        XCTAssertEqual(result.instances.map(\.name), ["main"])
        XCTAssertEqual(result.instances.first?.configGeneration, 4)
        XCTAssertEqual(result.schedulerOwnership, .legacyTimerReconciliationRequired)
    }

    func testOldCatalogueRefusesReceiptAndStartBeforeWireIO() async throws {
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
            _ = try await connection.receiptList(FleetReceiptListParams(limit: 1))
            XCTFail("old catalogue must refuse receipt reads")
        } catch let error as FleetConnectionError {
            XCTAssertEqual(error, .missingNegotiatedCapability("fleet.receipt.read"))
        }
        do {
            _ = try await connection.start(FleetStartParams(requestID: "request", provider: .codex, cwd: "/tmp", prompt: nil))
            XCTFail("old catalogue must refuse fleet/start")
        } catch let error as FleetConnectionError {
            XCTAssertEqual(error, .missingNegotiatedCapability("fleet.start.execute"))
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

    func testTmuxTextCapabilityAllowsPromptAndBroadcastTarget() {
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

    @MainActor
    func testBroadcastReceiptMergePreservesDaemonInputOrder() {
        let existing = [receipt("old")]
        let daemonOrder = [receipt("first"), receipt("second")]

        XCTAssertEqual(FleetStore.mergedReceipts(daemonOrder, existing: existing).map(\.requestID), ["first", "second", "old"])
    }

    func testStartCWDPreflightRequiresExistingDirectory() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let file = root.appendingPathComponent("file")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data().write(to: file)

        XCTAssertTrue(FleetStartPreflight.isExistingDirectory(root.path))
        XCTAssertFalse(FleetStartPreflight.isExistingDirectory(file.path))
        XCTAssertFalse(FleetStartPreflight.isExistingDirectory(root.appendingPathComponent("missing").path))
    }

    func testStartPreflightSupportsOnlyCodex() {
        XCTAssertTrue(FleetStartPreflight.supports(.codex))
        XCTAssertFalse(FleetStartPreflight.supports(.claude))
        XCTAssertFalse(FleetStartPreflight.supports(.antigravity))
        XCTAssertFalse(FleetStartPreflight.supports(.unknown))
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

    func testReconnectReloadsReceiptsBeforeReturningLive() async throws {
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
        let firstDone = expectation(description: "first receipt server finished")
        let secondReady = expectation(description: "second receipt server ready")
        let releaseSecond = DispatchSemaphore(value: 0)
        let serverResult = SocketServerResult()

        DispatchQueue.global().async {
            defer { firstDone.fulfill() }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: firstServer,
                    subscriptionSnapshot: try Self.snapshotObject(head: 1, sessions: [try Self.sampleSessionObject(head: 1)]),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 1, sessions: [try Self.sampleSessionObject(head: 1)]),
                    capabilityIDs: ["fleet.receipt.read"],
                    receiptList: [try Self.receiptObject(requestID: "first")]
                )
                Darwin.shutdown(firstServer, SHUT_RDWR)
            } catch {
                serverResult.record(error)
            }
        }
        DispatchQueue.global().async {
            defer { Darwin.shutdown(secondServer, SHUT_RDWR) }
            do {
                try Self.serveStoreBootstrap(
                    descriptor: secondServer,
                    subscriptionSnapshot: try Self.snapshotObject(head: 1, sessions: [try Self.sampleSessionObject(head: 1)]),
                    eventBeforeSubscriptionResponse: false,
                    snapshotAfterEvent: try Self.snapshotObject(head: 1, sessions: [try Self.sampleSessionObject(head: 1)]),
                    capabilityIDs: ["fleet.receipt.read"],
                    receiptList: [try Self.receiptObject(requestID: "second")]
                )
                secondReady.fulfill()
                _ = releaseSecond.wait(timeout: .now() + 2)
            } catch {
                serverResult.record(error)
                secondReady.fulfill()
            }
        }

        await MainActor.run { store.start() }
        await fulfillment(of: [firstDone, secondReady], timeout: 2)
        let reloaded = await Self.waitUntil {
            await MainActor.run {
                guard case .live = store.connectionState else { return false }
                return factory.count == 2 && store.receipts.map(\.requestID) == ["second"]
            }
        }
        await MainActor.run { store.stop() }
        releaseSecond.signal()
        try serverResult.throwIfRecorded()
        XCTAssertTrue(reloaded, "reconnect must reload durable receipts before Fleet becomes live")
    }

    func testOptionalPreloadFailuresPreserveCachedProjections() async throws {
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
        let firstDone = expectation(description: "cached optional projections loaded")
        let secondReady = expectation(description: "optional preload failures returned")
        let releaseSecond = DispatchSemaphore(value: 0)
        let serverResult = SocketServerResult()

        DispatchQueue.global().async {
            defer {
                Darwin.shutdown(firstServer, SHUT_RDWR)
                firstDone.fulfill()
            }
            do {
                try Self.serveOptionalPreloads(descriptor: firstServer, succeed: true)
            } catch {
                serverResult.record(error)
            }
        }
        DispatchQueue.global().async {
            defer { Darwin.shutdown(secondServer, SHUT_RDWR) }
            do {
                try Self.serveOptionalPreloads(descriptor: secondServer, succeed: false)
                secondReady.fulfill()
                _ = releaseSecond.wait(timeout: .now() + 2)
            } catch {
                serverResult.record(error)
                secondReady.fulfill()
            }
        }

        await MainActor.run { store.start() }
        await fulfillment(of: [firstDone, secondReady], timeout: 2)
        try serverResult.throwIfRecorded()
        let preserved = await Self.waitUntil {
            await MainActor.run {
                guard case .live = store.connectionState else { return false }
                return factory.count == 2
                    && store.receipts.map(\.requestID) == ["cached"]
                    && store.atcInstances.map(\.name) == ["main"]
                    && store.atcSchedulerOwnership == .legacyTimerReconciliationRequired
                    && store.timeline.map(\.revision) == [7]
            }
        }
        await MainActor.run { store.stop() }
        releaseSecond.signal()

        XCTAssertTrue(preserved, "transient optional preload failures must keep last known good projections")
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

    private func receipt(_ requestID: String) -> FleetActionReceipt {
        FleetActionReceipt(
            requestID: requestID,
            sessionKey: "session-\(requestID)",
            actionKind: "send_prompt",
            actionFingerprint: "fingerprint-\(requestID)",
            expectedVersion: 1,
            idempotencyKey: nil,
            status: .pending,
            detail: nil,
            sessionVersion: nil,
            createdAt: 1,
            updatedAt: 1
        )
    }

    /// The copilot session is minted ONCE per connection, not once per poll,
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
    func testCopilotSessionIsMintedOncePerConnectionUntilTheSessionIsGone() async throws {
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
        await MainActor.run { store.forgetCopilotSession(inScope: "channel:c1") }
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
    /// because the scope stays resolved: a busy copilot would invalidate the
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
                "an event with no pane open was rendered anyway, so every copilot message redraws the roster"
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
            await MainActor.run { store.forgetCopilotSession(inScope: "channel:c1") }
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

    /// Bring a store up against the scripted chat daemon and hand both it and
    /// the SERVER end of the socket to `body`.
    ///
    /// Handing over the raw descriptor is what lets a test push a notification.
    /// It is safe because the server thread is blocked reading (or held at the
    /// gate) whenever `body` runs, so there is never a second writer.
    private func withChatStore(
        refuseMessageSubscribe: Bool = false,
        gate: ChatServerGate? = nil,
        pagesReturnMarkerRows: Bool = false,
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
        let counts = ChatServerCounts(pagesReturnMarkerRows: pagesReturnMarkerRows)

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
                    ],
                    refuseMessageSubscribe: refuseMessageSubscribe
                )
                while true {
                    try Self.answerChatRequest(
                        try Self.readRequest(from: serverDescriptor),
                        to: serverDescriptor,
                        counts: counts,
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

        try await body(store, serverDescriptor)

        await MainActor.run { store.stop() }
        await fulfillment(of: [serverDone], timeout: 5)
        try serverResult.throwIfRecorded()
    }

    /// One `FleetMessage` in the shape the daemon frames it.
    private static func messageObject(id: String, scope: String) -> [String: Any] {
        [
            "id": id, "scope_key": scope, "sender": "copilot",
            "kind": "agent", "body": "from the copilot", "created_at": 1_700_000_000_000,
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
        gate: ChatServerGate? = nil
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
            try writeResponse(to: descriptor, request: request, result: [
                "session_key": "acp:1",
                "scope_key": "channel:c1",
                "turn_deadline_ms": 1_800_000,
            ])
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
        receiptList: [Any] = [],
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
        if capabilityIDs.contains("fleet.receipt.read") {
            let receiptRequest = try readRequest(from: descriptor)
            XCTAssertEqual(receiptRequest["method"] as? String, "fleet/receipt_list")
            try writeResponse(to: descriptor, request: receiptRequest, result: ["receipts": receiptList])
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

    private static func serveOptionalPreloads(descriptor: Int32, succeed: Bool) throws {
        let authentication = try readRequest(from: descriptor)
        try writeResponse(to: descriptor, request: authentication, result: [:])
        let negotiation = try readRequest(from: descriptor)
        try writeResponse(to: descriptor, request: negotiation, result: [
            "daemon_version": "fixture-daemon",
            "protocol_version": 1,
            "read_compatible": true,
            "write_compatible": true,
            "capability_ids": ["fleet.receipt.read", "fleet.atc.read", "fleet.timeline.read"],
        ])
        let subscription = try readRequest(from: descriptor)
        try writeResponse(to: descriptor, request: subscription, result: [
            "snapshot": try snapshotObject(head: 0, sessions: []),
            "replay": [],
            "replay_state": ["state": "complete"],
        ])
        let results: [String: Any] = [
            "fleet/receipt_list": ["receipts": [try receiptObject(requestID: "cached")]],
            "atc/list": [
                "instances": [[
                    "name": "main", "cwd": "/tmp", "tmux_session": "atc-main",
                    "heartbeat_cron": "*/2 * * * *", "err_retry_cap": 3,
                    "idle_pause_min": 60, "next_tick_at": 2_000, "enabled": true,
                    "last_heartbeat_at": 1_000, "config_generation": 4,
                ]],
                "scheduler_ownership": "legacy_timer_reconciliation_required",
            ],
            "fleet/timeline": [
                "entries": [[
                    "revision": 7, "session_key": "s1", "observed_at": 1_000,
                    "provenance": "authoritative", "kind": "turn_completed",
                    "applied": true, "session_version": 3,
                ]],
                "next_after_revision": 7,
            ],
        ]
        for method in ["fleet/receipt_list", "atc/list", "fleet/timeline"] {
            let request = try readRequest(from: descriptor)
            XCTAssertEqual(request["method"] as? String, method)
            if succeed {
                try writeResponse(to: descriptor, request: request, result: results[method]!)
            } else {
                try writeError(to: descriptor, request: request)
            }
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

    private static func receiptObject(requestID: String) throws -> Any {
        let receipt = FleetActionReceipt(
            requestID: requestID,
            sessionKey: "s1",
            actionKind: "send_prompt",
            actionFingerprint: "fingerprint-\(requestID)",
            expectedVersion: 3,
            idempotencyKey: nil,
            status: .pending,
            detail: nil,
            sessionVersion: nil,
            createdAt: 1,
            updatedAt: 1
        )
        return try JSONSerialization.jsonObject(with: FleetWire.encoder().encode(receipt))
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
/// The mint count is the assertion the copilot cache exists to make: it is a
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

    init(rejectionDetail: String = "target_not_running", pagesReturnMarkerRows: Bool = false) {
        self.rejectionDetail = rejectionDetail
        self.pagesReturnMarkerRows = pagesReturnMarkerRows
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
