// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import os
@testable import OwnPulse

@MainActor
final class MockNetworkClient: NetworkClientProtocol, @unchecked Sendable {
    var requestHandler: ((String, String, (any Encodable & Sendable)?) throws -> Any)?
    var requestNoContentHandler: ((String, String, (any Encodable & Sendable)?) throws -> Void)?

    /// Optional async variant. When non-nil, takes precedence over
    /// `requestHandler`. Used by tests that need to stall a request mid-flight
    /// (e.g. the "events during in-flight sync" suite).
    ///
    /// Returns `any Sendable` rather than `Any` because Swift 6 refuses to
    /// send non-Sendable values across actor hops. Every response type in
    /// this codebase already conforms to `Sendable`, so callers return the
    /// same values as before — we cast to `T` at the call site the same way
    /// the sync path does.
    var asyncRequestHandler: (@Sendable (String, String, (any Encodable & Sendable)?) async throws -> any Sendable)?

    /// Async variant of `requestNoContentHandler`. When set, takes precedence —
    /// used by pipeline-overlap tests that need to stall an upload in flight
    /// and timestamp the call boundaries.
    var asyncRequestNoContentHandler: (@Sendable (String, String, (any Encodable & Sendable)?) async throws -> Void)?

    private(set) var requestCalls: [(method: String, path: String)] = []

    /// Records the start and end timestamp of each `requestNoContent` call.
    /// Used by `testPipelineOverlap` to verify upload calls overlap with
    /// HealthKit reads.
    private(set) var requestNoContentTimings: [(path: String, startedAt: Date, endedAt: Date)] = []

    /// Snapshot of how many `requestNoContent` invocations were active when
    /// each new call started. Used by `testTaskGroupBoundedConcurrency`.
    ///
    /// Guarded by `inFlightLock`. `nonisolated(unsafe)` because the lock
    /// closure runs in a non-isolated context — the lock is what makes the
    /// reads/writes thread-safe, not the MainActor.
    nonisolated(unsafe) private var _maxConcurrentUploads: Int = 0
    nonisolated(unsafe) private var inFlightUploads: Int = 0
    /// `OSAllocatedUnfairLock` is async-safe (synchronous scoped locking),
    /// unlike `NSLock` which Swift 6 flags as unavailable from async contexts.
    private let inFlightLock = OSAllocatedUnfairLock()

    nonisolated var maxConcurrentUploads: Int {
        inFlightLock.withLock { _maxConcurrentUploads }
    }

    func request<T: Decodable & Sendable>(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws -> T {
        requestCalls.append((method: method, path: path))

        let result: Any
        if let asyncHandler = asyncRequestHandler {
            result = try await asyncHandler(method, path, body)
        } else if let handler = requestHandler {
            result = try handler(method, path, body)
        } else {
            fatalError("MockNetworkClient.requestHandler not set")
        }

        guard let typed = result as? T else {
            fatalError("MockNetworkClient handler returned wrong type")
        }
        return typed
    }

    func requestNoContent(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws {
        requestCalls.append((method: method, path: path))
        inFlightLock.withLock {
            inFlightUploads += 1
            if inFlightUploads > _maxConcurrentUploads {
                _maxConcurrentUploads = inFlightUploads
            }
        }
        let start = Date()
        defer {
            inFlightLock.withLock {
                inFlightUploads -= 1
            }
            let end = Date()
            requestNoContentTimings.append((path: path, startedAt: start, endedAt: end))
        }
        if let asyncHandler = asyncRequestNoContentHandler {
            try await asyncHandler(method, path, body)
        } else {
            try requestNoContentHandler?(method, path, body)
        }
    }
}
