// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("Dashboard Models")
struct DashboardModelsTests {
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()

    // MARK: - DashboardSummary

    @Test("DashboardSummary decodes from JSON")
    func decodeDashboardSummary() throws {
        let json = """
        {
            "latest_checkin": {
                "energy": 7,
                "mood": 8,
                "focus": 6,
                "recovery": 7,
                "libido": null,
                "date": "2026-03-28"
            },
            "checkin_count_7d": 5,
            "health_record_count_7d": 42,
            "intervention_count_7d": 3,
            "observation_count_7d": 2,
            "latest_lab_date": "2026-03-15",
            "pending_friend_shares": 1
        }
        """.data(using: .utf8)!

        let summary = try decoder.decode(DashboardSummary.self, from: json)
        #expect(summary.checkinCount7d == 5)
        #expect(summary.healthRecordCount7d == 42)
        #expect(summary.latestCheckin?.energy == 7)
        #expect(summary.latestCheckin?.libido == nil)
        #expect(summary.latestLabDate == "2026-03-15")
        #expect(summary.pendingFriendShares == 1)
    }

    @Test("DashboardSummary decodes with null latest_checkin")
    func decodeDashboardSummaryNoCheckin() throws {
        let json = """
        {
            "latest_checkin": null,
            "checkin_count_7d": 0,
            "health_record_count_7d": 0,
            "intervention_count_7d": 0,
            "observation_count_7d": 0,
            "latest_lab_date": null,
            "pending_friend_shares": 0
        }
        """.data(using: .utf8)!

        let summary = try decoder.decode(DashboardSummary.self, from: json)
        #expect(summary.latestCheckin == nil)
        #expect(summary.checkinCount7d == 0)
    }

    // MARK: - LatestCheckin

