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

Sync runs automatically in four situations:

- **When you sign in.** OwnPulse runs an initial sync the moment authentication succeeds, so the dashboard is populated by the time you finish the login animation.
- **When you open the app.** OwnPulse triggers a sync every time the app becomes active, so opening the app gives you an up-to-date view of your recent Apple Health data.
- **When new samples arrive while the app is open.** OwnPulse listens for Apple Health change notifications and, after a short debounce, pulls any new data into your instance. This covers scenarios like finishing a workout on your Apple Watch while the iOS app is in the foreground.
- **In the background.** iOS periodically wakes OwnPulse to sync recent samples even when the app is closed. The wake frequency is controlled by iOS based on battery, network, and your usage patterns — it is not a guaranteed schedule, and the exact timing is up to the operating system. When you sign out, OwnPulse tells iOS to stop waking the app so a signed-out device doesn't spend battery on background work.

You can also tap **Sync Now** in **Settings → Sync Status** to trigger an immediate sync.

## Deduplication

When data arrives from Apple Health that overlaps with data already in OwnPulse from another source, the system detects duplicates automatically. It compares timestamps (within a 60-second window) and values (within 2% tolerance). Duplicates are never silently dropped -- they are linked and the authoritative source is determined by your source preferences.

!!! warning "Cycle prevention"
    Records that originated from Apple Health are never written back to Apple Health. This prevents infinite sync loops. The guard is unconditional and cannot be overridden.

## Supported data types

OwnPulse syncs the following categories with Apple Health: heart rate, resting heart rate, heart rate variability, blood oxygen, respiratory rate, body temperature, weight, body fat percentage, step count, active energy, sleep analysis, and workouts. Additional categories may be added in future updates.

## Troubleshooting

If sync appears stuck, open the OwnPulse iOS app and check the sync status on the Settings screen. If the last sync time is stale, try tapping **Sync Now**. If that does not help, verify HealthKit permissions in iOS Settings and ensure background app refresh is enabled for OwnPulse.

Background sync relies on iOS's background-delivery and app-refresh mechanisms, both of which can be throttled or disabled by the system to save power. If background sync stops working entirely, check:

1. **Settings → General → Background App Refresh → OwnPulse** is enabled.
2. **Settings → Privacy & Security → Health → OwnPulse** still has read permission for the data types you care about.
3. Low Power Mode is not enabled — it suspends most background tasks, including background delivery from HealthKit.

If all three look right and background sync is still missing data, opening the app will always trigger a foreground sync as a reliable fallback.
