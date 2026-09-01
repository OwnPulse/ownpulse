// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// Mirrors the production gate: the medication APIs require the iOS 26 SDK.
#if swift(>=6.3)

import Foundation
import HealthKit
import Testing
@testable import OwnPulse

@available(iOS 26.0, *)
private final class MockMedicationSyncProvider: MedicationSyncProviderProtocol, @unchecked Sendable {
    var records: [MedicationDoseRecord] = []
    var newAnchor: Data?
    private(set) var receivedAnchors: [Data?] = []

    func requestAuthorization() async throws {}

    func authorizedMedicationCount() async throws -> Int { records.count }

    func queryDoseEvents(anchor: Data?) async throws -> (records: [MedicationDoseRecord], newAnchor: Data?) {
        receivedAnchors.append(anchor)
        return (records, newAnchor)
    }
}

@Suite("Medication form → route mapping")
struct MedicationRouteMappingTests {
    @Test("unambiguous forms map to their route")
    func unambiguousForms() throws {
        guard #available(iOS 26.0, *) else { return }
        #expect(MedicationSyncProvider.mapFormToRoute(.capsule) == "oral")
        #expect(MedicationSyncProvider.mapFormToRoute(.liquid) == "oral")
        #expect(MedicationSyncProvider.mapFormToRoute(.powder) == "oral")
        #expect(MedicationSyncProvider.mapFormToRoute(.tablet) == "oral")
        #expect(MedicationSyncProvider.mapFormToRoute(.injection) == "injection")
        #expect(MedicationSyncProvider.mapFormToRoute(.inhaler) == "inhalation")
        #expect(MedicationSyncProvider.mapFormToRoute(.cream) == "topical")
        #expect(MedicationSyncProvider.mapFormToRoute(.gel) == "topical")
        #expect(MedicationSyncProvider.mapFormToRoute(.lotion) == "topical")
        #expect(MedicationSyncProvider.mapFormToRoute(.ointment) == "topical")
        #expect(MedicationSyncProvider.mapFormToRoute(.patch) == "topical")
        #expect(MedicationSyncProvider.mapFormToRoute(.topical) == "topical")
        #expect(MedicationSyncProvider.mapFormToRoute(.foam) == "topical")
        #expect(MedicationSyncProvider.mapFormToRoute(.suppository) == "rectal")
    }

    @Test("ambiguous or unknown forms map to no route")
    func ambiguousForms() throws {
        guard #available(iOS 26.0, *) else { return }
        #expect(MedicationSyncProvider.mapFormToRoute(.spray) == nil)
        #expect(MedicationSyncProvider.mapFormToRoute(.drops) == nil)
        #expect(MedicationSyncProvider.mapFormToRoute(.device) == nil)
        #expect(MedicationSyncProvider.mapFormToRoute(.unknown) == nil)
        #expect(MedicationSyncProvider.mapFormToRoute(nil) == nil)
    }
}

@MainActor
@Suite("SyncEngine medication dose sync")
struct MedicationDoseSyncTests {
    private func makeRecord(
        substance: String,
        dose: Double?,
        route: String?,
        sourceId: String
    ) -> MedicationDoseRecord {
        MedicationDoseRecord(
            substance: substance,
            dose: dose,
            unit: "mg",
            route: route,
            administeredAt: Date(),
            sourceId: sourceId,
            conceptIdentifier: "concept-\(sourceId)"
        )
    }

    private func build(provider: (any Sendable)?) -> (
        engine: SyncEngine,
        network: MockNetworkClient,
        anchors: AnchorStore
    ) {
        let db = DatabaseManager(inMemory: true)
        let network = MockNetworkClient()
        network.requestNoContentHandler = { _, _, _ in }
        let engine = SyncEngine(
            networkClient: network,
            healthKitProvider: MockHealthKitProvider(),
            medicationSyncProvider: provider,
            offlineQueue: OfflineQueue(databaseManager: db),
            anchorStore: AnchorStore(databaseManager: db),
            progress: SyncProgress()
        )
        return (engine, network, AnchorStore(databaseManager: db))
    }

