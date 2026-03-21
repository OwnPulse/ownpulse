# Sleep Tracking

The Timeline page includes a 14-day sleep chart that gives you a visual overview of your recent sleep patterns. Sleep data is one of the most valuable inputs for understanding how your daily habits and interventions affect recovery.

## What is tracked

OwnPulse records the following sleep metrics:

- **Total duration** -- time from sleep onset to final wake, minus time spent awake during the night.
- **Sleep stages** -- breakdown into deep sleep, light sleep, REM sleep, and awake periods. Stage data is available when synced from a device that tracks sleep stages (such as Apple Watch).
- **Sleep score** -- a composite metric summarizing sleep quality. Calculated from duration, stage distribution, and consistency with your typical pattern.

## Data sources

Sleep data can come from two places:

- **Apple Health sync** -- if you wear an Apple Watch or use a sleep tracking app that writes to HealthKit, OwnPulse pulls in your sleep data automatically. See [Apple Health](apple-health.md) for setup instructions.
- **Manual entry** -- on the Data Entry page, you can log sleep start time, end time, and subjective quality. Manual entries do not include stage breakdowns but do contribute to duration tracking and the timeline chart.

!!! tip "Combining sources"
    If you track sleep with a wearable on most nights but occasionally forget to wear it, you can fill in gaps with manual entries. OwnPulse deduplicates overlapping records automatically.

## Reading the timeline chart

The 14-day sleep chart on the Timeline page uses stacked horizontal bars. Each bar represents one night:

- The total width shows sleep duration.
- Colored segments represent sleep stages: deep (darkest), REM, light, and awake (lightest).
- Hover or tap a bar to see exact times and stage durations.

Nights with less than your average duration are visually distinct, making it easy to spot patterns like consistently short sleep on weekdays.

## iOS sleep chart

!!! note "iOS only"
    This chart is available in the OwnPulse iOS app on the home screen.

The iOS app shows a combined sleep and HRV chart. Sleep stages are displayed as stacked bars with the following colors:

- **Deep** -- dark blue
- **Core/Light** -- light blue
- **REM** -- purple
- **Awake** -- orange

HRV (heart rate variability) is shown as a white line overlay on the same chart, with the min and max range displayed in milliseconds. This gives you a quick view of both sleep quality and autonomic nervous system recovery in one visualization.

The iOS app reads sleep data directly from HealthKit. The web dashboard shows the same data after it has been synced to the backend.

## Manual sleep entry

Sleep records can also be entered manually via the Data Entry page on the web. Manual sleep entries include:

- **Duration** -- total sleep time
- **Stage breakdown** -- deep, light, REM, and awake minutes (optional)
- **Sleep score** -- an optional subjective or device-provided quality score
- **Notes** -- free-form text for anything relevant to that night

Manual entries are useful for filling in nights when you did not wear a tracking device. They appear on the timeline chart alongside synced data.

## Tips for better sleep data

For the most accurate sleep tracking, wear your device to bed consistently and charge it at a different time of day. If you use manual entry, log your sleep as close to waking as possible while your memory is fresh.
