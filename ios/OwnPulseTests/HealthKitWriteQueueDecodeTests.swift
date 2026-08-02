// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// Cross-boundary contract test: decodes the EXACT wire shape pinned by
/// `pact/contracts/ios-backend.json` ("a request to get the HealthKit
/// write-back queue"). This never existed before — `HealthKitWriteQueueItem.value`
/// was declared as a bare `Double` while the backend served a JSONB object,
/// so every sync with a non-empty write-queue threw `.decodingFailed`, which
/// `SyncEngine` swallowed. If a future backend change drifts from the pact
/// shape, this test (not a production crash) is where it should surface.
@Suite("HealthKitWriteQueueItem decode — pact contract")
struct HealthKitWriteQueueDecodeTests {
    private static let pactJSON = """
    {
      "id": "77777777-7777-7777-7777-777777777777",
      "user_id": "some-uuid",
      "hk_type": "body_mass",
      "value": { "value": 82.5, "unit": "kg", "start_time": "2026-03-20T10:00:00Z", "end_time": "2026-03-20T10:00:00Z" },
      "scheduled_at": "2026-03-20T10:00:00Z",
      "confirmed_at": null,
      "failed_at": null,
      "error": null,
      "source_record_id": null,
      "source_table": null
    }
    """

    private static func makeDecoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }

    @Test("decodes the exact pact-pinned write-queue item shape")
    func decodesPactShape() throws {
        let item = try Self.makeDecoder().decode(
            HealthKitWriteQueueItem.self,
            from: Data(Self.pactJSON.utf8)
        )

        let expectedTime = ISO8601DateFormatter().date(from: "2026-03-20T10:00:00Z")

        #expect(item.id == "77777777-7777-7777-7777-777777777777")
        #expect(item.userId == "some-uuid")
        #expect(item.hkType == "body_mass")
        #expect(item.value.value == 82.5)
        #expect(item.value.unit == "kg")
        #expect(item.value.startTime == expectedTime)
        #expect(item.value.endTime == expectedTime)
        #expect(item.scheduledAt == expectedTime)
        #expect(item.confirmedAt == nil)
        #expect(item.failedAt == nil)
        #expect(item.error == nil)
        #expect(item.sourceRecordId == nil)
        #expect(item.sourceTable == nil)
    }

    @Test("decodes an array response — the actual shape GET /healthkit/write-queue returns")
    func decodesArrayResponse() throws {
        let arrayJSON = "[\(Self.pactJSON)]"
        let items = try Self.makeDecoder().decode(
            [HealthKitWriteQueueItem].self,
            from: Data(arrayJSON.utf8)
        )

        #expect(items.count == 1)
        #expect(items[0].id == "77777777-7777-7777-7777-777777777777")
    }

    @Test("decodes a payload with a null end_time (instantaneous sample)")
    func decodesNullEndTime() throws {
        let json = """
        {
          "id": "1",
          "user_id": "u",
          "hk_type": "body_mass",
          "value": { "value": 1.0, "unit": null, "start_time": "2026-03-20T10:00:00Z", "end_time": null },
          "scheduled_at": "2026-03-20T10:00:00Z",
          "confirmed_at": null,
          "failed_at": null,
          "error": null,
          "source_record_id": null,
          "source_table": null
        }
        """
        let item = try Self.makeDecoder().decode(HealthKitWriteQueueItem.self, from: Data(json.utf8))
        #expect(item.value.unit == nil)
        #expect(item.value.endTime == nil)
    }

    /// The backend's `HealthRecordRow` has no route-level validation
    /// requiring a numeric value — a record enqueued without one serves
    /// `value.value == nil`, `value.unit == nil`. The happy-path pact
    /// fixture above only pins the case where a value IS present; this pins
    /// the null-value variant so the decoder (and `processWriteBack`'s
    /// nil-value guard) are exercised against the shape the backend can
    /// actually serve, not just the common case.
    private static let nullValuePactJSON = """
    {"id":"1","user_id":"u","hk_type":"body_mass","value":{"value":null,"unit":null,"start_time":"2026-03-20T10:00:00Z","end_time":null},"scheduled_at":"2026-03-20T10:00:00Z","confirmed_at":null,"failed_at":null,"error":null,"source_record_id":null,"source_table":null}
    """

    @Test("decodes a null-value write-queue item (no route-level guarantee of a numeric value)")
    func decodesNullValueVariant() throws {
        let item = try Self.makeDecoder().decode(
            HealthKitWriteQueueItem.self,
            from: Data(Self.nullValuePactJSON.utf8)
        )

        #expect(item.value.value == nil)
        #expect(item.value.unit == nil)
        #expect(item.value.startTime == ISO8601DateFormatter().date(from: "2026-03-20T10:00:00Z"))
        #expect(item.value.endTime == nil)
    }
}
