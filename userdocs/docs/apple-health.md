# Apple Health

!!! note "iOS only"
    Apple Health integration is available on iPhone and iPad only. It requires the OwnPulse iOS app installed from the App Store or TestFlight.

## Setting up HealthKit access

1. Open the OwnPulse iOS app.
2. Go to **Settings** within the app.
3. Tap **Request HealthKit Access**.
4. iOS will present a permissions screen listing the health data categories OwnPulse can read and write. Toggle on the categories you want to sync and tap **Allow**.

You can change these permissions at any time in the iOS Settings app under **Privacy & Security > Health > OwnPulse**.

## How sync works

OwnPulse uses bidirectional sync with Apple Health:

- **Inbound:** Health data recorded by your Apple Watch, other apps, or manual entries in the Health app flows into OwnPulse automatically.
- **Outbound:** Manual entries you create in OwnPulse (via the web or iOS app) are written back to Apple Health, so other apps on your device can access them.

Sync runs automatically in the background whenever the iOS app is active or receives a background refresh. You can also tap the **Sync Now** button in the app to trigger an immediate sync.

## Deduplication

When data arrives from Apple Health that overlaps with data already in OwnPulse from another source, the system detects duplicates automatically. It compares timestamps (within a 60-second window) and values (within 2% tolerance). Duplicates are never silently dropped -- they are linked and the authoritative source is determined by your source preferences.

!!! warning "Cycle prevention"
    Records that originated from Apple Health are never written back to Apple Health. This prevents infinite sync loops. The guard is unconditional and cannot be overridden.

## Supported data types

OwnPulse syncs the following categories with Apple Health: heart rate, resting heart rate, heart rate variability, blood oxygen, respiratory rate, body temperature, weight, body fat percentage, step count, active energy, sleep analysis, and workouts. Additional categories may be added in future updates.

## Troubleshooting

If sync appears stuck, open the OwnPulse iOS app and check the sync status on the Settings screen. If the last sync time is stale, try tapping **Sync Now**. If that does not help, verify HealthKit permissions in iOS Settings and ensure background app refresh is enabled for OwnPulse.
