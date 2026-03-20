// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Typed error domain for the OwnPulse iOS app.
/// Services throw these; views pattern-match to show user-facing messages.
/// Never use `fatalError` in production paths — add a case here instead.
enum AppError: Error, Sendable {

    // MARK: - HealthKit

    /// The device does not support HealthKit (e.g. iPad without Health app).
    case healthKitNotAvailable

    /// The user denied the specific HealthKit permission we requested.
    case healthKitAuthorizationDenied

    /// An `HKQuery` returned a framework-level error.
    case healthKitQueryFailed(any Error)

    // MARK: - Network

    /// The server responded with HTTP 409 Conflict (duplicate record).
    case httpConflict

    /// The server responded with an unexpected HTTP status code.
    case httpError(statusCode: Int)

    /// The response body could not be decoded into the expected type.
    case decodingFailed(any Error)

    /// A network-level error (no connectivity, TLS failure, timeout, etc.).
    case networkError(any Error)
}
