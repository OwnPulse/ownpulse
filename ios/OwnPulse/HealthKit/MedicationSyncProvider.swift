// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import os

struct MedicationDoseRecord: Sendable {
    let substance: String
    /// Nil when Apple Health has no recorded quantity for the dose event.
    /// Never substitute a placeholder — an invented dose misrepresents the
    /// user's data.
    let dose: Double?
    let unit: String
    /// Nil when the medication's form doesn't imply an unambiguous route.
    let route: String?
    let administeredAt: Date
    let sourceId: String
    let conceptIdentifier: String
}

// The medication APIs require the iOS 26 SDK, so this file compiles only
// with Swift 6.3 or newer.
#if swift(>=6.3)

private let logger = Logger(subsystem: "health.ownpulse.app", category: "medication-sync")

@available(iOS 26.0, *)
protocol MedicationSyncProviderProtocol: Sendable {
    func requestAuthorization() async throws
    func authorizedMedicationCount() async throws -> Int
    func queryDoseEvents(anchor: Data?) async throws -> (records: [MedicationDoseRecord], newAnchor: Data?)
}

@available(iOS 26.0, *)
final class MedicationSyncProvider: MedicationSyncProviderProtocol, @unchecked Sendable {
    private let store = HKHealthStore()

    // Concept identifier → medication info, refreshed each sync.
    // @unchecked Sendable is safe because `medicationCache` is only mutated
    // inside `queryDoseEvents`, which SyncEngine awaits sequentially.
    private var medicationCache: [String: (name: String, form: HKMedicationGeneralForm)] = [:]

    func requestAuthorization() async throws {
        try await store.requestPerObjectReadAuthorization(
            for: HKObjectType.userAnnotatedMedicationType(),
            predicate: nil
        )
    }

    func authorizedMedicationCount() async throws -> Int {
        let descriptor = HKUserAnnotatedMedicationQueryDescriptor(
            predicate: NSPredicate(
                format: "%K == NO",
                HKUserAnnotatedMedicationPredicateKeyPathIsArchived
            )
        )
        let medications = try await descriptor.result(for: store)
        return medications.count
    }

    func queryDoseEvents(anchor: Data?) async throws -> (records: [MedicationDoseRecord], newAnchor: Data?) {
        try await refreshMedicationCache()

        let hkAnchor: HKQueryAnchor?
        if let anchorData = anchor {
            hkAnchor = try NSKeyedUnarchiver.unarchivedObject(
                ofClass: HKQueryAnchor.self,
                from: anchorData
            )
        } else {
            hkAnchor = nil
        }

        let doseEventType = HKObjectType.medicationDoseEventType()

        let cache = medicationCache
        return try await withCheckedThrowingContinuation { continuation in
            let query = HKAnchoredObjectQuery(
                type: doseEventType,
                predicate: takenDosesPredicate(),
                anchor: hkAnchor,
                limit: HKObjectQueryNoLimit
            ) { _, added, _, newAnchor, error in
                if let error {
                    continuation.resume(throwing: error)
                    return
                }

                let records = (added ?? []).compactMap { sample -> MedicationDoseRecord? in
                    guard let doseEvent = sample as? HKMedicationDoseEvent else { return nil }

                    let conceptID = doseEvent.medicationConceptIdentifier.description
                    let cached = cache[conceptID]
                    let substance = cached?.name ?? "Unknown Medication"

                    return MedicationDoseRecord(
                        substance: substance,
                        dose: doseEvent.doseQuantity,
                        unit: doseEvent.unit.unitString,
                        route: Self.mapFormToRoute(cached?.form),
                        administeredAt: doseEvent.startDate,
                        sourceId: doseEvent.uuid.uuidString,
                        conceptIdentifier: conceptID
                    )
                }

                var anchorData: Data?
                if let newAnchor {
                    do {
                        anchorData = try NSKeyedArchiver.archivedData(
                            withRootObject: newAnchor,
                            requiringSecureCoding: true
                        )
                    } catch {
                        // A nil anchor forces a full re-read next sync; the
                        // posted-IDs guard absorbs it, but log the cause.
                        logger.warning("Dose-event anchor archive failed: \(error.localizedDescription, privacy: .public)")
                    }
                }

                continuation.resume(returning: (records: records, newAnchor: anchorData))
            }

            store.execute(query)
        }
    }

    // MARK: - Private

    private func refreshMedicationCache() async throws {
        let descriptor = HKUserAnnotatedMedicationQueryDescriptor()
        let medications = try await descriptor.result(for: store)

        var cache: [String: (name: String, form: HKMedicationGeneralForm)] = [:]
        for med in medications {
            let id = med.medication.identifier.description
            let name = med.nickname ?? med.medication.displayText
            cache[id] = (name: name, form: med.medication.generalForm)
        }
        medicationCache = cache
    }

    /// No ADR-0008 cycle-prevention predicate here: `requestAuthorization`
    /// above requests per-object *read* only (never write), so there is no
    /// path by which this app writes a `HKMedicationDoseEvent` — the cycle
    /// the predicate guards against cannot occur for this type today. If
    /// dose-event write-back is ever added, apply
    /// `HealthKitProvider.makeReadPredicate()` here.
    private func takenDosesPredicate() -> NSPredicate {
        NSPredicate(
            format: "%K == %d",
            HKPredicateKeyPathStatus,
            HKMedicationDoseEvent.LogStatus.taken.rawValue
        )
    }

    /// Maps a medication's general form to an administration route, or nil
    /// when the form doesn't imply one (for example, sprays can be nasal,
    /// oral, or topical). Nil is preferable to a guess.
    static func mapFormToRoute(_ form: HKMedicationGeneralForm?) -> String? {
        guard let form else { return nil }
        switch form {
        case .capsule, .liquid, .powder, .tablet:
            return "oral"
        case .injection:
            return "injection"
        case .inhaler:
            return "inhalation"
        case .cream, .gel, .lotion, .ointment, .patch, .topical, .foam:
            return "topical"
        case .suppository:
            return "rectal"
        default:
            return nil
        }
    }
}

#endif
