// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("APIModels")
struct APIModelsTests {
    let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.dateEncodingStrategy = .iso8601
        return e
    }()

    let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    @Test("TokenResponse roundtrips")
    func tokenResponseRoundtrip() throws {
        let original = TokenResponse(accessToken: "jwt", tokenType: "Bearer", expiresIn: 3600)
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(TokenResponse.self, from: data)
        #expect(decoded.accessToken == "jwt")
        #expect(decoded.tokenType == "Bearer")
        #expect(decoded.expiresIn == 3600)
    }

    @Test("RefreshRequest uses snake_case")
    func refreshRequestEncoding() throws {
        let req = RefreshRequest(refreshToken: "abc")
        let data = try encoder.encode(req)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        #expect(json?["refresh_token"] as? String == "abc")
    }

    @Test("HealthKitBulkInsert roundtrips")
    func bulkInsertRoundtrip() throws {
        let record = CreateHealthRecord(
            source: "healthkit",
            recordType: "heart_rate",
            value: 72.0,
            unit: "bpm",
            startTime: Date(timeIntervalSince1970: 1000000),
            endTime: Date(timeIntervalSince1970: 1000000),
            metadata: nil,
            sourceId: "uuid-123"
        )
        let insert = HealthKitBulkInsert(records: [record])
        let data = try encoder.encode(insert)
        let decoded = try decoder.decode(HealthKitBulkInsert.self, from: data)
        #expect(decoded.records.count == 1)
        #expect(decoded.records[0].recordType == "heart_rate")
        #expect(decoded.records[0].source == "healthkit")
    }

    @Test("HealthKitConfirm roundtrips")
    func confirmRoundtrip() throws {
        let confirm = HealthKitConfirm(ids: ["id1", "id2"])
        let data = try encoder.encode(confirm)
        let decoded = try decoder.decode(HealthKitConfirm.self, from: data)
        #expect(decoded.ids == ["id1", "id2"])
    }
}
