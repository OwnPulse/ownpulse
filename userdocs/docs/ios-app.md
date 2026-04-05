# iOS App

The OwnPulse iOS app syncs your Apple Health data, provides a native dashboard with check-in score rings and protocol dose tracking, and includes a full protocol editor. Manual data entry for other record types, export, and account management are handled on the web.

## Installation

Download OwnPulse from TestFlight (during beta) or the App Store. Open the app and sign in with your Google account -- the same one you use on the web dashboard. Your account and data are shared across both platforms.

## Home screen

The home screen shows:

- **Sync status** -- whether the app is currently syncing or when the last sync completed.
- **Sleep + HRV chart** -- stacked bars for sleep stages with a white HRV line overlay. Sleep stage colors: Deep (dark blue), Core/Light (light blue), REM (purple), Awake (orange). HRV range is displayed as min/max in milliseconds.
- **Sync Now** -- tap to trigger an immediate sync with Apple Health and the backend.
- **Open Dashboard** -- opens the full web dashboard in your browser.

## HealthKit setup

1. In the OwnPulse app, go to **Settings**.
2. Tap **Request HealthKit Access**.
3. iOS presents a permissions screen listing health data categories. Grant permissions for the data types you want synced (heart rate, sleep, HRV, steps, and more).

You can change these permissions at any time in iOS Settings under **Privacy & Security > Health > OwnPulse**.

## Background sync

The app syncs automatically in the background and on every launch. iOS controls background refresh scheduling, so sync frequency depends on how often you use the app and your device's battery state. Failed syncs are queued locally in an offline database and retried automatically when connectivity returns.

## Offline mode

If you lose network connectivity, the iOS app continues to collect HealthKit data. Sync operations are queued locally and processed once the app reconnects to your OwnPulse backend. Data entry on the web is always available independently.

## Protocols

The iOS app includes a full native protocol editor. You can create, view, and manage protocols directly on your device.

- **Protocol list** -- browse your protocols with filter options (all, active, paused, completed) and progress bars showing adherence.
- **Protocol builder** -- create new protocols with substance, dose, route, timing, and a pattern picker for scheduling (Daily, 3x/Week, Every Other Day, Weekdays).
- **Protocol detail** -- view progress, today's doses with Log and Skip buttons, and substance summaries.

!!! note
    The sequencer grid for fine-grained day-by-day editing and copy-week-forward are web-only features. On iOS, use the pattern picker to set schedules.

## Notifications

The iOS app supports push notifications for protocol dose reminders. To set up notifications:

1. Go to **Settings** in the OwnPulse app.
2. Check the **Notifications** section. It shows whether dose reminders are enabled or disabled.
3. If not enabled, tap **Enable Notifications** to grant permission.

Notification times are configured per protocol run when you start it (see [Protocols -- Dose reminders](protocols.md#dose-reminders)). You can configure reminders per run from the web interface, and they will be delivered to your iOS device.

!!! warning
    If you previously denied notification permission, you will need to enable it manually in iOS Settings under **Notifications > OwnPulse**.

## What's on iOS vs web

| Feature | Web | iOS |
|---------|-----|-----|
| Google OAuth login | Yes | Yes |
| Manual data entry | Yes | No |
| Protocols | Yes | Yes (pattern picker; no sequencer grid) |
| Dashboard & charts | Yes | Yes (sleep+HRV, score rings, protocol doses) |
| Export data | Yes | No |
| Source management | Yes | HealthKit only |
| Account settings | Yes | No |
| HealthKit sync | No | Yes |
| Background sync | No | Yes |

The web app is the full-featured interface. The iOS app provides HealthKit sync, a native dashboard with check-in score rings and protocol dose tracking, and a native protocol editor.
