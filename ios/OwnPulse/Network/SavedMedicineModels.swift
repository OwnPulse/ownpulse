// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

struct SavedMedicine: Codable, Sendable, Identifiable {
    let id: String
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
    let sortOrder: Int
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id, substance, dose, unit, route
        case sortOrder = "sort_order"
        case createdAt = "created_at"
    }
}

struct CreateSavedMedicine: Codable, Sendable {
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
}
