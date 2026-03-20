// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Testing
@testable import OwnPulse

@Suite("JWTDecoder")
struct JWTDecoderTests {
    @Test("decodes sub and exp from a valid JWT")
    func decodesValidJWT() {
        // Header: {"alg":"HS256","typ":"JWT"}
        // Payload: {"sub":"550e8400-e29b-41d4-a716-446655440000","exp":9999999999,"iat":1700000000}
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiI1NTBlODQwMC1lMjliLTQxZDQtYTcxNi00NDY2NTU0NDAwMDAiLCJleHAiOjk5OTk5OTk5OTksImlhdCI6MTcwMDAwMDAwMH0.signature"

        let payload = JWTDecoder.decode(token)
        #expect(payload != nil)
        #expect(payload?.sub == "550e8400-e29b-41d4-a716-446655440000")
        #expect(payload?.exp.timeIntervalSince1970 == 9999999999)
    }

    @Test("returns nil for malformed token")
    func returnsNilForMalformed() {
        #expect(JWTDecoder.decode("not.a.jwt") == nil)
        #expect(JWTDecoder.decode("single") == nil)
    }

    @Test("isExpired returns true for past exp")
    func expiredToken() {
        // exp = 1 (1970)
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ0ZXN0IiwiZXhwIjoxfQ.signature"
        #expect(JWTDecoder.isExpired(token) == true)
    }

    @Test("isExpired returns false for future exp")
    func notExpiredToken() {
        // exp = 9999999999
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiI1NTBlODQwMC1lMjliLTQxZDQtYTcxNi00NDY2NTU0NDAwMDAiLCJleHAiOjk5OTk5OTk5OTksImlhdCI6MTcwMDAwMDAwMH0.signature"
        #expect(JWTDecoder.isExpired(token) == false)
    }
}
