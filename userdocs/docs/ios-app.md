# iOS App

The OwnPulse iOS app syncs your Apple Health data and provides a focused home screen for daily monitoring. Manual data entry, export, and account management are handled on the web.

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

## What's on iOS vs web

| Feature | Web | iOS |
|---------|-----|-----|
| Google OAuth login | Yes | Yes |
| Manual data entry | Yes | No |
| Dashboard & charts | Yes | Yes (sleep+HRV) |
| Export data | Yes | No |
| Source management | Yes | HealthKit only |
| Account settings | Yes | No |
| HealthKit sync | No | Yes |
| Background sync | No | Yes |

The web app is the full-featured interface. The iOS app is purpose-built for HealthKit sync and a quick daily glance at your sleep and HRV data.
