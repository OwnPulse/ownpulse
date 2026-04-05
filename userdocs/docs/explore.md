# Explore

The Explore page lets you chart any combination of your health metrics on a single timeline. You can overlay heart rate with mood scores, compare sleep duration against lab results, or visualize any data OwnPulse has collected -- all in one view.

## Adding metrics

To start building a chart, open the metric picker. Your available metrics are organized by source:

- **Health Records** -- heart rate, HRV, resting heart rate, body mass, body fat percentage, body temperature, blood pressure, blood glucose, blood oxygen, respiratory rate, steps, active energy, basal energy, and VO2 max
- **Check-ins** -- energy, mood, focus, recovery, and libido scores
- **Lab Results** -- any lab test you have recorded (e.g., testosterone, TSH, vitamin D)
- **Calendar** -- meeting minutes and meeting count
- **Sleep** -- sleep duration, deep sleep, REM sleep, and sleep score

Select a metric to add it to the chart. You can add up to eight metrics at once.

## Date range

Choose a date range to control which data appears on the chart. Preset options make it quick:

- **7 days** -- the past week
- **30 days** -- the past month
- **90 days** -- the past three months
- **1 year** -- the past year
- **All** -- everything OwnPulse has

You can also set a custom date range by specifying exact start and end dates.

## Resolution

The resolution toggle controls how data points are aggregated:

- **Daily** -- one point per day (average of all readings that day)
- **Weekly** -- one point per week
- **Monthly** -- one point per month

Higher resolutions show more detail. Lower resolutions smooth out noise and reveal trends.

Each data point shows the average value and the number of raw records that were combined to produce it. This helps you understand whether a data point represents a single reading or a full day of measurements.

## Dual Y-axis

When you chart metrics with different units (for example, heart rate in BPM alongside a mood score from 1 to 10), Explore automatically assigns them to separate Y-axes. This keeps the scales readable so that low-range scores are not flattened against high-range measurements.

## Intervention markers

When you have intervention data in the selected date range, Explore overlays dashed vertical lines on the chart at the time each dose was administered. Each marker is labeled with the substance name. This lets you visually correlate dosing events with changes in your health metrics -- for example, seeing how a supplement affects your HRV or sleep the following days.

Intervention markers appear in the chart legend alongside your metrics. Click a substance name in the legend to toggle its markers on or off.

## Chart interactions

The chart supports several interactive features:

- **Zoom** -- use the slider bar at the bottom of the chart to zoom into a specific date range. You can also scroll to zoom or pinch on touch devices.
- **Dark mode** -- the chart automatically adapts to your system theme, adjusting colors, backgrounds, and text for readability in both light and dark modes.
- **Interactive legend** -- click any metric or substance in the legend to toggle its visibility on the chart. Hidden items appear dimmed in the legend.
- **Line style variation** -- each metric uses a distinct line style (solid, dashed, dotted) in addition to color, improving accessibility when many metrics are charted together.
- **Tooltips** -- hover over any point to see exact values and timestamps for all visible metrics at that time.
- **Correlate** -- when you have two or more metrics selected, a **Correlate** button appears. Clicking it opens the analysis view with those metrics pre-loaded for correlation analysis.

## Saving charts

Once you have a chart configuration you want to revisit, save it. Saved charts store the full configuration:

1. Set up the metrics, date range, and resolution you want.
2. Save the chart and give it a name (up to 200 characters).
3. Access your saved charts from the charts list at any time.

You can update a saved chart's name or configuration later, or delete it when you no longer need it.

## Loading a saved chart

Open the saved charts list and select a chart to load it. The Explore page restores the exact configuration -- metrics, date range, resolution, and any custom colors you set.

## Live updates

The Explore page listens for server-sent events from your OwnPulse instance. When new data arrives -- from a manual entry, a HealthKit sync, or an integration -- the chart refreshes automatically without requiring a manual page reload.

## Tips

- Start with a single metric to get oriented, then layer on additional metrics to look for patterns.
- Use the weekly or monthly resolution to spot trends that daily noise might obscure.
- Save charts for metric combinations you check regularly, like morning vitals or a recovery dashboard.
- Lab results are especially useful at monthly resolution -- overlay them against daily metrics to see how biomarker changes correlate with how you feel.