    /// Helper: returns the user's local "today" as yyyy-MM-dd using the same
    /// formatter configuration `LatestCheckin.isToday` uses internally.
    private func localTodayString() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = .current
        return formatter.string(from: Date())
    }

    private func localDateString(daysOffset: Int) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = .current
        let date = Calendar.current.date(byAdding: .day, value: daysOffset, to: Date())!
        return formatter.string(from: date)
    }

    @Test("LatestCheckin.isToday returns true for today's local date")
    func latestCheckinIsToday() {
        let checkin = LatestCheckin(
            energy: 7, mood: 8, focus: 6, recovery: 7, libido: 5,
            date: localTodayString()
        )
        #expect(checkin.isToday == true)
    }

    @Test("LatestCheckin.isToday returns false for yesterday (the production bug)")
    func latestCheckinNotTodayYesterday() {
        // Regression test: the old ISO8601DateFormatter-based implementation
        // produced a Date whose timezone interpretation diverged from
        // Calendar.current.isDateInToday, causing yesterday's checkins to be
        // treated as today in some timezones. This test locks in the fix.
        let checkin = LatestCheckin(
            energy: 7, mood: 8, focus: 6, recovery: 7, libido: 5,
            date: localDateString(daysOffset: -1)
        )
        #expect(checkin.isToday == false)
    }

    @Test("LatestCheckin.isToday returns false for tomorrow")
    func latestCheckinNotTodayTomorrow() {
        let checkin = LatestCheckin(
            energy: 7, mood: 8, focus: 6, recovery: 7, libido: 5,
            date: localDateString(daysOffset: 1)
        )
        #expect(checkin.isToday == false)
    }

    @Test("LatestCheckin.isToday truncates full ISO 8601 strings to the date prefix")
    func latestCheckinIsTodayFullISOString() {
        // Confirms the existing truncation behaviour survives the formatter
        // swap: only the first 10 characters are parsed, so a full timestamp
        // like "2026-01-01T23:59:59Z" is read as "2026-01-01".
        let checkin = LatestCheckin(
            energy: 5, mood: 5, focus: 5, recovery: 5, libido: 5,
            date: "\(localTodayString())T23:59:59Z"
        )
        #expect(checkin.isToday == true)
    }

    @Test("LatestCheckin.isToday returns false for malformed date strings")
    func latestCheckinIsTodayMalformed() {
        let checkin = LatestCheckin(
            energy: 5, mood: 5, focus: 5, recovery: 5, libido: 5,
            date: "not-a-date"
        )
        #expect(checkin.isToday == false)
    }

    @Test("LatestCheckin decodes with a yyyy-MM-dd date and evaluates isToday in local TZ")
    func decodeLatestCheckinAndIsToday() throws {
        // The backend returns `latest_checkin.date` as a chrono::NaiveDate,
        // which serializes to a bare "YYYY-MM-DD" string. Confirm the decode
        // path preserves that string verbatim and that isToday works against
        // the user's local calendar without being tripped up by UTC offsets.
        //
        // Note: we can't force Calendar.current onto a non-current timezone
        // from a unit test, so this test documents why the
        // `timeZone = .current` + en_US_POSIX locale combo is correct: the
        // parsed date's calendar components match the string in the user's
        // local day, which is exactly what Calendar.current.isDateInToday
        // compares against.
        let today = localTodayString()
        let json = """
        {
            "energy": 7,
            "mood": 8,
            "focus": 6,
            "recovery": 7,
            "libido": 5,
            "date": "\(today)"
        }
        """.data(using: .utf8)!

        let checkin = try decoder.decode(LatestCheckin.self, from: json)
        #expect(checkin.date == today)
        #expect(checkin.isToday == true)
    }

    @Test("LatestCheckin decoded with yesterday's date is not today")
    func decodeLatestCheckinYesterdayNotToday() throws {
        let yesterday = localDateString(daysOffset: -1)
        let json = """
        {
            "energy": 4,
            "mood": 5,
            "focus": 5,
            "recovery": 6,
            "libido": null,
            "date": "\(yesterday)"
        }
        """.data(using: .utf8)!

        let checkin = try decoder.decode(LatestCheckin.self, from: json)
        #expect(checkin.date == yesterday)
        #expect(checkin.isToday == false)
    }

    // MARK: - BatchSeriesResponse

    @Test("BatchSeriesResponse decodes with multiple series")
    func decodeBatchSeries() throws {
        let json = """
        {
            "series": [
                {
                    "source": "checkins",
                    "field": "energy",
                    "unit": "",
                    "points": [
                        {"t": "2026-03-21", "v": 6.0, "n": 1},
                        {"t": "2026-03-22", "v": 7.0, "n": 1}
                    ]
                },
                {
                    "source": "health_records",
                    "field": "resting_heart_rate",
                    "unit": "bpm",
                    "points": [
                        {"t": "2026-03-21", "v": 58.0, "n": 1}
                    ]
                }
            ]
        }
        """.data(using: .utf8)!

        let response = try decoder.decode(BatchSeriesResponse.self, from: json)
        #expect(response.series.count == 2)
        #expect(response.series[0].field == "energy")
        #expect(response.series[0].points.count == 2)
        #expect(response.series[1].unit == "bpm")
    }

    // MARK: - Insight

    @Test("Insight decodes with all fields")
    func decodeInsight() throws {
        let json = """
        {
            "id": "ins-1",
            "insight_type": "correlation",
            "headline": "Sleep correlates with mood",
            "detail": "More sleep = better mood.",
            "created_at": "2026-03-28T10:00:00Z"
        }
        """.data(using: .utf8)!

        let insight = try decoder.decode(Insight.self, from: json)
        #expect(insight.id == "ins-1")
        #expect(insight.insightType == "correlation")
        #expect(insight.headline == "Sleep correlates with mood")
        #expect(insight.detail == "More sleep = better mood.")
    }

    @Test("Insight decodes with null detail")
    func decodeInsightNullDetail() throws {
        let json = """
        {
            "id": "ins-2",
            "insight_type": "trend",
            "headline": "Energy trending up",
            "detail": null,
            "created_at": "2026-03-28T10:00:00Z"
        }
        """.data(using: .utf8)!

        let insight = try decoder.decode(Insight.self, from: json)
        #expect(insight.detail == nil)
    }

    // MARK: - UpsertCheckin

    @Test("UpsertCheckin encodes correctly")
    func encodeUpsertCheckin() throws {
        let checkin = UpsertCheckin(
            date: "2026-03-28",
            energy: 8, mood: 7, focus: 6, recovery: 9, libido: 5,
            notes: "Great day"
        )

        let data = try encoder.encode(checkin)
        let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any]

        #expect(dict?["date"] as? String == "2026-03-28")
        #expect(dict?["energy"] as? Int == 8)
        #expect(dict?["notes"] as? String == "Great day")
    }

    @Test("UpsertCheckin encodes null notes when empty")
    func encodeUpsertCheckinNullNotes() throws {
        let checkin = UpsertCheckin(
            date: "2026-03-28",
            energy: 5, mood: 5, focus: 5, recovery: 5, libido: 5,
            notes: nil
        )

        let data = try encoder.encode(checkin)
        let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any]

        #expect(dict?["notes"] is NSNull || dict?["notes"] == nil)
    }

    // MARK: - CreateIntervention

    @Test("CreateIntervention encodes correctly with snake_case keys")
    func encodeCreateIntervention() throws {
        let intervention = CreateIntervention(
            substance: "Caffeine",
            dose: 200,
            unit: "mg",
            route: "oral",
            administeredAt: "2026-03-28T08:00:00Z",
            fasted: true,
            notes: nil
        )

        let data = try encoder.encode(intervention)
        let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any]

        #expect(dict?["substance"] as? String == "Caffeine")
        #expect(dict?["dose"] as? Double == 200)
        #expect(dict?["administered_at"] as? String == "2026-03-28T08:00:00Z")
        #expect(dict?["fasted"] as? Bool == true)
    }

    // MARK: - CreateObservation

    @Test("CreateObservation encodes with JSONB value")
    func encodeCreateObservation() throws {
        let observation = CreateObservation(
            type: "scale",
            name: "Wellbeing",
            startTime: "2026-03-28T10:00:00Z",
            endTime: nil,
            value: ["numeric": .int(7), "max": .int(10)]
        )

        let data = try encoder.encode(observation)
        let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any]

        #expect(dict?["type"] as? String == "scale")
        #expect(dict?["name"] as? String == "Wellbeing")
        #expect(dict?["start_time"] as? String == "2026-03-28T10:00:00Z")

        let value = dict?["value"] as? [String: Any]
        #expect(value?["numeric"] as? Int == 7)
        #expect(value?["max"] as? Int == 10)
    }

    // MARK: - AnyCodableValue

    @Test("AnyCodableValue encodes and decodes string")
    func anyCodableString() throws {
        let value = AnyCodableValue.string("hello")
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(AnyCodableValue.self, from: data)
        if case .string(let s) = decoded {
            #expect(s == "hello")
        } else {
            Issue.record("Expected string")
        }
    }

    @Test("AnyCodableValue encodes and decodes int")
    func anyCodableInt() throws {
        let value = AnyCodableValue.int(42)
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(AnyCodableValue.self, from: data)
        if case .int(let i) = decoded {
            #expect(i == 42)
        } else {
            Issue.record("Expected int")
        }
    }

    @Test("AnyCodableValue encodes and decodes double")
    func anyCodableDouble() throws {
        let value = AnyCodableValue.double(3.14)
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(AnyCodableValue.self, from: data)
        if case .double(let d) = decoded {
            #expect(abs(d - 3.14) < 0.001)
        } else {
            Issue.record("Expected double")
        }
    }

    @Test("AnyCodableValue encodes and decodes bool")
    func anyCodableBool() throws {
        let value = AnyCodableValue.bool(true)
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(AnyCodableValue.self, from: data)
        if case .bool(let b) = decoded {
            #expect(b == true)
        } else {
            Issue.record("Expected bool")
        }
    }

    // MARK: - ObservationType

    @Test("ObservationType rawValues match API expectations")
    func observationTypeRawValues() {
        #expect(ObservationType.eventInstant.rawValue == "event_instant")
        #expect(ObservationType.eventDuration.rawValue == "event_duration")
        #expect(ObservationType.scale.rawValue == "scale")
        #expect(ObservationType.symptom.rawValue == "symptom")
        #expect(ObservationType.note.rawValue == "note")
        #expect(ObservationType.contextTag.rawValue == "context_tag")
        #expect(ObservationType.environmental.rawValue == "environmental")
    }

    @Test("ObservationType displayName is human readable")
    func observationTypeDisplayName() {
        #expect(ObservationType.eventInstant.displayName == "Event (Instant)")
        #expect(ObservationType.scale.displayName == "Scale")
    }
}
