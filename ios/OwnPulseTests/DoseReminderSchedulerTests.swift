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

    // MARK: - Basic scheduling

    @Test("schedules one notification per notify time per scheduled day")
    func schedulesPerNotifyTimePerDay() {
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "BPC-157 Protocol",
            startDate: Self.date("2026-06-01"),
            notify: true,
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
        let run = DoseReminderRun(
            runId: "run-42",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-01"),
            notify: true,
            notifyTimes: ["09:00"],
            lines: []
        )
        let now = Self.date("2026-06-01", hour: 0, minute: 0)

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        let first = try! #require(specs.first)
        #expect(first.identifier == "dose-run-42-09:00-2026-06-01")
    }

    @Test("recomputing for the same inputs produces identical identifiers (replace, not duplicate)")
    func rebuildIsIdempotent() {
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-01"),
            notify: true,
            notifyTimes: ["09:00"],
            lines: []
        )
        let now = Self.date("2026-06-01")

        let (first, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)
        let (second, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(first.map(\.identifier) == second.map(\.identifier))
    }

    // MARK: - notify = false

    @Test("runs with notify disabled produce no specs")
    func notifyDisabledSkipped() {
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-01"),
            notify: false,
            notifyTimes: ["09:00"],
            lines: []
        )
        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: Self.date("2026-06-01"), calendar: Self.utcCalendar)
        #expect(specs.isEmpty)
    }

    // MARK: - Schedule pattern (per-day info)

    @Test("skips a day when the run's schedule_pattern marks every line unscheduled")
    func skipsUnscheduledDays() {
        // Every-other-day pattern: on, off, on, off...
        let pattern = (0..<30).map { $0 % 2 == 0 }
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-01"),
            notify: true,
            notifyTimes: ["09:00"],
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
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Fallback Protocol",
            startDate: Self.date("2026-06-01"),
            notify: true,
            notifyTimes: ["09:00"],
            lines: [] // detail fetch failed / not loaded
        )
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        #expect(specs.count == 7) // every day in the horizon, no gaps
        #expect(specs.allSatisfy { $0.body.contains("Fallback Protocol") })
    }

    @Test("a day index past the end of a line's schedule_pattern defaults to scheduled")
    func outOfRangeScheduleDefaultsToOn() {
        // Pattern only covers 2 days; day 3+ is unknown and should default to "on".
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-01"),
            notify: true,
            notifyTimes: ["09:00"],
            lines: [DoseReminderLine(substance: "X", dose: nil, unit: nil, schedulePattern: [true, false])]
        )
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        // Day 0: on. Day 1: off (known). Days 2-6: unknown -> on. Total = 6.
        #expect(specs.count == 6)
    }

    // MARK: - Run not started yet

    @Test("a run whose start date is in the future produces no reminders for pre-start days")
    func futureStartDateSkipsEarlyDays() {
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-05"),
            notify: true,
            notifyTimes: ["09:00"],
            lines: []
        )
        let now = Self.date("2026-06-01")

        let (specs, _) = DoseReminderScheduler.computeSpecs(runs: [run], now: now, calendar: Self.utcCalendar)

        // Horizon is 2026-06-01...2026-06-07; only 06-05, 06-06, 06-07 are >= start.
        #expect(specs.count == 3)
    }

    // MARK: - Past times are not scheduled

    @Test("does not schedule a reminder time that has already passed today")
    func skipsPastTimeToday() {
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-01"),
            notify: true,
            notifyTimes: ["08:00"],
            lines: []
        )
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
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-01"),
            notify: true,
            notifyTimes: [],
            lines: []
        )
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
            DoseReminderRun(
                runId: "run-\(i)",
                protocolId: "proto-\(i)",
                protocolName: "Protocol \(i)",
                startDate: Self.date("2026-06-01"),
                notify: true,
                notifyTimes: ["08:00", "20:00"],
                lines: []
            )
        }
        let now = Self.date("2026-06-01")

        let (specs, truncatedCount) = DoseReminderScheduler.computeSpecs(runs: runs, now: now, calendar: Self.utcCalendar)

        #expect(specs.count == 64)
        #expect(truncatedCount == 140 - 64)
        // Nearest-in-time first: every kept spec's fire date must be <= every dropped one's.
        let maxKept = specs.map(\.fireDate).max()!
        #expect(specs.allSatisfy { $0.fireDate <= maxKept })
        // Sorted ascending.
        #expect(specs == specs.sorted { $0.fireDate < $1.fireDate })
    }

    @Test("does not truncate when under the cap")
    func noTruncationUnderCap() {
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: Self.date("2026-06-01"),
            notify: true,
            notifyTimes: ["09:00"],
            lines: []
        )
        let (specs, truncatedCount) = DoseReminderScheduler.computeSpecs(
            runs: [run], now: Self.date("2026-06-01"), calendar: Self.utcCalendar
        )
        #expect(specs.count == 7)
        #expect(truncatedCount == 0)
    }

    // MARK: - Multiple runs, multiple substances same time

    @Test("multiple scheduled substances on the same run/day/time are aggregated into one notification")
    func aggregatesMultipleSubstances() {
        let run = DoseReminderRun(
            runId: "run-1",
            protocolId: "proto-1",
            protocolName: "Stack",
            startDate: Self.date("2026-06-01"),
            notify: true,
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
