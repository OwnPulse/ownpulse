// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import SwiftUI
import Testing
@testable import OwnPulse

@Suite("WriteBackQueueViewModel", .serialized)
@MainActor
struct WriteBackQueueViewModelTests {
    // MARK: - Helpers

    private func makeItem(
        id: String = "wb-1",
        hkType: String = "body_mass",
        value: Double = 72.5,
        unit: String? = "kg"
    ) -> HealthKitWriteQueueItem {
        HealthKitWriteQueueItem(
            id: id,
            userId: "user-1",
            hkType: hkType,
            value: HealthKitWriteQueuePayload(
                value: value,
                unit: unit,
                startTime: Date(timeIntervalSince1970: 1_700_000_000),
                endTime: Date(timeIntervalSince1970: 1_700_000_001)
            ),
            scheduledAt: Date(timeIntervalSince1970: 1_700_000_500),
            confirmedAt: nil,
            failedAt: nil,
            error: nil,
            sourceRecordId: nil,
            sourceTable: nil
        )
    }

    private func makeVM(
        network: MockNetworkClient,
        healthKit: MockHealthKitProvider = MockHealthKitProvider()
    ) -> WriteBackQueueViewModel {
        WriteBackQueueViewModel(networkClient: network, healthKitProvider: healthKit)
    }

    // MARK: - load

    @Test("load success populates items and transitions to loaded")
    func loadSuccess() async {
        let mock = MockNetworkClient()
        let items = [makeItem(id: "a"), makeItem(id: "b", hkType: "heart_rate", value: 60)]
        mock.requestHandler = { _, path, _ in
            #expect(path == Endpoints.healthKitWriteQueue)
            return items
        }

        let vm = makeVM(network: mock)
        #expect(vm.state == .idle)

        await vm.load()

        #expect(vm.state == .loaded)
        #expect(vm.items.count == 2)
        #expect(vm.items[0].id == "a")
    }

