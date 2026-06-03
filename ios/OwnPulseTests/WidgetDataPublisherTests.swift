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
