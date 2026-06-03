// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// In-memory `WidgetDefaultsStore` so the publisher can be exercised without a
/// real app-group container (the test process isn't a group member).
private final class InMemoryDefaultsStore: WidgetDefaultsStore, @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [String: Data] = [:]

    func data(forKey key: String) -> Data? {
        lock.lock(); defer { lock.unlock() }
        return storage[key]
    }

    func set(_ data: Data?, forKey key: String) {
        lock.lock(); defer { lock.unlock() }
        storage[key] = data
    }
}

private func sampleSnapshot(checkinFilled: Bool = true, value: String = "56") -> WidgetSnapshot {
    WidgetSnapshot(
        checkinFilledToday: checkinFilled,
        heroMetricName: "Resting Heart Rate",
        heroMetricValue: value,
        heroMetricUnit: "bpm",
        heroTrendText: "-4% vs 30d avg",
        heroTrendIsPositive: true,
        lastUpdated: Date(timeIntervalSince1970: 1_700_000_000)
    )
}

@Suite("WidgetSnapshot")
struct WidgetSnapshotTests {
    @Test("encodes and decodes round-trip")
    func codableRoundTrip() throws {
        let snapshot = sampleSnapshot()
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(WidgetSnapshot.self, from: data)
        #expect(decoded == snapshot)
    }

    @Test("placeholder has neutral, non-data values")
    func placeholderIsNeutral() {
        let p = WidgetSnapshot.placeholder
        #expect(p.checkinFilledToday == false)
        #expect(p.heroMetricValue == "—")
        #expect(p.heroMetricUnit == "bpm")
    }

    @Test("isStale is false for a fresh snapshot")
    func freshSnapshotNotStale() {
        let now = Date(timeIntervalSince1970: 1_700_000_000)
        var s = sampleSnapshot()
        s.lastUpdated = now.addingTimeInterval(-60 * 60) // 1h old
        #expect(s.isStale(asOf: now) == false)
    }

    @Test("isStale is true past the 24h threshold")
    func oldSnapshotIsStale() {
        let now = Date(timeIntervalSince1970: 1_700_000_000)
        var s = sampleSnapshot()
        s.lastUpdated = now.addingTimeInterval(-(25 * 60 * 60)) // 25h old
        #expect(s.isStale(asOf: now) == true)
    }

    @Test("isStale is true exactly past threshold, false exactly at it")
    func staleBoundary() {
        let now = Date(timeIntervalSince1970: 1_700_000_000)
        var atThreshold = sampleSnapshot()
        atThreshold.lastUpdated = now.addingTimeInterval(-WidgetSnapshot.stalenessThreshold)
        #expect(atThreshold.isStale(asOf: now) == false)

        var pastThreshold = sampleSnapshot()
        pastThreshold.lastUpdated = now.addingTimeInterval(-(WidgetSnapshot.stalenessThreshold + 1))
        #expect(pastThreshold.isStale(asOf: now) == true)
    }

    @Test("epoch-sentinel placeholder is always stale")
    func placeholderIsStale() {
        #expect(WidgetSnapshot.placeholder.isStale(asOf: Date()) == true)
    }
}

@Suite("WidgetDataPublisher")
struct WidgetDataPublisherTests {
    @Test("publish writes the snapshot and triggers a reload")
    func publishWritesAndReloads() {
        let store = InMemoryDefaultsStore()
        let reloaded = Locked(false)
        let publisher = WidgetDataPublisher(store: store) { reloaded.value = true }

        publisher.publish(sampleSnapshot())

        #expect(store.data(forKey: WidgetSharedConstants.snapshotKey) != nil)
        #expect(reloaded.value == true)
    }

    @Test("load returns the previously published snapshot")
    func loadRoundTrip() {
        let store = InMemoryDefaultsStore()
        let publisher = WidgetDataPublisher(store: store, reload: {})
        let snapshot = sampleSnapshot(checkinFilled: false, value: "61")

        publisher.publish(snapshot)
        let loaded = publisher.load()

        #expect(loaded == snapshot)
    }

    @Test("load returns nil when nothing has been published")
    func loadEmptyReturnsNil() {
        let store = InMemoryDefaultsStore()
        let publisher = WidgetDataPublisher(store: store, reload: {})
        #expect(publisher.load() == nil)
    }

    @Test("load returns nil for corrupt data")
    func loadCorruptReturnsNil() {
        let store = InMemoryDefaultsStore()
        store.set(Data([0x00, 0x01, 0x02]), forKey: WidgetSharedConstants.snapshotKey)
        let publisher = WidgetDataPublisher(store: store, reload: {})
        #expect(publisher.load() == nil)
    }

    @Test("publish is a no-op when the shared store is unavailable")
    func publishNoStoreIsNoOp() {
        let reloaded = Locked(false)
        // nil store simulates a missing app-group entitlement.
        let publisher = WidgetDataPublisher(store: nil) { reloaded.value = true }
        publisher.publish(sampleSnapshot())
        #expect(reloaded.value == false)
        #expect(publisher.load() == nil)
    }
}

@Suite("AppGroupDefaultsStore")
struct AppGroupDefaultsStoreTests {
    /// Uses a transient, named `UserDefaults` suite (not the real app group,
    /// which the test process can't access) to exercise the first-party
    /// wrapper that carries the scoped `@unchecked Sendable` conformance.
    @Test("wraps UserDefaults read/write round-trip")
    func roundTrip() throws {
        let suite = "test.widget.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }

        let store = AppGroupDefaultsStore(defaults)
        #expect(store.data(forKey: "k") == nil)

        let payload = Data([0xDE, 0xAD, 0xBE, 0xEF])
        store.set(payload, forKey: "k")
        #expect(store.data(forKey: "k") == payload)

        store.set(nil, forKey: "k")
        #expect(store.data(forKey: "k") == nil)
    }
}

/// Tiny thread-safe box so the reload closure can record into a value type.
private final class Locked<T>: @unchecked Sendable {
    private let lock = NSLock()
    private var _value: T
    init(_ value: T) { _value = value }
    var value: T {
        get { lock.lock(); defer { lock.unlock() }; return _value }
        set { lock.lock(); _value = newValue; lock.unlock() }
    }
}
