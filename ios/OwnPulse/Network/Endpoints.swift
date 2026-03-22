// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

enum Endpoints {
    static let authGoogleCallback = "/api/v1/auth/google/callback"
    static let authRefresh = "/api/v1/auth/refresh"
    static let healthKitSync = "/api/v1/healthkit/sync"
    static let healthKitWriteQueue = "/api/v1/healthkit/write-queue"
    static let healthKitConfirm = "/api/v1/healthkit/confirm"
}
