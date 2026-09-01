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

The app keeps your data current through several automatic sync paths:

- **On sign-in.** OwnPulse kicks off an initial sync the moment you authenticate, so your dashboard is populated when you land on it.
- **On every launch / when you foreground the app.** Becoming active always triggers a sync.
- **While the app is open.** An Apple Health change observer watches for new samples (e.g. a workout completing on your Apple Watch) and triggers a short-debounced sync so you see the update within seconds.
- **In the background.** iOS wakes the app on its own schedule for a few seconds at a time and we run a quick sync before yielding. Frequency depends on how often you use the app and your device's battery/thermal state — it is not a hard guarantee.

Signing out stops all background-sync registrations so a signed-out device doesn't spend battery on background work.

Failed syncs are queued locally in an offline database and retried automatically when connectivity returns.

## Offline mode

If you lose network connectivity, the iOS app continues to collect HealthKit data. Sync operations are queued locally and processed once the app reconnects to your OwnPulse backend. Data entry on the web is always available independently.

## Protocols

The iOS app includes a full native protocol editor. You can create, view, and manage protocols directly on your device.

- **Protocol list** -- browse your protocols with filter options (all, active, paused, completed) and progress bars showing adherence.
- **Protocol builder** -- create new protocols with substance, dose, route, timing, and a pattern picker for scheduling (Daily, 3x/Week, Every Other Day, Weekdays).
- **Protocol detail** -- view progress, today's doses with Log and Skip buttons, and substance summaries.
- **Adherence** -- once a protocol has a run and at least one closed day, the protocol detail screen shows an adherence summary ("83% adherence · 20 done · 2 skipped · 2 missed"). Skipped doses are excluded from the adherence percentage -- a deliberate skip is not treated as a failure. Before a run's first closed day, this reads "No closed days yet."
- **Dose log & backfill** -- below the adherence summary, a dose log lists every scheduled day for the current run, most recent first, with its status (completed, skipped, missed, or pending). Tap a missed or pending day to log or skip it after the fact, optionally with a skip reason. Tap-and-hold (or use the context menu) on a completed or skipped day to undo it.
- **Missed doses** -- if you have missed doses across any active protocol, the dashboard shows a "N missed doses -- Review" row. Tapping it opens a list of every missed dose (capped at 200) with quick Log/Skip actions, without needing to open each protocol individually.

!!! note
    The sequencer grid for fine-grained day-by-day editing and copy-week-forward are web-only features. On iOS, use the pattern picker to set schedules.

## Notifications

Dose reminders are **local notifications** -- scheduled directly on your iPhone using each run's notify settings, with no push-notification service or backend involved. To set them up:

1. Go to **Settings** in the OwnPulse app.
2. Check the **Notifications** section. It shows whether dose reminders are enabled or disabled.
3. If not enabled, tap **Enable Notifications** to grant permission.

Notification times are configured **on the web app** when you start a run, or from a run's settings there (see [Protocols -- Dose reminders](protocols.md#dose-reminders)) -- the iOS app has no notify-settings screen of its own. The iOS app reads whatever notify settings the run already has and schedules matching local notifications the next time it runs; it does not receive a push from the backend.

How reminders stay up to date:

- The iOS app schedules reminders up to **7 days ahead** for every active run with notifications enabled, and re-schedules that rolling window every time the app is opened or brought to the foreground, or the Protocols tab refreshes.
- Because reminders are scheduled entirely on-device, they only exist on devices where you've opened OwnPulse recently enough for the app to (re)schedule them. If you don't open the app for more than 7 days, reminders for that period will not fire.
- "Repeat until logged" is not implemented -- a reminder fires once at its configured time regardless of whether the dose is later logged or skipped.
- iOS allows at most **64 pending local notifications per app**. If your active runs and notify times add up to more than that, the furthest-out reminders are dropped in favor of the soonest ones -- in practice this only matters with many simultaneous multi-times-daily runs.
- A reminder's lock screen banner shows the substance name and dose for each scheduled item, since that's what makes the reminder useful -- avoid enabling notifications for a run whose substance names you'd rather not have visible on a locked device.

!!! warning
    If you previously denied notification permission, you will need to enable it manually in iOS Settings under **Notifications > OwnPulse**.

## Lock Screen & Home Screen widgets

OwnPulse ships three widgets you can add to your Lock Screen or Home Screen.
They are **read-only** — they display the latest values the app has already
loaded and never send anything anywhere. Their data lives only on your device,
shared with the app through a private, on-device app group.

| Widget | Where it fits | Shows |
|--------|---------------|-------|
| **Today's Check-in** | Lock Screen circular & rectangular | Whether you've logged today's check-in yet. Tap to open the check-in form. |
| **Hero Metric** | Lock Screen rectangular & Home Screen small | Your latest headline metric (e.g. resting heart rate) with its 30-day trend. |
| **Quick Log** | Lock Screen circular | A one-tap shortcut straight into logging an intervention. |

To add one:

1. Touch and hold the Lock Screen (or Home Screen) and tap **Customize** / the
   **+** button.
2. Choose **OwnPulse** and pick the widget and size you want.

The widgets refresh after each sync or when you open the Dashboard. If you
haven't synced recently, the Hero Metric widget falls back to a neutral
placeholder (a dash) instead of showing a stale reading: it only displays a
value and trend that are less than a day old.

!!! warning "Lock Screen visibility"
    The Hero Metric widget shows a real vital — your latest resting heart
    rate — directly on the **Lock Screen**, where it is readable by anyone
    holding your device **without unlocking it**. If you'd rather not surface a
    health number on your lock screen, simply don't add that widget (or add it
    to the Home Screen only). The Today's Check-in and Quick Log widgets show
    no health values.

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
| Lock Screen widgets | No | Yes |

The web app is the full-featured interface. The iOS app provides HealthKit sync, a native dashboard with check-in score rings and protocol dose tracking, and a native protocol editor.
