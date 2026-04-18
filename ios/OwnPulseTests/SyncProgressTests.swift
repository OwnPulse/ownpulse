// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("SyncProgress")
@MainActor
struct SyncProgressTests {
    private func types() -> [(recordType: String, displayName: String)] {
        [
            (recordType: "heart_rate", displayName: "Heart Rate"),
            (recordType: "steps", displayName: "Steps"),
        ]
    }

    @Test("reset initializes all types with zero counts")
    func resetInitializes() {
        let p = SyncProgress()
        p.reset(types: types(), timestamps: [:])

        #expect(p.totalTypes == 2)
        #expect(p.completedTypes == 0)
        #expect(p.typeStatuses["heart_rate"]?.status == .never)
        #expect(p.typeStatuses["heart_rate"]?.recordsSynced == 0)
        #expect(p.typeStatuses["heart_rate"]?.totalSamples == 0)
        #expect(p.totalRecordsUploaded == 0)
    }

    @Test("markSyncing clears prior counts")
    func markSyncingClears() {
        let p = SyncProgress()
        p.reset(types: types(), timestamps: [:])
        p.markSyncing("heart_rate")
        p.setTotalSamples("heart_rate", total: 500)
        p.updateUploadProgress("heart_rate", uploaded: 200)

        // A second markSyncing should reset counts for a fresh run
        p.markSyncing("heart_rate")
        #expect(p.typeStatuses["heart_rate"]?.recordsSynced == 0)
        #expect(p.typeStatuses["heart_rate"]?.totalSamples == 0)
        #expect(p.typeStatuses["heart_rate"]?.status == .syncing)
        #expect(p.currentType == "heart_rate")
    }

    @Test("upload progress accumulates and surfaces in session total")
    func uploadProgress() {
        let p = SyncProgress()
        p.reset(types: types(), timestamps: [:])

        p.markSyncing("heart_rate")
        p.setTotalSamples("heart_rate", total: 5691)
        p.updateUploadProgress("heart_rate", uploaded: 100)
        #expect(p.typeStatuses["heart_rate"]?.recordsSynced == 100)
        #expect(p.typeStatuses["heart_rate"]?.totalSamples == 5691)
        #expect(p.totalRecordsUploaded == 100)

        p.updateUploadProgress("heart_rate", uploaded: 500)
        #expect(p.totalRecordsUploaded == 500)

        // Second type running in a later iteration
        p.markSynced("heart_rate", count: 5691, at: Date())
        p.markSyncing("steps")
        p.setTotalSamples("steps", total: 300)
        p.updateUploadProgress("steps", uploaded: 150)

        #expect(p.totalRecordsUploaded == 5691 + 150)
    }

    @Test("markSynced normalizes totalSamples so progress bar reads full")
    func syncedFillsTotal() {
        let p = SyncProgress()
        p.reset(types: types(), timestamps: [:])

        p.markSyncing("steps")
        p.setTotalSamples("steps", total: 1000)
        p.updateUploadProgress("steps", uploaded: 600)
        p.markSynced("steps", count: 1000, at: Date())

        let s = p.typeStatuses["steps"]
        #expect(s?.status == .synced)
        #expect(s?.recordsSynced == 1000)
        #expect(s?.totalSamples == 1000)
        #expect(p.completedTypes == 1)
    }

    @Test("markFailed records error without touching counts")
    func failedPreservesCounts() {
        let p = SyncProgress()
        p.reset(types: types(), timestamps: [:])

        p.markSyncing("heart_rate")
        p.setTotalSamples("heart_rate", total: 1000)
        p.updateUploadProgress("heart_rate", uploaded: 250)
        p.markFailed("heart_rate", error: "boom")

        let s = p.typeStatuses["heart_rate"]
        #expect(s?.status == .failed)
        #expect(s?.error == "boom")
        #expect(s?.recordsSynced == 250)
        #expect(p.completedTypes == 1)
    }
}
