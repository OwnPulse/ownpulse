// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit

struct ClinicalLabResult: Sendable {
    let marker: String
    let loincCode: String?
    let value: Double
    let unit: String
    let panelDate: Date
    let labName: String?
    let referenceLow: Double?
    let referenceHigh: Double?
    let sourceId: String
}

struct ClinicalAnchoredResult: Sendable {
    let results: [ClinicalLabResult]
    let newAnchor: Data?
}

protocol ClinicalRecordProviderProtocol: Sendable {
    func requestAuthorization() async throws
    func isAvailable() -> Bool
    func queryLabResults(anchor: Data?) async throws -> ClinicalAnchoredResult
}

final class ClinicalRecordProvider: ClinicalRecordProviderProtocol, @unchecked Sendable {
    private let store = HKHealthStore()

    func isAvailable() -> Bool {
        HKHealthStore.isHealthDataAvailable()
    }

    func requestAuthorization() async throws {
        let types: Set<HKObjectType> = [
            HKObjectType.clinicalType(forIdentifier: .labResultRecord)!
        ]
        try await store.requestAuthorization(toShare: [], read: types)
    }

    func queryLabResults(anchor: Data?) async throws -> ClinicalAnchoredResult {
        let clinicalType = HKObjectType.clinicalType(forIdentifier: .labResultRecord)!

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
                type: clinicalType,
                predicate: nil,
                anchor: hkAnchor,
                limit: HKObjectQueryNoLimit
            ) { _, added, _, newAnchor, error in
                if let error {
                    continuation.resume(throwing: error)
                    return
                }

                let results = (added ?? []).compactMap { sample -> ClinicalLabResult? in
                    guard let record = sample as? HKClinicalRecord,
                          let fhirResource = record.fhirResource,
                          fhirResource.resourceType == .observation else {
                        return nil
                    }
                    return Self.parseFHIRObservation(
                        data: fhirResource.data,
                        sourceId: record.uuid.uuidString
                    )
                }

                let anchorData: Data?
                if let newAnchor {
                    anchorData = try? NSKeyedArchiver.archivedData(
                        withRootObject: newAnchor,
                        requiringSecureCoding: true
                    )
                } else {
                    anchorData = nil
                }

                continuation.resume(returning: ClinicalAnchoredResult(
                    results: results,
                    newAnchor: anchorData
                ))
            }

            store.execute(query)
        }
    }

    // MARK: - FHIR Parsing

    private static func parseFHIRObservation(data: Data, sourceId: String) -> ClinicalLabResult? {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              json["resourceType"] as? String == "Observation" else {
            return nil
        }

        // Extract valueQuantity (skip non-numeric results)
        guard let valueQuantity = json["valueQuantity"] as? [String: Any],
              let value = valueQuantity["value"] as? Double else {
            return nil
        }
        let unit = valueQuantity["unit"] as? String ?? ""

        // Extract code/marker name — prefer LOINC coding
        let code = json["code"] as? [String: Any]
        let codings = code?["coding"] as? [[String: Any]] ?? []
        let loincCoding = codings.first { ($0["system"] as? String) == "http://loinc.org" }
        let bestCoding = loincCoding ?? codings.first
        let marker = bestCoding?["display"] as? String ?? code?["text"] as? String ?? "Unknown"
        let loincCode = loincCoding?["code"] as? String

        // Extract effective date
        guard let dateString = json["effectiveDateTime"] as? String,
              let panelDate = Self.parseDate(dateString) else {
            return nil
        }

        // Extract reference range (first entry)
        var referenceLow: Double?
        var referenceHigh: Double?
        if let ranges = json["referenceRange"] as? [[String: Any]], let range = ranges.first {
            referenceLow = (range["low"] as? [String: Any])?["value"] as? Double
            referenceHigh = (range["high"] as? [String: Any])?["value"] as? Double
        }

        // Extract lab name from performer
        let performers = json["performer"] as? [[String: Any]]
        let labName = performers?.first?["display"] as? String

        return ClinicalLabResult(
            marker: marker,
            loincCode: loincCode,
            value: value,
            unit: unit,
            panelDate: panelDate,
            labName: labName,
            referenceLow: referenceLow,
            referenceHigh: referenceHigh,
            sourceId: sourceId
        )
    }

    private static func parseDate(_ string: String) -> Date? {
        // FHIR dates can be "2024-03-15", "2024-03-15T10:30:00Z", etc.
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = iso.date(from: string) { return date }

        iso.formatOptions = [.withInternetDateTime]
        if let date = iso.date(from: string) { return date }

        // Date-only format
        let df = DateFormatter()
        df.dateFormat = "yyyy-MM-dd"
        df.locale = Locale(identifier: "en_US_POSIX")
        df.timeZone = TimeZone(identifier: "UTC")
        return df.date(from: string)
    }
}
