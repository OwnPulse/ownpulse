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

    // Uses `NetworkClient.makeDecoder()` — the actual decoder every
    // `request(...)` call site uses in production — rather than a fresh
    // `JSONDecoder()`. A fresh decoder would be vacuous here: the whole
    // point of this suite is to catch drift between what the wire sends and
    // what the app's real decoder config accepts.
    private static func makeDecoder() -> JSONDecoder {
        NetworkClient.makeDecoder()
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

    /// The pact fixtures above happen to use whole-second timestamps, but
    /// real backend rows (chrono/Postgres `TIMESTAMPTZ`) serialize with
    /// fractional seconds on essentially every row, and anything written by
    /// the web client does too. `JSONDecoder`'s built-in `.iso8601` strategy
    /// rejects fractional seconds outright — this is the actual bug that
    /// would still break decoding in production even after the payload
    /// shape fix, which is why `NetworkClient.makeDecoder()` (not `.iso8601`)
    /// is what every test in this suite exercises.
    @Test("decodes fractional-second timestamps (real backend/web rows, not just the whole-second pact fixture)")
    func decodesFractionalSeconds() throws {
        let json = """
        {
          "id": "77777777-7777-7777-7777-777777777777",
          "user_id": "some-uuid",
          "hk_type": "body_mass",
          "value": { "value": 82.5, "unit": "kg", "start_time": "2026-03-20T10:00:00.123456Z", "end_time": "2026-03-20T10:00:00.123456Z" },
          "scheduled_at": "2026-03-20T10:00:00.123456Z",
          "confirmed_at": null,
          "failed_at": null,
          "error": null,
          "source_record_id": null,
          "source_table": null
        }
        """
        let item = try Self.makeDecoder().decode(HealthKitWriteQueueItem.self, from: Data(json.utf8))

        let fractionalFormatter = ISO8601DateFormatter()
        fractionalFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let expected = fractionalFormatter.date(from: "2026-03-20T10:00:00.123456Z")

        #expect(item.value.startTime == expected)
        #expect(item.value.endTime == expected)
        #expect(item.scheduledAt == expected)
    }
}
