// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("Protocol Models")
struct ProtocolModelsTests {
    // Exercise the production decoder config (fractional-second ISO8601
    // fallback) rather than a bare JSONDecoder(), so these fixtures catch
    // date-decoding regressions the real NetworkClient would hit.
    private let decoder = NetworkClient.makeDecoder()

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
        // date-ok
        let json = """
        { "id": "a1b2c3d4-0000-0000-0000-000000000001", "user_id": "a1b2c3d4-0000-0000-0000-000000000002", "name": "Morning routine", "description": "Daily supplements", "status": "active", "start_date": "2026-04-01", "duration_days": 30, "is_template": false, "tags": ["sleep", "focus"], "share_token": null, "share_expires_at": null, "created_at": "2026-04-01T08:00:00Z", "lines": [], "runs": [] }
        """.data(using: .utf8)!

        let detail = try decoder.decode(ProtocolDetail.self, from: json)
        #expect(detail.id == "a1b2c3d4-0000-0000-0000-000000000001")
        #expect(detail.name == "Morning routine")
        #expect(detail.status == .active)
        // date-ok
        #expect(detail.startDate == "2026-04-01")
        #expect(detail.durationDays == 30)
        #expect(detail.lines.isEmpty)
    }

    @Test("ProtocolDetail decodes when start_date is null (draft protocol)")
    func decodeDraftWithNullStartDate() throws {
        // This is the case that broke production — the old model had
        // `startDate: String` (non-optional) and failed to decode the
        // response below. Regression test for the field's optionality.
        // date-ok
        let json = """
        { "id": "a1b2c3d4-0000-0000-0000-000000000010", "user_id": "a1b2c3d4-0000-0000-0000-000000000002", "name": "Draft protocol", "description": null, "status": "draft", "start_date": null, "duration_days": 14, "is_template": false, "tags": [], "share_token": null, "share_expires_at": null, "created_at": "2026-04-15T12:00:00Z", "lines": [], "runs": [] }
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
        // date-ok
        let json = """
        { "id": "a1b2c3d4-0000-0000-0000-000000000020", "user_id": "a1b2c3d4-0000-0000-0000-000000000002", "name": "No updated_at", "description": null, "status": "active", "start_date": "2026-04-10", "duration_days": 7, "is_template": false, "tags": [], "share_token": null, "share_expires_at": null, "created_at": "2026-04-10T00:00:00Z", "lines": [], "runs": [] }
        """.data(using: .utf8)!

        let detail = try decoder.decode(ProtocolDetail.self, from: json)
        #expect(detail.name == "No updated_at")
    }

    @Test("ProtocolDetail decodes a full response with populated lines and doses")
    func decodeWithLinesAndDoses() throws {
        // date-ok
        let json = """
        { "id": "a1b2c3d4-0000-0000-0000-000000000030", "user_id": "a1b2c3d4-0000-0000-0000-000000000002", "name": "Stack", "description": null, "status": "active", "start_date": "2026-04-01", "duration_days": 30, "is_template": false, "tags": [], "share_token": null, "share_expires_at": null, "created_at": "2026-04-01T00:00:00Z", "lines": [ { "id": "a1b2c3d4-0000-0000-0000-000000000100", "protocol_id": "a1b2c3d4-0000-0000-0000-000000000030", "substance": "Creatine", "dose": 5.0, "unit": "g", "route": "oral", "time_of_day": "morning", "schedule_pattern": [true, true, true, true, true, true, true], "sort_order": 0, "created_at": "2026-04-01T00:00:00Z", "doses": [ { "id": "a1b2c3d4-0000-0000-0000-000000000200", "protocol_line_id": "a1b2c3d4-0000-0000-0000-000000000100", "day_number": 0, "status": "completed", "intervention_id": null, "logged_at": "2026-04-01T07:30:00Z" } ] } ], "runs": [] }
        """.data(using: .utf8)!

        let detail = try decoder.decode(ProtocolDetail.self, from: json)
        #expect(detail.lines.count == 1)
        let line = try #require(detail.lines.first)
        #expect(line.substance == "Creatine")
        #expect(line.schedulePattern.count == 7)
        #expect(line.doses.count == 1)
        #expect(line.doses.first?.status == .completed)
    }

    // MARK: - ActiveRunResponse decode regression tests
    //
    // Pins the iOS `ActiveRunResponse` model to the backend's `RunResponse`
    // shape, which carries the run's notify settings — added so on-device
    // dose reminders (`DoseReminderCoordinator`) can be built without a new
    // endpoint.

    @Test("ActiveRunResponse decodes notify settings with notify_times array")
    func decodeActiveRunWithNotifyTimes() throws {
        // date-ok
        let json = """
        { "id": "run-1", "protocol_id": "proto-1", "protocol_name": "BPC-157 Protocol", "start_date": "2026-03-28", "duration_days": 28, "status": "active", "notify": true, "notify_time": null, "notify_times": ["08:00", "20:00"], "repeat_reminders": true, "repeat_interval_minutes": 30, "progress_pct": 18.0, "doses_today": 2, "doses_completed_today": 1, "created_at": "2026-03-28T10:00:00Z" }
        """.data(using: .utf8)!

        let run = try decoder.decode(ActiveRunResponse.self, from: json)
        #expect(run.notify == true)
        #expect(run.notifyTimes == ["08:00", "20:00"])
        #expect(run.notifyTime == nil)
        #expect(run.repeatReminders == true)
        #expect(run.repeatIntervalMinutes == 30)
    }

    @Test("ActiveRunResponse decodes when notify is false and no times are configured")
    func decodeActiveRunNotifyDisabled() throws {
        // date-ok
        let json = """
        { "id": "run-2", "protocol_id": "proto-1", "protocol_name": "BPC-157 Protocol", "start_date": "2026-03-28", "duration_days": 28, "status": "active", "notify": false, "notify_time": null, "notify_times": null, "repeat_reminders": false, "repeat_interval_minutes": null, "progress_pct": 0.0, "doses_today": 1, "doses_completed_today": 0, "created_at": "2026-03-28T10:00:00Z" }
        """.data(using: .utf8)!

        let run = try decoder.decode(ActiveRunResponse.self, from: json)
        #expect(run.notify == false)
        #expect(run.notifyTimes == nil)
        #expect(run.repeatIntervalMinutes == nil)
    }

    // MARK: - ProtocolListItem decode smoke test

    @Test("ProtocolListItem decodes a list entry with a null next_dose")
    func decodeListItemNullNextDose() throws {
        // date-ok
        let json = """
        { "id": "a1b2c3d4-0000-0000-0000-000000000040", "name": "List item", "status": "active", "start_date": null, "duration_days": 14, "is_template": false, "progress_pct": 0.0, "next_dose": null, "created_at": "2026-04-15T00:00:00Z" }
        """.data(using: .utf8)!

        let item = try decoder.decode(ProtocolListItem.self, from: json)
        #expect(item.nextDose == nil)
        #expect(item.startDate == nil)
    }

    // MARK: - Dose backfill / adherence decode tests
    //
    // Fixtures below are copied verbatim from docs/architecture/api.md so a
    // future drift between the documented response shape and the actual
    // backend response is caught here rather than silently mismatched on
    // both sides.

    @Test("LogDoseRequest response body decodes with run_id and skip_reason")
    func decodeLogDoseResponse() throws {
        // date-ok
        let json = """
        { "id": "uuid", "protocol_line_id": "uuid", "day_number": 3, "status": "completed", "intervention_id": "uuid", "logged_at": "2026-04-03T08:30:00Z", "run_id": "uuid", "skip_reason": null }
        """.data(using: .utf8)!

        let dose = try decoder.decode(ProtocolDose.self, from: json)
        #expect(dose.status == .completed)
        #expect(dose.runId == "uuid")
        #expect(dose.skipReason == nil)
    }

    @Test("RunDoseDay decodes a missed day from GET /protocols/runs/:run_id/doses")
    func decodeRunDoseDayMissed() throws {
        // date-ok
        let json = """
        [ { "day_number": 3, "date": "2026-04-04", "protocol_line_id": "uuid", "substance": "BPC-157", "dose": 250.0, "unit": "mcg", "route": "subcutaneous", "time_of_day": "AM", "status": "missed", "dose_id": null, "intervention_id": null, "skip_reason": null, "logged_at": null } ]
        """.data(using: .utf8)!

        let days = try decoder.decode([RunDoseDay].self, from: json)
        let day = try #require(days.first)
        #expect(day.status == .missed)
        #expect(day.dayNumber == 3)
        #expect(day.dose == 250.0)
        #expect(day.doseId == nil)
    }

    @Test("MissedDoseItem decodes the missed-doses list response")
    func decodeMissedDoseItem() throws {
        // date-ok
        let json = """
        [ { "protocol_id": "uuid", "protocol_name": "BPC-157 — 4 weeks", "run_id": "uuid", "protocol_line_id": "uuid", "substance": "BPC-157", "dose": 250.0, "unit": "mcg", "route": "subcutaneous", "time_of_day": "AM", "day_number": 2, "date": "2026-04-03", "status": "missed" } ]
        """.data(using: .utf8)!

        let items = try decoder.decode([MissedDoseItem].self, from: json)
        let item = try #require(items.first)
        #expect(item.protocolName == "BPC-157 — 4 weeks")
        #expect(item.status == .missed)
        // date-ok
        #expect(item.date == "2026-04-03")
    }

    @Test("AdherenceResponse decodes overall totals and per-line breakdown, pct nullable")
    func decodeAdherenceResponse() throws {
        let json = """
        {
          "run_id": "uuid",
          "scheduled_so_far": 8,
          "completed": 3,
          "skipped": 2,
          "missed": 3,
          "adherence_pct": 50.0,
          "lines": [
            {
              "protocol_line_id": "uuid",
              "substance": "BPC-157",
              "scheduled_so_far": 5,
              "completed": 2,
              "skipped": 1,
              "missed": 2,
              "adherence_pct": 50.0
            }
          ]
        }
        """.data(using: .utf8)!

        let adherence = try decoder.decode(AdherenceResponse.self, from: json)
        #expect(adherence.adherencePct == 50.0)
        #expect(adherence.lines.count == 1)
        #expect(adherence.lines[0].substance == "BPC-157")
    }

    @Test("AdherenceResponse decodes null adherence_pct (no closed days yet)")
    func decodeAdherenceResponseNullPct() throws {
        let json = """
        {
          "run_id": "uuid",
          "scheduled_so_far": 0,
          "completed": 0,
          "skipped": 0,
          "missed": 0,
          "adherence_pct": null,
          "lines": []
        }
        """.data(using: .utf8)!

        let adherence = try decoder.decode(AdherenceResponse.self, from: json)
        #expect(adherence.adherencePct == nil)
    }

    // MARK: - Request encoding

    @Test("LogDoseRequest encodes tz_offset_minutes and optional administered_at/notes")
    func encodeLogDoseRequest() throws {
        let request = LogDoseRequest(
            protocolLineId: "line-1",
            dayNumber: 3,
            // date-ok
            administeredAt: "2026-04-03T09:15:00Z",
            notes: "logged a bit late",
            tzOffsetMinutes: -420
        )
        let data = try JSONEncoder().encode(request)
        let obj = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        #expect(obj["tz_offset_minutes"] as? Int == -420)
        // date-ok
        #expect(obj["administered_at"] as? String == "2026-04-03T09:15:00Z")
        #expect(obj["notes"] as? String == "logged a bit late")
        #expect(obj["protocol_line_id"] as? String == "line-1")
    }

    @Test("SkipDoseRequest encodes skip_reason")
    func encodeSkipDoseRequest() throws {
        let request = SkipDoseRequest(protocolLineId: "line-1", dayNumber: 1, skipReason: "traveling")
        let data = try JSONEncoder().encode(request)
        let obj = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        #expect(obj["skip_reason"] as? String == "traveling")
    }

    // MARK: - ActiveSubstance decode regression test
    //
    // Pinned to the backend's `ActiveSubstanceItem`
    // (`backend/api/src/models/protocol.rs`), which has no `protocol_id`
    // field — unlike web's `ActiveSubstance` TS interface, which declares
    // one the backend does not actually serialize.

    @Test("ActiveSubstance decodes a full entry with dose/unit/route present")
    func decodeActiveSubstanceFull() throws {
        let json = """
        [
          {
            "substance": "BPC-157",
            "dose": 250.0,
            "unit": "mcg",
            "route": "subcutaneous",
            "protocol_name": "Recovery Stack"
          }
        ]
        """.data(using: .utf8)!

        let items = try decoder.decode([ActiveSubstance].self, from: json)
        let item = try #require(items.first)
        #expect(item.substance == "BPC-157")
        #expect(item.dose == 250.0)
        #expect(item.unit == "mcg")
        #expect(item.route == "subcutaneous")
        #expect(item.protocolName == "Recovery Stack")
    }

    @Test("ActiveSubstance decodes with dose/unit/route absent")
    func decodeActiveSubstanceNullFields() throws {
        let json = """
        [
          {
            "substance": "Creatine",
            "dose": null,
            "unit": null,
            "route": null,
            "protocol_name": "Stack"
          }
        ]
        """.data(using: .utf8)!

        let items = try decoder.decode([ActiveSubstance].self, from: json)
        let item = try #require(items.first)
        #expect(item.dose == nil)
        #expect(item.unit == nil)
        #expect(item.route == nil)
    }

    // MARK: - ActiveSubstance.id uniqueness
    //
    // The backend's DISTINCT ON explicitly permits rows for the same
    // substance/protocol that differ only by route, and a nil dose must not
    // collide with a genuine 0-dose row. Either collision would silently
    // drop a row from a SwiftUI `ForEach` (and duplicate an accessibility
    // identifier).

    @Test("ActiveSubstance.id differs for rows that differ only by route")
    func activeSubstanceIdDiffersByRoute() {
        let oral = ActiveSubstance(substance: "BPC-157", dose: 250, unit: "mcg", route: "oral", protocolName: "Stack")
        let subq = ActiveSubstance(substance: "BPC-157", dose: 250, unit: "mcg", route: "subcutaneous", protocolName: "Stack")

        #expect(oral.id != subq.id)
    }

    @Test("ActiveSubstance.id differs between a nil dose and a zero dose")
    func activeSubstanceIdDiffersNilVsZeroDose() {
        let nilDose = ActiveSubstance(substance: "Creatine", dose: nil, unit: "g", route: "oral", protocolName: "Stack")
        let zeroDose = ActiveSubstance(substance: "Creatine", dose: 0, unit: "g", route: "oral", protocolName: "Stack")

        #expect(nilDose.id != zeroDose.id)
    }
}
