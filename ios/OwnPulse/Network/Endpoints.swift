// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

enum Endpoints {
    static let authRefresh = "/api/v1/auth/refresh"
    static let healthKitSync = "/api/v1/healthkit/sync"
    static let healthKitWriteQueue = "/api/v1/healthkit/write-queue"
    static let healthKitConfirm = "/api/v1/healthkit/confirm"
    static let adminUsers = "/api/v1/admin/users"
    static let adminInvites = "/api/v1/admin/invites"
}
