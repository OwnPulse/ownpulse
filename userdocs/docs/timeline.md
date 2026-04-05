# Dashboard & Timeline

The dashboard is your at-a-glance view of recent health data. Both the web app and iOS app provide dashboard views, each tailored to the platform.

## Web dashboard

The web dashboard shows several sections:

- **Sleep chart** -- a 14-day stacked bar chart showing your sleep duration and stage breakdown each night. Bars are color-coded by sleep stage, making it easy to spot trends in deep sleep, REM, or nights with excessive wake time.
- **Today's Doses** -- if you have active protocol runs, the dashboard shows a Today's Doses widget at the top with pending dose count and quick Log/Skip buttons. See [Protocols -- Today's doses](protocols.md#todays-doses) for details.
- **Check-in score rings** -- your latest subjective ratings for energy, mood, focus, recovery, and libido, displayed as colored progress rings. Each dimension has a distinct color: energy (gold), mood (terracotta), focus (teal), recovery (sage), and libido (violet). The ring fills proportionally to your score out of 10, giving you an instant visual read on your day.
- **Recent health records** -- the most recent metrics synced from Apple Health, integrations, or manual entry. This includes heart rate, HRV, weight, blood oxygen, and any other metrics you track.

The sleep chart uses stacked bars with color-coded segments for each sleep stage. Hover or tap a bar to see exact durations and times for that night. See [Sleep Tracking](sleep.md) for a detailed breakdown of the chart.

## iOS home screen

!!! note "iOS only"
    The iOS home screen is available in the OwnPulse iOS app.

The iOS app home screen provides a focused view centered on sync status and sleep data:

- **Sync status indicator** -- shows whether the app is currently syncing or displays the last successful sync time.
- **Sleep + HRV chart** -- a combined visualization showing sleep stages as stacked bars with an HRV (heart rate variability) line overlay in white. Sleep stage colors are:
    - **Deep** -- dark blue
    - **Core/Light** -- light blue
    - **REM** -- purple
    - **Awake** -- orange
- **HRV range** -- the chart displays your HRV as a white line with min and max values shown in milliseconds.
- **Sync Now** -- tap to trigger an immediate sync with Apple Health and the backend.
- **Open Dashboard** -- opens the full web dashboard in your browser for data entry, export, and detailed views.

The iOS home screen is designed for a quick daily glance. For full data entry, export, and settings, use the web dashboard.
