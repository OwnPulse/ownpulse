// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

enum Endpoints {
    static let authAppleCallback = "/api/v1/auth/apple/callback"
    static let authGoogleLogin = "/api/v1/auth/google/login"
    static let authGoogleCallback = "/api/v1/auth/google/callback"
    static let authLogin = "/api/v1/auth/login"
    static let authLink = "/api/v1/auth/link"
    static let authMethods = "/api/v1/auth/methods"
    static let authRefresh = "/api/v1/auth/refresh"
    static let healthKitSync = "/api/v1/healthkit/sync"
    static let healthKitWriteQueue = "/api/v1/healthkit/write-queue"
    static let healthKitConfirm = "/api/v1/healthkit/confirm"
    static let adminUsers = "/api/v1/admin/users"
    static let adminInvites = "/api/v1/admin/invites"
    static let healthRecords = "/api/v1/health-records"
    static let labsBulk = "/api/v1/labs/bulk"
    static let notificationsRegister = "/api/v1/notifications/register"
    static let notificationPreferences = "/api/v1/notifications/preferences"
    static let savedMedicines = "/api/v1/saved-medicines"
}