    /// Sets a handler that records every intervention POST body and answers
    /// the write-queue GET the rest of `sync()` performs. `failOn` makes the
    /// Nth intervention POST (1-based) throw.
    private func stubNetwork(
        _ network: MockNetworkClient,
        posted: PostedInterventions,
        failOn: Int? = nil
    ) {
        network.requestHandler = { method, path, body in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                return [HealthKitWriteQueueItem]()
            }
            if method == "POST" && path == Endpoints.interventions {
                guard let intervention = body as? CreateIntervention else {
                    throw NetworkError.serverError(statusCode: 400, body: "unexpected body")
                }
                let attempt = posted.recordAttempt(intervention)
                if attempt == failOn {
                    throw NetworkError.serverError(statusCode: 500, body: "boom")
                }
                posted.recordSuccess(intervention)
                return InterventionResponse(id: UUID().uuidString, substance: intervention.substance)
            }
            return []
        }
    }

    @Test("posts one intervention per dose event, preserving nil dose and route")
    func postsDoseEvents() async throws {
        guard #available(iOS 26.0, *) else { return }
        let provider = MockMedicationSyncProvider()
        provider.records = [
            makeRecord(substance: "med-a", dose: 250, route: "oral", sourceId: "a"),
            makeRecord(substance: "med-b", dose: nil, route: nil, sourceId: "b"),
        ]
        provider.newAnchor = Data([1])

        let (engine, network, anchors) = build(provider: provider)
        let posted = PostedInterventions()
        stubNetwork(network, posted: posted)

        await engine.sync()

        let bodies = posted.successes
        #expect(bodies.count == 2)
        #expect(bodies[0].substance == "med-a")
        #expect(bodies[0].dose == 250)
        #expect(bodies[0].route == "oral")
        #expect(bodies[1].substance == "med-b")
        #expect(bodies[1].dose == nil)
        #expect(bodies[1].route == nil)
        #expect(bodies[1].notes == "Synced from Apple Health")
        #expect(bodies[0].source == "healthkit")
        #expect(bodies[0].sourceId == "a")
        #expect(bodies[1].sourceId == "b")
        #expect(bodies[0].fasted == nil)
        #expect(try anchors.anchor(forRecordType: SyncEngine.medicationAnchorKey) == Data([1]))
    }

    @Test("empty query result still saves the new anchor")
    func emptyResultSavesAnchor() async throws {
        guard #available(iOS 26.0, *) else { return }
        let provider = MockMedicationSyncProvider()
        provider.newAnchor = Data([9])

        let (engine, network, anchors) = build(provider: provider)
        let posted = PostedInterventions()
        stubNetwork(network, posted: posted)

        await engine.sync()

        #expect(posted.successes.isEmpty)
        #expect(try anchors.anchor(forRecordType: SyncEngine.medicationAnchorKey) == Data([9]))
    }

    @Test("a mid-loop failure never re-uploads already-posted dose events")
    func partialFailureDoesNotDuplicate() async throws {
        guard #available(iOS 26.0, *) else { return }
        let provider = MockMedicationSyncProvider()
        provider.records = [
            makeRecord(substance: "med-a", dose: 1, route: "oral", sourceId: "a"),
            makeRecord(substance: "med-b", dose: 2, route: "oral", sourceId: "b"),
        ]
        provider.newAnchor = Data([7])

        let (engine, network, anchors) = build(provider: provider)
        let posted = PostedInterventions()
        stubNetwork(network, posted: posted, failOn: 2)

        await engine.sync()

        // The failure aborted the pass before the anchor could be saved.
        #expect(try anchors.anchor(forRecordType: SyncEngine.medicationAnchorKey) == nil)
        #expect(posted.successes.map(\.substance) == ["med-a"])

        // Next pass re-reads the same events; only the failed one uploads.
        stubNetwork(network, posted: posted)
        await engine.sync()

        #expect(posted.successes.map(\.substance) == ["med-a", "med-b"])
        #expect(try anchors.anchor(forRecordType: SyncEngine.medicationAnchorKey) == Data([7]))
        // A saved anchor covers all posted events, so the pending set resets.
        let pendingData = try anchors.anchor(forRecordType: SyncEngine.medicationPostedIDsKey)
        let pending = try JSONDecoder().decode(Set<String>.self, from: pendingData ?? Data())
        #expect(pending.isEmpty)
    }

    @Test("no medication provider means no intervention uploads")
    func noProviderNoPosts() async throws {
        guard #available(iOS 26.0, *) else { return }
        let (engine, network, _) = build(provider: nil)
        let posted = PostedInterventions()
        stubNetwork(network, posted: posted)

        await engine.sync()

        #expect(posted.attemptCount == 0)
        #expect(await engine.lastSyncDate != nil)
    }
}

/// Collects intervention POST bodies observed by the mock network handler.
/// A class so the `@Sendable` handler closure can mutate shared state; the
/// lock makes cross-actor access safe.
private final class PostedInterventions: @unchecked Sendable {
    private let lock = NSLock()
    private var _attempts: [CreateIntervention] = []
    private var _successes: [CreateIntervention] = []

    var attemptCount: Int {
        lock.lock(); defer { lock.unlock() }
        return _attempts.count
    }

    var successes: [CreateIntervention] {
        lock.lock(); defer { lock.unlock() }
        return _successes
    }

    /// Returns the 1-based attempt number.
    func recordAttempt(_ intervention: CreateIntervention) -> Int {
        lock.lock(); defer { lock.unlock() }
        _attempts.append(intervention)
        return _attempts.count
    }

    func recordSuccess(_ intervention: CreateIntervention) {
        lock.lock(); defer { lock.unlock() }
        _successes.append(intervention)
    }
}

#endif
