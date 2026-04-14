// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit

struct MedicationDoseRecord: Sendable {
    let substance: String
    let dose: Double
    let unit: String
    let route: String
    let administeredAt: Date
    let sourceId: String
    let conceptIdentifier: String
}

// The medication APIs require the iOS 26 SDK (Xcode 26+, Swift 6.1+).
// Gate at compile time so this file is inert when built with Xcode 16.x / Swift 6.0.
#if swift(>=6.3)

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
    // Form stored as String so the route mapper has no HealthKit dependency.
    private var medicationCache: [String: (name: String, form: String)] = [:]

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
                    let route = Self.mapFormToRoute(cached?.form)

                    return MedicationDoseRecord(
                        substance: substance,
                        dose: doseEvent.doseQuantity ?? 1.0,
                        unit: doseEvent.unit.unitString,
                        route: route,
                        administeredAt: doseEvent.startDate,
                        sourceId: doseEvent.uuid.uuidString,
                        conceptIdentifier: conceptID
                    )
                }

                var anchorData: Data?
                if let newAnchor {
                    anchorData = try? NSKeyedArchiver.archivedData(
                        withRootObject: newAnchor,
                        requiringSecureCoding: true
                    )
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

        var cache: [String: (name: String, form: String)] = [:]
        for med in medications {
            let id = med.medication.identifier.description
            let name = med.nickname ?? med.medication.displayText
            cache[id] = (name: name, form: String(med.medication.generalForm.rawValue))
        }
        medicationCache = cache
    }

    private func takenDosesPredicate() -> NSPredicate {
        NSPredicate(
            format: "%K == %d",
            HKPredicateKeyPathStatus,
            HKMedicationDoseEvent.LogStatus.taken.rawValue
        )
    }

    static func mapFormToRoute(_ form: String?) -> String {
        guard let form else { return "oral" }
        switch form {
        case "Capsule", "Liquid", "Powder", "Tablet":
            return "oral"
        case "Injection":
            return "injection"
        case "Inhaler":
            return "inhalation"
        case "Cream", "Gel", "Lotion", "Ointment", "Patch", "Topical", "Foam":
            return "topical"
        case "Suppository":
            return "rectal"
        case "Spray":
            return "nasal"
        case "Drops":
            return "sublingual"
        default:
            return "oral"
        }
    }
}

#endif
