// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("MockHealthKitProvider observer + background delivery")
struct MockHealthKitProviderTests {
    @Test("observeSampleUpdates yields each fireObserver call")
    func observerYields() async throws {
        let provider = MockHealthKitProvider()
        let stream = provider.observeSampleUpdates()

        // Consume in a detached task so we can fire events synchronously.
        let received: Task<Int, Never> = Task {
            var count = 0
            for await _ in stream {
                count += 1
                if count == 3 { return count }
            }
            return count
        }

        // Let the consumer set up.
        try await Task.sleep(nanoseconds: 20_000_000)

        provider.fireObserver()
        provider.fireObserver()
        provider.fireObserver()

        let result = await received.value
        #expect(result == 3)
    }

    @Test("endObserver terminates the stream")
    func endObserverTerminates() async throws {
        let provider = MockHealthKitProvider()
        let stream = provider.observeSampleUpdates()

        let done: Task<Void, Never> = Task {
            for await _ in stream {}
        }

        try await Task.sleep(nanoseconds: 20_000_000)
        provider.endObserver()
        _ = await done.value

        // If we got here without hanging, the stream finished cleanly.
        #expect(provider.observerStartCount == 1)
    }

    @Test("enableBackgroundDelivery records call and sets flag")
    func enableBackgroundDeliverySuccess() async throws {
        let provider = MockHealthKitProvider()
        try await provider.enableBackgroundDelivery()
        #expect(provider.backgroundDeliveryEnabled == true)
        #expect(provider.backgroundDeliveryCallCount == 1)
    }

    @Test("enableBackgroundDelivery surfaces configured errors")
    func enableBackgroundDeliveryFailure() async {
        struct Fail: Error {}
        let provider = MockHealthKitProvider()
        provider.backgroundDeliveryError = Fail()

        do {
            try await provider.enableBackgroundDelivery()
            Issue.record("expected throw")
        } catch {
            // Expected
        }
        #expect(provider.backgroundDeliveryEnabled == false)
        #expect(provider.backgroundDeliveryCallCount == 1)
    }

    @Test("disableAllBackgroundDelivery flips state and increments counter")
    func disableBackgroundDeliverySuccess() async throws {
        let provider = MockHealthKitProvider()
        try await provider.enableBackgroundDelivery()
        #expect(provider.backgroundDeliveryEnabled == true)

        try await provider.disableAllBackgroundDelivery()
        #expect(provider.backgroundDeliveryDisabled == true)
        #expect(provider.backgroundDeliveryEnabled == false)
        #expect(provider.disableBackgroundDeliveryCallCount == 1)
    }

    @Test("disableAllBackgroundDelivery surfaces configured errors")
    func disableBackgroundDeliveryFailure() async throws {
        struct Fail: Error {}
        let provider = MockHealthKitProvider()
        try await provider.enableBackgroundDelivery()

        provider.disableBackgroundDeliveryError = Fail()

        do {
            try await provider.disableAllBackgroundDelivery()
            Issue.record("expected throw")
        } catch {
            // Expected
        }
        #expect(provider.disableBackgroundDeliveryCallCount == 1)
        #expect(provider.backgroundDeliveryDisabled == false, "state must not flip when disable throws")
    }
}
