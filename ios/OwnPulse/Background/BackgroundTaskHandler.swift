// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks

enum BackgroundTaskHandler {
    static func handleSync(task: BGAppRefreshTask, syncEngine: SyncEngine) async {
        task.expirationHandler = {
            task.setTaskCompleted(success: false)
        }

        await syncEngine.sync()
        task.setTaskCompleted(success: true)

        // Schedule next
        let scheduler = SyncScheduler()
        scheduler.scheduleNextSync()
    }
}
