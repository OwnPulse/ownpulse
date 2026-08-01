// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("DoseReminderScheduler")
struct DoseReminderSchedulerTests {
    private static var utcCalendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC")!
        return calendar
    }

    private static func date(_ string: String, hour: Int = 0, minute: Int = 0) -> Date {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.timeZone = TimeZone(identifier: "UTC")
        formatter.locale = Locale(identifier: "en_US_POSIX")
        var components = utcCalendar.dateComponents([.year, .month, .day], from: formatter.date(from: string)!)
        components.hour = hour
        components.minute = minute
        components.timeZone = TimeZone(identifier: "UTC")
        return utcCalendar.date(from: components)!
    }

    /// `durationDays` defaults to `.max` (no cutoff) so tests that aren't
    /// exercising the duration cutoff itself don't need to think about it.
    private static func makeRun(
        runId: String = "run-1",
        protocolId: String = "proto-1",
        protocolName: String = "Test",
        startDate: Date,
        notify: Bool = true,
        notifyTimes: [String] = ["09:00"],
        lines: [DoseReminderLine] = [],
        durationDays: Int = .max
    ) -> DoseReminderRun {
        DoseReminderRun(
            runId: runId,
            protocolId: protocolId,
            protocolName: protocolName,
            startDate: startDate,
            notify: notify,
            notifyTimes: notifyTimes,
            lines: lines,
            durationDays: durationDays
        )
    }

    // MARK: - Basic scheduling

    @Test("schedules one notification per notify time per scheduled day")
    func schedulesPerNotifyTimePerDay() {
        let run = Self.makeRun(
            protocolName: "BPC-157 Protocol",
            startDate: Self.date("2026-06-01"),
            notifyTimes: ["08:00", "20:00"],
            lines: [
                DoseReminderLine(substance: "BPC-157", dose: 250, unit: "mcg", schedulePattern: Array(repeating: true, count: 30))
            ]
        )
        // "now" is before the first reminder each day, so nothing is dropped as past.
        let now = Self.date("2026-06-01", hour: 0, minute: 0)

        let (specs, truncated) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(truncated == 0)
        // 7 days * 2 times/day = 14
        #expect(specs.count == 14)
        #expect(specs.allSatisfy { $0.runId == "run-1" })
        #expect(specs.allSatisfy { $0.body.contains("BPC-157") })
    }

    @Test("identifiers are deterministic: dose-<runId>-<time>-<date>")
    func deterministicIdentifiers() {
        let run = Self.makeRun(runId: "run-42", startDate: Self.date("2026-06-01"))
        let now = Self.date("2026-06-01", hour: 0, minute: 0)

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        let first = try! #require(specs.first)
        #expect(first.identifier == "dose-run-42-09:00-2026-06-01")
    }

    @Test("recomputing for the same inputs produces identical identifiers (replace, not duplicate)")
    func rebuildIsIdempotent() {
        let run = Self.makeRun(startDate: Self.date("2026-06-01"))
        let now = Self.date("2026-06-01")

        let (first, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)
        let (second, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(first.map(\.identifier) == second.map(\.identifier))
    }

    // MARK: - notify = false

    @Test("runs with notify disabled produce no specs")
    func notifyDisabledSkipped() {
        let run = Self.makeRun(startDate: Self.date("2026-06-01"), notify: false)
        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: Self.date("2026-06-01"), calendar: Self.utcCalendar)
        #expect(specs.isEmpty)
    }

    // MARK: - Schedule pattern (per-day info)

    @Test("skips a day when the run's schedule_pattern marks every line unscheduled")
    func skipsUnscheduledDays() {
        // Every-other-day pattern: on, off, on, off...
        let pattern = (0..<30).map { $0 % 2 == 0 }
        let run = Self.makeRun(
            startDate: Self.date("2026-06-01"),
            lines: [DoseReminderLine(substance: "Creatine", dose: 5, unit: "g", schedulePattern: pattern)]
        )
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        // Days 0,2,4,6 are "on" within the 7-day horizon (0...6) = 4 reminders.
        #expect(specs.count == 4)
        let dates = Set(specs.map { $0.identifier.suffix(10) })
        #expect(dates.contains("2026-06-01"))
        #expect(!dates.contains("2026-06-02"))
    }

    @Test("falls back to a daily reminder with generic copy when no line data is available")
    func fallsBackToDailyWithoutLineData() {
        let run = Self.makeRun(protocolName: "Fallback Protocol", startDate: Self.date("2026-06-01"))
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(specs.count == 7) // every day in the horizon, no gaps
        #expect(specs.allSatisfy { $0.body.contains("Fallback Protocol") })
    }

    @Test("a day index past the end of a line's schedule_pattern is treated as NOT scheduled")
    func outOfRangeScheduleDefaultsToOff() {
        // Pattern only covers 2 days; day 2+ is unknown and must NOT be
        // treated as scheduled — otherwise a line whose pattern array is
        // shorter than the run's remaining days would nag forever.
        let run = Self.makeRun(
            startDate: Self.date("2026-06-01"),
            lines: [DoseReminderLine(substance: "X", dose: nil, unit: nil, schedulePattern: [true, false])]
        )
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        // Day 0: on. Days 1-6: off/unknown -> not scheduled. Total = 1.
        #expect(specs.count == 1)
        #expect(specs.first?.identifier.hasSuffix("2026-06-01") == true)
    }

    // MARK: - Duration cutoff

    @Test("a run past its duration_days schedules nothing, regardless of schedule_pattern")
    func runPastDurationSchedulesNothing() {
        // Every-day pattern that (bug scenario) is long enough to cover the
        // whole horizon, but the run itself is only a 3-day protocol that
        // started 5 days ago — it finished 2 days ago and must not remind.
        let run = Self.makeRun(
            startDate: Self.date("2026-05-27"), // 5 days before "now"
            lines: [DoseReminderLine(substance: "X", dose: nil, unit: nil, schedulePattern: Array(repeating: true, count: 30))],
            durationDays: 3
        )
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(specs.isEmpty)
    }

    @Test("a run schedules reminders only through its final day, then stops")
    func runStopsExactlyAtDuration() {
        // Started 2 days ago, 4-day protocol (days 0-3): today (day 2) and
        // tomorrow (day 3) are still within the run; day 4+ is not.
        let run = Self.makeRun(
            startDate: Self.date("2026-05-30"),
            durationDays: 4
        )
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(specs.count == 2)
        let dates = Set(specs.map { String($0.identifier.suffix(10)) })
        #expect(dates == ["2026-06-01", "2026-06-02"])
    }

    @Test("duration cutoff also applies to the daily fallback when line data is missing")
    func durationCutoffAppliesToFallback() {
        let run = Self.makeRun(
            startDate: Self.date("2026-05-30"),
            lines: [], // fallback path
            durationDays: 4
        )
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(specs.count == 2)
    }

    // MARK: - Run not started yet

    @Test("a run whose start date is in the future produces no reminders for pre-start days")
    func futureStartDateSkipsEarlyDays() {
        let run = Self.makeRun(startDate: Self.date("2026-06-05"))
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        // Horizon is 2026-06-01...2026-06-07; only 06-05, 06-06, 06-07 are >= start.
        #expect(specs.count == 3)
    }

    // MARK: - Past times are not scheduled

    @Test("does not schedule a reminder time that has already passed today")
    func skipsPastTimeToday() {
        let run = Self.makeRun(startDate: Self.date("2026-06-01"), notifyTimes: ["08:00"])
        // "now" is 10:00, after the 08:00 reminder for today.
        let now = Self.date("2026-06-01", hour: 10, minute: 0)

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        // 7 candidate days, minus today's already-passed 08:00 = 6.
        #expect(specs.count == 6)
        #expect(!specs.contains { $0.identifier.hasSuffix("2026-06-01") })
    }

    // MARK: - Default notify time

    @Test("empty notifyTimes falls back to a single default reminder")
    func emptyNotifyTimesDefaults() {
        let run = Self.makeRun(startDate: Self.date("2026-06-01"), notifyTimes: [])
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(specs.count == 7)
        #expect(specs.allSatisfy { $0.identifier.contains("-09:00-") })
    }

    // MARK: - 64-pending-notification cap

    @Test("truncates to the 64-pending-notification cap, keeping the nearest-in-time reminders")
    func truncatesAtSystemCap() {
        // 10 runs * 7 days * 2 times = 140 candidate reminders, well over 64.
        let runs = (0..<10).map { i in
            Self.makeRun(
                runId: "run-\(i)",
                protocolId: "proto-\(i)",
                protocolName: "Protocol \(i)",
                startDate: Self.date("2026-06-01"),
                notifyTimes: ["08:00", "20:00"]
            )
        }
        let now = Self.date("2026-06-01")

        let (specs, truncatedCount) = DoseReminderScheduler.computeSpecs(runs: runs, now: now, calendar: Self.utcCalendar)
        // The un-truncated set, to compute the "correct" answer independently
        // of the cap logic under test.
        let (allSpecs, _) = DoseReminderScheduler.computeSpecs(runs: runs, now: now, calendar: Self.utcCalendar, maxPending: .max)

        #expect(allSpecs.count == 140)
        #expect(specs.count == 64)
        #expect(truncatedCount == 140 - 64)

        // The kept set must be exactly the 64 nearest-in-time entries,
        // ordered deterministically by (fireDate, identifier) — not just
        // "some 64 whose max happens to be <= itself".
        let expectedKept = allSpecs
            .sorted { ($0.fireDate, $0.identifier) < ($1.fireDate, $1.identifier) }
            .prefix(64)
        #expect(specs.map(\.identifier) == Array(expectedKept.map(\.identifier)))
    }

    @Test("ties on fireDate are broken by identifier for deterministic ordering")
    func tiesBrokenByIdentifier() {
        // Two runs sharing the exact same notify time produce two specs
        // with identical fireDate but different identifiers (different
        // runId). The result must not depend on input order or on Swift's
        // (unstable) sort — it must always come out identifier-ascending.
        let runA = Self.makeRun(runId: "run-b", startDate: Self.date("2026-06-01"))
        let runB = Self.makeRun(runId: "run-a", startDate: Self.date("2026-06-01"))
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [runA, runB], now: now, calendar: Self.utcCalendar)

        let firstDaySpecs = specs.filter { $0.identifier.hasSuffix("2026-06-01") }
        #expect(firstDaySpecs.count == 2)
        #expect(firstDaySpecs.map(\.identifier) == firstDaySpecs.map(\.identifier).sorted())
        #expect(firstDaySpecs.first?.identifier.contains("run-a") == true)
    }

    @Test("does not truncate when under the cap")
    func noTruncationUnderCap() {
        let run = Self.makeRun(startDate: Self.date("2026-06-01"))
        let (specs, truncatedCount) = DoseReminderScheduler.computeSpecs(
            runs: [run], now: Self.date("2026-06-01"), calendar: Self.utcCalendar
        )
        #expect(specs.count == 7)
        #expect(truncatedCount == 0)
    }

    // MARK: - Multiple runs, multiple substances same time

    @Test("multiple scheduled substances on the same run/day/time are aggregated into one notification")
    func aggregatesMultipleSubstances() {
        let run = Self.makeRun(
            protocolName: "Stack",
            startDate: Self.date("2026-06-01"),
            notifyTimes: ["08:00"],
            lines: [
                DoseReminderLine(substance: "Creatine", dose: 5, unit: "g", schedulePattern: Array(repeating: true, count: 30)),
                DoseReminderLine(substance: "Vitamin D", dose: 2000, unit: "IU", schedulePattern: Array(repeating: true, count: 30)),
            ]
        )
        let (specs, _) = DoseReminderScheduler.computeSpecs(
            runs: [run], now: Self.date("2026-06-01"), calendar: Self.utcCalendar
        )

        // One notification per day (not per substance).
        #expect(specs.count == 7)
        let first = try! #require(specs.first)
        #expect(first.body.contains("Creatine"))
        #expect(first.body.contains("Vitamin D"))
    }
}
