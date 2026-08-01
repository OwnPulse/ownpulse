// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// A single protocol line's dosing schedule, as needed to decide whether a
/// reminder is warranted on a given day. Deliberately a small, Sendable,
/// value-type projection of `ProtocolLine` — NOT the network model itself —
/// so the scheduling math below has no dependency on decoding or networking.
struct DoseReminderLine: Sendable, Equatable {
    let substance: String
    let dose: Double?
    let unit: String?
    /// Day-indexed (0 = run start date). May be shorter than the run's
    /// horizon if the source data was truncated; indices past the end are
    /// treated as "unknown" (see `DoseReminderScheduler`).
    let schedulePattern: [Bool]
}

/// A client-side view of one active protocol run, bundling the run's notify
/// settings with the per-substance schedule needed to build reminder
/// copy. Built by `DoseReminderCoordinator` from `ActiveRunResponse` +
/// `ProtocolDetail`.
struct DoseReminderRun: Sendable, Equatable {
    let runId: String
    let protocolId: String
    let protocolName: String
    /// Midnight (start of day) of the run's start date, in the calendar used
    /// for scheduling.
    let startDate: Date
    let notify: Bool
    /// One or more `"HH:mm"` 24-hour reminder times. Ignored when `notify`
    /// is false.
    let notifyTimes: [String]
    /// Per-substance schedule. Empty when line-level detail could not be
    /// loaded (e.g. the protocol detail fetch failed) — in that case the
    /// scheduler falls back to a daily reminder with generic copy.
    let lines: [DoseReminderLine]
}

/// A fully-resolved local notification to schedule. Pure data — no
/// UserNotifications types — so the computation that produces it is
/// testable without a notification center.
struct DoseReminderSpec: Sendable, Equatable {
    /// Deterministic identifier: `dose-<runId>-<HH:mm>-<yyyy-MM-dd>`.
    /// Re-running the computation for the same run/time/date always
    /// produces the same identifier, so re-scheduling naturally replaces
    /// (rather than duplicates) a previously-scheduled request.
    let identifier: String
    let runId: String
    let fireDate: Date
    let title: String
    let body: String
}

/// Pure computation: given the set of active, notify-enabled runs and the
/// current time, decide exactly which local notifications should be
/// pending. No side effects, no UserNotifications dependency — everything
/// that touches `UNUserNotificationCenter` lives in `NotificationManager`.
enum DoseReminderScheduler {
    /// How many days ahead to schedule, starting today.
    static let horizonDays = 7

    /// `UNUserNotificationCenter` allows at most 64 pending local
    /// notification requests per app. We prioritize the soonest reminders
    /// and drop the rest rather than let `add(_:)` fail unpredictably.
    static let maxPendingNotifications = 64

    /// Computes the full set of dose reminder notifications that should be
    /// pending right now, plus how many were dropped to respect the pending
    /// cap (0 if nothing was truncated).
    static func computeSpecs(
        runs: [DoseReminderRun],
        now: Date,
        calendar: Calendar = .current,
        horizonDays: Int = DoseReminderScheduler.horizonDays,
        maxPending: Int = DoseReminderScheduler.maxPendingNotifications
    ) -> (specs: [DoseReminderSpec], truncatedCount: Int) {
        var specs: [DoseReminderSpec] = []
        let today = calendar.startOfDay(for: now)

        for run in runs where run.notify {
            let notifyTimes = run.notifyTimes.isEmpty ? ["09:00"] : run.notifyTimes
            let runStartDay = calendar.startOfDay(for: run.startDate)

            for offset in 0..<horizonDays {
                guard let day = calendar.date(byAdding: .day, value: offset, to: today) else { continue }
                let dayNumber = calendar.dateComponents([.day], from: runStartDay, to: day).day ?? -1
                guard dayNumber >= 0 else { continue } // run hasn't started yet on this date

                let scheduledLines: [DoseReminderLine]
                let isFallback = run.lines.isEmpty
                if isFallback {
                    // No per-line schedule data available — schedule daily
                    // and use generic copy rather than silently going quiet.
                    scheduledLines = []
                } else {
                    let matched = run.lines.filter { line in
                        dayNumber < line.schedulePattern.count ? line.schedulePattern[dayNumber] : true
                    }
                    guard !matched.isEmpty else { continue } // every known line is off today
                    scheduledLines = matched
                }

                let dateString = Self.dateString(day, calendar: calendar)
                for time in notifyTimes {
                    guard let fireDate = Self.combine(day: day, time: time, calendar: calendar),
                          fireDate > now else { continue }

                    let identifier = "dose-\(run.runId)-\(time)-\(dateString)"
                    let body = Self.body(
                        for: scheduledLines,
                        protocolName: run.protocolName,
                        isFallback: isFallback
                    )
                    specs.append(
                        DoseReminderSpec(
                            identifier: identifier,
                            runId: run.runId,
                            fireDate: fireDate,
                            title: "Dose Reminder",
                            body: body
                        )
                    )
                }
            }
        }

        specs.sort { $0.fireDate < $1.fireDate }
        let truncatedCount = max(0, specs.count - maxPending)
        if truncatedCount > 0 {
            specs = Array(specs.prefix(maxPending))
        }
        return (specs, truncatedCount)
    }

    private static func body(for lines: [DoseReminderLine], protocolName: String, isFallback: Bool) -> String {
        if isFallback {
            return "You have doses scheduled for \(protocolName) today."
        }
        let names = lines.map { line -> String in
            if let dose = line.dose, let unit = line.unit {
                return "\(line.substance) (\(Self.formatted(dose)) \(unit))"
            }
            return line.substance
        }
        return "Time for: \(names.joined(separator: ", "))"
    }

    private static func formatted(_ value: Double) -> String {
        value.truncatingRemainder(dividingBy: 1) == 0
            ? String(format: "%.0f", value)
            : String(format: "%.1f", value)
    }

    private static func combine(day: Date, time: String, calendar: Calendar) -> Date? {
        let parts = time.split(separator: ":")
        guard parts.count == 2, let hour = Int(parts[0]), let minute = Int(parts[1]) else { return nil }
        var components = calendar.dateComponents([.year, .month, .day], from: day)
        components.hour = hour
        components.minute = minute
        components.second = 0
        return calendar.date(from: components)
    }

    private static func dateString(_ date: Date, calendar: Calendar) -> String {
        let formatter = DateFormatter()
        formatter.calendar = calendar
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        return formatter.string(from: date)
    }
}
