// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("Protocol Models")
struct ProtocolModelsTests {
    private let decoder = JSONDecoder()

    // MARK: - ProtocolDetail decode regression tests
    //
    // These tests pin the iOS `ProtocolDetail` model to the shape returned by
    // `GET /api/v1/protocols/:id` (`ProtocolResponse` on the backend).
    //
    // Context: the protocol detail page shipped broken in April 2026 because
    // iOS declared `startDate: String` (non-optional) and `updatedAt: String`
    // (required) — but the backend's `ProtocolResponse` has
    // `start_date: Option<NaiveDate>` and does not emit `updated_at` at all.
    // Either mismatch caused a silent `DecodingError` in `loadProtocol`,
    // which the view surfaced as "Failed to load protocol".
    //
    // If you change `ProtocolResponse` on the backend, update the fixtures
    // below to match. Do NOT make a field optional on iOS to silence a test
    // failure without first confirming the backend will honor that.

    @Test("ProtocolDetail decodes a full response with start_date populated")
    func decodeFullProtocolDetail() throws {
        let json = """
        {
            "id": "a1b2c3d4-0000-0000-0000-000000000001",
            "user_id": "a1b2c3d4-0000-0000-0000-000000000002",
            "name": "Morning routine",
            "description": "Daily supplements",
            "status": "active",
            "start_date": "2026-04-01",
            "duration_days": 30,
            "is_template": false,
            "tags": ["sleep", "focus"],
            "share_token": null,
            "share_expires_at": null,
            "created_at": "2026-04-01T08:00:00Z",
            "lines": [],
            "runs": []
        }
        """.data(using: .utf8)!

        let detail = try decoder.decode(ProtocolDetail.self, from: json)
        #expect(detail.id == "a1b2c3d4-0000-0000-0000-000000000001")
        #expect(detail.name == "Morning routine")
        #expect(detail.status == .active)
        #expect(detail.startDate == "2026-04-01")
        #expect(detail.durationDays == 30)
        #expect(detail.lines.isEmpty)
    }

    @Test("ProtocolDetail decodes when start_date is null (draft protocol)")
    func decodeDraftWithNullStartDate() throws {
        // This is the case that broke production — the old model had
        // `startDate: String` (non-optional) and failed to decode the
        // response below. Regression test for the field's optionality.
        let json = """
        {
            "id": "a1b2c3d4-0000-0000-0000-000000000010",
            "user_id": "a1b2c3d4-0000-0000-0000-000000000002",
            "name": "Draft protocol",
            "description": null,
            "status": "draft",
            "start_date": null,
            "duration_days": 14,
            "is_template": false,
            "tags": [],
            "share_token": null,
            "share_expires_at": null,
            "created_at": "2026-04-15T12:00:00Z",
            "lines": [],
            "runs": []
        }
        """.data(using: .utf8)!

        let detail = try decoder.decode(ProtocolDetail.self, from: json)
        #expect(detail.status == .draft)
        #expect(detail.startDate == nil)
        #expect(detail.description == nil)
    }

    @Test("ProtocolDetail decodes without an updated_at field")
    func decodeIgnoresMissingUpdatedAt() throws {
        // The backend's ProtocolResponse struct has no `updated_at` field.
        // iOS used to require it as `updatedAt: String`, which caused every
        // detail decode to fail. This test asserts the model no longer
        // requires it.
        let json = """
        {
            "id": "a1b2c3d4-0000-0000-0000-000000000020",
            "user_id": "a1b2c3d4-0000-0000-0000-000000000002",
            "name": "No updated_at",
            "description": null,
            "status": "active",
            "start_date": "2026-04-10",
            "duration_days": 7,
            "is_template": false,
            "tags": [],
            "share_token": null,
            "share_expires_at": null,
            "created_at": "2026-04-10T00:00:00Z",
            "lines": [],
            "runs": []
        }
        """.data(using: .utf8)!

        let detail = try decoder.decode(ProtocolDetail.self, from: json)
        #expect(detail.name == "No updated_at")
    }

    @Test("ProtocolDetail decodes a full response with populated lines and doses")
    func decodeWithLinesAndDoses() throws {
        let json = """
        {
            "id": "a1b2c3d4-0000-0000-0000-000000000030",
            "user_id": "a1b2c3d4-0000-0000-0000-000000000002",
            "name": "Stack",
            "description": null,
            "status": "active",
            "start_date": "2026-04-01",
            "duration_days": 30,
            "is_template": false,
            "tags": [],
            "share_token": null,
            "share_expires_at": null,
            "created_at": "2026-04-01T00:00:00Z",
            "lines": [
                {
                    "id": "a1b2c3d4-0000-0000-0000-000000000100",
                    "protocol_id": "a1b2c3d4-0000-0000-0000-000000000030",
                    "substance": "Creatine",
                    "dose": 5.0,
                    "unit": "g",
                    "route": "oral",
                    "time_of_day": "morning",
                    "schedule_pattern": [true, true, true, true, true, true, true],
                    "sort_order": 0,
                    "created_at": "2026-04-01T00:00:00Z",
                    "doses": [
                        {
                            "id": "a1b2c3d4-0000-0000-0000-000000000200",
                            "protocol_line_id": "a1b2c3d4-0000-0000-0000-000000000100",
                            "day_number": 0,
                            "status": "completed",
                            "intervention_id": null,
                            "logged_at": "2026-04-01T07:30:00Z"
                        }
                    ]
                }
            ],
            "runs": []
        }
        """.data(using: .utf8)!

        let detail = try decoder.decode(ProtocolDetail.self, from: json)
        #expect(detail.lines.count == 1)
        let line = try #require(detail.lines.first)
        #expect(line.substance == "Creatine")
        #expect(line.schedulePattern.count == 7)
        #expect(line.doses.count == 1)
        #expect(line.doses.first?.status == .completed)
    }

    // MARK: - ProtocolListItem decode smoke test

    @Test("ProtocolListItem decodes a list entry with a null next_dose")
    func decodeListItemNullNextDose() throws {
        let json = """
        {
            "id": "a1b2c3d4-0000-0000-0000-000000000040",
            "name": "List item",
            "status": "active",
            "start_date": null,
            "duration_days": 14,
            "is_template": false,
            "progress_pct": 0.0,
            "next_dose": null,
            "created_at": "2026-04-15T00:00:00Z"
        }
        """.data(using: .utf8)!

        let item = try decoder.decode(ProtocolListItem.self, from: json)
        #expect(item.nextDose == nil)
        #expect(item.startDate == nil)
    }
}
