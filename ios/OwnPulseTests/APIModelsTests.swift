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

    @Test("AppleCallbackRequest roundtrips with snake_case keys")
    func appleCallbackRequestRoundtrip() throws {
        let original = AppleCallbackRequest(idToken: "apple-jwt", platform: "ios")
        let data = try encoder.encode(original)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        #expect(json?["id_token"] as? String == "apple-jwt")
        #expect(json?["platform"] as? String == "ios")

        let decoded = try decoder.decode(AppleCallbackRequest.self, from: data)
        #expect(decoded.idToken == "apple-jwt")
        #expect(decoded.platform == "ios")
    }

    @Test("LoginRequest roundtrips")
    func loginRequestRoundtrip() throws {
        let original = LoginRequest(username: "tony", password: "secret")
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(LoginRequest.self, from: data)
        #expect(decoded.username == "tony")
        #expect(decoded.password == "secret")
    }

    @Test("TokenResponseWithRefresh roundtrips with snake_case keys")
    func tokenResponseWithRefreshRoundtrip() throws {
        let original = TokenResponseWithRefresh(
            accessToken: "at", refreshToken: "rt",
            tokenType: "Bearer", expiresIn: 7200
        )
        let data = try encoder.encode(original)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        #expect(json?["access_token"] as? String == "at")
        #expect(json?["refresh_token"] as? String == "rt")

        let decoded = try decoder.decode(TokenResponseWithRefresh.self, from: data)
        #expect(decoded.accessToken == "at")
        #expect(decoded.refreshToken == "rt")
        #expect(decoded.tokenType == "Bearer")
        #expect(decoded.expiresIn == 7200)
    }

    @Test("AuthMethod.createdAt decodes from ISO 8601 string")
    func authMethodCreatedAtISO8601() throws {
        let json = """
        {
            "id": "uuid-1",
            "provider": "apple",
            "email": "user@example.com",
            "created_at": "2026-03-20T10:00:00Z"
        }
        """.data(using: .utf8)!

        let decoded = try decoder.decode(AuthMethod.self, from: json)
        #expect(decoded.id == "uuid-1")
        #expect(decoded.provider == "apple")
        #expect(decoded.email == "user@example.com")

        // Verify the date was parsed correctly (March 20, 2026 at 10:00 UTC)
        let calendar = Calendar(identifier: .gregorian)
        let components = calendar.dateComponents(
            in: TimeZone(identifier: "UTC")!,
            from: decoded.createdAt
        )
        #expect(components.year == 2026)
        #expect(components.month == 3)
        #expect(components.day == 20)
        #expect(components.hour == 10)
    }

    @Test("AuthMethod roundtrips")
    func authMethodRoundtrip() throws {
        let original = AuthMethod(
            id: "uuid-1", provider: "google",
            email: "user@gmail.com", createdAt: Date(timeIntervalSince1970: 1_700_000_000)
        )
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(AuthMethod.self, from: data)
        #expect(decoded.id == "uuid-1")
        #expect(decoded.provider == "google")
        #expect(decoded.email == "user@gmail.com")
        #expect(abs(decoded.createdAt.timeIntervalSince(original.createdAt)) < 1)
    }

    @Test("LinkAuthRequest roundtrips with snake_case keys")
    func linkAuthRequestRoundtrip() throws {
        let original = LinkAuthRequest(provider: "apple", idToken: "jwt-token", password: nil)
        let data = try encoder.encode(original)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        #expect(json?["id_token"] as? String == "jwt-token")
        #expect(json?["provider"] as? String == "apple")

        let decoded = try decoder.decode(LinkAuthRequest.self, from: data)
        #expect(decoded.provider == "apple")
        #expect(decoded.idToken == "jwt-token")
        #expect(decoded.password == nil)
    }
}
