// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import HealthKit

struct HealthKitSample: Sendable {
    let recordType: String
    let value: Double
    let unit: String
    let startTime: Date
    let endTime: Date
    let sourceId: String
}

struct AnchoredQueryResult: Sendable {
    let samples: [HealthKitSample]
    let newAnchor: Data?
    let deletedObjectIDs: [String]
}

protocol HealthKitProviderProtocol: Sendable {
    func requestAuthorization() async throws
    func isAuthorized() -> Bool
    func querySamples(
        type: HKSampleType,
        anchor: Data?
    ) async throws -> AnchoredQueryResult
    func writeSample(
        type: HKSampleType,
        value: Double,
        unit: HKUnit,
        start: Date,
        end: Date
    ) async throws
}

final class HealthKitProvider: HealthKitProviderProtocol, @unchecked Sendable {
    private let store = HKHealthStore()

    func requestAuthorization() async throws {
        try await store.requestAuthorization(
            toShare: HealthKitTypeMap.allWriteTypes,
            read: HealthKitTypeMap.allReadTypes
        )
    }

    func isAuthorized() -> Bool {
        HKHealthStore.isHealthDataAvailable()
    }

    func querySamples(
        type: HKSampleType,
        anchor: Data?
    ) async throws -> AnchoredQueryResult {
        guard let mapping = HealthKitTypeMap.mapping(forHKType: type) else {
            return AnchoredQueryResult(samples: [], newAnchor: nil, deletedObjectIDs: [])
        }

        let hkAnchor: HKQueryAnchor?
        if let anchorData = anchor {
            hkAnchor = try NSKeyedUnarchiver.unarchivedObject(
                ofClass: HKQueryAnchor.self,
                from: anchorData
            )
        } else {
            hkAnchor = nil
        }

        return try await withCheckedThrowingContinuation { continuation in
            let query = HKAnchoredObjectQuery(
                type: type,
                predicate: nil,
                anchor: hkAnchor,
                limit: HKObjectQueryNoLimit
            ) { _, added, deleted, newAnchor, error in
                if let error {
                    continuation.resume(throwing: error)
                    return
                }

                let samples = (added ?? []).compactMap { sample -> HealthKitSample? in
                    if let quantitySample = sample as? HKQuantitySample {
                        return HealthKitSample(
                            recordType: mapping.recordType,
                            value: quantitySample.quantity.doubleValue(for: mapping.unit),
                            unit: mapping.unitString,
                            startTime: quantitySample.startDate,
                            endTime: quantitySample.endDate,
                            sourceId: sample.uuid.uuidString
                        )
                    } else if let categorySample = sample as? HKCategorySample {
                        return HealthKitSample(
                            recordType: mapping.recordType,
                            value: Double(categorySample.value),
                            unit: mapping.unitString,
                            startTime: categorySample.startDate,
                            endTime: categorySample.endDate,
                            sourceId: sample.uuid.uuidString
                        )
                    }
                    return nil
                }

                var anchorData: Data?
                if let newAnchor {
                    anchorData = try? NSKeyedArchiver.archivedData(
                        withRootObject: newAnchor,
                        requiringSecureCoding: true
                    )
                }

                let deletedIDs = (deleted ?? []).map { $0.uuid.uuidString }

                continuation.resume(returning: AnchoredQueryResult(
                    samples: samples,
                    newAnchor: anchorData,
                    deletedObjectIDs: deletedIDs
                ))
            }

            store.execute(query)
        }
    }

    func writeSample(
        type: HKSampleType,
        value: Double,
        unit: HKUnit,
        start: Date,
        end: Date
    ) async throws {
        guard let quantityType = type as? HKQuantityType else { return }
        let quantity = HKQuantity(unit: unit, doubleValue: value)
        let sample = HKQuantitySample(
            type: quantityType,
            quantity: quantity,
            start: start,
            end: end
        )
        try await store.save(sample)
    }
}