    @Test("load empty queue transitions to loaded with no items")
    func loadEmpty() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in [HealthKitWriteQueueItem]() }

        let vm = makeVM(network: mock)
        await vm.load()

        #expect(vm.state == .loaded)
        #expect(vm.items.isEmpty)
    }

    @Test("load server error transitions to error state")
    func loadServerError() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }

        let vm = makeVM(network: mock)
        await vm.load()

        if case .error(let msg) = vm.state {
            #expect(!msg.isEmpty)
        } else {
            Issue.record("Expected error state, got \(vm.state)")
        }
        #expect(vm.items.isEmpty)
    }

    @Test("load auth failure transitions to error state")
    func loadAuthFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in throw NetworkError.unauthorized }

        let vm = makeVM(network: mock)
        await vm.load()

        if case .error = vm.state {
            // expected
        } else {
            Issue.record("Expected error state")
        }
    }

    // MARK: - confirm

    @Test("confirm writes sample to HealthKit, acknowledges, and removes item")
    func confirmSuccess() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        let item = makeItem()
        mock.requestHandler = { _, _, _ in [item] }
        var confirmedBody: HealthKitConfirm?
        mock.requestNoContentHandler = { _, path, body in
            #expect(path == Endpoints.healthKitConfirm)
            confirmedBody = body as? HealthKitConfirm
        }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.confirm(item)

        #expect(hk.writtenSamples.count == 1)
        #expect(hk.writtenSamples[0].value == 72.5)
        #expect(confirmedBody?.ids == ["wb-1"])
        #expect(vm.items.isEmpty)
        #expect(vm.actionError == nil)
    }

    @Test("confirm with a deterministic HealthKit write failure reports it in failures (not ids) and removes item")
    func confirmWriteFailure() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        // .errorInvalidArgument is deterministic — the sample itself is
        // malformed, so retrying won't help. See `WriteBackFailureClassifier`.
        hk.writeSampleError = HKError(.errorInvalidArgument)
        let item = makeItem()
        mock.requestHandler = { _, _, _ in [item] }
        var confirmedBody: HealthKitConfirm?
        mock.requestNoContentHandler = { _, path, body in
            #expect(path == Endpoints.healthKitConfirm)
            confirmedBody = body as? HealthKitConfirm
        }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.confirm(item)

        #expect(hk.writtenSamples.isEmpty)
        #expect(confirmedBody?.ids.isEmpty == true, "a failed write must never be confirmed as if it succeeded")
        #expect(confirmedBody?.failures.map(\.id) == ["wb-1"], "the failure must be reported so the server stops re-serving it")
        #expect(vm.items.isEmpty, "the item is reported as failed so it drops out of the pending queue, same as a confirmed one")
        #expect(vm.actionError != nil)
    }

    @Test("confirm with a deterministic HealthKit write failure AND failure-report network failure keeps item for retry")
    func confirmWriteFailureAndReportFailure() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        hk.writeSampleError = HKError(.errorInvalidArgument)
        let item = makeItem()
        mock.requestHandler = { _, _, _ in [item] }
        mock.requestNoContentHandler = { _, _, _ in throw NetworkError.serverError(statusCode: 500, body: "boom") }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.confirm(item)

        #expect(hk.writtenSamples.isEmpty)
        #expect(vm.items.count == 1, "if we can't even report the failure, keep the item so a later retry can try again")
        #expect(vm.actionError != nil)
    }

    @Test("confirm with a transient HealthKit write failure never reports it and leaves the item pending")
    func confirmTransientWriteFailureLeavesItemPending() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        // Reporting a failure permanently retires the item server-side.
        // A transient condition (e.g. HealthKit temporarily unavailable) is
        // expected to clear — the item must stay pending for the next
        // retry, never confirmed and never reported as a permanent failure.
        hk.writeSampleError = HKError(.errorHealthDataUnavailable)
        let item = makeItem()
        mock.requestHandler = { _, _, _ in [item] }
        var confirmCalled = false
        mock.requestNoContentHandler = { _, _, _ in confirmCalled = true }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.confirm(item)

        #expect(confirmCalled == false, "a transient failure must never call /healthkit/confirm")
        #expect(vm.items.count == 1, "the item must stay pending for a later retry")
        #expect(vm.actionError != nil, "the user still sees feedback that the write didn't happen")
    }

    @Test("confirm with acknowledge network failure keeps item and surfaces error")
    func confirmAckFailure() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        let item = makeItem()
        mock.requestHandler = { _, _, _ in [item] }
        mock.requestNoContentHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 502, body: "bad gateway")
        }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.confirm(item)

        #expect(hk.writtenSamples.count == 1)  // write happened
        #expect(vm.items.count == 1)           // but ack failed, keep item
        #expect(vm.actionError != nil)
    }

    @Test("confirm with unmapped type surfaces error and does not write")
    func confirmUnmappedType() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        let item = makeItem(id: "x", hkType: "not_a_real_type")
        mock.requestHandler = { _, _, _ in [item] }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.confirm(item)

        #expect(hk.writtenSamples.isEmpty)
        #expect(vm.items.count == 1)
        #expect(vm.actionError != nil)
    }

    @Test("confirm with a null numeric value surfaces error and never writes")
    func confirmNullValue() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        let item = HealthKitWriteQueueItem(
            id: "null-value",
            userId: "user-1",
            hkType: "body_mass",
            value: HealthKitWriteQueuePayload(
                value: nil,
                unit: nil,
                startTime: Date(timeIntervalSince1970: 1_700_000_000),
                endTime: nil
            ),
            scheduledAt: Date(timeIntervalSince1970: 1_700_000_500),
            confirmedAt: nil,
            failedAt: nil,
            error: nil,
            sourceRecordId: nil,
            sourceTable: nil
        )
        mock.requestHandler = { _, _, _ in [item] }
        var confirmCalled = false
        mock.requestNoContentHandler = { _, _, _ in confirmCalled = true }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.confirm(item)

        #expect(hk.writtenSamples.isEmpty, "a nil value must never reach HealthKitProvider.writeSample")
        #expect(confirmCalled == false, "a nil value is rejected up front, before any network call")
        #expect(vm.items.count == 1)
        #expect(vm.actionError != nil)
    }

    @Test("confirm with category / non-writable type does not write or acknowledge")
    func confirmNonWritableType() async {
        // sleep_analysis maps to an HKCategoryType with writable: false.
        // HealthKitProvider.writeSample would no-op silently for it, so the VM
        // must reject it up front rather than acknowledge a write that never
        // reached HealthKit.
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        let item = makeItem(id: "sleep-1", hkType: "sleep_analysis", value: 8)
        mock.requestHandler = { _, _, _ in [item] }
        var confirmCalled = false
        mock.requestNoContentHandler = { _, _, _ in confirmCalled = true }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.confirm(item)

        #expect(hk.writtenSamples.isEmpty)     // nothing written to HealthKit
        #expect(confirmCalled == false)        // server never told it succeeded
        #expect(vm.items.count == 1)           // row stays
        #expect(vm.actionError != nil)         // user sees feedback
    }

    // MARK: - deny

    @Test("deny reports the item as a failure (never as a fake success) without writing, and removes it")
    func denySuccess() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        let item = makeItem()
        mock.requestHandler = { _, _, _ in [item] }
        var confirmedBody: HealthKitConfirm?
        mock.requestNoContentHandler = { _, path, body in
            #expect(path == Endpoints.healthKitConfirm)
            confirmedBody = body as? HealthKitConfirm
        }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.deny(item)

        #expect(hk.writtenSamples.isEmpty)     // deny never writes to HealthKit
        // A denied item was never written — it must never be sent as `ids`,
        // which would falsely tell the server the write succeeded.
        #expect(confirmedBody?.ids.isEmpty == true)
        #expect(confirmedBody?.failures.map(\.id) == ["wb-1"])
        #expect(vm.items.isEmpty)
        #expect(vm.actionError == nil)
    }

    @Test("deny network failure keeps item and surfaces error")
    func denyFailure() async {
        let mock = MockNetworkClient()
        let hk = MockHealthKitProvider()
        let item = makeItem()
        mock.requestHandler = { _, _, _ in [item] }
        mock.requestNoContentHandler = { _, _, _ in throw NetworkError.unauthorized }

        let vm = makeVM(network: mock, healthKit: hk)
        await vm.load()
        await vm.deny(item)

        #expect(hk.writtenSamples.isEmpty)
        #expect(vm.items.count == 1)
        #expect(vm.actionError != nil)
    }

    // MARK: - display name

    @Test("displayName humanizes the hk_type record type")
    func displayNameHumanizes() {
        let mock = MockNetworkClient()
        let vm = makeVM(network: mock)
        let item = makeItem(hkType: "resting_heart_rate")
        #expect(vm.displayName(for: item) == "Resting Heart Rate")
    }

    // MARK: - View content mapping

    @Test("content identifier reflects each state")
    func contentIdentifierPerState() {
        #expect(WriteBackQueueView.contentIdentifier(state: .idle, isEmpty: true) == "writeBackLoading")
        #expect(WriteBackQueueView.contentIdentifier(state: .loading, isEmpty: true) == "writeBackLoading")
        #expect(WriteBackQueueView.contentIdentifier(state: .error("boom"), isEmpty: true) == "writeBackError")
        #expect(WriteBackQueueView.contentIdentifier(state: .loaded, isEmpty: true) == "writeBackEmpty")
        #expect(WriteBackQueueView.contentIdentifier(state: .loaded, isEmpty: false) == "writeBackList")
    }

    @Test("formattedValue renders integers without decimals and fractions with two places")
    func formattedValueRendering() {
        #expect(WriteBackQueueView.formattedValue(72) == "72")
        #expect(WriteBackQueueView.formattedValue(72.0) == "72")
        #expect(WriteBackQueueView.formattedValue(72.5) == "72.50")
        #expect(WriteBackQueueView.formattedValue(0) == "0")
    }
}
